use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    Key, XChaCha20Poly1305, XNonce,
};
use rand::RngCore;
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

pub const KEY_LEN: usize = 32;
pub const SALT_LEN: usize = 16;
pub const XNONCE_LEN: usize = 24;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("Falha na derivação de chave Argon2id: {0}")]
    KdfError(String),
    #[error("Falha ao criptografar dados")]
    EncryptionError,
    #[error("Falha ao descriptografar dados (senha mestra incorreta ou dados corrompidos)")]
    DecryptionError,
    #[error("Tamanho de nonce ou salt inválido")]
    InvalidLength,
}

/// Chave criptográfica de 256 bits que é automaticamente zerada da RAM ao sair de escopo (Drop).
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SecretKey([u8; KEY_LEN]);

impl SecretKey {
    pub fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(bytes)
    }

    pub fn generate_random() -> Self {
        let mut bytes = [0u8; KEY_LEN];
        rand::thread_rng().fill_bytes(&mut bytes);
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }
}

/// Gera um salt aleatório de 128 bits (16 bytes).
pub fn generate_salt() -> [u8; SALT_LEN] {
    let mut salt = [0u8; SALT_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    salt
}

/// Gera um XNonce aleatório de 192 bits (24 bytes) para XChaCha20-Poly1305.
pub fn generate_xnonce() -> [u8; XNONCE_LEN] {
    let mut nonce = [0u8; XNONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce);
    nonce
}

/// Deriva uma Master Key de 256 bits a partir da Senha Mestra e do Salt usando Argon2id.
/// Parâmetros recomendados OWASP: 64 MiB de memória, 3 iterações, 4 lanes de paralelismo.
pub fn derive_master_key(master_password: &[u8], salt: &[u8]) -> Result<SecretKey, CryptoError> {
    let params = Params::new(64 * 1024, 3, 4, Some(KEY_LEN))
        .map_err(|e| CryptoError::KdfError(e.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut output_key = [0u8; KEY_LEN];
    argon2
        .hash_password_into(master_password, salt, &mut output_key)
        .map_err(|e| CryptoError::KdfError(e.to_string()))?;

    Ok(SecretKey::from_bytes(output_key))
}

/// Criptografa `plaintext` usando XChaCha20-Poly1305 (AEAD).
/// Retorna `(ciphertext_with_poly1305_tag, xnonce)`.
pub fn encrypt_aead(
    key: &SecretKey,
    plaintext: &[u8],
) -> Result<(Vec<u8>, [u8; XNONCE_LEN]), CryptoError> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key.as_bytes()));
    let nonce_bytes = generate_xnonce();
    let nonce = XNonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| CryptoError::EncryptionError)?;

    Ok((ciphertext, nonce_bytes))
}

/// Descriptografa `ciphertext` usando XChaCha20-Poly1305 (AEAD) e verifica a tag de integridade.
pub fn decrypt_aead(
    key: &SecretKey,
    ciphertext: &[u8],
    nonce_bytes: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    if nonce_bytes.len() != XNONCE_LEN {
        return Err(CryptoError::InvalidLength);
    }
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key.as_bytes()));
    let nonce = XNonce::from_slice(nonce_bytes);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| CryptoError::DecryptionError)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_envelope_encryption_roundtrip() {
        let mut master_password = b"MinhaSenhaForte!2026".to_vec();
        let salt = generate_salt();

        // 1. Deriva a Master Key
        let master_key = derive_master_key(&master_password, &salt).unwrap();
        master_password.zeroize();

        // 2. Gera uma Vault Key aleatória e a criptografa com a Master Key (Envelope Encryption)
        let vault_key = SecretKey::generate_random();
        let (encrypted_vault_key, vk_nonce) =
            encrypt_aead(&master_key, vault_key.as_bytes()).unwrap();

        // 3. Descriptografa a Vault Key com a Master Key
        let decrypted_vk_bytes =
            decrypt_aead(&master_key, &encrypted_vault_key, &vk_nonce).unwrap();
        let mut vk_array = [0u8; KEY_LEN];
        vk_array.copy_from_slice(&decrypted_vk_bytes);
        let recovered_vault_key = SecretKey::from_bytes(vk_array);

        // 4. Criptografa uma senha de usuário com a Vault Key
        let secret = b"github_super_secret_password";
        let (ciphertext, nonce) = encrypt_aead(&recovered_vault_key, secret).unwrap();
        let plaintext = decrypt_aead(&recovered_vault_key, &ciphertext, &nonce).unwrap();

        assert_eq!(plaintext, secret);
    }
}
