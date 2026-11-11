use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
use vault_core::{get_socket_path, IpcRequest, IpcResponse};

/// Limite máximo de payload JSON do Native Messaging (1 MiB)
const MAX_MESSAGE_SIZE: usize = 1024 * 1024;

/// Lê uma mensagem no formato Native Messaging do navegador (4 bytes u32 nativo + payload UTF-8).
fn read_native_message<R: Read>(reader: &mut R) -> io::Result<Option<Vec<u8>>> {
    let mut len_bytes = [0u8; 4];
    match reader.read_exact(&mut len_bytes) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }

    let len = u32::from_ne_bytes(len_bytes) as usize;
    if len == 0 || len > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Tamanho de mensagem inválido: {len}"),
        ));
    }

    let mut buffer = vec![0u8; len];
    reader.read_exact(&mut buffer)?;
    Ok(Some(buffer))
}

/// Escreve uma resposta no formato Native Messaging para o navegador (4 bytes u32 nativo + payload UTF-8).
fn write_native_message<W: Write>(writer: &mut W, response: &IpcResponse) -> io::Result<()> {
    let payload = serde_json::to_vec(response)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    let len = payload.len() as u32;

    writer.write_all(&len.to_ne_bytes())?;
    writer.write_all(&payload)?;
    writer.flush()?;
    Ok(())
}

/// Tenta iniciar o binário Desktop (`password-vault-desktop`) em segundo plano caso esteja fechado.
fn spawn_desktop_app() -> Result<(), String> {
    let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let sibling_bin = current_exe
        .parent()
        .map(|p| p.join("password-vault-desktop"))
        .unwrap_or_else(|| "password-vault-desktop".into());

    let bin_to_run = if sibling_bin.exists() {
        sibling_bin
    } else if let Ok(home) = std::env::var("HOME") {
        std::path::PathBuf::from(home).join(".local/bin/password-vault")
    } else {
        "password-vault".into()
    };

    Command::new(bin_to_run)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Falha ao abrir janela do Password Vault: {e}"))?;

    // Aguarda até 1.5s para o socket do Tauri subir
    let socket_path = get_socket_path();
    for _ in 0..15 {
        if UnixStream::connect(&socket_path).is_ok() {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }

    Ok(())
}

/// Encaminha a requisição para o Unix Domain Socket do aplicativo Desktop Tauri.
fn forward_to_tauri_socket(request: &IpcRequest) -> IpcResponse {
    if matches!(request, IpcRequest::LaunchDesktopApp) {
        let socket_path = get_socket_path();
        if UnixStream::connect(&socket_path).is_err() {
            if let Err(e) = spawn_desktop_app() {
                return IpcResponse::Error { message: e };
            }
        }
    }

    let socket_path = get_socket_path();

    let mut stream = match UnixStream::connect(&socket_path) {
        Ok(s) => s,
        Err(_) => {
            return IpcResponse::Locked {
                message: "O aplicativo Desktop está fechado.".to_string(),
            };
        }
    };

    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

    let mut payload = match serde_json::to_vec(request) {
        Ok(p) => p,
        Err(e) => {
            return IpcResponse::Error {
                message: format!("Erro ao serializar requisição: {e}"),
            }
        }
    };
    payload.push(b'\n');

    if let Err(e) = stream.write_all(&payload) {
        return IpcResponse::Error {
            message: format!("Falha ao enviar para o cofre local: {e}"),
        };
    }

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    match reader.read_line(&mut line) {
        Ok(0) => IpcResponse::Error {
            message: "Conexão encerrada pelo aplicativo Desktop.".to_string(),
        },
        Ok(_) => {
            serde_json::from_str::<IpcResponse>(&line).unwrap_or_else(|e| IpcResponse::Error {
                message: format!("Resposta inválida do cofre: {e}"),
            })
        }
        Err(e) => IpcResponse::Error {
            message: format!("Erro ao ler resposta do cofre: {e}"),
        },
    }
}

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();

    while let Ok(Some(raw_msg)) = read_native_message(&mut reader) {
        let response = match serde_json::from_slice::<IpcRequest>(&raw_msg) {
            Ok(req) => forward_to_tauri_socket(&req),
            Err(e) => IpcResponse::Error {
                message: format!("JSON de requisição inválido: {e}"),
            },
        };

        if write_native_message(&mut writer, &response).is_err() {
            break;
        }
    }
}
