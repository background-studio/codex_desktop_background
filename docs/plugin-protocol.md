# Background Studio 插件协议

`pluginProtocol: 1`

Codex / Notion Background Studio 可作为 Background Studio 壳的插件进程运行。

## 启动

```text
CodexBackgroundStudio.exe --plugin
```

插件模式行为：

- 不创建系统托盘
- 不写本应用的 Windows 自启动项
- 主窗口默认隐藏；由壳通过 IPC `open-ui` 打开
- 在 Named Pipe 上提供控制接口
- 启动后台托管 worker：不自动打开 Codex，等待用户照常启动官方程序
- 识别到启用后新启动的普通官方进程后，按完整可执行路径确认，关闭并以 AppUserModelId 带本机调试参数重启，然后自动注入上次背景
- 插件启动前已经在运行的普通进程不会被自动关闭；状态为“已在运行，点立即接管可重启”，由现有 IPC `apply` 手动重启接管
- 已有有效调试会话会直接重连
- 目标退出后清理失效调试会话并重新等待
- `pause` / `restore` 会暂停本次插件进程内的自动接管；再次 `apply` 后重新武装
- 停用由壳结束插件进程，不改动当前 Codex

独立启动（无 `--plugin`）保持原有托盘与自启动行为，不启动托管 worker。

## Pipe 名称

| 插件 | Pipe |
|------|------|
| Codex | `\\.\pipe\background-studio-codex` |
| Notion | `\\.\pipe\background-studio-notion` |

## 消息格式

换行分隔 JSON（NDJSON）。主机发起请求，插件回复。

### 请求

```json
{"id":"1","cmd":"status"}
{"id":"2","cmd":"open-ui"}
{"id":"3","cmd":"apply"}
{"id":"4","cmd":"pause"}
{"id":"5","cmd":"restore"}
{"id":"6","cmd":"quit-keep-target"}
```

### 成功响应

```json
{
  "id": "1",
  "ok": true,
  "result": {
    "pluginProtocol": 1,
    "pluginId": "codex",
    "version": "0.5.0",
    "phase": "active",
    "message": "背景已自动应用",
    "activeTargets": 1,
    "paused": false
  }
}
```

### 失败响应

```json
{"id":"3","ok":false,"error":"……"}
```

`phase` / `message` 在插件模式下常见取值：

| phase | message | 含义 |
|-------|---------|------|
| `waiting` | 已启用，等待 Codex 启动 | 官方程序未运行，worker 只等待 |
| `waiting` | 正在等待 Codex 调试端口就绪 | 已看到调试参数，端口未就绪，不编码媒体、不 attach |
| `blocked` | Codex 已在运行，点立即接管可重启 | 启用前已有普通进程，需 IPC `apply` 手动接管 |
| `blocked` | 请先选择背景后再接管 Codex | 看到可接管进程但还没有可注入背景 |
| `starting` | 正在接管 Codex | 正在关闭普通进程并以调试参数重启 |
| `active` | 背景已自动应用 | 已连接并注入上次背景 |
| `paused` | 暂停托管 | `pause` / `restore` 后不再自动抓回 |
| `error` | Codex 调试端口未能在 45 秒内就绪，等待进程退出后重试 | 带调试参数的进程不会被强杀，退出后重新等待 |
| `error` | 具体错误 | 发现、启动或注入失败 |

协议版本仍为 `pluginProtocol: 1`，命令不变。旧壳可以忽略新增状态语义。

## Release 产物

除 NSIS 安装包外，每个插件仓发布：

- `CodexBackgroundStudio-<version>-plugin.zip`
- `NotionBackgroundStudio-<version>-plugin.zip`

壳解压到：

`%LOCALAPPDATA%\BackgroundStudio\plugins\<pluginId>\<version>\`
