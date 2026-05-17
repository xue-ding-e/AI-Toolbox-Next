use super::types::GatewayCliKey;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyGatewayPaths {
    root: PathBuf,
}

impl ProxyGatewayPaths {
    pub fn new(app_data_dir: impl Into<PathBuf>) -> Self {
        Self {
            root: app_data_dir.into().join("proxy-gateway"),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn cli_proxy_dir(&self, cli_key: GatewayCliKey) -> PathBuf {
        self.root.join("cli-proxy").join(cli_key.as_str())
    }

    pub fn manifest_path(&self, cli_key: GatewayCliKey) -> PathBuf {
        self.cli_proxy_dir(cli_key).join("manifest.json")
    }

    pub fn backup_dir(&self, cli_key: GatewayCliKey) -> PathBuf {
        self.cli_proxy_dir(cli_key).join("backups")
    }

    pub fn model_health_path(&self) -> PathBuf {
        self.root.join("state").join("model-health.json")
    }

    pub fn request_log_root(&self) -> PathBuf {
        self.root.join("request-logs")
    }

    pub fn metrics_root(&self) -> PathBuf {
        self.root.join("metrics")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_paths_are_cli_scoped() {
        let paths = ProxyGatewayPaths::new(PathBuf::from("app-data"));

        assert_eq!(
            paths.manifest_path(GatewayCliKey::Claude),
            PathBuf::from("app-data")
                .join("proxy-gateway")
                .join("cli-proxy")
                .join("claude")
                .join("manifest.json")
        );
        assert_eq!(
            paths.manifest_path(GatewayCliKey::Codex),
            PathBuf::from("app-data")
                .join("proxy-gateway")
                .join("cli-proxy")
                .join("codex")
                .join("manifest.json")
        );
        assert_eq!(
            paths.manifest_path(GatewayCliKey::Gemini),
            PathBuf::from("app-data")
                .join("proxy-gateway")
                .join("cli-proxy")
                .join("gemini")
                .join("manifest.json")
        );
    }

    #[test]
    fn metrics_root_is_file_based() {
        let paths = ProxyGatewayPaths::new(PathBuf::from("app-data"));
        assert_eq!(
            paths.metrics_root(),
            PathBuf::from("app-data")
                .join("proxy-gateway")
                .join("metrics")
        );
    }
}
