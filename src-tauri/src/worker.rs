use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, MutexGuard,
    },
};

use serde_json::{json, Value};

use crate::{
    configure::{fetch_loopback_media, parse_configure_params},
    controller::CodexController,
    managed_launch::{HostedAction, MSG_UNCONFIGURED, PHASE_ACTIVE, PHASE_WAITING},
    models::{DisplaySettings, RuntimeStatus},
    payload::{build_active_payload_from_bytes, ActivePayload},
    plugin::{version, PLUGIN_ID, PLUGIN_PROTOCOL},
    protocol::{
        error_response, hello_result, ok_response, parse_request_line, PluginRequest, StatusResult,
    },
};

fn data_directory() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("CodexBackgroundStudio")
}

fn lock<T>(value: &Mutex<T>) -> Result<MutexGuard<'_, T>, String> {
    value
        .lock()
        .map_err(|_| "工作线程状态锁已损坏。".to_string())
}

#[derive(Clone)]
struct ConfiguredSession {
    revision: String,
    payload: ActivePayload,
}

pub struct WorkerState {
    controller: Arc<Mutex<CodexController>>,
    runtime_status: Mutex<RuntimeStatus>,
    configured: Mutex<Option<ConfiguredSession>>,
    quitting: AtomicBool,
}

impl WorkerState {
    pub fn load() -> Result<Self, String> {
        Self::load_at(&data_directory())
    }

    pub fn load_at(data_directory: &std::path::Path) -> Result<Self, String> {
        let controller = CodexController::load(data_directory);
        let runtime_status = controller.status();
        Ok(Self {
            controller: Arc::new(Mutex::new(controller)),
            runtime_status: Mutex::new(runtime_status),
            configured: Mutex::new(None),
            quitting: AtomicBool::new(false),
        })
    }

    pub fn is_quitting(&self) -> bool {
        self.quitting.load(Ordering::SeqCst)
    }

    pub fn is_configured(&self) -> bool {
        lock(&self.configured)
            .map(|value| value.is_some())
            .unwrap_or(false)
    }

    #[cfg(test)]
    pub fn can_takeover(&self) -> bool {
        if !self.is_configured() {
            return false;
        }
        lock(&self.controller)
            .map(|controller| !controller.status().phase.eq("paused"))
            .unwrap_or(false)
            && !self
                .runtime_status()
                .map(|status| status.phase == "paused")
                .unwrap_or(false)
    }

    fn configured_session(&self) -> Result<Option<ConfiguredSession>, String> {
        Ok(lock(&self.configured)?.clone())
    }

    fn current_payload(&self) -> Result<ActivePayload, String> {
        lock(&self.configured)?
            .as_ref()
            .map(|session| session.payload.clone())
            .ok_or_else(|| MSG_UNCONFIGURED.to_string())
    }

    pub fn runtime_status(&self) -> Result<RuntimeStatus, String> {
        if let Ok(controller) = self.controller.try_lock() {
            let status = controller.status();
            *lock(&self.runtime_status)? = status.clone();
            return Ok(status);
        }
        Ok(lock(&self.runtime_status)?.clone())
    }

    fn refresh_runtime_status(&self) -> Result<RuntimeStatus, String> {
        let status = lock(&self.controller)?.status();
        *lock(&self.runtime_status)? = status.clone();
        Ok(status)
    }

    pub fn status_result(&self) -> Result<StatusResult, String> {
        let configured = self.configured_session()?;
        let mut status = self.runtime_status()?;
        if configured.is_none() && status.phase != "paused" {
            status.phase = PHASE_WAITING.to_string();
            status.message = MSG_UNCONFIGURED.to_string();
            status.last_error = None;
        }
        Ok(StatusResult {
            plugin_protocol: PLUGIN_PROTOCOL,
            plugin_id: PLUGIN_ID,
            version: version(),
            phase: status.phase.clone(),
            message: status.message,
            active_targets: status.active_targets,
            paused: status.phase == "paused",
            configured: configured.is_some(),
            revision: configured.map(|session| session.revision),
        })
    }

    pub fn note_unconfigured(&self) -> Result<(), String> {
        let mut controller = lock(&self.controller)?;
        controller.note_unconfigured_wait();
        drop(controller);
        self.refresh_runtime_status()?;
        Ok(())
    }

    pub async fn configure(&self, params: Option<Value>) -> Result<Value, String> {
        let params = params.ok_or_else(|| "configure 缺少 params。".to_string())?;
        let request = parse_configure_params(&params)?;
        let display = DisplaySettings::from_configure(&request.display)?;
        let fetched = fetch_loopback_media(&request.media).await?;
        let payload = build_active_payload_from_bytes(
            &fetched.bytes,
            &fetched.mime_type,
            &fetched.kind,
            &display,
        )?;
        let session = ConfiguredSession {
            revision: request.revision,
            payload: payload.clone(),
        };
        *lock(&self.configured)? = Some(session);

        let status = self.runtime_status()?;
        if status.phase == PHASE_ACTIVE {
            let controller = Arc::clone(&self.controller);
            let result =
                tokio::task::spawn_blocking(move || lock(&controller)?.apply(payload, false))
                    .await
                    .map_err(|error| error.to_string())?;
            let _ = self.refresh_runtime_status();
            result?;
        }
        serde_json::to_value(self.status_result()?).map_err(|error| error.to_string())
    }

    pub async fn apply(&self) -> Result<Value, String> {
        let payload = self.current_payload()?;
        let controller = Arc::clone(&self.controller);
        let first_payload = payload.clone();
        let first =
            tokio::task::spawn_blocking(move || lock(&controller)?.apply(first_payload, false))
                .await
                .map_err(|error| error.to_string())?;
        let _ = self.refresh_runtime_status();
        match first {
            Ok(_) => {}
            Err(error) if error.contains("需要重启一次") => {
                let controller = Arc::clone(&self.controller);
                tokio::task::spawn_blocking(move || lock(&controller)?.apply(payload, true))
                    .await
                    .map_err(|error| error.to_string())??;
                let _ = self.refresh_runtime_status();
            }
            Err(error) => return Err(error),
        }
        serde_json::to_value(self.status_result()?).map_err(|error| error.to_string())
    }

    pub async fn pause(&self) -> Result<Value, String> {
        let controller = Arc::clone(&self.controller);
        tokio::task::spawn_blocking(move || lock(&controller)?.pause())
            .await
            .map_err(|error| error.to_string())??;
        let _ = self.refresh_runtime_status();
        serde_json::to_value(self.status_result()?).map_err(|error| error.to_string())
    }

    pub async fn restore(&self) -> Result<Value, String> {
        let controller = Arc::clone(&self.controller);
        tokio::task::spawn_blocking(move || lock(&controller)?.restore())
            .await
            .map_err(|error| error.to_string())??;
        let _ = self.refresh_runtime_status();
        serde_json::to_value(self.status_result()?).map_err(|error| error.to_string())
    }

    pub fn shutdown(&self) -> Result<Value, String> {
        self.quitting.store(true, Ordering::SeqCst);
        Ok(json!({ "shutdown": true, "keptTarget": true }))
    }

    pub async fn dispatch(&self, request: PluginRequest) -> Value {
        let id = request.id.clone();
        let result = match request.cmd.as_str() {
            "hello" => serde_json::to_value(hello_result()).map_err(|error| error.to_string()),
            "configure" => self.configure(request.params).await,
            "status" => self
                .status_result()
                .and_then(|status| serde_json::to_value(status).map_err(|error| error.to_string())),
            "apply" => self.apply().await,
            "pause" => self.pause().await,
            "restore" => self.restore().await,
            "shutdown" | "quit-keep-target" => self.shutdown(),
            "open-ui" => Err("此插件没有独立界面。".to_string()),
            other => Err(format!("未知命令：{other}")),
        };
        match result {
            Ok(value) => ok_response(&id, value),
            Err(error) => error_response(&id, error),
        }
    }

    pub async fn handle_line(&self, line: &str) -> Value {
        match parse_request_line(line) {
            Ok(request) => self.dispatch(request).await,
            Err(error) => error_response("", error),
        }
    }
}

pub async fn run_managed_watcher(state: Arc<WorkerState>) {
    loop {
        if state.is_quitting() {
            break;
        }
        if !state.is_configured() {
            let _ = state.note_unconfigured();
        } else {
            let controller = Arc::clone(&state.controller);
            let decision =
                tokio::task::spawn_blocking(move || lock(&controller)?.probe_managed()).await;
            match decision {
                Ok(Ok(decision))
                    if matches!(
                        decision.action,
                        HostedAction::Attach | HostedAction::Takeover
                    ) =>
                {
                    if let Ok(payload) = state.current_payload() {
                        let controller = Arc::clone(&state.controller);
                        let result = tokio::task::spawn_blocking(move || {
                            lock(&controller)?.run_managed_action(payload)
                        })
                        .await;
                        if let Ok(Err(error)) = result {
                            eprintln!("自动接管失败：{error}");
                        }
                    }
                }
                Ok(Err(error)) => eprintln!("托管探测失败：{error}"),
                Err(error) => eprintln!("托管探测任务失败：{error}"),
                _ => {}
            }
            let _ = state.refresh_runtime_status();
        }
        if state.is_quitting() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}

pub fn run() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("创建 Tokio runtime 失败：{error}"))?;
    runtime.block_on(async {
        let state = Arc::new(WorkerState::load()?);
        let watcher_state = Arc::clone(&state);
        let watcher = tokio::spawn(async move {
            run_managed_watcher(watcher_state).await;
        });
        let ipc_state = Arc::clone(&state);
        let mut ipc = tokio::spawn(async move { crate::plugin_ipc::serve(ipc_state).await });
        let serve_result = loop {
            tokio::select! {
                result = &mut ipc => {
                    break result
                        .map_err(|error| format!("插件 IPC 任务失败：{error}"))?;
                }
                _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                    if state.is_quitting() {
                        ipc.abort();
                        break Ok(());
                    }
                }
            }
        };
        state.quitting.store(true, Ordering::SeqCst);
        watcher.abort();
        // 主动退出时不要走 InjectorEngine::stop，避免把已注入的 Codex 背景清掉。
        std::mem::forget(state);
        serve_result
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configure::sha256_hex;
    use crate::models::MediaKind;
    use serde_json::json;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use uuid::Uuid;

    async fn serve_bytes(body: &[u8], mime: &str) -> (u16, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let body = body.to_vec();
        let mime = mime.to_string();
        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buffer = vec![0u8; 2048];
            let _ = stream.read(&mut buffer).await;
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(header.as_bytes()).await;
            let _ = stream.write_all(&body).await;
        });
        (port, handle)
    }

    fn temp_state() -> WorkerState {
        let root = std::env::temp_dir().join(format!("codex-worker-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        WorkerState::load_at(&root).unwrap()
    }

    fn configure_params(url: String, bytes: &[u8]) -> Value {
        json!({
            "schemaVersion": 1,
            "revision": "rev-1",
            "media": {
                "url": url,
                "kind": "image",
                "mimeType": "image/png",
                "sha256": sha256_hex(bytes),
                "byteSize": bytes.len()
            },
            "display": { "opacity": 0.5 }
        })
    }

    #[tokio::test]
    async fn protocol_boundaries_and_unknown_commands() {
        let state = temp_state();
        let unknown = state.handle_line(r#"{"id":"1","cmd":"explode"}"#).await;
        assert_eq!(unknown["ok"], false);
        assert!(unknown["error"].as_str().unwrap().contains("未知命令"));

        let huge = format!(r#"{{"id":"1","cmd":"{}"}}"#, "a".repeat(40_000));
        let oversized = state.handle_line(&huge).await;
        assert_eq!(oversized["ok"], false);

        let hello = state.handle_line(r#"{"id":"hello-1","cmd":"hello"}"#).await;
        assert_eq!(hello["ok"], true);
        assert_eq!(hello["result"]["pluginProtocol"], 2);
        assert_eq!(hello["result"]["pluginId"], "codex");
    }

    #[tokio::test]
    async fn unconfigured_does_not_allow_takeover_or_apply() {
        let state = temp_state();
        state.note_unconfigured().unwrap();
        let status = state.status_result().unwrap();
        assert!(!status.configured);
        assert_eq!(status.message, MSG_UNCONFIGURED);
        assert!(!state.can_takeover());

        let apply = state.handle_line(r#"{"id":"2","cmd":"apply"}"#).await;
        assert_eq!(apply["ok"], false);
        assert!(apply["error"].as_str().unwrap().contains("尚未配置背景"));
    }

    #[tokio::test]
    async fn configure_then_allows_auto_takeover_and_reports_revision() {
        let body = b"worker-png";
        let (port, server) = serve_bytes(body, "image/png").await;
        let state = temp_state();
        let line = json!({
            "id": "3",
            "cmd": "configure",
            "params": configure_params(format!("http://127.0.0.1:{port}/media/1"), body)
        })
        .to_string();
        let response = state.handle_line(&line).await;
        let _ = server.await;
        assert_eq!(response["ok"], true, "{response}");
        assert_eq!(response["result"]["configured"], true);
        assert_eq!(response["result"]["revision"], "rev-1");
        assert!(state.can_takeover());
        assert_eq!(
            build_active_payload_from_bytes(
                body,
                "image/png",
                &MediaKind::Image,
                &DisplaySettings::from_configure(&json!({ "opacity": 0.5 })).unwrap()
            )
            .unwrap()
            .media_bytes
            .as_ref(),
            body
        );
    }

    #[tokio::test]
    async fn pause_restore_and_shutdown_keep_target_semantics() {
        let body = b"pause-png";
        let (port, server) = serve_bytes(body, "image/png").await;
        let state = temp_state();
        let configure = json!({
            "id": "4",
            "cmd": "configure",
            "params": configure_params(format!("http://127.0.0.1:{port}/media/1"), body)
        })
        .to_string();
        assert_eq!(state.handle_line(&configure).await["ok"], true);
        let _ = server.await;
        assert!(state.can_takeover());

        let paused = state.handle_line(r#"{"id":"5","cmd":"pause"}"#).await;
        assert_eq!(paused["ok"], true);
        assert_eq!(paused["result"]["paused"], true);
        assert!(!state.can_takeover());

        let restored = state.handle_line(r#"{"id":"6","cmd":"restore"}"#).await;
        assert!(restored.get("ok").is_some());
        assert!(!state.is_quitting());

        let shutdown = state.handle_line(r#"{"id":"7","cmd":"shutdown"}"#).await;
        assert_eq!(shutdown["ok"], true);
        assert_eq!(shutdown["result"]["shutdown"], true);
        assert_eq!(shutdown["result"]["keptTarget"], true);
        assert!(state.is_quitting());
    }
}
