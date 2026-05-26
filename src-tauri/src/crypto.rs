use crate::storage::{AppError, AppResult};
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::fs;
use tauri::{AppHandle, Manager};

const SERVICE_NAME: &str = "com.wxxd.desktop";
const MASTER_KEY_ACCOUNT: &str = "wx-xd-master-key-v1";
const KEY_VERSION: &str = "v1";

#[derive(Debug)]
pub struct EncryptedText {
    pub ciphertext: String,
    pub nonce: String,
    pub key_version: String,
}

pub fn encrypt_secret(app: &AppHandle, plaintext: &str) -> AppResult<EncryptedText> {
    encrypt_text(app, plaintext)
}

pub fn decrypt_secret(app: &AppHandle, ciphertext: &str, nonce: &str) -> AppResult<String> {
    decrypt_text(app, ciphertext, nonce)
}

pub fn encrypt_access_token(app: &AppHandle, plaintext: &str) -> AppResult<EncryptedText> {
    encrypt_text(app, plaintext)
}

pub fn decrypt_access_token(app: &AppHandle, ciphertext: &str, nonce: &str) -> AppResult<String> {
    decrypt_text(app, ciphertext, nonce)
}

pub fn secret_fingerprint(secret: &str) -> String {
    let digest = Sha256::digest(secret.as_bytes());
    digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn encrypt_text(app: &AppHandle, plaintext: &str) -> AppResult<EncryptedText> {
    let key_bytes = master_key(app)?;
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plaintext.as_bytes())
        .map_err(|_| AppError::Security("敏感信息加密失败".to_string()))?;
    Ok(EncryptedText {
        ciphertext: STANDARD.encode(ciphertext),
        nonce: STANDARD.encode(nonce_bytes),
        key_version: KEY_VERSION.to_string(),
    })
}

fn decrypt_text(app: &AppHandle, ciphertext: &str, nonce: &str) -> AppResult<String> {
    let key_bytes = master_key(app)?;
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    let ciphertext_bytes = STANDARD.decode(ciphertext)?;
    let nonce_bytes = STANDARD.decode(nonce)?;
    let plaintext = cipher
        .decrypt(Nonce::from_slice(&nonce_bytes), ciphertext_bytes.as_ref())
        .map_err(|_| AppError::Security("敏感信息解密失败".to_string()))?;
    String::from_utf8(plaintext)
        .map_err(|_| AppError::Security("敏感信息不是有效 UTF-8".to_string()))
}

fn master_key(app: &AppHandle) -> AppResult<[u8; 32]> {
    if let Ok(key) = load_keychain_key() {
        return Ok(key);
    }
    load_local_key(app)
}

fn load_keychain_key() -> AppResult<[u8; 32]> {
    let entry = keyring::Entry::new(SERVICE_NAME, MASTER_KEY_ACCOUNT)?;
    match entry.get_password() {
        Ok(value) => decode_master_key(&value),
        Err(keyring::Error::NoEntry) => {
            let key = generate_master_key();
            entry.set_password(&STANDARD.encode(key))?;
            Ok(key)
        }
        Err(error) => Err(AppError::Keyring(error)),
    }
}

fn load_local_key(app: &AppHandle) -> AppResult<[u8; 32]> {
    let key_dir = app.path().app_data_dir()?.join("keys");
    fs::create_dir_all(&key_dir)?;
    let key_path = key_dir.join("master.key");
    if key_path.exists() {
        return decode_master_key(&fs::read_to_string(key_path)?);
    }
    let key = generate_master_key();
    fs::write(key_path, STANDARD.encode(key))?;
    Ok(key)
}

fn generate_master_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    key
}

fn decode_master_key(value: &str) -> AppResult<[u8; 32]> {
    let bytes = STANDARD.decode(value.trim())?;
    if bytes.len() != 32 {
        return Err(AppError::Security("主密钥长度不正确".to_string()));
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Ok(key)
}
