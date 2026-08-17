# Background Studio 插件协议

`pluginProtocol: 2`

Codex Background Studio 是只由 Background Studio 壳启动的无界面 worker。不再提供独立窗口、托盘或安装器。

## 启动

壳启动：

```text
"Codex Background Studio.exe"
```

worker 创建 Named Pipe 后等待命令。未收到有效 `configure` 之前只报告「尚未配置背景」，不会接管或关闭 Codex。

## Pipe

`\\.\pipe\background-studio-codex`

## 消息格式

每行一个 JSON。

请求：

```json
{"id":"1","cmd":"hello|configure|status|apply|pause|restore|shutdown","params":{}}
```

成功：`{"id":"1","ok":true,"result":{...}}`

失败：`{"id":"1","ok":false,"error":"..."}`

### hello

```json
{
  "pluginProtocol": 2,
  "pluginId": "codex",
  "version": "0.5.4-beta.2",
  "capabilities": {
    "mediaKinds": ["image", "video"],
    "injection": "cdp-blob",
    "hotUpdate": true,
    "autoTakeover": true,
    "loopbackMediaOnly": true,
    "keepsTargetOnShutdown": true,
    "maxMediaBytes": 67108864,
    "commands": ["hello", "configure", "status", "apply", "pause", "restore", "shutdown"]
  }
}
```

### configure

```json
{
  "schemaVersion": 1,
  "revision": "sha256-or-digest",
  "media": {
    "url": "http://127.0.0.1:<port>/<token>/media/<id>?v=...",
    "kind": "image",
    "mimeType": "image/png",
    "sha256": "64-hex",
    "byteSize": 123
  },
  "display": {
    "fit": "cover",
    "opacity": 0.72
  }
}
```

约束：

- URL 只接受 `http://127.0.0.1` 或 `http://localhost`，必须带显式端口
- 拒绝 userinfo、fragment、非回环解析、端口 0、超长 URL/字段/JSON
- 使用 `no_proxy` 客户端拉取，上限 64 MiB
- 校验 `Content-Length`、实际大小、`sha256`、`mimeType`/`kind`

配置成功后 watcher 才允许 Attach/Takeover。若当前已是 `active`，会热更新注入。`apply` 使用最近一次有效 configure，必要时重启接管并重新武装 watcher。

### status

在原有 `phase` / `message` / `activeTargets` / `paused` 之外增加 `configured` 和 `revision`。

### shutdown

退出 worker，不关闭或恢复 Codex。成功结果为 `{"shutdown":true,"keptTarget":true}`。

## Release 产物

`CodexBackgroundStudio-<version>-plugin.zip`

内含 `Codex Background Studio.exe` 与 `plugin.json`。壳安装到：

`%LOCALAPPDATA%\BackgroundStudio\plugins\codex\<version>\`
