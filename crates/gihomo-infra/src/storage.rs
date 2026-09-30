use crate::error::InfraError;
use gihomo_core::{parse_user_info_header, Subscription, SubscriptionUserInfo};
use reqwest::header::{HeaderMap, HeaderValue, IF_NONE_MATCH, USER_AGENT};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs;
use tracing::info;

pub const APP_ID: &str = "art.artforge.Gihomo";

#[derive(Clone, Debug)]
pub struct StorageManager {
    base_dir: PathBuf,
}

impl Default for StorageManager {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageManager {
    pub fn new() -> Self {
        let base_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(APP_ID);
        Self { base_dir }
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    pub fn subscriptions_dir(&self) -> PathBuf {
        self.base_dir.join("subscriptions")
    }

    pub fn mihomo_dir(&self) -> PathBuf {
        self.base_dir.join("mihomo")
    }

    pub fn bin_dir(&self) -> PathBuf {
        self.base_dir.join("bin")
    }

    pub fn default_kernel_path(&self) -> PathBuf {
        self.bin_dir().join("mihomo")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.base_dir.join("logs")
    }

    pub fn kernel_log_path(&self) -> PathBuf {
        self.logs_dir().join("mihomo.log")
    }

    pub fn pid_file_path(&self) -> PathBuf {
        self.base_dir.join("mihomo.pid")
    }

    pub fn active_config_path(&self) -> PathBuf {
        self.mihomo_dir().join("config.yaml")
    }

    pub fn subscriptions_json_path(&self) -> PathBuf {
        self.base_dir.join("subscriptions.json")
    }

    pub fn geoip_path(&self) -> PathBuf {
        self.mihomo_dir().join("geoip.metadb")
    }

    pub fn geosite_path(&self) -> PathBuf {
        self.mihomo_dir().join("geosite.dat")
    }

    pub async fn get_geo_databases(&self) -> Vec<gihomo_core::GeoDatabaseInfo> {
        let mut list = Vec::new();

        // 1. GeoIP
        let geoip_p = self.geoip_path();
        let (exists, size, updated) = if geoip_p.exists() {
            if let Ok(meta) = fs::metadata(&geoip_p).await {
                let size = meta.len();
                let mtime = meta.modified().ok().map(chrono::DateTime::<chrono::Utc>::from);
                (true, size, mtime)
            } else {
                (true, 0, None)
            }
        } else {
            (false, 0, None)
        };
        list.push(gihomo_core::GeoDatabaseInfo {
            name: "GeoIP".to_string(),
            file_name: "geoip.metadb".to_string(),
            exists,
            size_bytes: size,
            updated_at: updated,
        });

        // 2. GeoSite
        let geosite_p = self.geosite_path();
        let (exists, size, updated) = if geosite_p.exists() {
            if let Ok(meta) = fs::metadata(&geosite_p).await {
                let size = meta.len();
                let mtime = meta.modified().ok().map(chrono::DateTime::<chrono::Utc>::from);
                (true, size, mtime)
            } else {
                (true, 0, None)
            }
        } else {
            (false, 0, None)
        };
        list.push(gihomo_core::GeoDatabaseInfo {
            name: "GeoSite".to_string(),
            file_name: "geosite.dat".to_string(),
            exists,
            size_bytes: size,
            updated_at: updated,
        });

        list
    }

    pub async fn download_file_to(
        &self,
        url: &str,
        dest_path: &Path,
    ) -> Result<(), InfraError> {
        info!("Downloading file from {} to {:?}", url, dest_path);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()?;

        let resp = client.get(url).send().await?;
        if !resp.status().is_success() {
            return Err(InfraError::DownloadFailed(
                resp.status().as_u16(),
                format!("HTTP error {}", resp.status()),
            ));
        }

        let bytes = resp.bytes().await?;
        let tmp_path = dest_path.with_extension("tmp");
        fs::write(&tmp_path, bytes).await?;
        fs::rename(&tmp_path, dest_path).await?;
        info!("Successfully saved downloaded file to {:?}", dest_path);
        Ok(())
    }

    /// Ensure all storage directories exist
    pub async fn init_environment(&self) -> Result<(), InfraError> {
        fs::create_dir_all(self.subscriptions_dir()).await?;
        fs::create_dir_all(self.mihomo_dir()).await?;
        fs::create_dir_all(self.bin_dir()).await?;
        fs::create_dir_all(self.logs_dir()).await?;
        #[cfg(unix)]
        fs::set_permissions(self.mihomo_dir(), std::fs::Permissions::from_mode(0o700)).await?;
        Ok(())
    }

    pub async fn load_or_create_controller_secret(&self) -> Result<String, InfraError> {
        let path = self.mihomo_dir().join("controller.secret");
        match fs::read_to_string(&path).await {
            Ok(secret) if !secret.trim().is_empty() => {
                #[cfg(unix)]
                fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).await?;
                return Ok(secret.trim().to_string());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }

        let secret = uuid::Uuid::new_v4().to_string();
        fs::write(&path, &secret).await?;
        #[cfg(unix)]
        fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).await?;
        Ok(secret)
    }

    pub async fn load_subscriptions(&self) -> Result<Vec<Subscription>, InfraError> {
        let path = self.subscriptions_json_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let data = fs::read(&path).await?;
        let subs: Vec<Subscription> = serde_json::from_slice(&data)?;
        Ok(subs)
    }

    pub async fn save_subscriptions(&self, subs: &[Subscription]) -> Result<(), InfraError> {
        let path = self.subscriptions_json_path();
        let data = serde_json::to_vec_pretty(subs)?;
        fs::write(path, data).await?;
        Ok(())
    }

    pub async fn save_subscription_content(
        &self,
        id: &str,
        content: &str,
    ) -> Result<PathBuf, InfraError> {
        let file_path = self.subscriptions_dir().join(format!("{}.yaml", id));
        fs::write(&file_path, content).await?;
        Ok(file_path)
    }

    pub async fn read_subscription_content(&self, id: &str) -> Result<String, InfraError> {
        let file_path = self.subscriptions_dir().join(format!("{}.yaml", id));
        let content = fs::read_to_string(&file_path).await?;
        Ok(content)
    }

    pub async fn delete_subscription(&self, id: &str) -> Result<(), InfraError> {
        let file_path = self.subscriptions_dir().join(format!("{}.yaml", id));
        if file_path.exists() {
            let _ = fs::remove_file(file_path).await;
        }

        let mut subs = self.load_subscriptions().await?;
        subs.retain(|s| s.id != id);
        self.save_subscriptions(&subs).await?;
        Ok(())
    }

    pub async fn write_active_config(&self, yaml_content: &str) -> Result<PathBuf, InfraError> {
        let path = self.active_config_path();
        fs::write(&path, yaml_content).await?;
        info!("Saved active synthesized config to {:?}", path);
        Ok(path)
    }

    /// Downloads a remote subscription, returns (content, etag, user_info)
    pub async fn download_remote_subscription(
        &self,
        url: &str,
        etag: Option<&str>,
    ) -> Result<(String, Option<String>, Option<SubscriptionUserInfo>), InfraError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("Clash.Meta; mihomo; art.artforge.Gihomo/0.1.0"),
        );

        if let Some(tag) = etag {
            if let Ok(v) = HeaderValue::from_str(tag) {
                headers.insert(IF_NONE_MATCH, v);
            }
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .default_headers(headers)
            .build()?;

        let resp = client.get(url).send().await?;

        if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
            return Err(InfraError::DownloadFailed(
                304,
                "Subscription not modified".to_string(),
            ));
        }

        if !resp.status().is_success() {
            return Err(InfraError::DownloadFailed(
                resp.status().as_u16(),
                resp.text().await.unwrap_or_default(),
            ));
        }

        let new_etag = resp
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let user_info = resp
            .headers()
            .get("subscription-userinfo")
            .and_then(|v| v.to_str().ok())
            .and_then(parse_user_info_header);

        let content = resp.text().await?;
        Ok((content, new_etag, user_info))
    }

    pub async fn read_recent_kernel_logs(&self, max_lines: usize) -> Vec<gihomo_core::LogMessage> {
        let p = self.kernel_log_path();
        if !p.exists() {
            return Vec::new();
        }
        let Ok(content) = fs::read_to_string(&p).await else {
            return Vec::new();
        };

        content
            .lines()
            .rev()
            .take(max_lines)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|line| {
                let lower = line.to_lowercase();
                let level = if lower.contains("error") || lower.contains("fatal") {
                    "error"
                } else if lower.contains("warn") {
                    "warning"
                } else if lower.contains("debug") {
                    "debug"
                } else {
                    "info"
                };
                gihomo_core::LogMessage {
                    level: level.to_string(),
                    payload: line.to_string(),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn controller_secret_is_stable_and_private() {
        let base_dir =
            std::env::temp_dir().join(format!("gihomo-storage-test-{}", uuid::Uuid::new_v4()));
        let storage = StorageManager {
            base_dir: base_dir.clone(),
        };
        storage
            .init_environment()
            .await
            .expect("storage directories should initialize");

        let first_secret = storage
            .load_or_create_controller_secret()
            .await
            .expect("controller secret should be created");
        let second_secret = storage
            .load_or_create_controller_secret()
            .await
            .expect("controller secret should be loaded");
        assert_eq!(first_secret, second_secret);

        #[cfg(unix)]
        {
            let secret_path = storage.mihomo_dir().join("controller.secret");
            let mode = std::fs::metadata(secret_path)
                .expect("controller secret should exist")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }

        fs::remove_dir_all(base_dir)
            .await
            .expect("test storage should be removed");
    }
}
