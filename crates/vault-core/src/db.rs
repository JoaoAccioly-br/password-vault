use crate::crypto::{
    decrypt_aead, derive_master_key, encrypt_aead, generate_salt, CryptoError, SecretKey, KEY_LEN,
};
use crate::ipc::{normalize_domain, CredentialDecrypted, CredentialSummary};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use uuid::Uuid;
use zeroize::Zeroize;

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("Erro no banco de dados SQLite: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Erro criptográfico: {0}")]
    Crypto(#[from] CryptoError),
    #[error("O cofre ainda não foi inicializado")]
    NotInitialized,
    #[error("O cofre já foi inicializado")]
    AlreadyInitialized,
    #[error("O cofre está bloqueado (Locked)")]
    Locked,
    #[error("Credencial não encontrada")]
    NotFound,
}

pub struct VaultDatabase {
    conn: Connection,
    /// A Vault Key só existe na memória RAM enquanto o cofre estiver desbloqueado.
    /// Ao bloquear (`lock()`) ou fechar o programa, o `SecretKey` sofre `ZeroizeOnDrop`.
    vault_key: Option<SecretKey>,
}

impl VaultDatabase {
    /// Abre ou cria o arquivo SQLite do cofre e inicializa o schema das tabelas.
    pub fn open<P: AsRef<Path>>(db_path: P) -> Result<Self, VaultError> {
        if let Some(parent) = db_path.as_ref().parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;

             CREATE TABLE IF NOT EXISTS vault_meta (
                 id INTEGER PRIMARY KEY CHECK (id = 1),
                 kdf_salt BLOB NOT NULL,
                 encrypted_vault_key BLOB NOT NULL,
                 vault_key_nonce BLOB NOT NULL,
                 created_at INTEGER NOT NULL
             );

             CREATE TABLE IF NOT EXISTS credentials (
                 id TEXT PRIMARY KEY,
                 domain TEXT NOT NULL,
                 title TEXT NOT NULL,
                 username TEXT NOT NULL,
                 encrypted_password BLOB NOT NULL,
                 password_nonce BLOB NOT NULL,
                 encrypted_notes BLOB,
                 notes_nonce BLOB,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL
             );

             CREATE INDEX IF NOT EXISTS idx_credentials_domain ON credentials(domain);",
        )?;

        Ok(Self {
            conn,
            vault_key: None,
        })
    }

    /// Verifica se o cofre já possui uma Senha Mestra configurada.
    pub fn is_initialized(&self) -> Result<bool, VaultError> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM vault_meta WHERE id = 1", [], |row| {
                row.get(0)
            })?;
        Ok(count > 0)
    }

    /// Verifica se o cofre está desbloqueado na memória RAM.
    pub fn is_unlocked(&self) -> bool {
        self.vault_key.is_some()
    }

    /// Bloqueia o cofre imediatamente, limpando a `Vault Key` da memória RAM via `Zeroize`.
    pub fn lock(&mut self) {
        self.vault_key = None;
    }

    /// Inicializa um novo cofre com a Senha Mestra usando Envelope Encryption:
    /// `Master Password` -> Argon2id -> `Master Key` -> criptografa `Vault Key` aleatória.
    pub fn initialize_vault(&mut self, mut master_password: String) -> Result<(), VaultError> {
        if self.is_initialized()? {
            master_password.zeroize();
            return Err(VaultError::AlreadyInitialized);
        }

        let salt = generate_salt();
        let master_key = derive_master_key(master_password.as_bytes(), &salt)?;
        master_password.zeroize();

        let vault_key = SecretKey::generate_random();
        let (encrypted_vault_key, vault_key_nonce) =
            encrypt_aead(&master_key, vault_key.as_bytes())?;

        let now = current_timestamp();
        self.conn.execute(
            "INSERT INTO vault_meta (id, kdf_salt, encrypted_vault_key, vault_key_nonce, created_at)
             VALUES (1, ?1, ?2, ?3, ?4)",
            params![
                salt.as_slice(),
                encrypted_vault_key,
                vault_key_nonce.as_slice(),
                now
            ],
        )?;

        self.vault_key = Some(vault_key);
        Ok(())
    }

    /// Desbloqueia um cofre existente derivando a `Master Key` e descriptografando a `Vault Key`.
    pub fn unlock(&mut self, mut master_password: String) -> Result<(), VaultError> {
        let row: Option<(Vec<u8>, Vec<u8>, Vec<u8>)> = self
            .conn
            .query_row(
                "SELECT kdf_salt, encrypted_vault_key, vault_key_nonce FROM vault_meta WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;

        let (salt, encrypted_vault_key, vault_key_nonce) = row.ok_or(VaultError::NotInitialized)?;

        let master_key = derive_master_key(master_password.as_bytes(), &salt)?;
        master_password.zeroize();

        let mut decrypted_vk = decrypt_aead(&master_key, &encrypted_vault_key, &vault_key_nonce)?;
        if decrypted_vk.len() != KEY_LEN {
            decrypted_vk.zeroize();
            return Err(VaultError::Crypto(CryptoError::InvalidLength));
        }

        let mut vk_bytes = [0u8; KEY_LEN];
        vk_bytes.copy_from_slice(&decrypted_vk);
        decrypted_vk.zeroize();

        self.vault_key = Some(SecretKey::from_bytes(vk_bytes));
        Ok(())
    }

    /// Adiciona ou atualiza uma credencial no cofre.
    pub fn save_credential(
        &self,
        domain: &str,
        title: &str,
        username: &str,
        mut password: String,
        mut notes: Option<String>,
    ) -> Result<CredentialSummary, VaultError> {
        let vault_key = self.vault_key.as_ref().ok_or(VaultError::Locked)?;
        let norm_domain = normalize_domain(domain);
        let clean_title = if title.trim().is_empty() {
            norm_domain.clone()
        } else {
            title.trim().to_string()
        };

        let (encrypted_password, password_nonce) = encrypt_aead(vault_key, password.as_bytes())?;
        password.zeroize();

        let (encrypted_notes, notes_nonce) = if let Some(ref mut n) = notes {
            let (enc, nonce) = encrypt_aead(vault_key, n.as_bytes())?;
            n.zeroize();
            (Some(enc), Some(nonce.to_vec()))
        } else {
            (None, None)
        };

        let now = current_timestamp();

        // Verifica se já existe credencial para o mesmo domínio + username (upsert para Auto-save)
        let existing_id: Option<(String, i64)> = self
            .conn
            .query_row(
                "SELECT id, created_at FROM credentials WHERE domain = ?1 AND username = ?2",
                params![norm_domain, username],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;

        if let Some((id, created_at)) = existing_id {
            self.conn.execute(
                "UPDATE credentials
                 SET title = ?1, encrypted_password = ?2, password_nonce = ?3,
                     encrypted_notes = ?4, notes_nonce = ?5, updated_at = ?6
                 WHERE id = ?7",
                params![
                    clean_title,
                    encrypted_password,
                    password_nonce.as_slice(),
                    encrypted_notes,
                    notes_nonce,
                    now,
                    id
                ],
            )?;

            Ok(CredentialSummary {
                id,
                domain: norm_domain,
                title: clean_title,
                username: username.to_string(),
                created_at,
                updated_at: now,
            })
        } else {
            let id = Uuid::new_v4().to_string();
            self.conn.execute(
                "INSERT INTO credentials (
                    id, domain, title, username,
                    encrypted_password, password_nonce,
                    encrypted_notes, notes_nonce,
                    created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    id,
                    norm_domain,
                    clean_title,
                    username,
                    encrypted_password,
                    password_nonce.as_slice(),
                    encrypted_notes,
                    notes_nonce,
                    now,
                    now
                ],
            )?;

            Ok(CredentialSummary {
                id,
                domain: norm_domain,
                title: clean_title,
                username: username.to_string(),
                created_at: now,
                updated_at: now,
            })
        }
    }

    /// Lista todos os logins (apenas metadados, sem descriptografar as senhas).
    pub fn list_summaries(&self) -> Result<Vec<CredentialSummary>, VaultError> {
        let _vault_key = self.vault_key.as_ref().ok_or(VaultError::Locked)?;
        let mut stmt = self.conn.prepare(
            "SELECT id, domain, title, username, created_at, updated_at
             FROM credentials
             ORDER BY updated_at DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(CredentialSummary {
                id: row.get(0)?,
                domain: row.get(1)?,
                title: row.get(2)?,
                username: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;

        let mut list = Vec::new();
        for item in rows {
            list.push(item?);
        }
        Ok(list)
    }

    /// Descriptografa uma credencial específica pelo ID.
    pub fn get_decrypted_by_id(&self, id: &str) -> Result<CredentialDecrypted, VaultError> {
        let vault_key = self.vault_key.as_ref().ok_or(VaultError::Locked)?;

        let row: Option<(
            String,
            String,
            String,
            String,
            Vec<u8>,
            Vec<u8>,
            Option<Vec<u8>>,
            Option<Vec<u8>>,
        )> = self
            .conn
            .query_row(
                "SELECT id, domain, title, username, encrypted_password, password_nonce, encrypted_notes, notes_nonce
                 FROM credentials WHERE id = ?1",
                params![id],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                        r.get(6)?,
                        r.get(7)?,
                    ))
                },
            )
            .optional()?;

        let (id, domain, title, username, enc_pw, pw_nonce, enc_notes, notes_nonce) =
            row.ok_or(VaultError::NotFound)?;

        let pw_bytes = decrypt_aead(vault_key, &enc_pw, &pw_nonce)?;
        let password = String::from_utf8_lossy(&pw_bytes).to_string();

        let notes = match (enc_notes, notes_nonce) {
            (Some(enc), Some(nonce)) => {
                let bytes = decrypt_aead(vault_key, &enc, &nonce)?;
                Some(String::from_utf8_lossy(&bytes).to_string())
            }
            _ => None,
        };

        Ok(CredentialDecrypted {
            id,
            domain,
            title,
            username,
            password,
            notes,
        })
    }

    /// Busca e descriptografa credenciais compatíveis com um domínio (para Auto-fill no navegador).
    pub fn get_logins_for_domain(
        &self,
        raw_domain: &str,
    ) -> Result<Vec<CredentialDecrypted>, VaultError> {
        let vault_key = self.vault_key.as_ref().ok_or(VaultError::Locked)?;
        let norm_domain = normalize_domain(raw_domain);

        let mut stmt = self.conn.prepare(
            "SELECT id, domain, title, username, encrypted_password, password_nonce, encrypted_notes, notes_nonce
             FROM credentials
             WHERE domain = ?1 OR ?1 LIKE '%.' || domain OR domain LIKE '%.' || ?1
             ORDER BY updated_at DESC",
        )?;

        let rows = stmt.query_map(params![norm_domain], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Vec<u8>>(4)?,
                r.get::<_, Vec<u8>>(5)?,
                r.get::<_, Option<Vec<u8>>>(6)?,
                r.get::<_, Option<Vec<u8>>>(7)?,
            ))
        })?;

        let mut results = Vec::new();
        for r in rows {
            let (id, domain, title, username, enc_pw, pw_nonce, enc_notes, notes_nonce) = r?;
            let pw_bytes = decrypt_aead(vault_key, &enc_pw, &pw_nonce)?;
            let password = String::from_utf8_lossy(&pw_bytes).to_string();
            let notes = match (enc_notes, notes_nonce) {
                (Some(enc), Some(nonce)) => {
                    let bytes = decrypt_aead(vault_key, &enc, &nonce)?;
                    Some(String::from_utf8_lossy(&bytes).to_string())
                }
                _ => None,
            };
            results.push(CredentialDecrypted {
                id,
                domain,
                title,
                username,
                password,
                notes,
            });
        }

        Ok(results)
    }

    /// Verifica se já existe credencial salva para (domain, username).
    pub fn check_login_exists(&self, raw_domain: &str, username: &str) -> Result<bool, VaultError> {
        let _vault_key = self.vault_key.as_ref().ok_or(VaultError::Locked)?;
        let norm_domain = normalize_domain(raw_domain);
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM credentials WHERE domain = ?1 AND username = ?2",
            params![norm_domain, username],
            |r| r.get(0),
        )?;
        Ok(count > 0)
    }

    /// Remove uma credencial pelo ID.
    pub fn delete_credential(&self, id: &str) -> Result<(), VaultError> {
        let _vault_key = self.vault_key.as_ref().ok_or(VaultError::Locked)?;
        let affected = self
            .conn
            .execute("DELETE FROM credentials WHERE id = ?1", params![id])?;
        if affected == 0 {
            return Err(VaultError::NotFound);
        }
        Ok(())
    }
}

fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
