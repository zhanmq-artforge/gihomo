use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SubscriptionSource {
    Url(String),
    LocalFile(String),
    ShareLinks(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionUserInfo {
    pub upload: u64,
    pub download: u64,
    pub total: u64,
    pub expire: Option<i64>,
}

impl SubscriptionUserInfo {
    /// Formats bytes into human readable string (e.g. 1.25 GB)
    pub fn format_bytes(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = 1024 * KB;
        const GB: u64 = 1024 * MB;
        const TB: u64 = 1024 * GB;

        if bytes >= TB {
            format!("{:.2} TB", bytes as f64 / TB as f64)
        } else if bytes >= GB {
            format!("{:.2} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.2} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.2} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} B", bytes)
        }
    }

    pub fn usage_percentage(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            let used = self.upload + self.download;
            (used as f64 / self.total as f64).clamp(0.0, 1.0)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub id: String,
    pub name: String,
    pub source: SubscriptionSource,
    pub file_name: String,
    pub updated_at: Option<DateTime<Utc>>,
    pub etag: Option<String>,
    pub user_info: Option<SubscriptionUserInfo>,
    pub is_active: bool,
}

impl Subscription {
    pub fn new_url(name: String, url: String) -> Self {
        let id = uuid::Uuid::new_v4().to_string();
        let file_name = format!("{}.yaml", id);
        Self {
            id,
            name,
            source: SubscriptionSource::Url(url),
            file_name,
            updated_at: None,
            etag: None,
            user_info: None,
            is_active: false,
        }
    }

    pub fn new_local(name: String, path: String) -> Self {
        let id = uuid::Uuid::new_v4().to_string();
        let file_name = format!("{}.yaml", id);
        Self {
            id,
            name,
            source: SubscriptionSource::LocalFile(path),
            file_name,
            updated_at: Some(Utc::now()),
            etag: None,
            user_info: None,
            is_active: false,
        }
    }

    pub fn new_share_links(name: String, raw_links: String) -> Self {
        let id = uuid::Uuid::new_v4().to_string();
        let file_name = format!("{}.yaml", id);
        Self {
            id,
            name,
            source: SubscriptionSource::ShareLinks(raw_links),
            file_name,
            updated_at: Some(Utc::now()),
            etag: None,
            user_info: None,
            is_active: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrafficStats {
    pub up: u64,
    pub down: u64,
    pub up_total: u64,
    pub down_total: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KernelStatus {
    Running,
    Stopped,
    NotFound,
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyNode {
    pub name: String,
    pub node_type: String,
    pub delay: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyGroup {
    pub name: String,
    pub group_type: String,
    pub now: String,
    pub nodes: Vec<ProxyNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunConfig {
    pub enable: bool,
    pub stack: String,
    pub dns_hijack: Vec<String>,
    pub auto_route: bool,
    pub auto_detect_interface: bool,
}

impl Default for TunConfig {
    fn default() -> Self {
        Self {
            enable: false,
            stack: "mixed".to_string(),
            dns_hijack: vec!["any:53".to_string(), "tcp://any:53".to_string()],
            auto_route: true,
            auto_detect_interface: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleItem {
    pub rule_type: String,
    pub payload: String,
    pub proxy: String,
    pub size: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleProvider {
    pub name: String,
    pub provider_type: String,
    pub vehicle_type: String,
    pub rule_count: usize,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoDatabaseInfo {
    pub name: String,
    pub file_name: String,
    pub exists: bool,
    pub size_bytes: u64,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConnectionMetadata {
    #[serde(default)]
    pub network: String,
    #[serde(default, rename = "type")]
    pub conn_type: String,
    #[serde(default, rename = "sourceIP")]
    pub source_ip: String,
    #[serde(default, rename = "destinationIP")]
    pub destination_ip: String,
    #[serde(default, rename = "sourcePort")]
    pub source_port: String,
    #[serde(default, rename = "destinationPort")]
    pub destination_port: String,
    #[serde(default)]
    pub host: String,
    #[serde(default, rename = "processPath")]
    pub process_path: String,
}

impl ConnectionMetadata {
    pub fn process_name(&self) -> String {
        if self.process_path.is_empty() {
            return String::new();
        }
        std::path::Path::new(&self.process_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&self.process_path)
            .to_string()
    }

    pub fn destination(&self) -> String {
        if !self.host.is_empty() {
            if !self.destination_port.is_empty() {
                format!("{}:{}", self.host, self.destination_port)
            } else {
                self.host.clone()
            }
        } else if !self.destination_ip.is_empty() {
            if !self.destination_port.is_empty() {
                format!("{}:{}", self.destination_ip, self.destination_port)
            } else {
                self.destination_ip.clone()
            }
        } else {
            "-".to_string()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionItem {
    pub id: String,
    pub metadata: ConnectionMetadata,
    #[serde(default)]
    pub upload: u64,
    #[serde(default)]
    pub download: u64,
    #[serde(default)]
    pub start: String,
    #[serde(default)]
    pub chains: Vec<String>,
    #[serde(default)]
    pub rule: String,
    #[serde(default, rename = "rulePayload")]
    pub rule_payload: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConnectionsSnapshot {
    #[serde(default, rename = "downloadTotal")]
    pub download_total: u64,
    #[serde(default, rename = "uploadTotal")]
    pub upload_total: u64,
    #[serde(default)]
    pub connections: Vec<ConnectionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogMessage {
    #[serde(default, rename = "type")]
    pub level: String,
    #[serde(default)]
    pub payload: String,
}

