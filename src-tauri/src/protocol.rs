use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::plugin::{version, PLUGIN_ID, PLUGIN_PROTOCOL};

pub const MAX_REQUEST_BYTES: usize = 32 * 1024;
pub const MAX_ID_CHARS: usize = 128;
pub const MAX_CMD_CHARS: usize = 64;
pub const MAX_URL_CHARS: usize = 2048;
pub const MAX_REVISION_CHARS: usize = 128;
pub const MAX_MIME_CHARS: usize = 80;
pub const MAX_MEDIA_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Deserialize)]
pub struct PluginRequest {
    pub id: String,
    pub cmd: String,
    #[serde(default)]
    pub params: Option<Value>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCapabilities {
    pub media_kinds: [&'static str; 2],
    pub injection: &'static str,
    pub hot_update: bool,
    pub auto_takeover: bool,
    pub loopback_media_only: bool,
    pub keeps_target_on_shutdown: bool,
    pub max_media_bytes: u64,
    pub commands: [&'static str; 7],
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloResult {
    pub plugin_protocol: u32,
    pub plugin_id: &'static str,
    pub version: &'static str,
    pub capabilities: PluginCapabilities,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusResult {
    pub plugin_protocol: u32,
    pub plugin_id: &'static str,
    pub version: &'static str,
    pub phase: String,
    pub message: String,
    pub active_targets: u32,
    pub paused: bool,
    pub configured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
}

pub fn capabilities() -> PluginCapabilities {
    PluginCapabilities {
        media_kinds: ["image", "video"],
        injection: "cdp-blob",
        hot_update: true,
        auto_takeover: true,
        loopback_media_only: true,
        keeps_target_on_shutdown: true,
        max_media_bytes: MAX_MEDIA_BYTES,
        commands: [
            "hello",
            "configure",
            "status",
            "apply",
            "pause",
            "restore",
            "shutdown",
        ],
    }
}

pub fn hello_result() -> HelloResult {
    HelloResult {
        plugin_protocol: PLUGIN_PROTOCOL,
        plugin_id: PLUGIN_ID,
        version: version(),
        capabilities: capabilities(),
    }
}

pub fn ok_response(id: &str, result: Value) -> Value {
    json!({ "id": id, "ok": true, "result": result })
}

pub fn error_response(id: &str, error: impl AsRef<str>) -> Value {
    json!({ "id": id, "ok": false, "error": error.as_ref() })
}

pub fn parse_request_line(line: &str) -> Result<PluginRequest, String> {
    if line.len() > MAX_REQUEST_BYTES {
        return Err("请求超过 32 KiB 上限。".to_string());
    }
    let request: PluginRequest =
        serde_json::from_str(line).map_err(|error| format!("无效请求：{error}"))?;
    if request.id.is_empty() || request.id.len() > MAX_ID_CHARS {
        return Err("请求 id 无效。".to_string());
    }
    if request.cmd.is_empty() || request.cmd.len() > MAX_CMD_CHARS {
        return Err("请求 cmd 无效。".to_string());
    }
    if let Some(params) = &request.params {
        let encoded = serde_json::to_string(params).map_err(|error| error.to_string())?;
        if encoded.len() > MAX_REQUEST_BYTES {
            return Err("params 超过 32 KiB 上限。".to_string());
        }
    }
    Ok(request)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_reports_protocol_two_and_codex_id() {
        let hello = hello_result();
        assert_eq!(hello.plugin_protocol, 2);
        assert_eq!(hello.plugin_id, "codex");
        assert!(hello.capabilities.commands.contains(&"configure"));
        assert!(hello.capabilities.commands.contains(&"shutdown"));
    }

    #[test]
    fn rejects_oversized_or_malformed_requests() {
        assert!(parse_request_line(&"x".repeat(MAX_REQUEST_BYTES + 1)).is_err());
        assert!(parse_request_line("{").is_err());
        assert!(parse_request_line(r#"{"id":"","cmd":"hello"}"#).is_err());
        assert!(parse_request_line(r#"{"id":"1","cmd":""}"#).is_err());
        let parsed = parse_request_line(r#"{"id":"1","cmd":"hello"}"#).unwrap();
        assert_eq!(parsed.cmd, "hello");
        assert!(parsed.params.is_none());
    }
}
