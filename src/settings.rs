use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use bincode;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::constants::{DEFAULT_SETTINGS_PATH, KSPK_ENCRYPTION_KEY};
use crate::utils::generate_username;
use kaspa_wallet_core::prelude::{Language, Mnemonic, WordCount};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsData {
    pub mnemonic: String,
    pub username: String,
}

impl Default for SettingsData {
    fn default() -> Self {
        SettingsData { mnemonic: "None".to_string(), username: "DefaultUser".to_string() }
    }
}

pub struct Settings {
    pub current: SettingsData,
    config_path: PathBuf,
}

impl Settings {
    pub fn new() -> Self {
        let path = PathBuf::from(DEFAULT_SETTINGS_PATH);
        Settings { current: SettingsData::default(), config_path: path }
    }

    pub fn load(&mut self) -> Result<(), String> {
        if !self.config_path.exists() {
            return Err("NoFile".to_string());
        }
        let encrypted_data = fs::read(&self.config_path).map_err(|e| format!("Error reading file {:?}: {}", self.config_path, e))?;
        let decrypted = self.decrypt_data(&encrypted_data).map_err(|e| format!("Error decrypting: {:?}", e))?;
        let data: SettingsData = bincode::deserialize(&decrypted).map_err(|e| format!("Bincode deserialize error: {:?}", e))?;
        self.current = data;
        Ok(())
    }

    /// Сохраняем конфиг в файл
    pub fn save(&self) -> Result<(), String> {
        let serialized = bincode::serialize(&self.current).map_err(|e| format!("Bincode serialize error: {:?}", e))?;
        let encrypted = self.encrypt_data(&serialized).map_err(|e| format!("Error encrypting: {:?}", e))?;
        fs::write(&self.config_path, encrypted).map_err(|e| format!("Error writing file {:?}: {}", self.config_path, e))?;
        Ok(())
    }

    /// Инициализация настроек при отсутствии файла
    pub fn initialize_settings(&mut self) -> Result<(), String> {
        let mnemonic =
            Mnemonic::random(WordCount::Words12, Language::English).map_err(|_| "Failed to generate mnemonic".to_string())?;
        let mnemonic_str = mnemonic.phrase().to_string();
        let username = generate_username(mnemonic_str.as_str());
        self.current.mnemonic = mnemonic_str;
        self.current.username = username;
        self.save()?;
        Ok(())
    }

    // ------------------------------------------------------
    // Методы шифрования/дешифрования
    // ------------------------------------------------------
    fn encrypt_data(&self, plaintext: &[u8]) -> Result<Vec<u8>, aes_gcm::Error> {
        let key = Key::<Aes256Gcm>::from_slice(&KSPK_ENCRYPTION_KEY);
        let cipher = Aes256Gcm::new(key);

        // Генерируем nonce (12 байт).
        let nonce_bytes: [u8; 12] = rand::random();
        let nonce = Nonce::from_slice(&nonce_bytes);

        let mut ciphertext = cipher.encrypt(nonce, plaintext)?;

        let mut combined = nonce_bytes.to_vec();
        combined.append(&mut ciphertext);
        Ok(combined)
    }

    fn decrypt_data(&self, combined: &[u8]) -> Result<Vec<u8>, aes_gcm::Error> {
        if combined.len() < 12 {
            return Err(aes_gcm::Error);
        }
        let key = Key::<Aes256Gcm>::from_slice(&KSPK_ENCRYPTION_KEY);
        let cipher = Aes256Gcm::new(key);
        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        let plaintext = cipher.decrypt(nonce, ciphertext)?;
        Ok(plaintext)
    }
}
