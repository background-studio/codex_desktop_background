use std::{
    collections::HashSet,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE};
use reqwest::redirect::Policy;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::net::lookup_host;
use url::Url;

use crate::models::MediaKind;
use crate::protocol::{MAX_MEDIA_BYTES, MAX_MIME_CHARS, MAX_REVISION_CHARS, MAX_URL_CHARS};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigureParams {
    pub schema_version: u8,
    pub revision: String,
    pub media: MediaSpec,
    pub display: Value,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MediaSpec {
    pub url: String,
    pub kind: MediaKind,
    pub mime_type: String,
    pub sha256: String,
    pub byte_size: u64,
}

#[derive(Clone, Debug)]
pub struct FetchedMedia {
    pub bytes: Vec<u8>,
    pub kind: MediaKind,
    pub mime_type: String,
}

pub fn parse_configure_params(value: &Value) -> Result<ConfigureParams, String> {
    if value.get("schemaVersion").and_then(Value::as_u64) != Some(1) {
        return Err("configure.schemaVersion 必须是 1。".to_string());
    }
    let params: ConfigureParams = serde_json::from_value(value.clone())
        .map_err(|error| format!("configure params 无效：{error}"))?;
    if params.schema_version != 1 {
        return Err("configure.schemaVersion 必须是 1。".to_string());
    }
    if params.revision.is_empty() || params.revision.len() > MAX_REVISION_CHARS {
        return Err("configure.revision 无效。".to_string());
    }
    validate_media_spec(&params.media)?;
    Ok(params)
}

pub fn validate_media_spec(media: &MediaSpec) -> Result<(), String> {
    if media.mime_type.is_empty() || media.mime_type.len() > MAX_MIME_CHARS {
        return Err("media.mimeType 无效。".to_string());
    }
    if !media
        .mime_type
        .chars()
        .all(|character| character.is_ascii() && !character.is_ascii_control())
    {
        return Err("media.mimeType 无效。".to_string());
    }
    let expected_kind = kind_for_mime(&media.mime_type)
        .ok_or_else(|| format!("不支持的 media.mimeType：{}。", media.mime_type))?;
    if expected_kind != media.kind {
        return Err("media.kind 与 mimeType 不匹配。".to_string());
    }
    if media.byte_size == 0 || media.byte_size > MAX_MEDIA_BYTES {
        return Err("media.byteSize 超出 64 MiB 上限。".to_string());
    }
    normalize_sha256(&media.sha256)?;
    validate_loopback_url(&media.url)?;
    Ok(())
}

pub fn kind_for_mime(mime_type: &str) -> Option<MediaKind> {
    match mime_type {
        "image/png" | "image/jpeg" | "image/webp" | "image/gif" | "image/avif" => {
            Some(MediaKind::Image)
        }
        "video/mp4" | "video/webm" | "video/ogg" | "video/quicktime" => Some(MediaKind::Video),
        _ => None,
    }
}

pub fn normalize_sha256(value: &str) -> Result<String, String> {
    if value.len() != 64 || !value.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err("media.sha256 必须是 64 位十六进制。".to_string());
    }
    Ok(value.to_ascii_lowercase())
}

pub fn validate_loopback_url(value: &str) -> Result<Url, String> {
    if value.len() > MAX_URL_CHARS {
        return Err("媒体 URL 超过长度上限。".to_string());
    }
    let url = Url::parse(value).map_err(|_| "媒体 URL 无效。".to_string())?;
    if url.scheme() != "http" {
        return Err("媒体 URL 只允许 http://127.0.0.1 或 http://localhost。".to_string());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("媒体 URL 不允许包含账号信息。".to_string());
    }
    if url.fragment().is_some() {
        return Err("媒体 URL 不允许包含 fragment。".to_string());
    }
    let host = url
        .host_str()
        .ok_or_else(|| "媒体 URL 缺少主机名。".to_string())?;
    if host != "127.0.0.1" && !host.eq_ignore_ascii_case("localhost") {
        return Err("媒体 URL 只允许 127.0.0.1 或 localhost。".to_string());
    }
    let port = url
        .port()
        .ok_or_else(|| "媒体 URL 必须包含显式端口。".to_string())?;
    if port == 0 {
        return Err("媒体 URL 端口无效。".to_string());
    }
    Ok(url)
}

async fn resolve_loopback(url: &Url) -> Result<Vec<SocketAddr>, String> {
    let host = url
        .host_str()
        .ok_or_else(|| "媒体 URL 缺少主机名。".to_string())?;
    let port = url
        .port()
        .ok_or_else(|| "媒体 URL 端口无效。".to_string())?;
    if host == "127.0.0.1" {
        return Ok(vec![SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)]);
    }
    let mut unique = HashSet::new();
    let mut addresses = Vec::new();
    for address in lookup_host((host, port))
        .await
        .map_err(|error| format!("媒体主机名解析失败：{error}"))?
    {
        if !address.ip().is_loopback() {
            return Err("媒体主机名解析到了非回环地址。".to_string());
        }
        if unique.insert(address) {
            addresses.push(address);
        }
    }
    if addresses.is_empty() {
        return Err("媒体主机名没有可用的回环地址。".to_string());
    }
    Ok(addresses)
}

fn header_text(response: &reqwest::Response, name: &reqwest::header::HeaderName) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().to_string())
}

pub async fn fetch_loopback_media(media: &MediaSpec) -> Result<FetchedMedia, String> {
    let url = validate_loopback_url(&media.url)?;
    let addresses = resolve_loopback(&url).await?;
    let host = url.host_str().unwrap_or("127.0.0.1");
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(Policy::none())
        .timeout(Duration::from_secs(30))
        .resolve_to_addrs(host, &addresses)
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .get(url)
        .header("Cache-Control", "no-store")
        .header("Accept", &media.mime_type)
        .send()
        .await
        .map_err(|error| format!("获取媒体失败：{error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!(
            "获取媒体失败，服务器返回 HTTP {}。",
            status.as_u16()
        ));
    }
    if let Some(length) = header_text(&response, &CONTENT_LENGTH) {
        let content_length = length
            .parse::<u64>()
            .map_err(|_| "媒体 Content-Length 无效。".to_string())?;
        if content_length > MAX_MEDIA_BYTES {
            return Err("媒体 Content-Length 超过 64 MiB 上限。".to_string());
        }
        if content_length != media.byte_size {
            return Err("媒体 Content-Length 与 byteSize 不一致。".to_string());
        }
    }
    let content_type = header_text(&response, &CONTENT_TYPE)
        .ok_or_else(|| "媒体响应缺少 Content-Type。".to_string())?;
    let mime = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if mime.is_empty() || mime != media.mime_type.to_ascii_lowercase() {
        return Err("媒体 Content-Type 与 mimeType 不一致。".to_string());
    }

    let mut bytes = Vec::new();
    let mut response = response;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("读取媒体中断：{error}"))?
    {
        let next = bytes.len().saturating_add(chunk.len());
        if next as u64 > MAX_MEDIA_BYTES {
            return Err("媒体超过 64 MiB 上限。".to_string());
        }
        if next as u64 > media.byte_size {
            return Err("媒体实际大小超过 byteSize。".to_string());
        }
        bytes.extend_from_slice(&chunk);
    }
    if bytes.len() as u64 != media.byte_size {
        return Err("媒体实际大小与 byteSize 不一致。".to_string());
    }
    let digest = format!("{:x}", Sha256::digest(&bytes));
    let expected = normalize_sha256(&media.sha256)?;
    if digest != expected {
        return Err("媒体 sha256 校验失败。".to_string());
    }
    Ok(FetchedMedia {
        bytes,
        kind: media.kind.clone(),
        mime_type: media.mime_type.clone(),
    })
}

#[cfg(test)]
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    async fn serve_http(
        status: &str,
        headers: &[(&str, String)],
        body: &[u8],
    ) -> (u16, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let status = status.to_string();
        let headers = headers
            .iter()
            .map(|(name, value)| (name.to_string(), value.clone()))
            .collect::<Vec<_>>();
        let body = body.to_vec();
        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buffer = vec![0u8; 4096];
            let _ = stream.read(&mut buffer).await;
            let mut response = format!("HTTP/1.1 {status}\r\nConnection: close\r\n");
            for (name, value) in &headers {
                response.push_str(&format!("{name}: {value}\r\n"));
            }
            response.push_str("\r\n");
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.write_all(&body).await;
        });
        (port, handle)
    }

    fn spec(url: String, bytes: &[u8], mime: &str, kind: MediaKind) -> MediaSpec {
        MediaSpec {
            url,
            kind,
            mime_type: mime.to_string(),
            sha256: sha256_hex(bytes),
            byte_size: bytes.len() as u64,
        }
    }

    #[test]
    fn rejects_non_loopback_or_credentialed_urls() {
        for url in [
            "https://127.0.0.1:9/media",
            "http://192.168.1.2:9/media",
            "http://127.0.0.1/media",
            "http://127.0.0.1:0/media",
            "http://user@127.0.0.1:9/media",
            "http://user:pass@127.0.0.1:9/media",
            "http://localhost:9/media#x",
            "file:///C:/secret.png",
            &format!("http://127.0.0.1:9/{}", "a".repeat(MAX_URL_CHARS)),
        ] {
            assert!(validate_loopback_url(url).is_err(), "{url}");
        }
        assert!(validate_loopback_url("http://127.0.0.1:34100/token/media/1?v=2").is_ok());
        assert!(validate_loopback_url("http://localhost:34100/token/media/1").is_ok());
    }

    #[test]
    fn rejects_invalid_configure_envelope() {
        assert!(parse_configure_params(&json!({})).is_err());
        assert!(parse_configure_params(&json!({
            "schemaVersion": 2,
            "revision": "abc",
            "media": {
                "url": "http://127.0.0.1:9/m",
                "kind": "image",
                "mimeType": "image/png",
                "sha256": "a".repeat(64),
                "byteSize": 1
            },
            "display": {}
        }))
        .is_err());
        assert!(parse_configure_params(&json!({
            "schemaVersion": 1,
            "revision": "abc",
            "media": {
                "url": "http://127.0.0.1:9/m",
                "kind": "video",
                "mimeType": "image/png",
                "sha256": "a".repeat(64),
                "byteSize": 1
            },
            "display": {}
        }))
        .is_err());
    }

    #[tokio::test]
    async fn fetches_loopback_media_and_checks_hash_size_and_mime() {
        let body = b"png-bytes";
        let (port, server) = serve_http(
            "200 OK",
            &[
                ("Content-Type", "image/png".to_string()),
                ("Content-Length", body.len().to_string()),
            ],
            body,
        )
        .await;
        let media = spec(
            format!("http://127.0.0.1:{port}/media/1"),
            body,
            "image/png",
            MediaKind::Image,
        );
        let fetched = fetch_loopback_media(&media).await.unwrap();
        server.await.unwrap();
        assert_eq!(fetched.bytes, body);
        assert_eq!(sha256_hex(&fetched.bytes), sha256_hex(body));
    }

    #[tokio::test]
    async fn rejects_content_length_or_hash_mismatch() {
        let body = b"png-bytes";
        let (port, server) = serve_http(
            "200 OK",
            &[
                ("Content-Type", "image/png".to_string()),
                ("Content-Length", "3".to_string()),
            ],
            body,
        )
        .await;
        let media = spec(
            format!("http://127.0.0.1:{port}/media/1"),
            body,
            "image/png",
            MediaKind::Image,
        );
        assert!(fetch_loopback_media(&media).await.is_err());
        let _ = server.await;

        let (port, server) = serve_http(
            "200 OK",
            &[
                ("Content-Type", "image/png".to_string()),
                ("Content-Length", body.len().to_string()),
            ],
            body,
        )
        .await;
        let mut media = spec(
            format!("http://127.0.0.1:{port}/media/1"),
            body,
            "image/png",
            MediaKind::Image,
        );
        media.sha256 = "0".repeat(64);
        assert!(fetch_loopback_media(&media)
            .await
            .unwrap_err()
            .contains("sha256"));
        let _ = server.await;
    }
}
