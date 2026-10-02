use crate::error::InfraError;
use gihomo_core::TrafficStats;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde_json::Value;
use std::time::Duration;
use tracing::{debug, info};

#[derive(Clone, Debug)]
pub struct MihomoApiClient {
    client: reqwest::Client,
    base_url: String,
    secret: String,
}

impl MihomoApiClient {
    pub fn new(base_url: String, secret: &str) -> Self {
        let mut headers = HeaderMap::new();
        if !secret.is_empty() {
            let auth_str = format!("Bearer {}", secret);
            if let Ok(mut val) = HeaderValue::from_str(&auth_str) {
                val.set_sensitive(true);
                headers.insert(AUTHORIZATION, val);
            }
        }

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let clean_base = base_url.trim_end_matches('/').to_string();
        Self {
            client,
            base_url: clean_base,
            secret: secret.to_string(),
        }
    }

    pub async fn get_version(&self) -> Result<Value, InfraError> {
        let url = format!("{}/version", self.base_url);
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(InfraError::ApiError {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }
        let json: Value = resp.json().await?;
        Ok(json)
    }

    pub async fn get_configs(&self) -> Result<Value, InfraError> {
        let url = format!("{}/configs", self.base_url);
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(InfraError::ApiError {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }
        let json: Value = resp.json().await?;
        Ok(json)
    }

    pub async fn patch_configs(&self, payload: Value) -> Result<(), InfraError> {
        let url = format!("{}/configs", self.base_url);
        debug!("Patching Mihomo configs: {:?}", payload);
        let resp = self.client.patch(&url).json(&payload).send().await?;
        if !resp.status().is_success() {
            return Err(InfraError::ApiError {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }
        Ok(())
    }

    /// Reloads config from path or triggers reload
    pub async fn reload_config(&self, config_path: &str, force: bool) -> Result<(), InfraError> {
        let url = format!("{}/configs?force={}", self.base_url, force);
        info!("Reloading Mihomo config at {}", config_path);
        let payload = serde_json::json!({
            "path": config_path
        });
        let resp = self.client.put(&url).json(&payload).send().await?;
        if !resp.status().is_success() {
            return Err(InfraError::ApiError {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }
        Ok(())
    }

    /// Dynamic toggle TUN mode
    pub async fn set_tun_enabled(&self, enabled: bool) -> Result<(), InfraError> {
        let payload = serde_json::json!({
            "tun": {
                "enable": enabled
            }
        });
        self.patch_configs(payload).await
    }

    pub async fn get_proxies(&self) -> Result<Value, InfraError> {
        let url = format!("{}/proxies", self.base_url);
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(InfraError::ApiError {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }
        let json: Value = resp.json().await?;
        Ok(json)
    }

    pub async fn select_proxy(&self, group: &str, node: &str) -> Result<(), InfraError> {
        let url = format!("{}/proxies/{}", self.base_url, urlencoding_encode(group));
        let payload = serde_json::json!({
            "name": node
        });
        let resp = self.client.put(&url).json(&payload).send().await?;
        if !resp.status().is_success() {
            return Err(InfraError::ApiError {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }
        Ok(())
    }

    pub async fn test_delay(
        &self,
        node: &str,
        test_url: &str,
        timeout_ms: u64,
    ) -> Result<u32, InfraError> {
        let url = format!(
            "{}/proxies/{}/delay?url={}&timeout={}",
            self.base_url,
            urlencoding_encode(node),
            urlencoding_encode(test_url),
            timeout_ms
        );
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(InfraError::ApiError {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }
        let json: Value = resp.json().await?;
        let delay = json.get("delay").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        Ok(delay)
    }

    pub async fn get_proxy_groups(&self) -> Result<Vec<gihomo_core::ProxyGroup>, InfraError> {
        let json = self.get_proxies().await?;
        let proxies = json
            .get("proxies")
            .and_then(|v| v.as_object())
            .ok_or_else(|| InfraError::ApiError {
                status: 500,
                message: "Missing 'proxies' object in response".to_string(),
            })?;

        let mut groups = Vec::new();

        for (name, val) in proxies {
            let group_type = val.get("type").and_then(|v| v.as_str()).unwrap_or_default();
            // Mihomo groups are Selector, URLTest, Fallback, LoadBalance
            if matches!(
                group_type,
                "Selector" | "URLTest" | "Fallback" | "LoadBalance"
            ) {
                let now = val
                    .get("now")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let all_names = val.get("all").and_then(|v| v.as_array());

                let mut nodes = Vec::new();
                if let Some(arr) = all_names {
                    for n_val in arr {
                        if let Some(node_name) = n_val.as_str() {
                            let (node_type, delay) = if let Some(node_obj) = proxies.get(node_name)
                            {
                                let t = node_obj
                                    .get("type")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Proxy");
                                let d = node_obj
                                    .get("history")
                                    .and_then(|h| h.as_array())
                                    .and_then(|arr| arr.last())
                                    .and_then(|last| last.get("delay"))
                                    .and_then(|d| d.as_u64())
                                    .map(|d| d as u32);
                                (t.to_string(), d)
                            } else {
                                ("Proxy".to_string(), None)
                            };

                            nodes.push(gihomo_core::ProxyNode {
                                name: node_name.to_string(),
                                node_type,
                                delay,
                            });
                        }
                    }
                }

                groups.push(gihomo_core::ProxyGroup {
                    name: name.clone(),
                    group_type: group_type.to_string(),
                    now,
                    nodes,
                });
            }
        }

        // Sort groups: Put user custom groups first, then GLOBAL, then automatic groups
        groups.sort_by(|a, b| {
            let priority = |g: &gihomo_core::ProxyGroup| {
                if g.name == "GLOBAL" {
                    2
                } else if g.group_type == "Selector" {
                    0
                } else {
                    1
                }
            };
            priority(a)
                .cmp(&priority(b))
                .then_with(|| a.name.cmp(&b.name))
        });

        Ok(groups)
    }

    pub async fn test_group_delay(
        &self,
        group: &str,
        test_url: &str,
        timeout_ms: u64,
    ) -> Result<(), InfraError> {
        let url = format!(
            "{}/group/{}/delay?url={}&timeout={}",
            self.base_url,
            urlencoding_encode(group),
            urlencoding_encode(test_url),
            timeout_ms
        );
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(InfraError::ApiError {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }
        Ok(())
    }

    /// Fetch all active routing rules from Mihomo
    pub async fn get_rules(&self) -> Result<Vec<gihomo_core::RuleItem>, InfraError> {
        let url = format!("{}/rules", self.base_url);
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(InfraError::ApiError {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }
        let json: Value = resp.json().await?;
        let mut rules = Vec::new();
        if let Some(arr) = json.get("rules").and_then(|v| v.as_array()) {
            for item in arr {
                let rule_type = item
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Match")
                    .to_string();
                let payload = item
                    .get("payload")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let proxy = item
                    .get("proxy")
                    .and_then(|v| v.as_str())
                    .unwrap_or("DIRECT")
                    .to_string();
                let size = item.get("size").and_then(|v| v.as_i64());
                rules.push(gihomo_core::RuleItem {
                    rule_type,
                    payload,
                    proxy,
                    size,
                });
            }
        }
        Ok(rules)
    }

    /// Fetch all rule providers from Mihomo
    pub async fn get_rule_providers(&self) -> Result<Vec<gihomo_core::RuleProvider>, InfraError> {
        let url = format!("{}/providers/rules", self.base_url);
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(InfraError::ApiError {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }
        let json: Value = resp.json().await?;
        let mut providers = Vec::new();
        if let Some(map) = json.get("providers").and_then(|v| v.as_object()) {
            for (name, item) in map {
                let p_type = item
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Rule")
                    .to_string();
                let vehicle = item
                    .get("vehicleType")
                    .and_then(|v| v.as_str())
                    .unwrap_or("HTTP")
                    .to_string();
                let count = item.get("ruleCount").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let updated = item
                    .get("updatedAt")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                providers.push(gihomo_core::RuleProvider {
                    name: name.clone(),
                    provider_type: p_type,
                    vehicle_type: vehicle,
                    rule_count: count,
                    updated_at: updated,
                });
            }
        }
        Ok(providers)
    }

    /// Trigger rule provider update
    pub async fn update_rule_provider(&self, name: &str) -> Result<(), InfraError> {
        let url = format!(
            "{}/providers/rules/{}",
            self.base_url,
            urlencoding_encode(name)
        );
        let resp = self.client.put(&url).send().await?;
        if !resp.status().is_success() {
            return Err(InfraError::ApiError {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }
        Ok(())
    }

    pub async fn get_mode(&self) -> Result<String, InfraError> {
        let configs = self.get_configs().await?;
        let mode = configs
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("rule")
            .to_string();
        Ok(mode)
    }

    pub async fn set_mode(&self, mode: &str) -> Result<(), InfraError> {
        let payload = serde_json::json!({
            "mode": mode
        });
        self.patch_configs(payload).await
    }

    /// Fetches instantaneous traffic and cumulative totals from `/traffic` or `/connections`
    pub async fn get_traffic_and_totals(&self) -> Result<TrafficStats, InfraError> {
        let url = format!("{}/connections", self.base_url);
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(InfraError::ApiError {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }
        let json: Value = resp.json().await?;
        let up_total = json
            .get("uploadTotal")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let down_total = json
            .get("downloadTotal")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        Ok(TrafficStats {
            up: 0,
            down: 0,
            up_total,
            down_total,
        })
    }

    pub fn websocket_traffic_url(&self) -> String {
        let ws_scheme = if self.base_url.starts_with("https://") {
            format!("wss://{}/traffic", &self.base_url[8..])
        } else if self.base_url.starts_with("http://") {
            format!("ws://{}/traffic", &self.base_url[7..])
        } else {
            format!("ws://{}/traffic", self.base_url)
        };

        if !self.secret.is_empty() {
            format!("{}?token={}", ws_scheme, urlencoding_encode(&self.secret))
        } else {
            ws_scheme
        }
    }

    /// Connect to Mihomo's WebSocket `/traffic` stream and continuously push events to the sender channel.
    pub async fn stream_traffic(
        &self,
        sender: async_channel::Sender<TrafficStats>,
    ) -> Result<(), InfraError> {
        use futures_util::StreamExt;
        use tokio_tungstenite::connect_async;

        let ws_url = self.websocket_traffic_url();
        debug!("Connecting to WebSocket traffic stream: {}", ws_url);

        let (ws_stream, _) = connect_async(&ws_url)
            .await
            .map_err(|e| InfraError::WebSocket(e.to_string()))?;

        info!("Successfully connected to Mihomo WebSocket traffic stream");

        let (_, mut read) = ws_stream.split();
        let mut cum_up = 0u64;
        let mut cum_down = 0u64;

        while let Some(msg_result) = read.next().await {
            match msg_result {
                Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                    #[derive(serde::Deserialize)]
                    struct WsTraffic {
                        #[serde(default)]
                        up: u64,
                        #[serde(default)]
                        down: u64,
                        #[serde(default, alias = "upTotal", alias = "uploadTotal")]
                        up_total: Option<u64>,
                        #[serde(default, alias = "downTotal", alias = "downloadTotal")]
                        down_total: Option<u64>,
                    }

                    if let Ok(data) = serde_json::from_str::<WsTraffic>(&text) {
                        let up_total = data.up_total.unwrap_or_else(|| {
                            cum_up += data.up;
                            cum_up
                        });
                        let down_total = data.down_total.unwrap_or_else(|| {
                            cum_down += data.down;
                            cum_down
                        });
                        if sender
                            .send(TrafficStats {
                                up: data.up,
                                down: data.down,
                                up_total,
                                down_total,
                            })
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                }
                Ok(tokio_tungstenite::tungstenite::Message::Close(_)) => {
                    info!("WebSocket traffic stream closed by server");
                    break;
                }
                Err(e) => {
                    debug!("WebSocket traffic stream error: {}", e);
                    return Err(InfraError::WebSocket(e.to_string()));
                }
                _ => {}
            }
        }

        Ok(())
    }

    pub async fn get_connections(&self) -> Result<gihomo_core::ConnectionsSnapshot, InfraError> {
        let url = format!("{}/connections", self.base_url);
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(InfraError::ApiError {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }
        let data: gihomo_core::ConnectionsSnapshot = resp.json().await?;
        Ok(data)
    }

    pub async fn close_connection(&self, id: &str) -> Result<(), InfraError> {
        let url = format!("{}/connections/{}", self.base_url, urlencoding_encode(id));
        let resp = self.client.delete(&url).send().await?;
        if !resp.status().is_success() {
            return Err(InfraError::ApiError {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }
        Ok(())
    }

    pub async fn close_all_connections(&self) -> Result<(), InfraError> {
        let url = format!("{}/connections", self.base_url);
        let resp = self.client.delete(&url).send().await?;
        if !resp.status().is_success() {
            return Err(InfraError::ApiError {
                status: resp.status().as_u16(),
                message: resp.text().await.unwrap_or_default(),
            });
        }
        Ok(())
    }

    pub fn websocket_logs_url(&self, level: &str) -> String {
        let ws_scheme = if self.base_url.starts_with("https://") {
            format!("wss://{}/logs", &self.base_url[8..])
        } else if self.base_url.starts_with("http://") {
            format!("ws://{}/logs", &self.base_url[7..])
        } else {
            format!("ws://{}/logs", self.base_url)
        };

        let level_param = if level.is_empty() { "info" } else { level };

        if !self.secret.is_empty() {
            format!(
                "{}?level={}&token={}",
                ws_scheme,
                urlencoding_encode(level_param),
                urlencoding_encode(&self.secret)
            )
        } else {
            format!("{}?level={}", ws_scheme, urlencoding_encode(level_param))
        }
    }

    pub async fn stream_logs(
        &self,
        level: &str,
        sender: async_channel::Sender<gihomo_core::LogMessage>,
    ) -> Result<(), InfraError> {
        use futures_util::StreamExt;
        use tokio_tungstenite::connect_async;

        let ws_url = self.websocket_logs_url(level);
        debug!("Connecting to WebSocket logs stream: {}", ws_url);

        let (ws_stream, _) = connect_async(&ws_url)
            .await
            .map_err(|e| InfraError::WebSocket(e.to_string()))?;

        let (_, mut read) = ws_stream.split();

        while let Some(msg) = read.next().await {
            match msg {
                Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                    if let Ok(log_item) = serde_json::from_str::<gihomo_core::LogMessage>(&text) {
                        if sender.send(log_item).await.is_err() {
                            break;
                        }
                    }
                }
                Ok(tokio_tungstenite::tungstenite::Message::Close(_)) => {
                    info!("WebSocket logs stream closed by server");
                    break;
                }
                Err(e) => {
                    debug!("WebSocket logs stream error: {}", e);
                    return Err(InfraError::WebSocket(e.to_string()));
                }
                _ => {}
            }
        }

        Ok(())
    }
}

fn urlencoding_encode(s: &str) -> String {
    let mut encoded = String::new();
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
    }
    encoded
}
