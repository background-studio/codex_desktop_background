---
name: codex-background-development
description: 快速维护 Codex Background Studio 的 CDP 背景注入和透明度样式。用于用户反馈 Codex 更新后页面出现黑块、黑色阴影、渐变遮罩、Shadow DOM 闪烁、背景不生效，或需要修改、验证、打包这个仓库时；优先采用“真实页面检查、临时验证、精确修改、页面矩阵回归”的短流程。
---

# Codex Background Studio 开发

维护 Windows 官方 Codex 桌面应用的可逆背景注入。不要修改 `WindowsApps`、`app.asar`、签名、登录状态或对话数据。详细选择器和故障案例按需读取：

- 页面入口与定位：[windows-and-selectors.md](windows-and-selectors.md)
- 注入架构与安全边界：[architecture-and-pitfalls.md](architecture-and-pitfalls.md)

## UI/CSS 回归的短流程

### 1. 先保护现场，不要先重启

- 先运行 `git status --short --branch`，保留用户已有改动，不使用 `reset --hard` 或回退陌生文件。
- 优先复用已经运行的 Codex CDP；CSS 问题不要为了“重现”而杀掉或重启 Codex/Studio，这会丢失用户页面和当前任务。
- CDP 只连接 `127.0.0.1`、状态文件中相同的 browser ID，以及主 `app://-/index.html` target；排除 `avatar-overlay`。找不到现成端点时先修源码和测试，不要擅自启动受管进程。
- 视觉回归先取 DOM 证据；定位前不跑 release 构建、打包或全页面巡检。

### 2. 用真实 DOM 找到遮挡层

先记录当前路由和选中的聊天，最后恢复它。对目标页面收集：

- `document.documentElement.className`、`#codex-background-layer` 的 `opacity`，以及 `--cbg-*` 变量；先排除任务强度为 `0` 造成的“假黑屏”。
- 异常中心的 `document.elementsFromPoint(x, y)`、元素祖先链、标签/id/role/完整 class 和 `getBoundingClientRect()`。
- 计算后的 `backgroundColor`、`backgroundImage`、`boxShadow`、`backdropFilter`，并检查 `::before`、`::after`、`shadowRoot` 和 Shadow CSS 变量。

不要只看截图颜色猜选择器。页面导航优先用 CDP `Input.dispatchMouseEvent` 产生真实鼠标事件；React/Radix 页面可能不响应裸 `HTMLElement.click()`。每次导航后等待渲染，再重新取 DOM 和计算样式。

一次性探查若需要 Python，使用 `uv run`（例如 `uv run --with websocket-client python -`）；不要留下脚本或临时截图。

### 3. 先做临时验证，再写源码

先保存原 `textContent`，再把当前 `src/main/payload.ts` 的 `BACKGROUND_CSS` 临时写入页面现有的 `#codex-background-style`；保留原有 `data-cbg-revision`，观察页面是否立即恢复。这个动作只用于验证假设，不是最终修复；失败或结束时还原旧内容，不要直接把临时样式写入 Codex 安装目录。

每次只验证一个假设：

- 主壳/列表壳实底：清内部 surface，由 viewport 单层打底。
- 卡片/面板：跟随 `--cbg-menu-opacity`，内部同 token 层透明。
- 输入和搜索：跟随 `--cbg-composer-opacity`，清 sticky 实底及渐变伪元素。
- 阴影：清 `box-shadow`、`electron:elevation-prominent` 和不透明 `backdrop-filter`，不要降低整块内容的 opacity。
- Shadow DOM：只能通过早期 `attachShadow` 注入；不要用延迟 200ms 的补丁掩盖首帧黑屏。

确认临时规则有效后，把最小规则写入 `BACKGROUND_CSS` 或 `REVIEW_SHADOW_CSS`。兼容 Codex 新旧 token 时优先使用精确的 `class~=` 和稳定入口，保留旧选择器并补新 token；不要用 `[class*="..."]` 扫整棵树，也不要用 `:is(*)` 之类的宽泛清空规则。一个视觉区域只保留一层有色背景，透明度必须允许 `0`。

### 4. 用页面矩阵回归

至少检查本次涉及的页面，并在最后恢复原聊天：

1. 首页四张推荐卡片和 composer。
2. 对话页文件卡片、`产出/来源` 面板、底栏渐变和活动/审阅行。
3. 设置页主壳、分组卡片和搜索输入。
4. 拉取请求、站点、已安排、插件的整页 surface 与 sticky 搜索栏。
5. 任务审阅、文件树、浏览器、终端和 Shadow DOM（若本次涉及）。

每页都核对：主壳是否透明、目标元素是否保留文字和交互、`boxShadow`/渐变是否消失、左右区域是否只叠一层。再等待定时安装周期确认 MutationObserver 没有把规则改回去。需要视觉证据时捕获临时截图并在验证后删除。

### 5. 再做源码验证

CSS 修订号会自动混入 `BACKGROUND_CSS`/`REVIEW_SHADOW_CSS` 哈希；不要绕过 `buildRendererPayload()`。完成修改后运行：

```powershell
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml injector::tests::connects_to_live_codex_page_session -- --ignored
```

只有在源码和真实页面都通过后，才运行 `cargo build --release --manifest-path src-tauri/Cargo.toml`。显示设置变更还要同步 `plugin.json`、`models.rs`、payload 和测试。

## 不能破坏的实现约束

- `earlyPayloadFor()` 必须在 `documentElement` 出现时生效；动态更新用 `requestAnimationFrame` 合并，不用固定可见延迟。
- 新增 style、layer、Shadow style、observer、timer、Blob 或原型包装时，必须在 `cleanup()` 中全部移除或恢复。
- `body > #root` 可以锁定视口高度以保留对话滚动，但不要用 `body > :not(layer)` 改写 portal dialog 的定位。
- `backdrop-filter` 默认关闭；不要用整块 opacity 让文字和按钮一起变淡。
- 不改变进程接管安全边界：只接受回环 CDP、官方 `app://` target 和完整路径匹配的 Codex 进程。

## 代码入口

- `src/main/payload.ts`：页面 CSS、背景层、路由识别、Shadow DOM 和清理。
- `src-tauri/src/payload.rs`、`build.rs`：共享 payload 生成、修订哈希和 Rust 测试。
- `src-tauri/src/injector.rs`：CDP target、早期脚本、运行时更新和恢复。
- `src-tauri/src/controller.rs`、`managed_launch.rs`：官方进程发现、接管和生命周期。
- `src-tauri/src/models.rs`、`plugin.json`：显示参数、协议和壳 manifest。

## 提交与发布

只有用户明确要求时才提交、推送或打 tag。提交前检查完整 diff、版本、临时文件和工作区状态；发布包只包含 `Codex Background Studio.exe` 与 `plugin.json`，版本必须和 Cargo、协议文档及 tag 一致。发布优先走仓库 Release workflow，不在仓库留下本地 zip 或测试脚本。

## 完成条件

- 真实 Codex 页面确认目标样式恢复，且导航、滚动、重载和定时安装后仍稳定。
- 透明度叠加、原生实底、阴影、渐变和首帧闪烁均有证据排除。
- Rust 格式检查、单元测试和必要的 live CDP 测试通过。
- 所有临时脚本、截图和手工注入痕迹已清理；恢复官方外观仍可逆。
