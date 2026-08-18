# Codex 页面入口与选择器

Codex class 名可能随版本变化。这里记录的是稳定入口和定位方法，不是允许盲目复制的永久 API。
每次 Codex 更新后都要用 CDP 复核。

## 全局窗口骨架

### Windows 顶部应用菜单栏

- 用户入口：窗口最上方“文件、编辑、视图、帮助”。
- 特征（旧）：`[class~="app-header-tint"][class*="application-menu-top-bar"]`
- 特征（新）：`[class*="ApplicationMenuTopBar"]`
- 兼容写法：`:is([class~="app-header-tint"][class*="application-menu-top-bar"], [class*="ApplicationMenuTopBar"])`
- 透明度：内容区 `--cbg-surface-opacity`。
- 注意：它位于主内容 `main` 外，必须单独打底。

### 左侧导航栏

- 用户入口：Codex 标题、新建任务、拉取请求、站点、已安排、插件、项目和任务列表。
- 稳定入口：`aside.app-shell-left-panel`
- 透明度：`--cbg-sidebar-opacity`。
- 内部 `nav` 应透明，避免与 aside 叠加。
- 隐藏常驻侧栏后，鼠标贴左边缘会创建独立悬浮侧栏。它不带
  `app-shell-left-panel`，实际结构是
  `div.fixed.left-0.z-[42].top-(--height-toolbar-sm) > aside.bg-token-main-surface-primary`。
- 悬浮 aside 同样跟随 `--cbg-sidebar-opacity`，内部 `nav` 透明，并清除原生
  elevation、`backdrop-filter` 和 `box-shadow`。

### 主内容根

- 稳定入口（旧）：`main.main-surface`
- 稳定入口（26.727+ CSS Modules）：`main[class*="MainContentSurface"]`
- 兼容写法：`main:is(.main-surface, [class*="MainContentSurface"])`
- 规则：自身透明，不在这里打内容区底色。
- 必须保留：`pointer-events: auto`。Codex 会给 `body` 设 `pointer-events: none`，侧栏有
  `pointer-events-auto`，但临时聊天等主区布局不会补回；缺了输入框会点穿。
- 背景层叠只抬 `body > #root`，不要写成 `body > :not(#codex-background-layer)`，
  否则临时聊天确认框等 portal dialog 会丢掉 `position: fixed` 并抢走输入焦点。
- 内容区唯一打底层（旧）：`.app-shell-main-content-viewport`
- 内容区唯一打底层（新）：`[class*="MainContentViewport"]`
- 兼容写法：`:is(.app-shell-main-content-viewport, [class*="MainContentViewport"])`
- 内容框：`:is(.app-shell-main-content-frame, [class*="MainContentFrame"])`
- 顶部淡出：`:is(.app-shell-main-content-top-fade, [class*="MainContentTopFade"])`
- 顶部栏：`:is(header.app-header-tint, header[class*="Header"])`
- 透明度：`--cbg-surface-opacity`。
- `.app-shell-main-content-frame`、`[role="main"]` 和全高页面壳应透明。
- 拉取请求 / 站点 / 已安排 / 插件等 list-detail 页常常没有 `[role="main"]`，
  而是用 token surface 铺满；这些壳要清透明。
  - 旧：`:is(div, section, aside)[class~="bg-token-main-surface-primary"]`
  - 26.810+：`:is(div, section, aside):is([class~="bg-surface"], [class~="electron:bg-surface"])`
    设置页整页壳还带 `electron:elevation-prominent`，清底时一并去掉 box-shadow。
- 旧版设置页会在 viewport 内再嵌一层 `div.main-surface`；新版不再嵌套，卡片直接挂在 viewport 下。
- 设置分组卡片：`[class~="overflow-hidden"][class~="rounded-2xl"][class~="border"]`
  加上 `:is([class*="border-token-border"], [class~="border-default"])`，
  跟随 `--cbg-menu-opacity`；选择器不要再强制要求嵌套 `div.main-surface`。
- 设置页提示横幅：`aside.rounded-2xl` 且带
  `:is([class*="bg-token-main-surface-primary"], [class~="bg-surface"])`，
  跟随 `--cbg-menu-opacity`；内部警告色叠加层清掉，避免实底回潮。
- 设置页输入：`[class*="bg-token-input-background"]`（例如 `#personal-agents-editor`）
  跟随 `--cbg-composer-opacity`。

### 首页与任务页识别

- 首页检测：`[role="main"]:has([data-testid="home-icon"])`
- 登录页检测：`#root div.fixed.inset-0 > div.flex.h-full.w-full.items-center.justify-center.bg-token-main-surface-primary`
- 根 class：`codex-background-home` 或 `codex-background-task`
- 背景强度：`homeIntensity`、`taskIntensity`
- 登录页按首页强度控制（无 home-icon，但属于落地页）。
- 不要按 URL 猜路由；Codex 内部导航可能不改变可依赖的普通 URL。

### 登录页（登录 ChatGPT）

- 用户入口：未登录时的「登录 ChatGPT / 继续登录 / 使用其他方式登录 / 注册」。
- 无 `main.main-surface` / `MainContentSurface`；背景层虽已注入，仍会被全屏壳挡住。
- 挡住背景的实底层：
  `#root div.fixed.inset-0 > div[class~="flex"][class~="h-full"][class~="w-full"][class~="items-center"][class~="justify-center"][class~="bg-token-main-surface-primary"]`
- 处理：该壳 `background` / `background-color` 透明，并保留 `pointer-events: auto`。
- 不要给登录按钮去底色；白底「继续登录」与描边「使用其他方式登录」保持原生样式。

## 左侧入口页面

### 新建任务 / 首页

- 用户入口：左侧“新建任务”。
- 主内容：普通 `[role="main"]`，显示“我们该构建什么？”和 composer。
- 首页横幅：`.home-banners > aside:is([class*="bg-token-main-surface-primary"], [class~="bg-surface"])`
- 横幅示例：“启用快速模式”。
- 横幅透明度：`--cbg-menu-opacity`；清原生 shadow，不隐藏文字和按钮。
- 26.810+ 首页四个推荐卡片：
  `[role="main"]:has([data-testid="home-icon"]) button[class~="rounded-2xl"][class~="bg-surface"]`
  例如“探索并理解代码”。跟随 `--cbg-menu-opacity`，并清 `shadow-md-strong`。

### 拉取请求

- 用户入口：左侧“拉取请求”。
- 主内容使用内容区透明度。
- 打开详情后可能出现右侧辅助栏；右侧栏必须单独由菜单/面板透明度打底。
- 检查左右区域是否因主内容底色和右栏底色重叠而深浅不一致。

### 站点

- 用户入口：左侧“站点”。
- 可能出现全页壳：
  `[class~="h-full"][class~="min-h-0"][class~="flex-col"][class*="bg-token-main-surface-primary"]`
- 搜索输入 id：`#appgen-site-search`
- 不要写死 id 处理外层；站点、已安排、插件共享相同 sticky 搜索结构。

### 已安排

- 用户入口：左侧“已安排”。
- 搜索输入 id：`#scheduled-page-search`
- 搜索外层是带 surface 实底和 `::after` 渐变的 sticky。
  旧类是 `bg-token-main-surface-primary`，26.810+ 改成 `bg-surface`
  和 `after:from-surface`。
- 清 sticky 实底和 `::after`，输入框自身继续跟随 composer 透明度。

### 插件

- 用户入口：左侧“插件”。
- 搜索输入 id：`#plugins-page-search`
- 与站点、已安排共享 sticky 搜索规则。
- 页面长列表不应给每个列表分组重复打内容区底色。

### 通用页面搜索栏

稳定结构：

```css
:is(.app-shell-main-content-viewport, [class*="MainContentViewport"])
  [class~="sticky"]:is([class*="bg-token-main-surface-primary"], [class~="bg-surface"]):has(input[type="text"])
```

处理内容：

- sticky 的 `background-color` 透明；
- sticky 的 `::after` 背景和渐变关闭（含 Tailwind v4 的 `after:bg-linear-to-b` / `after:from-surface`）；
- 内部 `div.no-drag:has(> input[type="text"])` 跟随 `--cbg-composer-opacity`。

## 输入和弹层

### 任务时间线工具小图标

- 用户入口：任务页中间「已使用 Fast Context / 浏览器 / Zhi…」活动行左侧小图标。
- 稳定入口：`[class*="activity-header"] :is(svg, img)[class*="bg-token-main-surface-primary"]`
- 汇总行多为 `button…activity-header`；单独展开的 MCP 行（如 Zhi）是
  `div.group/activity-header`。不要只写 `button`，否则 MCP 小图标仍留 `#181818` 小黑框。
- 原生为 `#181818` 实底；背景工具开启时必须透明。
- 「已编辑 N 个文件」左侧还有 `bg-token-bg-secondary` 图标壳（约 92% 黑），一并清掉。
- 26.810+ 文件卡片本体是 `[class~="bg-surface-elevated-secondary/50"]`，行是
  `[class~="bg-surface/70"]`。外层跟随 `--cbg-menu-opacity`，行清透明。
- 右侧「产出/来源」是 `rounded-3xl` + 精确 token `[class~="bg-surface-elevated-secondary"]`
  （没有 `/50`），挂在对话区 `z-40` 浮层里，**不在** `aside[class*="z-[41]"]`。
  外壳跟菜单透明度；内部同 token 页签（含 `::before`）清透明，避免叠两层。
- 禁止 `[class*="bg-surface-elevated-secondary"]`。拉取请求 / 已安排 / 插件 /
  设置的搜索框带 `electron:dark:bg-surface-elevated-secondary`，那层必须继续走
  composer 透明度。

### dnd-kit 无障碍节点

- `#DndDescribedBy-*`、`#DndLiveRegion-*` 必须 `display: none`。
- 透明化后若露出英文 “To pick up a draggable item…”，就是这俩节点漏隐。

### 发送 / 停止圆钮图标

- 稳定入口：`button[class*="size-token-button-composer"] svg[class*="text-token-dropdown-background"]`
- 白底钮 + dropdown-background 色图标；菜单透明度为 0 时图标会变成白色而消失，
  需固定深色前景。

### Composer

- 稳定入口（旧）：`.composer-surface-chrome`
- 稳定入口（新根）：`[data-composer-surface-variant]` / `[class*="ComposerLayoutRoot"]`
- 稳定入口（新首页子层）：`[class*="ComposerLayoutBody"]` 或
  `[data-composer-surface-variant] [data-composer-layout]`
  首页把实底/blur 画在 Body 上，只透明 Root 不够。
- 兼容入口：`div.no-drag:has(> textarea)`
- 透明度：`--cbg-composer-opacity`
- 新版常带 `backdrop-filter: blur(...)` 与 elevation 描边，必须一并清除。
- 必须清：
  - `box-shadow`
  - `border-color`
  - `backdrop-filter`
  - `bg-gradient-to-t` + `from-token-main-surface-primary` / `from-surface` 底部渐变
    （含只有 `from`/`to-transparent`、没有 `via` 的那条；输入框上方「第 N 步 / 文件已更改」
    背后的 `h-7` 遮罩，以及 26.810+ 对话页 `from-surface via-surface` 底栏，
    漏清时会变成胶囊小黑底或整条黑阴影）
  - 底部文件变更胶囊本身：`rounded-3xl border-token-border bg-token-input-background`，
    透明度为 0 时还要 `border-width: 0`，父层 `overflow: visible`

### 普通搜索输入

- 兼容入口：`div.no-drag:has(> input[type="text"])`
- 透明度：`--cbg-composer-opacity`
- 不要让外部 sticky 再叠一层相同色。

### 下拉菜单、右键菜单、命令面板、环境信息浮层

- `[role="menu"]`
- `[role="listbox"]`
- `[class*="bg-token-dropdown-background"]:not(.composer-surface-chrome)`
- 26.810+ 个人资料菜单、项目弹出层：`[class~="bg-surface-elevated-secondary/90"]`
- 透明度：`--cbg-menu-opacity`
- 必须清：`box-shadow` / `electron:elevation-prominent`。菜单透明度为 0 时底色全透，
  原生 0.5px elevation 描边会变成「变更」浮层上的小黑边。
- 内部同 token 子层透明，避免菜单内部再叠色。
- 任务页右上「环境信息 / 变更 / 提交或推送」是绝对定位浮层，不是
  `aside.ml-auto.z-[41]`；它用 `rounded-3xl bg-token-dropdown-background`，走菜单透明度。

## 右侧辅助栏

### 稳定外层

```css
main:is(.main-surface, [class*="MainContentSurface"]) aside[class*="z-[41]"]
```

- 旧版右栏常有 `ml-auto`，26.727+ 可能改成 `ltr:ms-auto rtl:me-auto`；
  `z-[41]` 是当前版本仍保留的稳定锚点。
- 透明度：`--cbg-menu-opacity`
- 它承载审阅、文件树、浏览器、终端等不同内容。
- 只给这个稳定 aside 打一层底；内部 `bg-token-main-surface-primary` 默认透明。

### 审阅

- 用户入口：任务页右上方“审阅”。
- 文件卡片：`.codex-review-diff-card`
- diff 自定义元素：`diffs-container`
- 文件标题 sticky：`.codex-review-diff-card > [class~="sticky"][class~="backdrop-blur-sm"]`
- 普通 CSS 只能处理宿主，不能进入 diff 的 Shadow DOM。

Shadow DOM 内部关键结构：

- `[data-diffs-header]`
- `[data-diff]`、`[data-file]`
- `[data-line-type="change-addition"][data-line]`
- `[data-line-type="change-deletion"][data-line]`

实现要求：

- 普通文档 CSS 先在 `diffs-container` 宿主固定 `--diffs-bg`、surface、context、
  separator 和 hover 变量；真实 diff 内容出现前的占位阶段也必须透明；
- 文档早期接管 `Element.prototype.attachShadow`；
- `diffs-container` 创建 shadow 后，在首帧前追加 `REVIEW_SHADOW_CSS`；
- Shadow CSS 必须覆盖 `:host`，不能只覆盖加载完成后才出现的 `[data-diff]`；
- 样式节点必须位于 Shadow DOM 原生样式之后；
- 新增/删除行保留低强度绿/红，不能直接抹掉审阅语义；
- cleanup 恢复原始 `attachShadow` 并移除 Shadow style；
- MutationObserver/4 秒 timer 只能作为兜底，不能制造可见延迟。

### 文件树

- 用户入口：审阅右侧“文件”区域或文件按钮。
- 自定义元素：`file-tree-container`
- 宿主背景透明，并设置：
  `--color-token-main-surface-primary: transparent`
- CSS 自定义属性会继承进 Shadow DOM，因此文件树不需要单独注入 Shadow style。

### 浏览器

- 仍使用同一个右侧 aside。
- 先检查内部是否新增不透明 `bg-token-main-surface-primary` 壳。
- 不要按内部按钮文本识别整个右栏；按钮和内容会动态切换。

### 集成终端

- 面板 id：`[id^="terminal-panel-"]`
- 外部壳：`div[class*="bg-token-main-surface-primary"]:has([id^="terminal-panel-"])`
- 工具栏：直接子级 `[class~="h-toolbar-pane"]`
- 透明度：`--cbg-terminal-opacity`

处理顺序：

1. 外部多层壳透明。
2. 仅终端面板和必要工具栏打一层 terminal 底色。
3. 工具栏内部子层透明。
4. 清 `.xterm-rows span[class*="xterm-bg-"]` 的 ANSI 字符背景。
5. 清 `.xterm-selection` 和 `.xterm-selection-layer` 的实色选区。
6. 对 `.xterm-bg-257.xterm-fg-257` 恢复可读前景色。

## 透明度归属

- 背景媒体：`--cbg-opacity`
- 首页强度：`--cbg-home-intensity`
- 任务页强度：`--cbg-task-intensity`
- 左侧栏：`--cbg-sidebar-opacity`
- 主内容区、顶部菜单栏：`--cbg-surface-opacity`
- composer、搜索输入：`--cbg-composer-opacity`
- 菜单、右侧辅助栏、首页横幅：`--cbg-menu-opacity`
- 集成终端：`--cbg-terminal-opacity`

所有值范围均为 0 到 1。

## 新页面定位步骤

1. 进入目标页面并截图。
2. 从异常区域中心用 `document.elementsFromPoint()` 获取元素栈。
3. 找到第一个非透明背景、渐变、shadow 或 backdrop-filter。
4. 沿祖先链确认它是页面壳、卡片还是稳定面板。
5. 检查 `::before`、`::after`。
6. 如果是自定义元素，检查 `shadowRoot` 和内部 CSS 变量。
7. 先在 CDP 临时设置样式并截图对比。
8. 再把精确规则写入 payload，补测试并删除探查文件。

## 视觉回归重点

- 相同不透明度的左右区域看起来应同深度。
- 页面导航后不能恢复原生实底。
- 滚动到新 diff 卡片时不能先黑后透明。
- 透明度为 0 时仍保留文字、按钮、边框和 diff 语义。
- 浅色主题不能继续使用深色 surface，深色主题不能回落到浅灰色。
