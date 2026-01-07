use serde::{Deserialize, Serialize};

/// WebDAV configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WebDAVConfig {
    pub url: String,
    pub username: String,
    pub password: String,
    pub remote_path: String,
}

/// S3 configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct S3Config {
    pub access_key: String,
    pub secret_key: String,
    pub bucket: String,
    pub region: String,
    pub prefix: String,
    pub endpoint_url: String,
    pub force_path_style: bool,
    pub public_domain: String,
}

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSettings {
    pub language: String,
    pub current_module: String,
    pub current_sub_tab: String,
    pub backup_type: String,
    pub local_backup_path: String,
    pub webdav: WebDAVConfig,
    pub s3: S3Config,
    pub last_backup_time: Option<String>,
}
