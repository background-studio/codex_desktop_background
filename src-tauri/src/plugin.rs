pub const PLUGIN_PROTOCOL: u32 = 2;
pub const PLUGIN_ID: &str = "codex";
pub const PIPE_NAME: &str = r"\\.\pipe\background-studio-codex";
#[allow(dead_code)]
pub const EXE_NAME: &str = "Codex Background Studio.exe";

pub fn runtime_pipe_name() -> Result<String, String> {
    #[cfg(feature = "integration-test-pipe")]
    if let Some(value) = std::env::var_os("BACKGROUND_STUDIO_TEST_PIPE") {
        let value = value
            .into_string()
            .map_err(|_| "测试管道名称不是有效 UTF-8。".to_string())?;
        if !value.starts_with(r"\\.\pipe\background-studio-test-") || value.len() > 160 {
            return Err("测试管道名称无效。".to_string());
        }
        return Ok(value);
    }
    Ok(PIPE_NAME.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn plugin_manifest_matches_runtime_constants() {
        let manifest: Value =
            serde_json::from_str(include_str!("../../plugin.json")).expect("plugin.json");
        assert_eq!(manifest["schemaVersion"], 1);
        assert_eq!(manifest["pluginProtocol"], PLUGIN_PROTOCOL);
        assert_eq!(manifest["id"], PLUGIN_ID);
        assert_eq!(manifest["exeName"], EXE_NAME);
        assert_eq!(manifest["pipeName"], PIPE_NAME);
        assert_eq!(
            manifest["capabilities"]["maxMediaBytes"],
            crate::protocol::MAX_MEDIA_BYTES
        );
        assert!(manifest["settingsSchema"]["properties"]["terminalOpacity"].is_object());
        assert!(manifest["capabilities"]["commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "configure"));
    }
}

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
