//! Storage manager and disk file orchestration for Gihomo.

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, IF_NONE_MATCH, USER_AGENT};
use tokio::fs;
use tracing::info;

use gihomo_core::{parse_user_info_header, Subscription, SubscriptionUserInfo};

use crate::error::InfraError;

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

/// Result of downloading a remote subscription resource, supporting HTTP 304 Not Modified.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum RemoteSubscriptionDownload {
    Modified {
        content: String,
        etag: Option<String>,
        user_info: Option<SubscriptionUserInfo>,
    },
    NotModified {
        etag: Option<String>,
        user_info: Option<SubscriptionUserInfo>,
    },
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

    /// Inspects and lists local GeoIP and GeoSite database metadata.
    pub async fn geo_databases(&self) -> Vec<gihomo_core::GeoDatabaseInfo> {
        let mut list = Vec::new();

        // 1. GeoIP
        let geoip_p = self.geoip_path();
        let (exists, size, updated) = if geoip_p.exists() {
            if let Ok(meta) = fs::metadata(&geoip_p).await {
                let size = meta.len();
                let mtime = meta
                    .modified()
                    .ok()
                    .map(chrono::DateTime::<chrono::Utc>::from);
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
                let mtime = meta
                    .modified()
                    .ok()
                    .map(chrono::DateTime::<chrono::Utc>::from);
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

    pub async fn download_file_to(&self, url: &str, dest_path: &Path) -> Result<(), InfraError> {
        info!("Downloading file from {} to {:?}", url, dest_path);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()?;

        let resp = client.get(url).send().await?;
        if !resp.status().is_success() {
            return Err(InfraError::DownloadFailed {
                status: resp.status().as_u16(),
                message: format!("HTTP error {}", resp.status()),
            });
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

    /// Downloads a remote subscription, handles HTTP 304 Not Modified
    pub async fn download_remote_subscription(
        &self,
        url: &str,
        etag: Option<&str>,
    ) -> Result<RemoteSubscriptionDownload, InfraError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(
                "Elephant Network/v2.1.0 ClashMeta; Clash.Meta; mihomo; art.artforge.Gihomo/1.3.3",
            ),
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

        if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(RemoteSubscriptionDownload::NotModified {
                etag: new_etag,
                user_info,
            });
        }

        if !resp.status().is_success() {
            return Err(InfraError::DownloadFailed {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }

        let content = resp.text().await?;
        Ok(RemoteSubscriptionDownload::Modified {
            content,
            etag: new_etag,
            user_info,
        })
    }

    /// Computes total disk space consumed by all kernel log files in bytes.
    pub async fn kernel_logs_total_size(&self) -> u64 {
        let logs_dir = self.logs_dir();
        if !logs_dir.exists() {
            return 0;
        }

        let mut total = 0;
        let Ok(mut entries) = fs::read_dir(&logs_dir).await else {
            return 0;
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            if let Ok(meta) = entry.metadata().await {
                if meta.is_file() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("log") {
                        total += meta.len();
                    }
                }
            }
        }

        total
    }

    /// Truncates active kernel log and removes all archived log files to free disk space.
    pub async fn clear_all_kernel_logs(&self) -> Result<(), InfraError> {
        let logs_dir = self.logs_dir();
        if !logs_dir.exists() {
            return Ok(());
        }

        // Truncate active mihomo.log to zero bytes
        let active_log = self.kernel_log_path();
        if active_log.exists() {
            let _ = fs::write(&active_log, b"").await;
        }

        // Delete all archived / rotated log files
        let Ok(mut entries) = fs::read_dir(&logs_dir).await else {
            return Ok(());
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path != active_log && path.extension().and_then(|s| s.to_str()) == Some("log") {
                let _ = fs::remove_file(&path).await;
            }
        }

        Ok(())
    }

    /// Rotates oversized active log file and purges archived logs older than the retention period.
    pub async fn rotate_and_clean_kernel_logs(
        &self,
        retention: Option<std::time::Duration>,
        safety_cap_bytes: u64,
    ) -> Result<u64, InfraError> {
        let logs_dir = self.logs_dir();
        if !logs_dir.exists() {
            return Ok(0);
        }

        let mut freed_bytes: u64 = 0;
        let active_log = self.kernel_log_path();

        // 1. Safety Cap Enforcement on Active Log
        if active_log.exists() {
            if let Ok(meta) = fs::metadata(&active_log).await {
                if meta.len() >= safety_cap_bytes {
                    tracing::info!(
                        size = meta.len(),
                        cap = safety_cap_bytes,
                        "Active log exceeded safety cap; rotating archive"
                    );
                    let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
                    let archive_path = logs_dir.join(format!("mihomo-{}.log", timestamp));

                    if fs::rename(&active_log, &archive_path).await.is_ok() {
                        let _ = fs::write(&active_log, b"").await;
                    }
                }
            }
        }

        // 2. Prune Archived Logs Exceeding Retention Window
        if let Some(max_age) = retention {
            let now = std::time::SystemTime::now();
            let Ok(mut entries) = fs::read_dir(&logs_dir).await else {
                return Ok(freed_bytes);
            };

            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path == active_log {
                    continue;
                }

                if path.extension().and_then(|s| s.to_str()) == Some("log") {
                    if let Ok(meta) = entry.metadata().await {
                        if let Ok(mtime) = meta.modified() {
                            if let Ok(age) = now.duration_since(mtime) {
                                if age > max_age {
                                    tracing::info!(
                                        file = ?path.file_name(),
                                        age_secs = age.as_secs(),
                                        "Pruning expired kernel log archive"
                                    );
                                    let size = meta.len();
                                    if fs::remove_file(&path).await.is_ok() {
                                        freed_bytes += size;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(freed_bytes)
    }

    /// Reads recent kernel log messages using tail-seeking to prevent large heap allocations.
    pub async fn read_recent_kernel_logs(&self, max_lines: usize) -> Vec<gihomo_core::LogMessage> {
        let p = self.kernel_log_path();
        if !p.exists() {
            return Vec::new();
        }

        use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};
        let Ok(mut file) = fs::File::open(&p).await else {
            return Vec::new();
        };

        let Ok(meta) = file.metadata().await else {
            return Vec::new();
        };

        let file_len = meta.len();
        if file_len == 0 {
            return Vec::new();
        }

        // Only read at most the last 256KB to maintain O(1) heap allocation even on large files
        const MAX_CHUNK_BYTES: u64 = 256 * 1024;
        let seek_pos = file_len.saturating_sub(MAX_CHUNK_BYTES);
        if file.seek(SeekFrom::Start(seek_pos)).await.is_err() {
            return Vec::new();
        }

        let mut buffer = Vec::with_capacity((file_len - seek_pos) as usize);
        if file.read_to_end(&mut buffer).await.is_err() {
            return Vec::new();
        }

        let content = String::from_utf8_lossy(&buffer);
        let lines: Vec<&str> = content.lines().collect();
        let start = lines.len().saturating_sub(max_lines);
        lines[start..]
            .iter()
            .map(|&line| {
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

    #[tokio::test]
    async fn test_kernel_logs_lifecycle() {
        let base_dir =
            std::env::temp_dir().join(format!("gihomo-log-test-{}", uuid::Uuid::new_v4()));
        let storage = StorageManager {
            base_dir: base_dir.clone(),
        };
        storage
            .init_environment()
            .await
            .expect("storage directories should initialize");

        let active_log = storage.logs_dir().join("mihomo.log");
        let archive_log = storage.logs_dir().join("mihomo-2026-01-01T00-00-00.log");

        // Write some log lines
        let sample_lines = "line 1: info\nline 2: debug\nline 3: warn\n";
        tokio::fs::write(&active_log, sample_lines)
            .await
            .expect("active log write should succeed");
        tokio::fs::write(&archive_log, "old line\n")
            .await
            .expect("archive log write should succeed");

        // Verify total size
        let total = storage.kernel_logs_total_size().await;
        assert_eq!(total, (sample_lines.len() + "old line\n".len()) as u64);

        // Verify recent logs tail reading
        let logs = storage.read_recent_kernel_logs(2).await;
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].payload, "line 2: debug");
        assert_eq!(logs[1].payload, "line 3: warn");

        // Test rotation and cleanup with zero retention (should delete archive log)
        let removed = storage
            .rotate_and_clean_kernel_logs(Some(Duration::from_secs(0)), 10)
            .await
            .expect("clean should succeed");
        assert!(removed >= 1);
        assert!(!archive_log.exists());

        // Test clear_all_kernel_logs
        storage
            .clear_all_kernel_logs()
            .await
            .expect("clear all logs should succeed");
        assert_eq!(storage.kernel_logs_total_size().await, 0);

        fs::remove_dir_all(base_dir)
            .await
            .expect("test storage should be removed");
    }
}
