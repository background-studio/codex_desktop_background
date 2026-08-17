# Codex Background Studio

[![org](https://img.shields.io/badge/org-background--studio-0ea5e9)](https://github.com/background-studio)
[![release](https://img.shields.io/github/v/release/background-studio/codex_desktop_background)](https://github.com/background-studio/codex_desktop_background/releases)

属于 [Background Studio](https://github.com/background-studio) 组织。本仓库是壳启动的无界面 Codex 背景 worker，不再提供独立窗口、托盘或 NSIS 安装器。

壳从 Release 下载 `CodexBackgroundStudio-<version>-plugin.zip`，按 [docs/plugin-protocol.md](./docs/plugin-protocol.md) 通过 Named Pipe 下发 `hello` / `configure` / `apply` 等命令。worker 在收到有效配置之前不会接管或关闭 Codex；配置完成后才会自动接管用户之后启动的官方进程，并用本机回环 CDP 注入背景。

> 非 OpenAI 官方产品。Codex 及相关商标归其权利人所有。

## 行为

- 只接受壳提供的本机回环媒体 URL，校验大小、哈希和 MIME
- 从内存 bytes 构造注入 payload，不维护本地媒体库
- 通过本机回环 Chromium DevTools Protocol 动态加载背景
- 不修改 `WindowsApps`、`app.asar`、应用签名、登录状态或对话数据
- `pause` / `restore` 暂停本次自动接管；再次 `apply` 后重新武装
- `shutdown` 只退出 worker，不关闭 Codex

## 开发

要求 Rust stable，以及 Visual Studio Build Tools 的“使用 C++ 的桌面开发”工作负载。完整接管测试还需要 Microsoft Store 的官方 `OpenAI.Codex` 应用。

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
cargo build --release --manifest-path src-tauri/Cargo.toml
```

维护 Codex 页面样式或 CDP 注入前，请先阅读项目 Skill：
[`codex-background-development`](./.cursor/skills/codex-background-development/SKILL.md)。

## 发布

推送与 `src-tauri/Cargo.toml` 版本一致的 `v*` 标签会触发 GitHub Actions，构建 worker 并上传：

`CodexBackgroundStudio-<version>-plugin.zip`
