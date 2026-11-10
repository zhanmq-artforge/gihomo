//! Domain models for subscription management, proxy groups, rules, connections, and system states.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Identifies the provenance or storage source of a subscription.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub enum SubscriptionSource {
    /// Remote HTTP/HTTPS subscription URL.
    Url(String),
    /// Local file path on disk.
    LocalFile(String),
    /// Raw batch share links text (e.g. ss://, vmess://).
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

/// Operational status of the supervised Mihomo kernel process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum KernelStatus {
    /// Kernel process is actively running and responsive.
    Running,
    /// Kernel process is stopped.
    Stopped,
    /// Kernel executable was not found on system or local paths.
    NotFound,
    /// Error encountered when launching or monitoring kernel.
    Error(String),
}

impl std::fmt::Display for KernelStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running => write!(f, "Running"),
            Self::Stopped => write!(f, "Stopped"),
            Self::NotFound => write!(f, "NotFound"),
            Self::Error(e) => write!(f, "Error: {}", e),
        }
    }
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

/// Returns whether the proxy node is a virtual/system proxy (e.g. Direct, Reject, Selector, etc.)
/// rather than a real remote outbound proxy that can be delay-tested.
#[must_use]
pub fn is_virtual_node(node_type: &str) -> bool {
    matches!(
        node_type,
        "Direct"
            | "Reject"
            | "RejectDrop"
            | "Pass"
            | "PassRule"
            | "Compatible"
            | "Selector"
            | "URLTest"
            | "Fallback"
            | "LoadBalance"
    )
}

/// Computes the minimal set of proxy group names that cover all unique testable proxy nodes.
///
/// Uses a greedy set-cover heuristic to minimize redundant delay tests across groups that share
/// the same underlying nodes. Returns `(selected_group_names, total_unique_node_count)`.
#[must_use]
pub fn calculate_minimal_group_coverage<'a>(groups: &'a [ProxyGroup]) -> (Vec<&'a str>, usize) {
    use std::collections::HashSet;

    let mut all_unique_nodes = HashSet::new();
    let mut group_node_sets: Vec<(&'a str, HashSet<&'a str>)> = Vec::with_capacity(groups.len());

    for group in groups {
        let node_set: HashSet<&'a str> = group
            .nodes
            .iter()
            .filter(|node| !is_virtual_node(&node.node_type))
            .map(|node| node.name.as_str())
            .collect();

        for &name in &node_set {
            all_unique_nodes.insert(name);
        }
        group_node_sets.push((group.name.as_str(), node_set));
    }

    let total_unique_count = all_unique_nodes.len();
    if total_unique_count == 0 {
        return (Vec::new(), 0);
    }

    let mut uncovered: HashSet<&'a str> = all_unique_nodes;
    let mut selected_groups = Vec::new();

    while !uncovered.is_empty() {
        let mut best_group_idx = None;
        let mut best_overlap_count = 0;
        let mut best_total_size = 0;

        for (idx, (_, node_set)) in group_node_sets.iter().enumerate() {
            let overlap = node_set
                .iter()
                .filter(|node| uncovered.contains(*node))
                .count();
            if overlap > best_overlap_count
                || (overlap == best_overlap_count
                    && overlap > 0
                    && node_set.len() > best_total_size)
            {
                best_overlap_count = overlap;
                best_total_size = node_set.len();
                best_group_idx = Some(idx);
            }
        }

        match best_group_idx {
            Some(idx) if best_overlap_count > 0 => {
                let (group_name, node_set) = &group_node_sets[idx];
                selected_groups.push(*group_name);
                for node in node_set {
                    uncovered.remove(node);
                }
            }
            _ => {
                break;
            }
        }
    }

    (selected_groups, total_unique_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_node(name: &str, node_type: &str) -> ProxyNode {
        ProxyNode {
            name: name.to_string(),
            node_type: node_type.to_string(),
            delay: None,
        }
    }

    fn mock_group(name: &str, nodes: Vec<ProxyNode>) -> ProxyGroup {
        ProxyGroup {
            name: name.to_string(),
            group_type: "Selector".to_string(),
            now: String::new(),
            nodes,
        }
    }

    #[test]
    fn test_is_virtual_node() {
        assert!(is_virtual_node("Direct"));
        assert!(is_virtual_node("Reject"));
        assert!(is_virtual_node("RejectDrop"));
        assert!(is_virtual_node("Pass"));
        assert!(is_virtual_node("PassRule"));
        assert!(is_virtual_node("Compatible"));
        assert!(is_virtual_node("Selector"));
        assert!(is_virtual_node("URLTest"));
        assert!(is_virtual_node("Fallback"));
        assert!(is_virtual_node("LoadBalance"));

        assert!(!is_virtual_node("Shadowsocks"));
        assert!(!is_virtual_node("Vmess"));
        assert!(!is_virtual_node("Vless"));
        assert!(!is_virtual_node("Trojan"));
        assert!(!is_virtual_node("Hysteria2"));
        assert!(!is_virtual_node("WireGuard"));
    }

    #[test]
    fn test_calculate_minimal_group_coverage_empty() {
        let (groups, count) = calculate_minimal_group_coverage(&[]);
        assert!(groups.is_empty());
        assert_eq!(count, 0);
    }

    #[test]
    fn test_calculate_minimal_group_coverage_only_virtual() {
        let groups = vec![mock_group(
            "GLOBAL",
            vec![
                mock_node("DIRECT", "Direct"),
                mock_node("REJECT", "Reject"),
                mock_node("ProxyGroup", "Selector"),
            ],
        )];
        let (selected, count) = calculate_minimal_group_coverage(&groups);
        assert!(selected.is_empty());
        assert_eq!(count, 0);
    }

    #[test]
    fn test_calculate_minimal_group_coverage_single_super_group() {
        let groups = vec![
            mock_group(
                "GLOBAL",
                vec![
                    mock_node("DIRECT", "Direct"),
                    mock_node("Node-A", "Shadowsocks"),
                    mock_node("Node-B", "Vmess"),
                    mock_node("Node-C", "Trojan"),
                ],
            ),
            mock_group(
                "Proxy",
                vec![
                    mock_node("Node-A", "Shadowsocks"),
                    mock_node("Node-B", "Vmess"),
                ],
            ),
            mock_group("YouTube", vec![mock_node("Node-C", "Trojan")]),
        ];

        let (selected, count) = calculate_minimal_group_coverage(&groups);
        assert_eq!(count, 3);
        assert_eq!(selected, vec!["GLOBAL"]);
    }

    #[test]
    fn test_calculate_minimal_group_coverage_disjoint_groups() {
        let groups = vec![
            mock_group(
                "Group1",
                vec![mock_node("Node-1", "Vless"), mock_node("Node-2", "Vless")],
            ),
            mock_group(
                "Group2",
                vec![mock_node("Node-3", "Trojan"), mock_node("Node-4", "Trojan")],
            ),
        ];

        let (selected, count) = calculate_minimal_group_coverage(&groups);
        assert_eq!(count, 4);
        assert_eq!(selected.len(), 2);
        assert!(selected.contains(&"Group1"));
        assert!(selected.contains(&"Group2"));
    }
}
