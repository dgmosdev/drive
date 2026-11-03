//! Ayarlar: API adresi ve Access Key düz JSON dosyasında, Secret Key ise
//! işletim sisteminin anahtar zincirinde (macOS Keychain, Windows Credential
//! Manager, Linux Secret Service) tutulur. Secret Key diske yazılmaz.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const KEYRING_SERVICE: &str = "com.dgmos.drive";
pub const DEFAULT_BASE_URL: &str = "https://sandbox.dgmos.com/api";
pub const DEFAULT_WEB_URL: &str = "https://go.dgmos.com";

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Settings {
    pub base_url: String,
    /// "Dosyalarımı aç" ve yardım bağlantılarının açıldığı web uygulaması.
    pub web_url: String,
    pub access_key: String,
    pub is_public: bool,
    /// Aynı anda S3'e giden PUT sayısı.
    pub concurrency: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.into(),
            web_url: DEFAULT_WEB_URL.into(),
            access_key: String::new(),
            is_public: true,
            concurrency: 4,
        }
    }
}

fn settings_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("dgmos-drive").join("settings.json"))
}

impl Settings {
    pub fn load() -> Self {
        settings_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = settings_path().context("Ayar klasörü bulunamadı")?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}

fn secret_entry(access_key: &str) -> Result<keyring::Entry> {
    Ok(keyring::Entry::new(KEYRING_SERVICE, access_key)?)
}

pub fn load_secret(access_key: &str) -> Option<String> {
    if access_key.is_empty() {
        return None;
    }
    secret_entry(access_key).ok()?.get_password().ok()
}

pub fn save_secret(access_key: &str, secret: &str) -> Result<()> {
    secret_entry(access_key)?
        .set_password(secret)
        .context("Secret Key anahtar zincirine kaydedilemedi")
}

pub fn delete_secret(access_key: &str) {
    if let Ok(entry) = secret_entry(access_key) {
        let _ = entry.delete_credential();
    }
}
