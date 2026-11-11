use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use zeroize::Zeroize;

/// Resumo público de uma credencial (sem a senha aberta).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialSummary {
    pub id: String,
    pub domain: String,
    pub title: String,
    pub username: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Credencial completa com a senha descriptografada (usada para Auto-fill ou visualização explícita).
#[derive(Debug, Clone, Serialize, Deserialize, Zeroize)]
#[zeroize(drop)]
pub struct CredentialDecrypted {
    pub id: String,
    pub domain: String,
    pub title: String,
    pub username: String,
    pub password: String,
    pub notes: Option<String>,
}

/// Mensagens enviadas da Extensão Firefox/Zen -> vault-native-host -> Tauri Unix Socket.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum IpcRequest {
    /// Verifica se o App Desktop está aberto e se o cofre está desbloqueado.
    GetStatus,
    /// Abre a janela do aplicativo Desktop caso esteja fechado.
    LaunchDesktopApp,
    /// Busca credenciais que correspondem a um domínio (ex: "github.com") para Auto-fill.
    GetLoginsForDomain { domain: String },
    /// Verifica se uma combinação de domínio + username já existe (para sugerir Auto-save).
    CheckLoginExists { domain: String, username: String },
    /// Salva ou atualiza uma credencial capturada automaticamente no navegador.
    SaveLogin {
        domain: String,
        title: String,
        username: String,
        password: String,
    },
}

/// Respostas enviadas do Tauri Unix Socket -> vault-native-host -> Extensão Firefox/Zen.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum IpcResponse {
    Ok {
        #[serde(skip_serializing_if = "Option::is_none")]
        unlocked: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        exists: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        logins: Option<Vec<CredentialDecrypted>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        saved: Option<CredentialSummary>,
    },
    Locked {
        message: String,
    },
    Error {
        message: String,
    },
}

/// Retorna o caminho seguro do Unix Domain Socket para o usuário atual.
/// Usa `~/.local/share/password-vault/password-vault.sock` para que funcione perfeitamente
/// mesmo quando o navegador roda em sandbox Flatpak.
pub fn get_socket_path() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        let dir = PathBuf::from(home).join(".local/share/password-vault");
        let _ = std::fs::create_dir_all(&dir);
        dir.join("password-vault.sock")
    } else if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("password-vault.sock")
    } else {
        PathBuf::from("/tmp/password-vault.sock")
    }
}

/// Normaliza uma URL ou hostname para comparação de domínio (ex: "https://www.github.com/login" -> "github.com").
pub fn normalize_domain(raw: &str) -> String {
    let trimmed = raw.trim().to_lowercase();
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(&trimmed);
    let host_port = without_scheme.split('/').next().unwrap_or(without_scheme);
    let host = host_port.split(':').next().unwrap_or(host_port);
    host.strip_prefix("www.").unwrap_or(host).to_string()
}
