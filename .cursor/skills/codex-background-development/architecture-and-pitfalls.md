# 架构与故障经验

## CDP 注入链路

完整流程：

1. `src-tauri/src/controller.rs` 用 PowerShell 查找 `OpenAI.Codex` Store 包。
2. 校验包签名类型、Manifest、`app/ChatGPT.exe` 和 AppUserModelId。
3. 若已有未启用 CDP 的 Codex，要求用户确认重启。
4. 通过 `IApplicationActivationManager` 启动 Codex，参数只绑定 `127.0.0.1`。
5. 选择 9335 起的可用端口，并等待 `/json/version`。
6. 保存 port、browser ID、包名和 executable 到 `runtime.json`。
7. `src-tauri/src/injector.rs` 只接受同一 browser ID、回环 WebSocket、`app://` page target。
8. 为每个 target 开启 Runtime/Page、临时 bypass CSP。
9. `Page.addScriptToEvaluateOnNewDocument` 注册早期 payload。
10. `Runtime.evaluate` 立即应用当前页面。
11. 每 1200ms 同步新 target；导航和重载由早期脚本覆盖。

安全边界不要放松：

- 不连接任意调试端口。
- 不接受非回环 WebSocket。
- 不向非 `app://` target 注入。
- 不按进程名粗暴结束所有 `ChatGPT.exe`；必须比较官方包 executable 的完整路径。

## Payload 生命周期

`payload.ts` 生成自包含 IIFE，状态保存在：

```text
window.__CODEX_BACKGROUND_STUDIO__
```

主要对象：

- `#codex-background-style`：普通 DOM 样式。
- `#codex-background-layer`：固定背景媒体层。
- `#codex-background-media`：图片或视频。
- `#codex-background-tile`：平铺模式。
- `#codex-background-overlay`：颜色遮罩。
- `#codex-background-review-shadow-style`：每个 diff Shadow Root 内的审阅覆盖。

重应用前先调用旧状态的 `cleanup()`，避免：

- observer/timer 重复；
- Blob URL 泄漏；
- `attachShadow` 多层包装；
- style 和 layer 重复；
- 旧版本 revision 阻止新 CSS 生效。

修订号同时混入：

- 媒体 sha256；
- display 设置；
- 媒体 kind；
- `BACKGROUND_CSS`；
- `REVIEW_SHADOW_CSS`。

## 为什么媒体使用 base64 + Blob URL

Codex 的 `app://` 渲染页会阻止访问 Studio 的回环 HTTP 媒体 URL。即使 bypass CSP，
回环 fetch 仍可能被 scheme/sandbox 策略拦截。

因此 Rust 后端的 `active_payload()`：

1. 读取受管媒体文件；
2. 限制内嵌大小为 64 MB；
3. 生成 data URL；
4. payload 内解码为 Blob；
5. 使用 Blob URL 给 `<img>` 或 `<video>`；
6. cleanup 时 revoke。

媒体加载失败时必须整体 cleanup，不能留下黑色空背景层。

`preview.rs` 主要服务 Studio 预览和视频 Range，不应重新拿来作为 Codex 背景源。

## 媒体库与动态随机 API

数据目录：

```text
%LOCALAPPDATA%/CodexBackgroundStudio
```

主要文件：

- `settings.json`
- `library.json`
- `runtime.json`
- `media/`
- `temporary/`

动态 API 条目：

- `origin` 为 `api`；
- `sourceUrl` 保存用户输入的随机 API 地址，不保存最终重定向地址；
- 每次轮播或手动刷新重新请求；
- 即使播放列表只有一个动态条目也允许轮播；
- 下载失败保留当前缓存，不中断轮播；
- 预览 URL附加 sha256 查询参数避免缓存旧图。

Windows 文件锁注意：

- 不覆盖当前媒体文件；
- 新内容使用 `<id>-<hash-prefix>.<ext>`；
- catalog 更新后再最佳努力删除旧文件；
- 这是为了避免预览服务正在读取旧文件时产生 `EBUSY`。

## 网络安全

远程媒体只允许无账号信息的 HTTP/HTTPS。

每次请求和重定向都要：

1. 验证 URL；
2. DNS 解析全部地址；
3. 拒绝 loopback、私网、链路本地、保留、文档和组播地址；
4. 把已校验结果固定传给请求 lookup，防止 DNS 重绑定。

Node 20+ 可能以 `options.all = true` 调用 lookup。必须返回地址数组；只返回单地址会导致：

```text
Invalid IP address: undefined
```

限制：

- 图片 50 MB；
- 视频 1 GB；
- 图片边长 16384；
- 总像素 5000 万；
- 最多 5 次重定向；
- 校验 Content-Type、扩展名和文件头。

## 设置扩展流程

新增显示设置时同时修改：

1. `src/shared/contracts.ts`
   - `DisplaySettings`
   - `DEFAULT_SETTINGS`
2. `src-tauri/src/models.rs`
   - Rust 数据结构、默认值、patch 和 normalize
3. `src/renderer/App.tsx`
   - 控件、标签和预览变量
4. `src/main/payload.ts`
   - `ROOT_PROPERTIES`
   - `setProp()`
   - CSS 消费变量
5. 测试

透明度设置统一 clamp 到 0..1。不要让 UI 最小值和 normalize 最小值不一致。

## 已解决的典型故障

### Codex 显示未连接

原因曾是 `ApplicationActivationManager` CLSID 最后一位错误。

正确值：

```text
45ba127d-10a8-46ea-8ab7-56ea9078943c
```

关闭 Codex 时使用经过路径校验的进程，并允许进程已提前退出，避免 Stop-Process 竞态。

Tauri 迁移时还遇到过 Windows 上 `std::net::TcpStream::connect_timeout` 访问已监听的
Codex 回环 CDP 却返回 10060；同一端点通过 reqwest 可以立即连接。CDP 的
`/json/version` 和 `/json/list` 必须使用带总超时并禁用代理的 reqwest blocking
客户端，WebSocket 再交给 tungstenite。应用失败时也必须广播 controller 的错误状态，
否则界面会一直显示失败前的“尚未连接”。

启动 Studio 时要用 `runtime.json` 自动校验并恢复现有 browser ID；若状态失效则删除
旧状态，不能要求用户每次重开管理器都再次应用。状态文件丢失但 Codex 仍带调试参数时，
只允许从“可执行文件完整路径已匹配官方 Store 包”的进程命令行恢复调试端口。

Tauri 单实例插件必须是 Builder 注册的第一个插件。第二次启动只唤醒现有主窗口，
不能再创建第二套托盘、预览端口和 controller 状态。

### 页面冻结

原因：`install()` 每次无条件改写 style `textContent`，触发 MutationObserver，
形成无限微任务循环。

防护：

- style 保存 `data-cbg-revision`；
- revision 不同才写 textContent；
- CSS 变量值不同才 setProperty；
- DOM 更新按 requestAnimationFrame 合并。

### 页面黑屏或背景不显示

原因：Codex `app://` 页面不能稳定读取 Studio 回环媒体服务。

处理：base64 内嵌后转 Blob URL；设置 64 MB 上限；媒体 error 时 cleanup。

### 深色主题出现浅色界面

原因：依赖不存在的原生 CSS 变量，fallback 到浅灰。

处理：从根 class、data theme、computed color-scheme 和系统偏好检测主题，
设置 `--cbg-surface-color`。

### 左右区域同数值不同深浅

原因：`main.main-surface`、content viewport、right aside 多层半透明叠加，
加上 backdrop-filter。

处理：

- main 透明；
- content viewport 单层打底；
- right aside 单层打底；
- 内部壳透明；
- 关闭 backdrop-filter。

### 大文件夹导入卡在“正在处理”

原因：旧实现会递归扫描文件夹后，把每一张图/视频都复制进受管
`media/` 目录并计算 sha256。几千张时会复制数 GB 并长时间无响应。

上游 `vscode-background-cover` 的做法是只保存 `randomImageFolder` 路径，
轮播时再 `readdir` 挑选一张应用，不入库复制。

处理：

- 「添加文件夹」只写入一条 `origin: "folder"` 目录引用；
- 应用 / 轮播 / 刷新时再按需从目录挑选文件；
- 删除文件夹源只移除引用，不触碰用户原目录。

### Codex 更新后主内容整页实底、侧栏仍透出背景

原因：26.727+ 把 `main.main-surface`、`.app-shell-main-content-viewport`、
`.app-shell-main-content-frame`、`.app-shell-main-content-top-fade`、
`app-header-tint` / `application-menu-top-bar` 收成 CSS Modules
（例如 `_MainContentSurface_*`、`_MainContentViewport_*`、
`_ApplicationMenuTopBar_*`）。旧全局类选择器全部失效后，原生
`rgb(24,24,24)` 实底重新盖住背景层；侧栏仍保留 `aside.app-shell-left-panel`，
所以会出现“只有侧栏透出背景”的假象。

处理：

- 主壳、viewport、frame、top-fade、顶栏全部用旧类 + `[class*="MainContent…"]` /
  `[class*="ApplicationMenuTopBar"]` 的 `:is(...)` 兼容写法；
- list/detail 页额外清
  `main … :is(div, section, aside)[class~="bg-token-main-surface-primary"]`；
- Tailwind v4 的 `bg-linear-to-t` 与旧 `bg-gradient-to-t` 一并清掉。

### 临时聊天输入框无光标、无法输入

原因有两层：

- Codex 给 `body` 设 `pointer-events: none`，主区需显式 `pointer-events: auto`；
- 背景层叠规则若写成 `body > :not(#codex-background-layer)`，会把 portal 出来的
  `position: fixed` 临时聊天确认框改成 `relative`，弹窗移出视口后仍作为 dialog
  焦点陷阱，把输入框焦点立刻抢到「继续」按钮上。

处理：叠层只抬 `body > #root`；给 `main.main-surface` 与 `body > [role="dialog"]`
  / `.codex-dialog` 补 `pointer-events: auto`。不要给所有 body 子节点写 `position: relative`。

### 底部英文无障碍文案 / 已编辑图标小黑底 / 发送钮图标消失

原因分三条：

- dnd-kit 的 `#DndDescribedBy-*` / `#DndLiveRegion-*` 本应 `display:none`，透明化后
  若内联隐藏失效，会在侧栏底和主区底露出 “To pick up a draggable item…”；
- 「已编辑 N 个文件」左侧图标壳用 `bg-token-bg-secondary`（约 92% 黑），表面透明后
  变成小黑块；
- 发送/停止白底圆钮上的图标是 `text-token-dropdown-background`；菜单透明度为 0 时
  该 token 色失效变成白色，白图标贴白钮等于图标消失。

处理：强制隐藏 DndDescribedBy/LiveRegion；清 turn-diff-header / activity-header 内
  `bg-token-bg-secondary`；给 `size-token-button-composer` 上的 dropdown-background
  图标固定深色前景（如 `#16181c`）。

### 任务页 / 拉取请求 / 站点 / 已安排 / 插件整页变黑

先别当成 MainContentSurface 选择器又失效。CDP 核对：

1. `main` 壳是否已透明（选择器正常时 `background` 为 transparent）；
2. `#codex-background-layer` 的 `opacity` 与 `--cbg-task-intensity` / `--cbg-route-intensity`。

非首页统一走 `codex-background-task`，层透明度是
`opacity * taskIntensity`。若 `taskIntensity` 为 `0`（或 `enabledOnTasks` 关），
墙纸层完全看不见；再叠加 `html.electron-opaque` 的
`--color-background-surface-under`（约 `#141414`），看起来就像整页实心黑。

处理：在 Studio「页面」里把任务页显示强度调回非 0（默认 `0.32`）并点「应用」。
只改 `settings.json` 不够——Studio 进程内存里的旧值会写回磁盘，且已注入的
payload 闭包仍带着旧 `taskIntensity`，MutationObserver 会持续把 CSS 变量打回 `0`。

### 登录页（登录 ChatGPT）仍是实心深色

原因：背景层与样式已注入，但登录页没有 `main.main-surface`。全屏居中壳
`#root div.fixed.inset-0 > div…bg-token-main-surface-primary` 直接铺约 `#181818`，
把墙纸盖住。同时该页会被误判为任务页。

处理：清掉该壳实底；登录页按首页强度（`codex-background-home`）控制。

### 应用后卡在「正在处理」、退出也无响应

常见组合：

1. 激活了超大文件夹源（例如几千张图），每次应用都会扫描并挑一张图；
2. 挑中的单张图有数 MB，base64 后经 CDP `addScriptToEvaluateOnNewDocument` **再**
   `Runtime.evaluate` 各发一遍，WebSocket 写入长时间占住注入锁；
3. 退出要先 `restore`，而 restore 等同一把 controller 锁 → 看起来退出失灵；
4. 旧版批量拷贝残留的数千失效 `playlistIds` 让每次存盘/轮播更慢。

处理要点：大媒体只 evaluate 一次（early 仅透明化）；CDP 超时按 payload 体积放大；
文件夹列表缓存；只向主 `app://-/index.html` 注入，排除 `avatar-overlay` 等辅助页；
实时换图和设置更新进入单工的「最新状态优先」后台队列，UI 不等待媒体注入；启动时清理
失效 playlist；退出恢复加超时强制退出。

### 任务页对话滚不动

原因：透明化后 `#root` 下主壳 `div.relative.flex.flex-col` 会被长对话撑到内容高度
（可达数千 px）。子级 `thread-scroll-container`（`h-full` + `column-reverse`）跟着
变成内容高，`scrollHeight === clientHeight`，内部失去滚动；外层 `body` 又是
`overflow: hidden`，整页也滚不了。

处理：背景开启时把 `body > #root` 与其下主壳锁在 `height/max-height: 100%`，主壳
加 `min-height: 0` 与 `overflow: hidden`，让滚动回到 `thread-scroll-container`。

### Composer 黑色渐变 / 「第 N 步」胶囊小黑底

原因：输入框上方浮层背后有独立 `bg-gradient-to-t from-token-main-surface-primary
to-transparent`（`h-7`，不一定带 `via-`）。表面透明度为 0 时，这条 `#181818`
渐变会衬在「第 N 步 / 文件已更改」胶囊后面，看起来像小黑底/黑边。

处理：清所有 `bg-gradient-to-t` + `from-token-main-surface-primary` 的
background-image（不要只匹配带 `via-` 的）；同时清胶囊 border/shadow，父层
`overflow: visible`；composer 本身按独立变量打底。

### 站点、已安排、插件搜索黑条

原因：输入框透明，但外部 sticky 和 `::after` 仍有 main surface 与渐变。
26.810+ 把 `bg-token-main-surface-primary` 收成 `bg-surface`，
`after:from-token-*` 收成 `after:from-surface`，旧选择器会整条漏掉。

处理：按共享结构同时匹配旧 token 和新 `bg-surface` / `from-surface`，
清 sticky 与伪元素，不写三个页面专用补丁。

### 26.810+ 首页四张黑卡片 / 设置页实底 / 拉取请求大黑块 / 对话底栏阴影

原因：同一轮 token 更名。首页推荐从 `.home-banners` 变成
`button.rounded-2xl.bg-surface`；设置页整页壳改成 `electron:bg-surface`，
分组卡片描边改成 `border-default`；拉取请求左右栏用 `bg-surface` 铺满；
对话底栏改成 `bg-gradient-to-t from-surface via-surface`。

处理：旧 token 选择器保留，并补 `bg-surface`、`electron:bg-surface`、
`from-surface`、`border-default`。首页四张卡只打在带 `home-icon` 的主区。
文件卡片和「产出/来源」用精确 `class~=` token，不要写成
`[class*="bg-surface-elevated-secondary"]`，否则搜索框
`electron:dark:bg-surface-elevated-secondary` 会被菜单透明度误伤。

### 终端字符白底、选区黑底

原因：xterm ANSI `xterm-bg-*` 类和 selection layer 单独绘制背景。

处理：清字符背景和 selection div；恢复反相字符前景色。

### 审阅滚动时先黑后透明

原因有两层：

- diff 使用 Shadow DOM；普通 CSS 无法穿透，旧实现等 MutationObserver 约 200ms 后注入；
- `diffs-container` 在真实 `[data-diff]` 出现前，会先在 Shadow `:host` 写入
  `background-color: #111111` 和 `--diffs-bg: #111111`。只覆盖 `[data-diff]`
  会让虚拟滚动的新代码块先按这个宿主默认值绘制，再在内容节点挂载后变透明。

处理：

- 早期 payload 在 documentElement 阶段运行；
- 在普通文档 CSS 的 `diffs-container` 宿主上预先固定透明 surface、context、
  separator、hover 和 `--diffs-bg` 变量，让占位阶段直接继承透明值；
- 根级包装 `Element.prototype.attachShadow`；
- Shadow Root 创建同一帧注入，并在 Shadow CSS 的 `:host` 再固定同一组变量；
- 微任务和 requestAnimationFrame 各确保一次样式位于末尾；
- cleanup 恢复原方法；
- timer 仅兜底。

不要回退到“出现后延迟覆盖”的实现。

## 调试策略

### 推荐

- 用 `cargo test --manifest-path src-tauri/Cargo.toml` 跑 worker 单测。
- 用忽略的 Rust live test 连接 Codex CDP。
- 把 DOM、计算样式和截图作为证据。
- 对动态页面测试导航后、滚动后、重载后状态。

### 避免

- 不用类名关键词大范围 `background: transparent`。
- 不根据截图颜色猜元素。
- 不依赖随机生成的 CSS module 类。
- 不用 `setInterval` 作为正常首帧方案。
- 不用 opacity 作用于整个代码组件；这会连文字一起淡化。
- 不隐藏用户仍需交互的原生提示，仅调整其表面透明度。

## 测试和发布

`cargo test --manifest-path src-tauri/Cargo.toml` 覆盖协议、媒体校验、payload 和托管状态机。

payload 测试至少应断言：

- 关键稳定选择器仍存在；
- Shadow style id 和 diff 变量存在；
- `attachShadow`、`requestAnimationFrame` 首帧方案存在；
- 不再出现 200ms 固定延迟；
- 不引入 backdrop blur；
- 不残留参考项目私有标记。

发布前：

1. 删除 `poc/` 一次性文件。
2. 跑 `cargo fmt` 和 `cargo test --manifest-path src-tauri/Cargo.toml`。
3. 在真实 Codex 完成页面矩阵验证。
4. 更新 `src-tauri/Cargo.toml` 版本。
5. 确认 `plugin.json` 的 exeName/pipeName 仍与壳 catalog 兼容。
6. 查看 Git 完整 diff。
7. 用户明确同意后提交、推送。
