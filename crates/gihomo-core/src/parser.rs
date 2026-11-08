use crate::error::CoreError;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};
use std::collections::HashSet;
use url::Url;

#[inline]
fn invalid_sub(msg: impl Into<String>) -> CoreError {
    CoreError::InvalidSubscription(msg.into())
}

/// Represents a successfully parsed proxy node from a share link
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParsedProxyNode {
    pub name: String,
    pub protocol: String,
    pub server: String,
    pub port: u16,
    pub raw_yaml: Value,
}

/// Decodes a base64 string flexibly (standard or URL-safe, with or without padding).
///
/// # Errors
///
/// Returns [`CoreError::InvalidSubscription`] if the input is not valid Base64
/// or if the decoded bytes cannot be parsed as valid UTF-8.
pub fn decode_base64_flexible(input: &str) -> Result<String, CoreError> {
    let s = input.trim().replace(['\n', '\r', ' '], "");
    let bytes = STANDARD
        .decode(&s)
        .or_else(|_| STANDARD_NO_PAD.decode(&s))
        .or_else(|_| URL_SAFE.decode(&s))
        .or_else(|_| URL_SAFE_NO_PAD.decode(&s))
        .map_err(|e| invalid_sub(format!("Base64 解码失败: {}", e)))?;

    String::from_utf8(bytes).map_err(|e| invalid_sub(format!("UTF-8 字符解码失败: {}", e)))
}

/// Robust UTF-8 percent-decoding helper
#[must_use]
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

/// Parses a Shadowsocks (`ss://`) share link into a [`ParsedProxyNode`].
///
/// # Errors
///
/// Returns [`CoreError::InvalidSubscription`] if the link lacks the `ss://` prefix,
/// has invalid base64 encoding, or is missing required host/port/cipher/password fields.
pub fn parse_ss(link: &str) -> Result<ParsedProxyNode, CoreError> {
    let raw = link.trim();
    let without_scheme = raw
        .strip_prefix("ss://")
        .ok_or_else(|| invalid_sub("缺少 ss:// 前缀"))?;

    let (main_part, tag) = match without_scheme.split_once('#') {
        Some((m, t)) => (m, percent_decode(t)),
        None => (without_scheme, String::new()),
    };

    let (cipher, password, host, port) = if main_part.contains('@') {
        let (user_part, host_port_part) = main_part
            .split_once('@')
            .ok_or_else(|| invalid_sub("无效的 Shadowsocks 格式"))?;

        let user_info = if user_part.contains(':') {
            user_part.to_string()
        } else {
            decode_base64_flexible(user_part)?
        };

        let (c, p) = user_info
            .split_once(':')
            .ok_or_else(|| invalid_sub("Shadowsocks 用户信息缺少密码"))?;

        let host_port_clean = host_port_part.split('?').next().unwrap_or(host_port_part);
        let (h, pt) = parse_host_port(host_port_clean)?;
        (c.to_string(), p.to_string(), h, pt)
    } else {
        let decoded = decode_base64_flexible(main_part)?;
        let (user_info, host_port_part) = decoded
            .split_once('@')
            .ok_or_else(|| invalid_sub("Base64 解码后缺少 @ 分隔符"))?;
        let (c, p) = user_info
            .split_once(':')
            .ok_or_else(|| invalid_sub("Shadowsocks 用户信息缺少密码"))?;
        let (h, pt) = parse_host_port(host_port_part)?;
        (c.to_string(), p.to_string(), h, pt)
    };

    let name = if tag.trim().is_empty() {
        format!("SS - {}:{}", host, port)
    } else {
        tag
    };

    let mut map = Mapping::new();
    map.insert(
        Value::String("name".to_string()),
        Value::String(name.clone()),
    );
    map.insert(
        Value::String("type".to_string()),
        Value::String("ss".to_string()),
    );
    map.insert(
        Value::String("server".to_string()),
        Value::String(host.clone()),
    );
    map.insert(
        Value::String("port".to_string()),
        Value::Number(port.into()),
    );
    map.insert(Value::String("cipher".to_string()), Value::String(cipher));
    map.insert(
        Value::String("password".to_string()),
        Value::String(password),
    );
    map.insert(Value::String("udp".to_string()), Value::Bool(true));

    Ok(ParsedProxyNode {
        name,
        protocol: "SS".to_string(),
        server: host,
        port,
        raw_yaml: Value::Mapping(map),
    })
}

/// Parses a VMess (`vmess://`) share link (standard V2RayN Base64 JSON) into a [`ParsedProxyNode`].
///
/// # Errors
///
/// Returns [`CoreError::InvalidSubscription`] if the link lacks the `vmess://` prefix,
/// base64 decoding fails, JSON parsing fails, or required fields are missing.
pub fn parse_vmess(link: &str) -> Result<ParsedProxyNode, CoreError> {
    let raw = link.trim();
    let b64 = raw
        .strip_prefix("vmess://")
        .ok_or_else(|| invalid_sub("缺少 vmess:// 前缀"))?;

    let json_str = decode_base64_flexible(b64)?;
    let val: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| invalid_sub(format!("VMess JSON 解析失败: {}", e)))?;

    let server = val
        .get("add")
        .and_then(|v| v.as_str())
        .ok_or_else(|| invalid_sub("VMess 缺少服务器地址 (add)"))?
        .trim()
        .to_string();

    let port: u16 = match val.get("port") {
        Some(serde_json::Value::Number(n)) => n.as_u64().unwrap_or(443) as u16,
        Some(serde_json::Value::String(s)) => s.parse::<u16>().unwrap_or(443),
        _ => return Err(invalid_sub("VMess 缺少有效端口 (port)")),
    };

    let uuid = val
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| invalid_sub("VMess 缺少 UUID (id)"))?
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
        .or_else(|| {
            if !host.is_empty() {
                Some(host.clone())
            } else {
                None
            }
        });

    let mut map = Mapping::new();
    map.insert(
        Value::String("name".to_string()),
        Value::String(name.clone()),
    );
    map.insert(
        Value::String("type".to_string()),
        Value::String("vmess".to_string()),
    );
    map.insert(
        Value::String("server".to_string()),
        Value::String(server.clone()),
    );
    map.insert(
        Value::String("port".to_string()),
        Value::Number(port.into()),
    );
    map.insert(Value::String("uuid".to_string()), Value::String(uuid));
    map.insert(
        Value::String("alterId".to_string()),
        Value::Number(alter_id.into()),
    );
    map.insert(Value::String("cipher".to_string()), Value::String(cipher));
    map.insert(Value::String("udp".to_string()), Value::Bool(true));
    map.insert(Value::String("tls".to_string()), Value::Bool(tls));

    if let Some(s) = sni {
        map.insert(Value::String("servername".to_string()), Value::String(s));
    }

    if net == "ws" {
        map.insert(
            Value::String("network".to_string()),
            Value::String("ws".to_string()),
        );
        let mut ws_opts = Mapping::new();
        let ws_path = if path.is_empty() {
            "/".to_string()
        } else {
            path
        };
        ws_opts.insert(Value::String("path".to_string()), Value::String(ws_path));
        if !host.is_empty() {
            let mut headers = Mapping::new();
            headers.insert(Value::String("Host".to_string()), Value::String(host));
            ws_opts.insert(
                Value::String("headers".to_string()),
                Value::Mapping(headers),
            );
        }
        map.insert(
            Value::String("ws-opts".to_string()),
            Value::Mapping(ws_opts),
        );
    } else if net == "grpc" {
        map.insert(
            Value::String("network".to_string()),
            Value::String("grpc".to_string()),
        );
        let mut grpc_opts = Mapping::new();
        let service_name = if !path.is_empty() { path } else { host };
        if !service_name.is_empty() {
            grpc_opts.insert(
                Value::String("grpc-service-name".to_string()),
                Value::String(service_name),
            );
        }
        map.insert(
            Value::String("grpc-opts".to_string()),
            Value::Mapping(grpc_opts),
        );
    } else if net == "h2" {
        map.insert(
            Value::String("network".to_string()),
            Value::String("h2".to_string()),
        );
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
        map.insert(
            Value::String("h2-opts".to_string()),
            Value::Mapping(h2_opts),
        );
    }

    Ok(ParsedProxyNode {
        name,
        protocol: "VMess".to_string(),
        server,
        port,
        raw_yaml: Value::Mapping(map),
    })
}

/// Parses a VLESS (`vless://`) share link into a [`ParsedProxyNode`].
///
/// # Errors
///
/// Returns [`CoreError::InvalidSubscription`] if the URL is invalid, missing a UUID
/// or server host, or fails to parse.
pub fn parse_vless(link: &str) -> Result<ParsedProxyNode, CoreError> {
    let raw = link.trim();
    // Split fragment beforehand to prevent RFC 3986 violation on illegal chars like '|', emojis, spaces
    let (url_part, raw_fragment) = match raw.split_once('#') {
        Some((u, f)) => (u.trim(), Some(f.trim())),
        None => (raw, None),
    };

    let parsed_url =
        Url::parse(url_part).map_err(|e| invalid_sub(format!("VLESS URL 解析失败: {}", e)))?;

    let uuid = parsed_url.username().to_string();
    if uuid.is_empty() {
        return Err(invalid_sub("VLESS 链接缺少 UUID"));
    }

    let server = parsed_url
        .host_str()
        .ok_or_else(|| invalid_sub("VLESS 链接缺少服务器地址"))?
        .to_string();

    let port = parsed_url.port().unwrap_or(443);

    let mut query_map = std::collections::HashMap::new();
    for (k, v) in parsed_url.query_pairs() {
        query_map.insert(k.to_string(), v.to_string());
    }

    let tag = raw_fragment
        .map(percent_decode)
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            query_map
                .get("remarks")
                .or_else(|| query_map.get("name"))
                .map(|s| percent_decode(s))
                .filter(|s| !s.trim().is_empty())
        })
        .unwrap_or_else(|| format!("VLESS - {}:{}", server, port));

    let net = query_map
        .get("type")
        .or_else(|| query_map.get("network"))
        .map(|s| s.as_str())
        .unwrap_or("tcp");
    let security = query_map
        .get("security")
        .map(|s| s.as_str())
        .unwrap_or("none");
    let flow = query_map.get("flow").cloned();
    let sni = query_map
        .get("sni")
        .or_else(|| query_map.get("servername"))
        .or_else(|| query_map.get("peer"))
        .cloned();
    let pbk = query_map.get("pbk").cloned();
    let sid = query_map.get("sid").cloned();
    let spx = query_map.get("spx").cloned();
    let fp = query_map.get("fp").cloned();
    let path = query_map.get("path").cloned();
    let host = query_map.get("host").cloned();
    let service_name = query_map.get("serviceName").cloned();
    let encryption = query_map.get("encryption").cloned();
    let allow_insecure = query_map
        .get("allowInsecure")
        .or_else(|| query_map.get("insecure"))
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let packet_encoding = query_map
        .get("packetEncoding")
        .or_else(|| query_map.get("packet-encoding"))
        .cloned();
    let alpn = query_map.get("alpn").cloned();

    let mut map = Mapping::new();
    map.insert(
        Value::String("name".to_string()),
        Value::String(tag.clone()),
    );
    map.insert(
        Value::String("type".to_string()),
        Value::String("vless".to_string()),
    );
    map.insert(
        Value::String("server".to_string()),
        Value::String(server.clone()),
    );
    map.insert(
        Value::String("port".to_string()),
        Value::Number(port.into()),
    );
    map.insert(Value::String("uuid".to_string()), Value::String(uuid));
    map.insert(Value::String("udp".to_string()), Value::Bool(true));

    let is_tls = security.eq_ignore_ascii_case("tls") || security.eq_ignore_ascii_case("reality");
    map.insert(Value::String("tls".to_string()), Value::Bool(is_tls));

    if allow_insecure {
        map.insert(
            Value::String("skip-cert-verify".to_string()),
            Value::Bool(true),
        );
    }

    if let Some(enc) = encryption {
        if !enc.trim().is_empty() {
            map.insert(Value::String("encryption".to_string()), Value::String(enc));
        }
    }

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
            map.insert(
                Value::String("client-fingerprint".to_string()),
                Value::String(f),
            );
        }
    }

    if let Some(pe) = packet_encoding {
        if !pe.is_empty() {
            map.insert(
                Value::String("packet-encoding".to_string()),
                Value::String(pe),
            );
        }
    }

    if let Some(a) = alpn {
        let parts: Vec<Value> = a
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| Value::String(s.to_string()))
            .collect();
        if !parts.is_empty() {
            map.insert(Value::String("alpn".to_string()), Value::Sequence(parts));
        }
    }

    if security.eq_ignore_ascii_case("reality") {
        let mut reality_opts = Mapping::new();
        if let Some(p) = pbk {
            reality_opts.insert(Value::String("public-key".to_string()), Value::String(p));
        }
        if let Some(s) = sid {
            reality_opts.insert(Value::String("short-id".to_string()), Value::String(s));
        }
        if let Some(sx) = spx {
            if !sx.is_empty() {
                reality_opts.insert(
                    Value::String("spider-x".to_string()),
                    Value::String(percent_decode(&sx)),
                );
            }
        }
        map.insert(
            Value::String("reality-opts".to_string()),
            Value::Mapping(reality_opts),
        );
    }

    if net.eq_ignore_ascii_case("ws") || net.eq_ignore_ascii_case("websocket") {
        map.insert(
            Value::String("network".to_string()),
            Value::String("ws".to_string()),
        );
        let mut ws_opts = Mapping::new();
        let ws_path = path.unwrap_or_else(|| "/".to_string());
        ws_opts.insert(Value::String("path".to_string()), Value::String(ws_path));
        if let Some(h) = host {
            if !h.is_empty() {
                let mut headers = Mapping::new();
                headers.insert(Value::String("Host".to_string()), Value::String(h));
                ws_opts.insert(
                    Value::String("headers".to_string()),
                    Value::Mapping(headers),
                );
            }
        }
        map.insert(
            Value::String("ws-opts".to_string()),
            Value::Mapping(ws_opts),
        );
    } else if net.eq_ignore_ascii_case("grpc") {
        map.insert(
            Value::String("network".to_string()),
            Value::String("grpc".to_string()),
        );
        let mut grpc_opts = Mapping::new();
        if let Some(s) = service_name {
            if !s.is_empty() {
                grpc_opts.insert(
                    Value::String("grpc-service-name".to_string()),
                    Value::String(s),
                );
            }
        }
        map.insert(
            Value::String("grpc-opts".to_string()),
            Value::Mapping(grpc_opts),
        );
    } else if net.eq_ignore_ascii_case("xhttp") || net.eq_ignore_ascii_case("splithttp") {
        map.insert(
            Value::String("network".to_string()),
            Value::String("xhttp".to_string()),
        );
        let mut xhttp_opts = Mapping::new();
        let xhttp_path = path.unwrap_or_else(|| "/".to_string());
        xhttp_opts.insert(Value::String("path".to_string()), Value::String(xhttp_path));
        if let Some(h) = host {
            if !h.is_empty() {
                xhttp_opts.insert(Value::String("host".to_string()), Value::String(h));
            }
        }
        if let Some(m) = query_map.get("mode").or_else(|| query_map.get("xhttpMode")) {
            if !m.is_empty() {
                xhttp_opts.insert(Value::String("mode".to_string()), Value::String(m.clone()));
            }
        }
        map.insert(
            Value::String("xhttp-opts".to_string()),
            Value::Mapping(xhttp_opts),
        );
    } else if net.eq_ignore_ascii_case("h2") || net.eq_ignore_ascii_case("http") {
        map.insert(
            Value::String("network".to_string()),
            Value::String("h2".to_string()),
        );
        let mut h2_opts = Mapping::new();
        if let Some(p) = path {
            if !p.is_empty() {
                h2_opts.insert(Value::String("path".to_string()), Value::String(p));
            }
        }
        if let Some(h) = host {
            if !h.is_empty() {
                h2_opts.insert(
                    Value::String("host".to_string()),
                    Value::Sequence(vec![Value::String(h)]),
                );
            }
        }
        map.insert(
            Value::String("h2-opts".to_string()),
            Value::Mapping(h2_opts),
        );
    } else if net.eq_ignore_ascii_case("tcp") {
        if let Some(header_type) = query_map
            .get("headerType")
            .or_else(|| query_map.get("header-type"))
        {
            if header_type.eq_ignore_ascii_case("http") {
                let mut tcp_opts = Mapping::new();
                let mut req_map = Mapping::new();
                if let Some(p) = path {
                    if !p.is_empty() {
                        req_map.insert(
                            Value::String("path".to_string()),
                            Value::Sequence(vec![Value::String(p)]),
                        );
                    }
                }
                if let Some(h) = host {
                    if !h.is_empty() {
                        let mut headers = Mapping::new();
                        headers.insert(
                            Value::String("Host".to_string()),
                            Value::Sequence(vec![Value::String(h)]),
                        );
                        req_map.insert(
                            Value::String("headers".to_string()),
                            Value::Mapping(headers),
                        );
                    }
                }
                tcp_opts.insert(
                    Value::String("type".to_string()),
                    Value::String("http".to_string()),
                );
                if !req_map.is_empty() {
                    tcp_opts.insert(
                        Value::String("request".to_string()),
                        Value::Mapping(req_map),
                    );
                }
                map.insert(
                    Value::String("tcp-opts".to_string()),
                    Value::Mapping(tcp_opts),
                );
            }
        }
    }

    Ok(ParsedProxyNode {
        name: tag,
        protocol: "VLESS".to_string(),
        server,
        port,
        raw_yaml: Value::Mapping(map),
    })
}

/// Parses a Trojan (`trojan://`) share link into a [`ParsedProxyNode`].
///
/// # Errors
///
/// Returns [`CoreError::InvalidSubscription`] if the URL is invalid, missing a password
/// or server host.
pub fn parse_trojan(link: &str) -> Result<ParsedProxyNode, CoreError> {
    let raw = link.trim();
    let (url_part, raw_fragment) = match raw.split_once('#') {
        Some((u, f)) => (u.trim(), Some(f.trim())),
        None => (raw, None),
    };

    let parsed_url =
        Url::parse(url_part).map_err(|e| invalid_sub(format!("Trojan URL 解析失败: {}", e)))?;

    let password = percent_decode(parsed_url.username());
    if password.is_empty() {
        return Err(invalid_sub("Trojan 链接缺少密码"));
    }

    let server = parsed_url
        .host_str()
        .ok_or_else(|| invalid_sub("Trojan 链接缺少服务器地址"))?
        .to_string();

    let port = parsed_url.port().unwrap_or(443);

    let mut query_map = std::collections::HashMap::new();
    for (k, v) in parsed_url.query_pairs() {
        query_map.insert(k.to_string(), v.to_string());
    }

    let tag = raw_fragment
        .map(percent_decode)
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            query_map
                .get("remarks")
                .or_else(|| query_map.get("name"))
                .map(|s| percent_decode(s))
                .filter(|s| !s.trim().is_empty())
        })
        .unwrap_or_else(|| format!("Trojan - {}:{}", server, port));

    let sni = query_map
        .get("sni")
        .or_else(|| query_map.get("servername"))
        .or_else(|| query_map.get("peer"))
        .cloned()
        .unwrap_or_else(|| server.clone());
    let allow_insecure = query_map
        .get("allowInsecure")
        .or_else(|| query_map.get("insecure"))
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let mut map = Mapping::new();
    map.insert(
        Value::String("name".to_string()),
        Value::String(tag.clone()),
    );
    map.insert(
        Value::String("type".to_string()),
        Value::String("trojan".to_string()),
    );
    map.insert(
        Value::String("server".to_string()),
        Value::String(server.clone()),
    );
    map.insert(
        Value::String("port".to_string()),
        Value::Number(port.into()),
    );
    map.insert(
        Value::String("password".to_string()),
        Value::String(password),
    );
    map.insert(Value::String("udp".to_string()), Value::Bool(true));
    map.insert(Value::String("sni".to_string()), Value::String(sni));
    if allow_insecure {
        map.insert(
            Value::String("skip-cert-verify".to_string()),
            Value::Bool(true),
        );
    }

    Ok(ParsedProxyNode {
        name: tag,
        protocol: "Trojan".to_string(),
        server,
        port,
        raw_yaml: Value::Mapping(map),
    })
}

/// Parses a Hysteria 2 (`hysteria2://` or `hy2://`) share link into a [`ParsedProxyNode`].
///
/// # Errors
///
/// Returns [`CoreError::InvalidSubscription`] if the URL is invalid or missing a server host.
pub fn parse_hysteria2(link: &str) -> Result<ParsedProxyNode, CoreError> {
    let raw = link.trim();
    let (url_part, raw_fragment) = match raw.split_once('#') {
        Some((u, f)) => (u.trim(), Some(f.trim())),
        None => (raw, None),
    };

    let normalized = if let Some(rest) = url_part.strip_prefix("hy2://") {
        format!("hysteria2://{}", rest)
    } else {
        url_part.to_string()
    };

    let parsed_url = Url::parse(&normalized)
        .map_err(|e| invalid_sub(format!("Hysteria 2 URL 解析失败: {}", e)))?;

    let auth = percent_decode(parsed_url.username());
    let server = parsed_url
        .host_str()
        .ok_or_else(|| invalid_sub("Hysteria 2 链接缺少服务器地址"))?
        .to_string();

    let port = parsed_url.port().unwrap_or(443);

    let mut query_map = std::collections::HashMap::new();
    for (k, v) in parsed_url.query_pairs() {
        query_map.insert(k.to_string(), v.to_string());
    }

    let tag = raw_fragment
        .map(percent_decode)
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            query_map
                .get("remarks")
                .or_else(|| query_map.get("name"))
                .map(|s| percent_decode(s))
                .filter(|s| !s.trim().is_empty())
        })
        .unwrap_or_else(|| format!("HY2 - {}:{}", server, port));

    let sni = query_map
        .get("sni")
        .or_else(|| query_map.get("servername"))
        .or_else(|| query_map.get("peer"))
        .cloned()
        .unwrap_or_else(|| server.clone());
    let insecure = query_map
        .get("insecure")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let mut map = Mapping::new();
    map.insert(
        Value::String("name".to_string()),
        Value::String(tag.clone()),
    );
    map.insert(
        Value::String("type".to_string()),
        Value::String("hysteria2".to_string()),
    );
    map.insert(
        Value::String("server".to_string()),
        Value::String(server.clone()),
    );
    map.insert(
        Value::String("port".to_string()),
        Value::Number(port.into()),
    );
    if !auth.is_empty() {
        map.insert(Value::String("password".to_string()), Value::String(auth));
    }
    map.insert(Value::String("sni".to_string()), Value::String(sni));
    if insecure {
        map.insert(
            Value::String("skip-cert-verify".to_string()),
            Value::Bool(true),
        );
    }

    if let Some(obfs) = query_map.get("obfs") {
        map.insert(
            Value::String("obfs".to_string()),
            Value::String(obfs.clone()),
        );
    }
    if let Some(obfs_pass) = query_map.get("obfs-password") {
        map.insert(
            Value::String("obfs-password".to_string()),
            Value::String(obfs_pass.clone()),
        );
    }

    Ok(ParsedProxyNode {
        name: tag,
        protocol: "HY2".to_string(),
        server,
        port,
        raw_yaml: Value::Mapping(map),
    })
}

/// Dispatches a single share link to its corresponding protocol parser.
///
/// # Errors
///
/// Returns [`CoreError::InvalidSubscription`] if the protocol scheme is unsupported
/// or the underlying parser fails.
pub fn parse_single_share_link(link: &str) -> Result<ParsedProxyNode, CoreError> {
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
        Err(invalid_sub(format!("暂不支持的协议链接: {}", raw)))
    }
}

/// Result of parsing multi-line share links.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedShareLinks {
    /// Successfully parsed proxy nodes.
    pub nodes: Vec<ParsedProxyNode>,
    /// Malformed or unsupported lines that failed to parse.
    pub failed_lines: Vec<String>,
}

/// Parses multi-line share links text and deduplicates node names
#[must_use]
pub fn parse_share_links(text: &str) -> ParsedShareLinks {
    let trimmed = text.trim();
    // If the input is Base64 encoded containing share links, decode it first
    let effective_text = if !trimmed.starts_with("ss://")
        && !trimmed.starts_with("vmess://")
        && !trimmed.starts_with("vless://")
        && !trimmed.starts_with("trojan://")
        && !trimmed.starts_with("hysteria2://")
        && !trimmed.starts_with("hy2://")
    {
        if let Ok(decoded) = decode_base64_flexible(trimmed) {
            if decoded.contains("://") {
                decoded
            } else {
                trimmed.to_string()
            }
        } else {
            trimmed.to_string()
        }
    } else {
        trimmed.to_string()
    };

    let mut nodes = Vec::new();
    let mut failed_lines = Vec::new();
    let mut seen_names: HashSet<String> = HashSet::new();

    for line in effective_text.lines() {
        let trimmed_line = line.trim();
        if trimmed_line.is_empty()
            || trimmed_line.starts_with('#')
            || trimmed_line.starts_with("//")
        {
            continue;
        }

        match parse_single_share_link(trimmed_line) {
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
                        m.insert(
                            Value::String("name".to_string()),
                            Value::String(unique_name),
                        );
                    }
                }

                nodes.push(node);
            }
            Err(e) => {
                failed_lines.push(format!("{}: {}", trimmed_line, e));
            }
        }
    }

    ParsedShareLinks {
        nodes,
        failed_lines,
    }
}

/// Normalizes any subscription content (Clash YAML, Base64 share links, or plain text share links)
/// into a valid, standard Clash/Mihomo YAML configuration profile string.
///
/// # Errors
///
/// Returns [`CoreError::InvalidSubscription`] if the subscription content is empty.
pub fn normalize_subscription_to_yaml(content: &str) -> Result<String, CoreError> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err(invalid_sub("订阅内容为空"));
    }

    // 1. If it's already valid YAML containing proxies or proxy-providers, preserve it directly
    if let Ok(val) = serde_yaml::from_str::<Value>(trimmed) {
        if let Some(map) = val.as_mapping() {
            let has_proxies = map.contains_key("proxies");
            let has_providers = map.contains_key("proxy-providers");
            if has_proxies || has_providers {
                return Ok(trimmed.to_string());
            }
        }
    }

    // 2. Try Base64 decoding
    if let Ok(decoded) = decode_base64_flexible(trimmed) {
        if decoded.contains("://") {
            let parsed = parse_share_links(&decoded);
            if !parsed.nodes.is_empty() {
                return generate_profile_from_proxies(&parsed.nodes);
            }
        }
    }

    // 3. Try plain text share links
    if trimmed.contains("://") {
        let parsed = parse_share_links(trimmed);
        if !parsed.nodes.is_empty() {
            return generate_profile_from_proxies(&parsed.nodes);
        }
    }

    // 4. Fallback: return original content to let YAML parser handle or report errors
    Ok(trimmed.to_string())
}

/// Generates a complete, functional Mihomo configuration YAML from parsed proxy nodes.
///
/// # Errors
///
/// Returns [`CoreError::InvalidSubscription`] if `nodes` is empty.
/// Returns [`CoreError::ConfigMerge`] if YAML serialization fails.
pub fn generate_profile_from_proxies(nodes: &[ParsedProxyNode]) -> Result<String, CoreError> {
    if nodes.is_empty() {
        return Err(CoreError::InvalidSubscription(
            "节点列表不能为空".to_string(),
        ));
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
    select_group.insert(
        Value::String("name".to_string()),
        Value::String("PROXY".to_string()),
    );
    select_group.insert(
        Value::String("type".to_string()),
        Value::String("select".to_string()),
    );
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
    auto_group.insert(
        Value::String("name".to_string()),
        Value::String("AUTO".to_string()),
    );
    auto_group.insert(
        Value::String("type".to_string()),
        Value::String("url-test".to_string()),
    );
    auto_group.insert(
        Value::String("url".to_string()),
        Value::String("http://www.gstatic.com/generate_204".to_string()),
    );
    auto_group.insert(
        Value::String("interval".to_string()),
        Value::Number(300.into()),
    );
    auto_group.insert(
        Value::String("tolerance".to_string()),
        Value::Number(50.into()),
    );
    auto_group.insert(
        Value::String("proxies".to_string()),
        Value::Sequence(proxy_names.clone()),
    );
    proxy_groups.push(Value::Mapping(auto_group));

    // FALLBACK group
    let mut fallback_group = Mapping::new();
    fallback_group.insert(
        Value::String("name".to_string()),
        Value::String("FALLBACK".to_string()),
    );
    fallback_group.insert(
        Value::String("type".to_string()),
        Value::String("fallback".to_string()),
    );
    fallback_group.insert(
        Value::String("url".to_string()),
        Value::String("http://www.gstatic.com/generate_204".to_string()),
    );
    fallback_group.insert(
        Value::String("interval".to_string()),
        Value::Number(300.into()),
    );
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

    root.insert(Value::String("rules".to_string()), Value::Sequence(rules));

    serde_yaml::to_string(&Value::Mapping(root))
        .map_err(|e| CoreError::InvalidSubscription(format!("序列化节点配置失败: {}", e)))
}

fn parse_host_port(input: &str) -> Result<(String, u16), CoreError> {
    let clean = input.trim();
    if clean.starts_with('[') {
        let end = clean
            .find(']')
            .ok_or_else(|| invalid_sub("IPv6 地址缺少封闭中括号 ']'"))?;
        let host = &clean[1..end];
        let rest = &clean[end + 1..];
        let port_str = rest
            .strip_prefix(':')
            .ok_or_else(|| invalid_sub("缺少端口分隔符 ':'"))?;
        let port: u16 = port_str
            .parse()
            .map_err(|_| invalid_sub(format!("无效的端口号: {}", port_str)))?;
        Ok((host.to_string(), port))
    } else {
        let (host, port_str) = clean
            .rsplit_once(':')
            .ok_or_else(|| invalid_sub("缺少端口分隔符 ':'"))?;
        let port: u16 = port_str
            .parse()
            .map_err(|_| invalid_sub(format!("无效的端口号: {}", port_str)))?;
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
    fn test_parse_vless_with_encryption_reality_and_pipe_in_name() {
        let link = "vless://dab6ccfe-6fac-41f7-88b3-a68729ccaaf9@taiwan.443ds443.com:443?mode=multi&security=reality&encryption=mlkem768x25519plus&type=tcp&flow=xtls-rprx-vision&pbk=rH2NBg0-Zk1Vak8BLS9O6UG44WarEccy1i2XhYoMQXE&sid=d084a6ef27e0c0&servername=www.apple.com&spx=%2F&fp=edge&insecure=1&packetEncoding=xudp&alpn=h2,http/1.1#🇭🇰香港高速01|BGP|CMCU";
        let node = parse_vless(link).expect("parse vless with advanced features failed");
        assert_eq!(node.name, "🇭🇰香港高速01|BGP|CMCU");
        assert_eq!(node.protocol, "VLESS");

        let yaml_val = node.raw_yaml;
        assert_eq!(
            yaml_val.get("encryption").and_then(|v| v.as_str()),
            Some("mlkem768x25519plus")
        );
        assert_eq!(
            yaml_val.get("servername").and_then(|v| v.as_str()),
            Some("www.apple.com")
        );
        assert_eq!(
            yaml_val.get("skip-cert-verify").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            yaml_val.get("packet-encoding").and_then(|v| v.as_str()),
            Some("xudp")
        );

        let reality = yaml_val.get("reality-opts").expect("missing reality-opts");
        assert_eq!(reality.get("spider-x").and_then(|v| v.as_str()), Some("/"));
        assert_eq!(
            reality.get("public-key").and_then(|v| v.as_str()),
            Some("rH2NBg0-Zk1Vak8BLS9O6UG44WarEccy1i2XhYoMQXE")
        );
        assert_eq!(
            reality.get("short-id").and_then(|v| v.as_str()),
            Some("d084a6ef27e0c0")
        );
    }

    #[test]
    fn test_parse_vless_xhttp() {
        let link = "vless://00000000-0000-0000-0000-000000000000@example.com:443?type=xhttp&security=tls&sni=example.com&path=%2Fxhttp-path&host=example.com&mode=packet-up#XHTTP%20Node";
        let node = parse_vless(link).expect("parse xhttp failed");
        assert_eq!(node.name, "XHTTP Node");
        let yaml_val = node.raw_yaml;
        assert_eq!(
            yaml_val.get("network").and_then(|v| v.as_str()),
            Some("xhttp")
        );
        let xhttp_opts = yaml_val.get("xhttp-opts").expect("missing xhttp-opts");
        assert_eq!(
            xhttp_opts.get("path").and_then(|v| v.as_str()),
            Some("/xhttp-path")
        );
        assert_eq!(
            xhttp_opts.get("host").and_then(|v| v.as_str()),
            Some("example.com")
        );
        assert_eq!(
            xhttp_opts.get("mode").and_then(|v| v.as_str()),
            Some("packet-up")
        );
    }

    #[test]
    fn test_normalize_subscription_to_yaml_base64() {
        let links = "vless://00000000-0000-0000-0000-000000000000@example.com:443?type=tcp&security=none#Node1\nvless://00000000-0000-0000-0000-000000000001@example.com:443?type=tcp&security=none#Node2";
        use base64::engine::general_purpose::STANDARD;
        let b64 = STANDARD.encode(links);

        let yaml = normalize_subscription_to_yaml(&b64).expect("normalize base64 failed");
        assert!(yaml.contains("proxies:"));
        assert!(yaml.contains("Node1"));
        assert!(yaml.contains("Node2"));
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
        let parsed = parse_share_links(text);
        assert_eq!(parsed.failed_lines.len(), 0);
        let nodes = parsed.nodes;
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
