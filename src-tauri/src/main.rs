#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod auto_setup;
mod socket_server;

use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::State;
use vault_core::{get_socket_path, CredentialDecrypted, CredentialSummary, VaultDatabase};

pub struct AppState {
    pub db: Arc<Mutex<VaultDatabase>>,
    pub db_path: PathBuf,
}

#[derive(Serialize)]
pub struct VaultStatus {
    pub initialized: bool,
    pub unlocked: bool,
    pub socket_path: String,
    pub db_path: String,
}

#[tauri::command]
fn get_vault_status(state: State<'_, AppState>) -> Result<VaultStatus, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let initialized = guard.is_initialized().map_err(|e| e.to_string())?;
    let unlocked = guard.is_unlocked();

    Ok(VaultStatus {
        initialized,
        unlocked,
        socket_path: get_socket_path().to_string_lossy().to_string(),
        db_path: state.db_path.to_string_lossy().to_string(),
    })
}

#[tauri::command]
fn initialize_vault(master_password: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.db.lock().map_err(|e| e.to_string())?;
    guard
        .initialize_vault(master_password)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn unlock_vault(master_password: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.db.lock().map_err(|e| e.to_string())?;
    guard.unlock(master_password).map_err(|e| e.to_string())
}

#[tauri::command]
fn lock_vault(state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.db.lock().map_err(|e| e.to_string())?;
    guard.lock();
    Ok(())
}

#[tauri::command]
fn list_credentials(state: State<'_, AppState>) -> Result<Vec<CredentialSummary>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    guard.list_summaries().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_credential_decrypted(
    id: String,
    state: State<'_, AppState>,
) -> Result<CredentialDecrypted, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    guard.get_decrypted_by_id(&id).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_credential(
    domain: String,
    title: String,
    username: String,
    password: String,
    notes: Option<String>,
    state: State<'_, AppState>,
) -> Result<CredentialSummary, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    guard
        .save_credential(&domain, &title, &username, password, notes)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_credential(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    guard.delete_credential(&id).map_err(|e| e.to_string())
}

fn resolve_vault_db_path() -> PathBuf {
    let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("password-vault").join("vault.db")
}

fn main() {
    // Auto-configura os manifestos do Native Messaging para o usuário ao abrir o app
    auto_setup::ensure_native_messaging_installed();

    let db_path = resolve_vault_db_path();
    let vault_db = VaultDatabase::open(&db_path).expect("Falha ao abrir banco SQLite do cofre");
    let shared_db = Arc::new(Mutex::new(vault_db));

    tauri::Builder::default()
        .manage(AppState {
            db: Arc::clone(&shared_db),
            db_path,
        })
        .setup(move |app| {
            let handle = app.handle().clone();
            socket_server::start_unix_socket_server(Arc::clone(&shared_db), handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_vault_status,
            initialize_vault,
            unlock_vault,
            lock_vault,
            list_credentials,
            get_credential_decrypted,
            save_credential,
            delete_credential
        ])
        .run(tauri::generate_context!())
        .expect("Erro ao executar aplicativo Tauri");
}
