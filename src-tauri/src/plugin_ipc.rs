use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::plugin::runtime_pipe_name;
use crate::worker::WorkerState;

pub async fn serve(state: Arc<WorkerState>) -> Result<(), String> {
    #[cfg(windows)]
    {
        serve_windows(state).await
    }
    #[cfg(not(windows))]
    {
        let _ = state;
        Err("插件 IPC 仅支持 Windows。".to_string())
    }
}

#[cfg(windows)]
async fn serve_windows(state: Arc<WorkerState>) -> Result<(), String> {
    use tokio::net::windows::named_pipe::ServerOptions;

    let pipe_name = runtime_pipe_name()?;
    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&pipe_name)
        .map_err(|error| format!("创建插件管道失败：{error}"))?;

    loop {
        if state.is_quitting() {
            return Ok(());
        }
        server
            .connect()
            .await
            .map_err(|error| format!("等待插件管道连接失败：{error}"))?;
        let connected = server;
        server = ServerOptions::new()
            .create(&pipe_name)
            .map_err(|error| format!("重建插件管道失败：{error}"))?;

        let state = Arc::clone(&state);
        tokio::spawn(async move {
            if let Err(error) = handle_client(state, connected).await {
                eprintln!("插件 IPC 会话结束：{error}");
            }
        });
    }
}

#[cfg(windows)]
async fn handle_client(
    state: Arc<WorkerState>,
    client: impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
) -> Result<(), String> {
    let (reader, mut writer) = tokio::io::split(client);
    let mut lines = BufReader::new(reader).lines();
    while let Some(line) = lines.next_line().await.map_err(|error| error.to_string())? {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }
        let response = state.handle_line(&line).await;
        let mut payload = serde_json::to_string(&response).map_err(|error| error.to_string())?;
        payload.push('\n');
        writer
            .write_all(payload.as_bytes())
            .await
            .map_err(|error| error.to_string())?;
        writer.flush().await.map_err(|error| error.to_string())?;
        if state.is_quitting() {
            return Ok(());
        }
    }
    Ok(())
}
