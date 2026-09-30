use crate::error::CoreError;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};
use std::collections::HashSet;
use url::Url;

/// Represents a successfully parsed proxy node from a share link
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParsedProxyNode {
    pub name: String,
    pub protocol: String,
    pub server: String,
    pub port: u16,
    pub raw_yaml: Value,
}

/// Decodes base64 string flexibly (standard or url-safe, with or without padding)
pub fn decode_base64_flexible(input: &str) -> Result<String, String> {
    let s = input.trim().replace(['\n', '\r', ' '], "");
    let bytes = STANDARD
        .decode(&s)
        .or_else(|_| STANDARD_NO_PAD.decode(&s))
        .or_else(|_| URL_SAFE.decode(&s))
        .or_else(|_| URL_SAFE_NO_PAD.decode(&s))
        .map_err(|e| format!("Base64 解码失败: {}", e))?;

    String::from_utf8(bytes).map_err(|e| format!("UTF-8 字符解码失败: {}", e))
}

/// Robust UTF-8 percent-decoding helper
pub fn percent_decode(input: &str) -> String {
    let mut bytes = Vec::new();
    let mut chars = input.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            if let (Some(h1), Some(h2)) = (chars.next(), chars.next()) {
                if let Ok(byte) = u8::from_str_radix(&format!("{}{}", h1 as char, h2 as char), 16) {
                    bytes.push(byte);
                    continue;
                }
                bytes.push(b'%');
                bytes.push(h1);
                bytes.push(h2);
            } else {
                bytes.push(b'%');
            }
        } else if b == b'+' {
            bytes.push(b' ');
        } else {
            bytes.push(b);
        }
    }
    String::from_utf8_lossy(&bytes).to_string()
}

/// Parses a Shadowsocks (`ss://`) share link
pub fn parse_ss(link: &str) -> Result<ParsedProxyNode, String> {
    let raw = link.trim();
    let without_scheme = raw
        .strip_prefix("ss://")
        .ok_or_else(|| "缺少 ss:// 前缀".to_string())?;

    let (main_part, tag) = match without_scheme.split_once('#') {
        Some((m, t)) => (m, percent_decode(t)),
        None => (without_scheme, String::new()),
    };

    let (cipher, password, host, port) = if main_part.contains('@') {
        let (user_part, host_port_part) = main_part
            .split_once('@')
            .ok_or_else(|| "无效的 Shadowsocks 格式".to_string())?;

        let user_info = if user_part.contains(':') {
            user_part.to_string()
        } else {
            decode_base64_flexible(user_part)?
        };

        let (c, p) = user_info
            .split_once(':')
            .ok_or_else(|| "Shadowsocks 用户信息缺少密码".to_string())?;

        let host_port_clean = host_port_part.split('?').next().unwrap_or(host_port_part);
        let (h, pt) = parse_host_port(host_port_clean)?;
        (c.to_string(), p.to_string(), h, pt)
    } else {
        let decoded = decode_base64_flexible(main_part)?;
        let (user_info, host_port_part) = decoded
            .split_once('@')
            .ok_or_else(|| "Base64 解码后缺少 @ 分隔符".to_string())?;
        let (c, p) = user_info
            .split_once(':')
            .ok_or_else(|| "Shadowsocks 用户信息缺少密码".to_string())?;
        let (h, pt) = parse_host_port(host_port_part)?;
        (c.to_string(), p.to_string(), h, pt)
    };

    let name = if tag.trim().is_empty() {
        format!("SS - {}:{}", host, port)
    } else {
        tag
    };

    let mut map = Mapping::new();
    map.insert(Value::String("name".to_string()), Value::String(name.clone()));
    map.insert(Value::String("type".to_string()), Value::String("ss".to_string()));
    map.insert(Value::String("server".to_string()), Value::String(host.clone()));
    map.insert(Value::String("port".to_string()), Value::Number(port.into()));
    map.insert(Value::String("cipher".to_string()), Value::String(cipher));
    map.insert(Value::String("password".to_string()), Value::String(password));
    map.insert(Value::String("udp".to_string()), Value::Bool(true));

    Ok(ParsedProxyNode {
        name,
        protocol: "SS".to_string(),
        server: host,
        port,
        raw_yaml: Value::Mapping(map),
    })
}

/// Parses a VMess (`vmess://`) share link (standard V2RayN Base64 JSON)
pub fn parse_vmess(link: &str) -> Result<ParsedProxyNode, String> {
    let raw = link.trim();
    let b64 = raw
        .strip_prefix("vmess://")
        .ok_or_else(|| "缺少 vmess:// 前缀".to_string())?;

    let json_str = decode_base64_flexible(b64)?;
    let val: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| format!("VMess JSON 解析失败: {}", e))?;

    let server = val
        .get("add")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "VMess 缺少服务器地址 (add)".to_string())?
        .trim()
        .to_string();

    let port: u16 = match val.get("port") {
        Some(serde_json::Value::Number(n)) => n.as_u64().unwrap_or(443) as u16,
        Some(serde_json::Value::String(s)) => s.parse::<u16>().unwrap_or(443),
        _ => return Err("VMess 缺少有效端口 (port)".to_string()),
    };

    let uuid = val
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "VMess 缺少 UUID (id)".to_string())?
        .trim()
        .to_string();

    let alter_id: u64 = match val.get("aid") {
        Some(serde_json::Value::Number(n)) => n.as_u64().unwrap_or(0),
        Some(serde_json::Value::String(s)) => s.parse::<u64>().unwrap_or(0),
        _ => 0,
    };

    let cipher = val
        .get("scy")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("auto")
        .to_string();

    let name = val
        .get("ps")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| format!("VMess - {}:{}", server, port));

    let net = val
        .get("net")
        .and_then(|v| v.as_str())
        .unwrap_or("tcp")
        .to_lowercase();

    let tls = val
        .get("tls")
        .and_then(|v| v.as_str())
        .map(|s| s.eq_ignore_ascii_case("tls"))
        .unwrap_or(false);

    let host = val
        .get("host")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    let path = val
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    let sni = val
        .get("sni")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_string())
        .or_else(|| if !host.is_empty() { Some(host.clone()) } else { None });

    let mut map = Mapping::new();
    map.insert(Value::String("name".to_string()), Value::String(name.clone()));
    map.insert(Value::String("type".to_string()), Value::String("vmess".to_string()));
    map.insert(Value::String("server".to_string()), Value::String(server.clone()));
    map.insert(Value::String("port".to_string()), Value::Number(port.into()));
    map.insert(Value::String("uuid".to_string()), Value::String(uuid));
    map.insert(Value::String("alterId".to_string()), Value::Number(alter_id.into()));
    map.insert(Value::String("cipher".to_string()), Value::String(cipher));
    map.insert(Value::String("udp".to_string()), Value::Bool(true));
    map.insert(Value::String("tls".to_string()), Value::Bool(tls));

    if let Some(s) = sni {
        map.insert(Value::String("servername".to_string()), Value::String(s));
    }

    if net == "ws" {
        map.insert(Value::String("network".to_string()), Value::String("ws".to_string()));
        let mut ws_opts = Mapping::new();
        let ws_path = if path.is_empty() { "/".to_string() } else { path };
        ws_opts.insert(Value::String("path".to_string()), Value::String(ws_path));
        if !host.is_empty() {
            let mut headers = Mapping::new();
            headers.insert(Value::String("Host".to_string()), Value::String(host));
            ws_opts.insert(Value::String("headers".to_string()), Value::Mapping(headers));
        }
        map.insert(Value::String("ws-opts".to_string()), Value::Mapping(ws_opts));
    } else if net == "grpc" {
        map.insert(Value::String("network".to_string()), Value::String("grpc".to_string()));
        let mut grpc_opts = Mapping::new();
        let service_name = if !path.is_empty() { path } else { host };
        if !service_name.is_empty() {
            grpc_opts.insert(
                Value::String("grpc-service-name".to_string()),
                Value::String(service_name),
            );
        }
        map.insert(Value::String("grpc-opts".to_string()), Value::Mapping(grpc_opts));
    } else if net == "h2" {
        map.insert(Value::String("network".to_string()), Value::String("h2".to_string()));
        let mut h2_opts = Mapping::new();
        if !path.is_empty() {
            h2_opts.insert(Value::String("path".to_string()), Value::String(path));
        }
        if !host.is_empty() {
            h2_opts.insert(
                Value::String("host".to_string()),
                Value::Sequence(vec![Value::String(host)]),
            );
        }
        map.insert(Value::String("h2-opts".to_string()), Value::Mapping(h2_opts));
    }

    Ok(ParsedProxyNode {
        name,
        protocol: "VMess".to_string(),
        server,
        port,
        raw_yaml: Value::Mapping(map),
    })
}

/// Parses a VLESS (`vless://`) share link
pub fn parse_vless(link: &str) -> Result<ParsedProxyNode, String> {
    let raw = link.trim();
    let parsed_url = Url::parse(raw).map_err(|e| format!("VLESS URL 解析失败: {}", e))?;

    let uuid = parsed_url.username().to_string();
    if uuid.is_empty() {
        return Err("VLESS 链接缺少 UUID".to_string());
    }

    let server = parsed_url
        .host_str()
        .ok_or_else(|| "VLESS 链接缺少服务器地址".to_string())?
        .to_string();

    let port = parsed_url.port().unwrap_or(443);

    let tag = parsed_url
        .fragment()
        .map(percent_decode)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| format!("VLESS - {}:{}", server, port));

    let mut query_map = std::collections::HashMap::new();
    for (k, v) in parsed_url.query_pairs() {
        query_map.insert(k.to_string(), v.to_string());
    }

    let net = query_map.get("type").map(|s| s.as_str()).unwrap_or("tcp");
    let security = query_map.get("security").map(|s| s.as_str()).unwrap_or("none");
    let flow = query_map.get("flow").cloned();
    let sni = query_map.get("sni").cloned();
    let pbk = query_map.get("pbk").cloned();
    let sid = query_map.get("sid").cloned();
    let fp = query_map.get("fp").cloned();
    let path = query_map.get("path").cloned();
    let host = query_map.get("host").cloned();
    let service_name = query_map.get("serviceName").cloned();

    let mut map = Mapping::new();
    map.insert(Value::String("name".to_string()), Value::String(tag.clone()));
    map.insert(Value::String("type".to_string()), Value::String("vless".to_string()));
    map.insert(Value::String("server".to_string()), Value::String(server.clone()));
    map.insert(Value::String("port".to_string()), Value::Number(port.into()));
    map.insert(Value::String("uuid".to_string()), Value::String(uuid));
    map.insert(Value::String("udp".to_string()), Value::Bool(true));

    let is_tls = security == "tls" || security == "reality";
    map.insert(Value::String("tls".to_string()), Value::Bool(is_tls));

    if let Some(f) = flow {
        if !f.is_empty() {
            map.insert(Value::String("flow".to_string()), Value::String(f));
        }
    }

    if let Some(s) = sni {
        if !s.is_empty() {
            map.insert(Value::String("servername".to_string()), Value::String(s));
        }
    }

    if let Some(f) = fp {
        if !f.is_empty() {
            map.insert(Value::String("client-fingerprint".to_string()), Value::String(f));
        }
    }

    if security == "reality" {
        let mut reality_opts = Mapping::new();
        if let Some(p) = pbk {
            reality_opts.insert(Value::String("public-key".to_string()), Value::String(p));
        }
        if let Some(s) = sid {
            reality_opts.insert(Value::String("short-id".to_string()), Value::String(s));
        }
        map.insert(Value::String("reality-opts".to_string()), Value::Mapping(reality_opts));
    }

    if net == "ws" {
        map.insert(Value::String("network".to_string()), Value::String("ws".to_string()));
        let mut ws_opts = Mapping::new();
        let ws_path = path.unwrap_or_else(|| "/".to_string());
        ws_opts.insert(Value::String("path".to_string()), Value::String(ws_path));
        if let Some(h) = host {
            if !h.is_empty() {
                let mut headers = Mapping::new();
                headers.insert(Value::String("Host".to_string()), Value::String(h));
                ws_opts.insert(Value::String("headers".to_string()), Value::Mapping(headers));
            }
        }
        map.insert(Value::String("ws-opts".to_string()), Value::Mapping(ws_opts));
    } else if net == "grpc" {
        map.insert(Value::String("network".to_string()), Value::String("grpc".to_string()));
        let mut grpc_opts = Mapping::new();
        if let Some(s) = service_name {
            if !s.is_empty() {
                grpc_opts.insert(Value::String("grpc-service-name".to_string()), Value::String(s));
            }
        }
        map.insert(Value::String("grpc-opts".to_string()), Value::Mapping(grpc_opts));
    }

    Ok(ParsedProxyNode {
        name: tag,
        protocol: "VLESS".to_string(),
        server,
        port,
        raw_yaml: Value::Mapping(map),
    })
}

/// Parses a Trojan (`trojan://`) share link
pub fn parse_trojan(link: &str) -> Result<ParsedProxyNode, String> {
    let raw = link.trim();
    let parsed_url = Url::parse(raw).map_err(|e| format!("Trojan URL 解析失败: {}", e))?;

    let password = percent_decode(parsed_url.username());
    if password.is_empty() {
        return Err("Trojan 链接缺少密码".to_string());
    }

    let server = parsed_url
        .host_str()
        .ok_or_else(|| "Trojan 链接缺少服务器地址".to_string())?
        .to_string();

    let port = parsed_url.port().unwrap_or(443);

    let tag = parsed_url
        .fragment()
        .map(percent_decode)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| format!("Trojan - {}:{}", server, port));

    let mut query_map = std::collections::HashMap::new();
    for (k, v) in parsed_url.query_pairs() {
        query_map.insert(k.to_string(), v.to_string());
    }

    let sni = query_map.get("sni").cloned().unwrap_or_else(|| server.clone());
    let allow_insecure = query_map
        .get("allowInsecure")
        .or_else(|| query_map.get("insecure"))
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let mut map = Mapping::new();
    map.insert(Value::String("name".to_string()), Value::String(tag.clone()));
    map.insert(Value::String("type".to_string()), Value::String("trojan".to_string()));
    map.insert(Value::String("server".to_string()), Value::String(server.clone()));
    map.insert(Value::String("port".to_string()), Value::Number(port.into()));
    map.insert(Value::String("password".to_string()), Value::String(password));
    map.insert(Value::String("udp".to_string()), Value::Bool(true));
    map.insert(Value::String("sni".to_string()), Value::String(sni));
    if allow_insecure {
        map.insert(Value::String("skip-cert-verify".to_string()), Value::Bool(true));
    }

    Ok(ParsedProxyNode {
        name: tag,
        protocol: "Trojan".to_string(),
        server,
        port,
        raw_yaml: Value::Mapping(map),
    })
}

/// Parses a Hysteria 2 (`hysteria2://` or `hy2://`) share link
pub fn parse_hysteria2(link: &str) -> Result<ParsedProxyNode, String> {
    let raw = link.trim();
    let normalized = if let Some(rest) = raw.strip_prefix("hy2://") {
        format!("hysteria2://{}", rest)
    } else {
        raw.to_string()
    };

    let parsed_url = Url::parse(&normalized).map_err(|e| format!("Hysteria 2 URL 解析失败: {}", e))?;

    let auth = percent_decode(parsed_url.username());
    let server = parsed_url
        .host_str()
        .ok_or_else(|| "Hysteria 2 链接缺少服务器地址".to_string())?
        .to_string();

    let port = parsed_url.port().unwrap_or(443);

    let tag = parsed_url
        .fragment()
        .map(percent_decode)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| format!("HY2 - {}:{}", server, port));

    let mut query_map = std::collections::HashMap::new();
    for (k, v) in parsed_url.query_pairs() {
        query_map.insert(k.to_string(), v.to_string());
    }

    let sni = query_map.get("sni").cloned().unwrap_or_else(|| server.clone());
    let insecure = query_map
        .get("insecure")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let mut map = Mapping::new();
    map.insert(Value::String("name".to_string()), Value::String(tag.clone()));
    map.insert(Value::String("type".to_string()), Value::String("hysteria2".to_string()));
    map.insert(Value::String("server".to_string()), Value::String(server.clone()));
    map.insert(Value::String("port".to_string()), Value::Number(port.into()));
    if !auth.is_empty() {
        map.insert(Value::String("password".to_string()), Value::String(auth));
    }
    map.insert(Value::String("sni".to_string()), Value::String(sni));
    if insecure {
        map.insert(Value::String("skip-cert-verify".to_string()), Value::Bool(true));
    }

    if let Some(obfs) = query_map.get("obfs") {
        map.insert(Value::String("obfs".to_string()), Value::String(obfs.clone()));
    }
    if let Some(obfs_pass) = query_map.get("obfs-password") {
        map.insert(Value::String("obfs-password".to_string()), Value::String(obfs_pass.clone()));
    }

    Ok(ParsedProxyNode {
        name: tag,
        protocol: "HY2".to_string(),
        server,
        port,
        raw_yaml: Value::Mapping(map),
    })
}

/// Dispatches a single share link to its corresponding protocol parser
pub fn parse_single_share_link(link: &str) -> Result<ParsedProxyNode, String> {
    let raw = link.trim();
    if raw.starts_with("ss://") {
        parse_ss(raw)
    } else if raw.starts_with("vmess://") {
        parse_vmess(raw)
    } else if raw.starts_with("vless://") {
        parse_vless(raw)
    } else if raw.starts_with("trojan://") {
        parse_trojan(raw)
    } else if raw.starts_with("hysteria2://") || raw.starts_with("hy2://") {
        parse_hysteria2(raw)
    } else {
        Err(format!("暂不支持的协议链接: {}", raw))
    }
}

/// Parses multi-line share links text and deduplicates node names
pub fn parse_share_links(text: &str) -> (Vec<ParsedProxyNode>, Vec<String>) {
    let mut nodes = Vec::new();
    let mut errors = Vec::new();
    let mut seen_names: HashSet<String> = HashSet::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }

        match parse_single_share_link(trimmed) {
            Ok(mut node) => {
                // Ensure unique node name within proxies array
                let base_name = node.name.clone();
                let mut unique_name = base_name.clone();
                let mut counter = 2;
                while seen_names.contains(&unique_name) {
                    unique_name = format!("{} ({})", base_name, counter);
                    counter += 1;
                }
                seen_names.insert(unique_name.clone());

                if unique_name != node.name {
                    node.name = unique_name.clone();
                    if let Value::Mapping(ref mut m) = node.raw_yaml {
                        m.insert(Value::String("name".to_string()), Value::String(unique_name));
                    }
                }

                nodes.push(node);
            }
            Err(e) => {
                errors.push(format!("{}: {}", trimmed, e));
            }
        }
    }

    (nodes, errors)
}

/// Generates a complete, functional Mihomo configuration YAML from parsed proxy nodes
pub fn generate_profile_from_proxies(nodes: &[ParsedProxyNode]) -> Result<String, CoreError> {
    if nodes.is_empty() {
        return Err(CoreError::InvalidSubscription("节点列表不能为空".to_string()));
    }

    let mut root = Mapping::new();

    // 1. Proxies
    let proxies_val: Vec<Value> = nodes.iter().map(|n| n.raw_yaml.clone()).collect();
    let proxy_names: Vec<Value> = nodes
        .iter()
        .map(|n| Value::String(n.name.clone()))
        .collect();

    root.insert(
        Value::String("proxies".to_string()),
        Value::Sequence(proxies_val),
    );

    // 2. Proxy Groups
    let mut proxy_groups = Vec::new();

    // PROXY (Manual select group)
    let mut select_group = Mapping::new();
    select_group.insert(Value::String("name".to_string()), Value::String("PROXY".to_string()));
    select_group.insert(Value::String("type".to_string()), Value::String("select".to_string()));
    let mut select_proxies = vec![Value::String("AUTO".to_string())];
    select_proxies.extend(proxy_names.clone());
    select_proxies.push(Value::String("DIRECT".to_string()));
    select_group.insert(
        Value::String("proxies".to_string()),
        Value::Sequence(select_proxies),
    );
    proxy_groups.push(Value::Mapping(select_group));

    // AUTO (URL-Test auto speed test group)
    let mut auto_group = Mapping::new();
    auto_group.insert(Value::String("name".to_string()), Value::String("AUTO".to_string()));
    auto_group.insert(Value::String("type".to_string()), Value::String("url-test".to_string()));
    auto_group.insert(
        Value::String("url".to_string()),
        Value::String("http://www.gstatic.com/generate_204".to_string()),
    );
    auto_group.insert(Value::String("interval".to_string()), Value::Number(300.into()));
    auto_group.insert(Value::String("tolerance".to_string()), Value::Number(50.into()));
    auto_group.insert(
        Value::String("proxies".to_string()),
        Value::Sequence(proxy_names.clone()),
    );
    proxy_groups.push(Value::Mapping(auto_group));

    // FALLBACK group
    let mut fallback_group = Mapping::new();
    fallback_group.insert(Value::String("name".to_string()), Value::String("FALLBACK".to_string()));
    fallback_group.insert(Value::String("type".to_string()), Value::String("fallback".to_string()));
    fallback_group.insert(
        Value::String("url".to_string()),
        Value::String("http://www.gstatic.com/generate_204".to_string()),
    );
    fallback_group.insert(Value::String("interval".to_string()), Value::Number(300.into()));
    fallback_group.insert(
        Value::String("proxies".to_string()),
        Value::Sequence(proxy_names),
    );
    proxy_groups.push(Value::Mapping(fallback_group));

    root.insert(
        Value::String("proxy-groups".to_string()),
        Value::Sequence(proxy_groups),
    );

    // 3. Standard default routing rules
    let rules = vec![
        Value::String("DOMAIN-SUFFIX,local,DIRECT".to_string()),
        Value::String("IP-CIDR,127.0.0.0/8,DIRECT".to_string()),
        Value::String("IP-CIDR,172.16.0.0/12,DIRECT".to_string()),
        Value::String("IP-CIDR,192.168.0.0/16,DIRECT".to_string()),
        Value::String("IP-CIDR,10.0.0.0/8,DIRECT".to_string()),
        Value::String("IP-CIDR,17.0.0.0/8,DIRECT".to_string()),
        Value::String("IP-CIDR,100.64.0.0/10,DIRECT".to_string()),
        Value::String("GEOIP,CN,DIRECT".to_string()),
        Value::String("MATCH,PROXY".to_string()),
    ];

    root.insert(
        Value::String("rules".to_string()),
        Value::Sequence(rules),
    );

    serde_yaml::to_string(&Value::Mapping(root))
        .map_err(|e| CoreError::InvalidSubscription(format!("序列化节点配置失败: {}", e)))
}

fn parse_host_port(input: &str) -> Result<(String, u16), String> {
    let clean = input.trim();
    if clean.starts_with('[') {
        let end = clean
            .find(']')
            .ok_or_else(|| "IPv6 地址缺少封闭中括号 ']'".to_string())?;
        let host = &clean[1..end];
        let rest = &clean[end + 1..];
        let port_str = rest
            .strip_prefix(':')
            .ok_or_else(|| "缺少端口分隔符 ':'".to_string())?;
        let port: u16 = port_str
            .parse()
            .map_err(|_| format!("无效的端口号: {}", port_str))?;
        Ok((host.to_string(), port))
    } else {
        let (host, port_str) = clean
            .rsplit_once(':')
            .ok_or_else(|| "缺少端口分隔符 ':'".to_string())?;
        let port: u16 = port_str
            .parse()
            .map_err(|_| format!("无效的端口号: {}", port_str))?;
        Ok((host.to_string(), port))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ss_sip002() {
        let link = "ss://YWVzLTEyOC1nY206dGVzdA==@192.168.100.1:8888#Test%20SS";
        let node = parse_ss(link).expect("parse ss sip002 failed");
        assert_eq!(node.name, "Test SS");
        assert_eq!(node.protocol, "SS");
        assert_eq!(node.server, "192.168.100.1");
        assert_eq!(node.port, 8888);
    }

    #[test]
    fn test_parse_ss_legacy() {
        let link = "ss://YWVzLTEyOC1nY206dGVzdEAxOTIuMTY4LjEwMC4xOjg4ODg=#Legacy%20SS";
        let node = parse_ss(link).expect("parse legacy ss failed");
        assert_eq!(node.name, "Legacy SS");
        assert_eq!(node.protocol, "SS");
        assert_eq!(node.server, "192.168.100.1");
        assert_eq!(node.port, 8888);
    }

    #[test]
    fn test_parse_vmess() {
        let json = r#"{"v":"2","ps":"HK Node","add":"1.2.3.4","port":443,"id":"a0b1c2d3-e4f5-6789-0123-456789abcdef","aid":0,"scy":"auto","net":"ws","type":"none","host":"example.com","path":"/ws","tls":"tls"}"#;
        use base64::engine::general_purpose::STANDARD;
        let b64 = STANDARD.encode(json);
        let link = format!("vmess://{}", b64);
        let node = parse_vmess(&link).expect("parse vmess failed");
        assert_eq!(node.name, "HK Node");
        assert_eq!(node.protocol, "VMess");
        assert_eq!(node.server, "1.2.3.4");
        assert_eq!(node.port, 443);
    }

    #[test]
    fn test_parse_vless() {
        let link = "vless://a0b1c2d3-e4f5-6789-0123-456789abcdef@example.com:443?type=ws&security=reality&pbk=pubkey123&sid=sid123&sni=example.com#VLESS%20Node";
        let node = parse_vless(link).expect("parse vless failed");
        assert_eq!(node.name, "VLESS Node");
        assert_eq!(node.protocol, "VLESS");
        assert_eq!(node.server, "example.com");
        assert_eq!(node.port, 443);
    }

    #[test]
    fn test_parse_trojan() {
        let link = "trojan://password123@example.com:443?sni=example.com#Trojan%20Node";
        let node = parse_trojan(link).expect("parse trojan failed");
        assert_eq!(node.name, "Trojan Node");
        assert_eq!(node.protocol, "Trojan");
        assert_eq!(node.server, "example.com");
        assert_eq!(node.port, 443);
    }

    #[test]
    fn test_parse_hysteria2() {
        let link = "hy2://secret123@example.com:8443?sni=example.com#HY2%20Node";
        let node = parse_hysteria2(link).expect("parse hy2 failed");
        assert_eq!(node.name, "HY2 Node");
        assert_eq!(node.protocol, "HY2");
        assert_eq!(node.server, "example.com");
        assert_eq!(node.port, 8443);
    }

    #[test]
    fn test_parse_batch_and_generate_profile() {
        let text = r#"
        # Comment line
        ss://YWVzLTEyOC1nY206dGVzdA==@192.168.100.1:8888#SameName
        ss://YWVzLTEyOC1nY206dGVzdA==@192.168.100.2:8888#SameName
        trojan://password123@example.com:443?sni=example.com#SameName
        "#;
        let (nodes, errors) = parse_share_links(text);
        assert_eq!(errors.len(), 0);
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0].name, "SameName");
        assert_eq!(nodes[1].name, "SameName (2)");
        assert_eq!(nodes[2].name, "SameName (3)");

        let yaml = generate_profile_from_proxies(&nodes).expect("generate profile failed");
        assert!(yaml.contains("proxies:"));
        assert!(yaml.contains("proxy-groups:"));
        assert!(yaml.contains("rules:"));
    }
}
