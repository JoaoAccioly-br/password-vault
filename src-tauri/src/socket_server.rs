use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter, Manager};
use vault_core::{get_socket_path, IpcRequest, IpcResponse, VaultDatabase, VaultError};

pub fn start_unix_socket_server(db: Arc<Mutex<VaultDatabase>>, app_handle: AppHandle) {
    let db_unix = Arc::clone(&db);
    let app_unix = app_handle.clone();

    // 1. Servidor principal Unix Domain Socket (0600) para Native Messaging
    thread::spawn(move || {
        let socket_path = get_socket_path();

        if socket_path.exists() {
            let _ = fs::remove_file(&socket_path);
        }

        let listener = match UnixListener::bind(&socket_path) {
            Ok(l) => l,
            Err(e) => {
                eprintln!(
                    "[socket_server] Falha ao criar Unix Socket em {:?}: {}",
                    socket_path, e
                );
                return;
            }
        };

        if let Ok(metadata) = fs::metadata(&socket_path) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o600);
            let _ = fs::set_permissions(&socket_path, perms);
        }

        for stream in listener.incoming() {
            if let Ok(stream) = stream {
                let db_clone = Arc::clone(&db_unix);
                let app_clone = app_unix.clone();
                thread::spawn(move || {
                    handle_unix_client(stream, db_clone, app_clone);
                });
            }
        }
    });

    // 2. Ponte Loopback para navegadores isolados em sandbox Flatpak/Snap (127.0.0.1:49215)
    // Aceita apenas conexões vindas de extensões do navegador (moz-extension:// ou chrome-extension://)
    thread::spawn(move || {
        let listener = match TcpListener::bind("127.0.0.1:49215") {
            Ok(l) => l,
            Err(_) => return,
        };

        for stream in listener.incoming() {
            if let Ok(stream) = stream {
                let db_clone = Arc::clone(&db);
                let app_clone = app_handle.clone();
                thread::spawn(move || {
                    handle_loopback_http(stream, db_clone, app_clone);
                });
            }
        }
    });
}

fn handle_unix_client(
    mut stream: UnixStream,
    db: Arc<Mutex<VaultDatabase>>,
    app_handle: AppHandle,
) {
    let reader_stream = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };
    let mut reader = BufReader::new(reader_stream);
    let mut line = String::new();

    if reader.read_line(&mut line).is_ok() && !line.trim().is_empty() {
        let response = match serde_json::from_str::<IpcRequest>(&line) {
            Ok(req) => process_ipc_request(req, &db, &app_handle),
            Err(e) => IpcResponse::Error {
                message: format!("Requisição IPC inválida: {e}"),
            },
        };

        if let Ok(mut bytes) = serde_json::to_vec(&response) {
            bytes.push(b'\n');
            let _ = stream.write_all(&bytes);
            let _ = stream.flush();
        }
    }
}

fn handle_loopback_http(
    mut stream: TcpStream,
    db: Arc<Mutex<VaultDatabase>>,
    app_handle: AppHandle,
) {
    let reader_stream = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };
    let mut reader = BufReader::new(reader_stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }

    let mut content_length = 0usize;
    let mut origin = String::new();
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).is_err() || header == "\r\n" || header.is_empty() {
            break;
        }
        let lower = header.to_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:") {
            content_length = rest.trim().parse::<usize>().unwrap_or(0);
        } else if let Some(rest) = lower.strip_prefix("origin:") {
            origin = rest.trim().to_string();
        }
    }

    if request_line.starts_with("OPTIONS ") {
        let resp = "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\n\r\n";
        let _ = stream.write_all(resp.as_bytes());
        return;
    }

    // Bloqueia páginas web normais (http:// ou https://) de chamar a ponte; permite apenas extensões
    if !origin.is_empty()
        && !origin.starts_with("moz-extension://")
        && !origin.starts_with("chrome-extension://")
    {
        let resp = "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n";
        let _ = stream.write_all(resp.as_bytes());
        return;
    }

    if content_length == 0 || content_length > 1024 * 1024 {
        return;
    }

    let mut body = vec![0u8; content_length];
    if reader.read_exact(&mut body).is_err() {
        return;
    }

    let response = match serde_json::from_slice::<IpcRequest>(&body) {
        Ok(req) => process_ipc_request(req, &db, &app_handle),
        Err(e) => IpcResponse::Error {
            message: format!("JSON inválido: {e}"),
        },
    };

    if let Ok(json_bytes) = serde_json::to_vec(&response) {
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n",
            json_bytes.len()
        );
        let _ = stream.write_all(header.as_bytes());
        let _ = stream.write_all(&json_bytes);
        let _ = stream.flush();
    }
}

fn process_ipc_request(
    req: IpcRequest,
    db: &Arc<Mutex<VaultDatabase>>,
    app_handle: &AppHandle,
) -> IpcResponse {
    let guard = match db.lock() {
        Ok(g) => g,
        Err(_) => {
            return IpcResponse::Error {
                message: "Falha ao adquirir lock do banco de dados.".to_string(),
            }
        }
    };

    match req {
        IpcRequest::GetStatus => IpcResponse::Ok {
            unlocked: Some(guard.is_unlocked()),
            exists: None,
            logins: None,
            saved: None,
        },
        IpcRequest::LaunchDesktopApp => {
            if let Some(window) = app_handle.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
            IpcResponse::Ok {
                unlocked: Some(guard.is_unlocked()),
                exists: None,
                logins: None,
                saved: None,
            }
        }
        IpcRequest::GetLoginsForDomain { domain } => match guard.get_logins_for_domain(&domain) {
            Ok(logins) => IpcResponse::Ok {
                unlocked: Some(true),
                exists: None,
                logins: Some(logins),
                saved: None,
            },
            Err(VaultError::Locked) => IpcResponse::Locked {
                message: "O cofre está bloqueado. Desbloqueie no aplicativo Desktop.".to_string(),
            },
            Err(e) => IpcResponse::Error {
                message: e.to_string(),
            },
        },
        IpcRequest::CheckLoginExists { domain, username } => {
            match guard.check_login_exists(&domain, &username) {
                Ok(exists) => IpcResponse::Ok {
                    unlocked: Some(true),
                    exists: Some(exists),
                    logins: None,
                    saved: None,
                },
                Err(VaultError::Locked) => IpcResponse::Locked {
                    message: "O cofre está bloqueado.".to_string(),
                },
                Err(e) => IpcResponse::Error {
                    message: e.to_string(),
                },
            }
        }
        IpcRequest::SaveLogin {
            domain,
            title,
            username,
            password,
        } => match guard.save_credential(&domain, &title, &username, password, None) {
            Ok(summary) => {
                // Notifica a interface React do Tauri para atualizar a lista em tempo real
                let _ = app_handle.emit("vault-updated", &summary);
                IpcResponse::Ok {
                    unlocked: Some(true),
                    exists: None,
                    logins: None,
                    saved: Some(summary),
                }
            }
            Err(VaultError::Locked) => IpcResponse::Locked {
                message: "O cofre está bloqueado. Desbloqueie o Password Vault para salvar."
                    .to_string(),
            },
            Err(e) => IpcResponse::Error {
                message: e.to_string(),
            },
        },
    }
}
