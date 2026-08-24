import { createHash } from "node:crypto";
import { DisplaySettings, MediaKind } from "../shared/contracts.js";

const BACKGROUND_CSS = String.raw`
html.codex-background-active,
html.codex-background-active body {
  background: transparent !important;
}

/* 登录页（登录 ChatGPT）没有 main.main-surface：全屏居中壳直接铺
   bg-token-main-surface-primary（约 #181818），会整页盖住已注入的背景层。
   清掉这层实底即可透出墙纸；按钮本身仍保留原生底色。 */
html.codex-background-active #root div.fixed.inset-0 > div[class~="flex"][class~="h-full"][class~="w-full"][class~="items-center"][class~="justify-center"][class~="bg-token-main-surface-primary"] {
  background: transparent !important;
  background-color: transparent !important;
  pointer-events: auto !important;
}

/* 只抬主应用根节点，不要用 body > :not(layer) 扫到 portal。
   否则临时聊天确认框等 fixed dialog 会被改成 relative，移出视口后仍拦截焦点。
   同时把主壳锁在视口高度：透明化后 #root 下 flex 壳会被长对话撑高，
   thread-scroll-container 跟着变成内容高，内部失去滚动（body 又是 overflow:hidden）。 */
html.codex-background-active body > #root {
  position: relative;
  z-index: 1;
  height: 100%;
  max-height: 100%;
}
html.codex-background-active body > #root > div.relative.flex.flex-col {
  height: 100% !important;
  max-height: 100% !important;
  min-height: 0 !important;
  overflow: hidden !important;
}

#codex-background-layer {
  position: fixed;
  inset: 0;
  z-index: 0;
  overflow: hidden;
  pointer-events: none;
  opacity: calc(var(--cbg-opacity) * var(--cbg-route-intensity));
  background-color: #101416;
  transition: opacity 220ms ease;
  contain: strict;
}

#codex-background-media,
#codex-background-tile {
  width: 100%;
  height: 100%;
  transform: scale(var(--cbg-scale));
  filter: blur(var(--cbg-blur));
  transform-origin: center;
}

#codex-background-media {
  display: block;
  object-fit: var(--cbg-fit);
  object-position: var(--cbg-position-x) var(--cbg-position-y);
}

#codex-background-tile {
  display: none;
  background-image: var(--cbg-media-url);
  background-repeat: repeat;
  background-position: var(--cbg-position-x) var(--cbg-position-y);
}

html.codex-background-fit-tile #codex-background-media { display: none; }
html.codex-background-fit-tile #codex-background-tile { display: block; }

#codex-background-overlay {
  position: absolute;
  inset: 0;
  background: var(--cbg-overlay-color);
  opacity: var(--cbg-overlay-opacity);
}

html.codex-background-home { --cbg-route-intensity: var(--cbg-home-intensity); }
html.codex-background-task { --cbg-route-intensity: var(--cbg-task-intensity); }
html.codex-background-home.codex-background-home-disabled,
html.codex-background-task.codex-background-task-disabled { --cbg-route-intensity: 0; }

html.codex-background-active aside.app-shell-left-panel,
html.codex-background-active div[class~="fixed"][class~="left-0"][class~="z-[42]"][class*="top-(--height-toolbar-sm)"] > aside[class*="bg-token-main-surface-primary"] {
  background: color-mix(in srgb, var(--cbg-surface-color, #f6f7f7) calc(var(--cbg-sidebar-opacity) * 100%), transparent) !important;
  backdrop-filter: none !important;
  box-shadow: none !important;
}

/* 26.727+ 把旧全局类 main-surface / app-shell-main-content-* / app-header-tint
   收成 CSS Modules（_MainContentSurface_*、_MainContentViewport_* 等）。
   26.810+ 再把 bg-token-main-surface-primary / from-token-main-surface-primary
   / border-token-border 收成 bg-surface、from-surface、border-default。
   旧全局类、模块局部类与新旧 token 并存，避免再因 Codex 更新整页变实底。 */
html.codex-background-active aside.app-shell-left-panel nav,
html.codex-background-active div[class~="fixed"][class~="left-0"][class~="z-[42]"][class*="top-(--height-toolbar-sm)"] > aside[class*="bg-token-main-surface-primary"] nav,
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) > :is(header.app-header-tint, header[class*="Header"]),
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) :is(.app-shell-main-content-top-fade, [class*="MainContentTopFade"]) {
  background: transparent !important;
}

/* Codex 桌面端 body 默认 pointer-events:none，侧栏会自己开回 auto。
   主区与 portal 弹层需要显式接回点击；只恢复交互，不改透明底。 */
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]),
html.codex-background-active body > [role="dialog"],
html.codex-background-active body > .codex-dialog {
  pointer-events: auto !important;
}

html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) {
  background: transparent !important;
  backdrop-filter: none !important;
  /* Codex 26.810+ applies electron:elevation-prominent to the main shell
     itself.  Once its surface is transparent, the elevation shadow becomes
     an isolated dark frame around every route (settings, chat, lists). */
  box-shadow: none !important;
}

html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) :is(.app-shell-main-content-viewport, [class*="MainContentViewport"]) {
  background: color-mix(in srgb, var(--cbg-surface-color, #f6f7f7) calc(var(--cbg-surface-opacity) * 100%), transparent) !important;
  backdrop-filter: none !important;
}

html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) [role="main"],
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) :is(.app-shell-main-content-frame, [class*="MainContentFrame"]),
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) [class~="bg-token-main-surface-primary"][class~="h-full"][class~="w-full"] {
  background: transparent !important;
}
/* 新版 list/detail（拉取请求、站点、已安排、插件）不再用 [role=main]，
   而用 token surface 铺满。26.810+ 改成 bg-surface / electron:bg-surface。
   整页壳统一清掉，卡片/横幅再按菜单透明度单独打底。 */
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) :is(div, section, aside):is([class~="bg-token-main-surface-primary"], [class~="bg-surface"], [class~="electron:bg-surface"]) {
  background: transparent !important;
  background-color: transparent !important;
  box-shadow: none !important;
}
/* 设置页等内容会在 viewport 内再嵌一层 div.main-surface（原生实底 #181818），
   外层 main 透明后仍会被这层挡住全局背景。 */
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) :is(.app-shell-main-content-viewport, [class*="MainContentViewport"]) div:is(.main-surface, [class*="MainContentSurface"]) {
  background: transparent !important;
  backdrop-filter: none !important;
}
/* 设置页分组卡片（rounded-2xl + border）原生约 #232323 实底。
   新版设置页不再嵌套 div.main-surface，卡片直接挂在 viewport 下。
   26.810+ 描边从 border-token-border 换成 border-default。 */
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) :is(.app-shell-main-content-viewport, [class*="MainContentViewport"]) [class~="overflow-hidden"][class~="rounded-2xl"][class~="border"]:is([class*="border-token-border"], [class~="border-default"]),
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) :is(.app-shell-main-content-viewport, [class*="MainContentViewport"]) div:is(.main-surface, [class*="MainContentSurface"]) [class~="overflow-hidden"][class~="rounded-2xl"][class~="border"]:is([class*="border-token-border"], [class~="border-default"]) {
  background-color: color-mix(in srgb, var(--cbg-surface-color, #f6f7f7) calc(var(--cbg-menu-opacity) * 100%), transparent) !important;
  backdrop-filter: none !important;
}
/* 设置页提示横幅（例如个性化页“并非所有模型都支持…”），含警告色叠加层。 */
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) :is(.app-shell-main-content-viewport, [class*="MainContentViewport"]) aside[class~="rounded-2xl"]:is([class*="bg-token-main-surface-primary"], [class~="bg-surface"]),
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) :is(.app-shell-main-content-viewport, [class*="MainContentViewport"]) div:is(.main-surface, [class*="MainContentSurface"]) aside[class~="rounded-2xl"]:is([class*="bg-token-main-surface-primary"], [class~="bg-surface"]) {
  background-color: color-mix(in srgb, var(--cbg-surface-color, #f6f7f7) calc(var(--cbg-menu-opacity) * 100%), transparent) !important;
  backdrop-filter: none !important;
  box-shadow: none !important;
}
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) :is(.app-shell-main-content-viewport, [class*="MainContentViewport"]) aside[class~="rounded-2xl"]:is([class*="bg-token-main-surface-primary"], [class~="bg-surface"]) [class*="bg-token-input-validation-warning-background"],
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) :is(.app-shell-main-content-viewport, [class*="MainContentViewport"]) div:is(.main-surface, [class*="MainContentSurface"]) aside[class~="rounded-2xl"]:is([class*="bg-token-main-surface-primary"], [class~="bg-surface"]) [class*="bg-token-input-validation-warning-background"] {
  background-color: transparent !important;
}
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) :is(.app-shell-main-content-viewport, [class*="MainContentViewport"]) [class~="h-full"][class~="min-h-0"][class~="flex-col"]:is([class*="bg-token-main-surface-primary"], [class~="bg-surface"], [class~="electron:bg-surface"]) {
  background-color: transparent !important;
  box-shadow: none !important;
}

/* 新版 Composer：
   - 任务页实底常在 [data-composer-surface-variant] / ComposerLayoutRoot
   - 首页实底挪到了子层 ComposerLayoutBody（仍带 data-composer-layout）
   旧版仍是 .composer-surface-chrome。三者都跟随输入框不透明度。 */
html.codex-background-active .composer-surface-chrome,
html.codex-background-active [data-composer-surface-variant],
html.codex-background-active [data-composer-surface-variant] [data-composer-layout],
html.codex-background-active [class*="ComposerLayoutBody"],
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) div.no-drag:has(> input[type="text"]),
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) div.no-drag:has(> textarea),
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) [class*="bg-token-input-background"] {
  background: color-mix(in srgb, var(--cbg-surface-color, #f6f7f7) calc(var(--cbg-composer-opacity) * 100%), transparent) !important;
  backdrop-filter: none !important;
  box-shadow: none !important;
  border-color: transparent !important;
}
/* 底部「N 个文件已更改」胶囊：透明度为 0 时 1px border + overflow 裁剪会留下黑边 */
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) div.rounded-3xl:has(> [class*="bg-token-input-background"][class*="rounded-3xl"]) {
  overflow: visible !important;
}
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) div[class*="rounded-3xl"][class*="border-token-border"][class*="bg-token-input-background"] {
  border-width: 0 !important;
  border-color: transparent !important;
  box-shadow: none !important;
}
/* 任务时间线工具小图标：原生 svg/img 带 main-surface 实底 #181818。
   「已使用 xxx」汇总行是 button.activity-header；单独展开的 MCP 行（如 Zhi）是
   div.group/activity-header，必须一起清，否则只修了汇总行、MCP 仍留小黑框。 */
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) [class*="activity-header"] :is(svg, img)[class*="bg-token-main-surface-primary"] {
  background: transparent !important;
  background-color: transparent !important;
}
/* 「已编辑 N 个文件」左侧圆角图标底：bg-token-bg-secondary 约 92% 黑，
   表面透明后会变成明显小黑块。 */
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) [class*="turn-diff-header"] [class*="bg-token-bg-secondary"],
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) [class*="activity-header"] [class*="bg-token-bg-secondary"] {
  background: transparent !important;
  background-color: transparent !important;
}
/* 26.810+ 对话文件卡片是 bg-surface-elevated-secondary/50，
   右侧「产出/来源」是精确 token bg-surface-elevated-secondary（没有 /50）。
   禁止 [class*="bg-surface-elevated-secondary"]：会误伤搜索框
   electron:dark:bg-surface-elevated-secondary（那层跟 composer 透明度）。 */
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) :is([class~="bg-surface-elevated-secondary"], [class~="bg-surface-elevated-secondary/50"]) {
  background-color: color-mix(in srgb, var(--cbg-surface-color, #f6f7f7) calc(var(--cbg-menu-opacity) * 100%), transparent) !important;
  box-shadow: none !important;
}
/* 「产出/来源」外壳打底后，内部同 token 页签不要再叠一层。 */
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) :is([class~="bg-surface-elevated-secondary"], [class~="bg-surface-elevated-secondary/50"]) :is([class~="bg-surface-elevated-secondary"], [class~="bg-surface-elevated-secondary/50"]),
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) :is([class~="bg-surface-elevated-secondary"], [class~="bg-surface-elevated-secondary/50"]) :is([class~="bg-surface-elevated-secondary"], [class~="bg-surface-elevated-secondary/50"])::before {
  background-color: transparent !important;
  background-image: none !important;
  box-shadow: none !important;
}
/* 旧版右侧 aside 已经按菜单透明度打过一层，里面的 elevated 卡片清透明。 */
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) aside[class*="z-[41]"] :is([class~="bg-surface-elevated-secondary"], [class~="bg-surface-elevated-secondary/50"]) {
  background-color: transparent !important;
  background-image: none !important;
  box-shadow: none !important;
}
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) [class*="turn-diff"] [class~="bg-surface/70"] {
  background: transparent !important;
  background-color: transparent !important;
}
/* dnd-kit 拖拽无障碍说明：原生应 display:none。侧栏/主区透明后若内联样式丢失，
   会在窗口底部露出大段英文 “To pick up a draggable item…”。 */
html.codex-background-active [id^="DndDescribedBy-"],
html.codex-background-active [id^="DndLiveRegion-"] {
  display: none !important;
}
/* 发送/停止白底圆钮：图标用 text-token-dropdown-background。菜单透明度为 0 时
   该 token 色失效，图标变成白色，贴在白钮上等于“图标没了”。 */
html.codex-background-active button[class*="size-token-button-composer"] svg[class*="text-token-dropdown-background"] {
  color: #16181c !important;
}
/* 输入框上方「第 N 步 / 文件已更改」浮层背后的遮罩：只有 from + to-transparent，没有 via。
   旧选择器要求 via-token，会漏掉这条 h-7 渐变，透明度为 0 时看起来就像胶囊小黑底。 */
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) [class*="bg-gradient-to-t"]:is([class*="from-token-main-surface-primary"], [class*="from-surface"]),
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) [class*="bg-linear-to-t"]:is([class*="from-token-main-surface-primary"], [class*="from-surface"]) {
  background-color: transparent !important;
  background-image: none !important;
}
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) :is(.app-shell-main-content-viewport, [class*="MainContentViewport"]) [class~="sticky"]:is([class*="bg-token-main-surface-primary"], [class~="bg-surface"]):has(input[type="text"]),
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) :is(.app-shell-main-content-viewport, [class*="MainContentViewport"]) [class~="sticky"]:is([class*="bg-token-main-surface-primary"], [class~="bg-surface"]):has(input[type="text"])::after {
  background-color: transparent !important;
  background-image: none !important;
}

/* 窗口级应用菜单栏：独立于主内容 main，原生依赖下层不透明底色，
   需要单独给表面色打底，否则背景图会从这条全宽栏里直接透出来 */
html.codex-background-active :is([class~="app-header-tint"][class*="application-menu-top-bar"], [class*="ApplicationMenuTopBar"]) {
  background: color-mix(in srgb, var(--cbg-surface-color, #f6f7f7) calc(var(--cbg-surface-opacity) * 100%), transparent) !important;
  backdrop-filter: none !important;
}

/* 弹出层：下拉菜单、右键上下文菜单、命令面板、任务页右上「环境信息/变更」浮层等。
   统一按菜单不透明度打底；清 elevation 描边，避免透明度为 0 时只剩 0.5px 黑边。 */
html.codex-background-active [role="menu"],
html.codex-background-active [role="listbox"],
html.codex-background-active [class*="bg-token-dropdown-background"]:not(.composer-surface-chrome),
html.codex-background-active [class~="bg-surface-elevated-secondary/90"] {
  background-color: color-mix(in srgb, var(--cbg-surface-color, #f6f7f7) calc(var(--cbg-menu-opacity) * 100%), transparent) !important;
  backdrop-filter: none !important;
  box-shadow: none !important;
}
/* 菜单内部的分组标题/子层继承透明，避免叠加出不透明色块 */
html.codex-background-active [class*="bg-token-dropdown-background"]:not(.composer-surface-chrome) [class*="bg-token-dropdown-background"],
html.codex-background-active [class~="bg-surface-elevated-secondary/90"] [class~="bg-surface-elevated-secondary/90"] {
  background-color: transparent !important;
  backdrop-filter: none !important;
}

/* 首页推荐横幅（例如“启用快速模式”）是独立于输入栏的原生卡片。
   让卡片跟随菜单/右侧面板不透明度，避免固定实底悬在透明首页上。 */
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) .home-banners > aside:is([class*="bg-token-main-surface-primary"], [class~="bg-surface"]) {
  background-color: color-mix(in srgb, var(--cbg-surface-color, #f6f7f7) calc(var(--cbg-menu-opacity) * 100%), transparent) !important;
  backdrop-filter: none !important;
  box-shadow: none !important;
}
/* 26.810+ 首页四个推荐卡片：button.rounded-2xl.bg-surface + shadow-md-strong。
   只打在带 home-icon 的主区，避免误伤其它 rounded-2xl 按钮。 */
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) [role="main"]:has([data-testid="home-icon"]) button[class~="rounded-2xl"][class~="bg-surface"] {
  background-color: color-mix(in srgb, var(--cbg-surface-color, #f6f7f7) calc(var(--cbg-menu-opacity) * 100%), transparent) !important;
  backdrop-filter: none !important;
  box-shadow: none !important;
}

/* 任务页右侧辅助栏的内容会在任务/浏览器/终端间切换，不能依赖内部按钮识别。
   新版 Codex 使用 ltr:ms-auto/rtl:me-auto，旧版使用 ml-auto；z-[41] 是两版
   共同的稳定锚点。使用右侧 aside 统一打底，并清掉内容页自带的 main-surface 实底。 */
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) aside[class*="z-[41]"] {
  background-color: color-mix(in srgb, var(--cbg-surface-color, #f6f7f7) calc(var(--cbg-menu-opacity) * 100%), transparent) !important;
  backdrop-filter: none !important;
  box-shadow: none !important;
}
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) aside[class*="z-[41]"] [class*="bg-token-main-surface-primary"] {
  background-color: transparent !important;
  background-image: none !important;
}
/* 用户附件/图像预览的 tabpanel 内容壳使用 bg-token-bg-primary，和右侧
   普通面板的 main-surface token 不同；保留图片本身，只清掉包围画布的实底。 */
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) aside[class*="z-[41]"] [role="tabpanel"] > [class*="bg-token-bg-primary"] {
  background: transparent !important;
  background-color: transparent !important;
  background-image: none !important;
  backdrop-filter: none !important;
  box-shadow: none !important;
}
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) aside[class*="z-[41]"] .codex-review-diff-card,
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) aside[class*="z-[41]"] diffs-container,
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) aside[class*="z-[41]"] file-tree-container {
  background-color: transparent !important;
  background-image: none !important;
  --color-token-main-surface-primary: transparent !important;
}
/* diffs-container 渲染真实 [data-diff] 前，会先在 Shadow :host 写入 #111111。
   把所有底色变量直接固定在宿主上，让占位、虚拟滚动和正式内容从首帧起就继承透明值。 */
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) aside[class*="z-[41]"] diffs-container {
  --codex-diffs-surface: transparent !important;
  --codex-diffs-context-surface: transparent !important;
  --codex-diffs-separator-surface: transparent !important;
  --codex-diffs-hover-surface: transparent !important;
  --codex-diffs-header-surface: transparent !important;
  --diffs-bg: transparent !important;
  --diffs-bg-context-override: transparent !important;
  --diffs-bg-separator-override: transparent !important;
  --diffs-bg-hover-override: transparent !important;
  --diffs-bg-addition: color-mix(in srgb, var(--diffs-addition-base, #40c977) 12%, transparent) !important;
  --diffs-bg-deletion: color-mix(in srgb, var(--diffs-deletion-base, #fa423e) 12%, transparent) !important;
  --codex-diffs-addition-number: color-mix(in srgb, var(--diffs-addition-base, #40c977) 20%, transparent) !important;
  --codex-diffs-deletion-number: color-mix(in srgb, var(--diffs-deletion-base, #fa423e) 20%, transparent) !important;
}
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) aside[class*="z-[41]"] .codex-review-diff-card > [class~="sticky"][class~="backdrop-blur-sm"],
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) aside[class*="z-[41]"] [class~="sticky"][class*="backdrop-blur-sm"] {
  background-color: transparent !important;
  background-image: none !important;
  backdrop-filter: none !important;
  box-shadow: none !important;
}

/* 26.727+ 的 diff 文件壳移到 light DOM，类名是 group/file-diff；其自身仍保留
   #181818，而 diffs-container 的 Shadow CSS 只覆盖内部节点。清掉文件壳和 sticky
   标题，保留新增/删除行在 Shadow DOM 中的低强度语义色。 */
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) aside[class*="z-[41]"] [class*="group/file-diff"] {
  background-color: transparent !important;
  background-image: none !important;
  box-shadow: none !important;
  --codex-diffs-surface-override: transparent !important;
}
html.codex-background-active main:is(.main-surface, [class*="MainContentSurface"]) aside[class*="z-[41]"] [class*="group/diff-header"] {
  background-color: color-mix(in srgb, var(--cbg-surface-color, #f6f7f7) calc(var(--cbg-menu-opacity) * 100%), transparent) !important;
  background-image: none !important;
}

/* 集成终端面板：原生为多层嵌套的不透明实底(bg-token-main-surface-primary)。
   先把所有包裹层设为透明，避免多层半透明叠加，再只给终端面板本体打一层
   按内容区不透明度控制的底色并模糊，让背景可控透出、终端文字仍清晰。 */
html.codex-background-active div[class*="bg-token-main-surface-primary"]:has([id^="terminal-panel-"]) {
  background-color: transparent !important;
}
html.codex-background-active [id^="terminal-panel-"] {
  background-color: color-mix(in srgb, var(--cbg-surface-color, #f6f7f7) calc(var(--cbg-terminal-opacity) * 100%), transparent) !important;
  backdrop-filter: none !important;
}
html.codex-background-active div[class*="bg-token-main-surface-primary"]:has([id^="terminal-panel-"]) > [class~="h-toolbar-pane"] {
  background-color: color-mix(in srgb, var(--cbg-surface-color, #f6f7f7) calc(var(--cbg-terminal-opacity) * 100%), transparent) !important;
}
html.codex-background-active div[class*="bg-token-main-surface-primary"]:has([id^="terminal-panel-"]) > [class~="h-toolbar-pane"] * {
  background-color: transparent !important;
  background-image: none !important;
}
/* xterm 会用 ANSI 背景类绘制白底/黑底字符块。背景工具模式下清掉字符底色，
   对反相文本恢复终端前景色；选区保留轻量半透明提示而不使用黑色实底。 */
html.codex-background-active [id^="terminal-panel-"] .xterm-rows span[class*="xterm-bg-"] {
  background-color: transparent !important;
}
html.codex-background-active [id^="terminal-panel-"] .xterm-rows .xterm-bg-257.xterm-fg-257 {
  color: var(--vscode-terminal-foreground, var(--color-text-foreground, #fff)) !important;
}
html.codex-background-active [id^="terminal-panel-"] .xterm-selection > div,
html.codex-background-active [id^="terminal-panel-"] .xterm-selection-layer > div {
  background-color: transparent !important;
}

html.codex-background-dark #codex-background-layer {
  background-color: #0d0f12;
}

@media (prefers-reduced-motion: reduce) {
  #codex-background-layer { transition: none; }
}
`;

const REVIEW_SHADOW_STYLE_ID = "codex-background-review-shadow-style";
const REVIEW_SHADOW_CSS = String.raw`
:host,
[data-diffs-header],
:is([data-diff], [data-file]) {
  --color-token-main-surface-primary: transparent !important;
  --codex-diffs-surface: transparent !important;
  --codex-diffs-context-surface: transparent !important;
  --codex-diffs-separator-surface: transparent !important;
  --codex-diffs-hover-surface: transparent !important;
  --codex-diffs-header-surface: transparent !important;
  --diffs-bg: transparent !important;
  --diffs-bg-context-override: transparent !important;
  --diffs-bg-separator-override: transparent !important;
  --diffs-bg-hover-override: transparent !important;
  --diffs-bg-addition: color-mix(in srgb, var(--diffs-addition-base, #40c977) 12%, transparent) !important;
  --diffs-bg-deletion: color-mix(in srgb, var(--diffs-deletion-base, #fa423e) 12%, transparent) !important;
  --codex-diffs-addition-number: color-mix(in srgb, var(--diffs-addition-base, #40c977) 20%, transparent) !important;
  --codex-diffs-deletion-number: color-mix(in srgb, var(--diffs-deletion-base, #fa423e) 20%, transparent) !important;
  background-color: transparent !important;
}
`;

export interface PayloadInput {
  mediaUrl: string;
  mediaKind: MediaKind;
  display: DisplaySettings;
  revision: string;
}

export function buildRendererPayload(input: PayloadInput) {
  // 修订号混入 CSS 内容哈希：工具升级改了注入样式后，
  // 即使媒体和显示设置没变也要强制重写页面里的 <style>
  const revision = createHash("sha256")
    .update(input.revision)
    .update(BACKGROUND_CSS)
    .update(REVIEW_SHADOW_CSS)
    .digest("hex");
  const serialized = JSON.stringify({ ...input, revision }).replace(/</g, "\\u003c");
  const css = JSON.stringify(BACKGROUND_CSS);
  const reviewShadowCss = JSON.stringify(REVIEW_SHADOW_CSS);
  const reviewShadowStyleId = JSON.stringify(REVIEW_SHADOW_STYLE_ID);
  return String.raw`((config, cssText, reviewShadowCssText, reviewShadowStyleId) => {
    const STATE = "__CODEX_BACKGROUND_STUDIO__";
    const STYLE_ID = "codex-background-style";
    const LAYER_ID = "codex-background-layer";
    const REVIEW_HOST_SELECTOR = "diffs-container";
    const ROOT_CLASSES = [
      "codex-background-active", "codex-background-home", "codex-background-task",
      "codex-background-home-disabled", "codex-background-task-disabled",
      "codex-background-fit-tile", "codex-background-dark"
    ];
    const ROOT_PROPERTIES = [
      "--cbg-opacity", "--cbg-blur", "--cbg-scale", "--cbg-fit",
      "--cbg-position-x", "--cbg-position-y", "--cbg-overlay-color",
      "--cbg-overlay-opacity", "--cbg-home-intensity", "--cbg-task-intensity",
      "--cbg-route-intensity", "--cbg-sidebar-opacity", "--cbg-surface-opacity",
      "--cbg-composer-opacity", "--cbg-menu-opacity", "--cbg-terminal-opacity",
      "--cbg-media-url", "--cbg-surface-color"
    ];

    const RUN_SEQ = "__CODEX_BACKGROUND_RUN_SEQ__";
    const runToken = (window[RUN_SEQ] = (Number(window[RUN_SEQ]) || 0) + 1);
    const superseded = () => window[RUN_SEQ] !== runToken;
    const previous = window[STATE];
    let scheduled = null;
    let shadowPatch = null;
    let observer = null;
    let timer = null;
    let state = null;
    let layer = null;
    let tile = null;
    let overlay = null;
    let activeMedia = null;

    // Codex 渲染页无法访问本机 HTTP 服务，媒体以 base64 内嵌传入，
    // 在页面内转成 Blob URL 使用
    const blobUrl = (() => {
      const comma = config.mediaUrl.indexOf(",");
      if (!config.mediaUrl.startsWith("data:") || comma < 0) return config.mediaUrl;
      const binary = atob(config.mediaUrl.slice(comma + 1));
      const bytes = new Uint8Array(binary.length);
      for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index);
      const mime = /^data:([^;,]+)/.exec(config.mediaUrl)?.[1] || "application/octet-stream";
      return URL.createObjectURL(new Blob([bytes], { type: mime }));
    })();

    const candidate = document.createElement(config.mediaKind === "video" ? "video" : "img");
    candidate.setAttribute("aria-hidden", "true");
    if (config.mediaKind === "video") {
      candidate.autoplay = true;
      candidate.loop = true;
      candidate.muted = Boolean(config.display.videoMuted);
      candidate.defaultMuted = Boolean(config.display.videoMuted);
      candidate.playsInline = true;
      candidate.preload = "auto";
      candidate.playbackRate = Number(config.display.videoPlaybackRate) || 1;
    }

    // 新媒体在脱离文档的节点中完成首帧加载/解码。旧背景在此期间继续显示，
    // 避免先拆旧层、再等待新媒体产生可绘制像素时露出原生底色。
    const mediaReady = new Promise((resolve, reject) => {
      let settled = false;
      let readyTimeout = null;
      const finish = (error) => {
        if (settled) return;
        settled = true;
        if (readyTimeout) clearTimeout(readyTimeout);
        candidate.removeEventListener("load", onReady);
        candidate.removeEventListener("loadeddata", onReady);
        candidate.removeEventListener("error", onError);
        if (error) reject(error);
        else resolve();
      };
      const onReady = () => finish();
      const onError = () => finish(new Error("背景媒体加载失败"));
      candidate.addEventListener("error", onError);
      if (config.mediaKind === "video") {
        candidate.addEventListener("loadeddata", onReady);
      } else {
        candidate.addEventListener("load", onReady);
      }
      readyTimeout = setTimeout(() => {
        const ready = config.mediaKind === "video"
          ? candidate.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA
          : candidate.complete && candidate.naturalWidth > 0;
        if (ready) onReady();
        else onError();
      }, 10000);
      candidate.src = blobUrl;
      if (config.mediaKind === "video") {
        candidate.load();
      } else if (typeof candidate.decode === "function") {
        candidate.decode().then(onReady).catch(() => {
          // 某些 Chromium 版本会在 load 事件前提前拒绝 decode；继续等 load，
          // 只有最终仍没有可用像素时才保留旧背景并报告失败。
          if (candidate.complete && candidate.naturalWidth > 0) onReady();
        });
      }
    });

    const installReviewShadowStyle = (host, shadow = host?.shadowRoot) => {
      if (!shadow) return false;
      let shadowStyle = shadow.getElementById(reviewShadowStyleId);
      if (!shadowStyle) {
        shadowStyle = document.createElement("style");
        shadowStyle.id = reviewShadowStyleId;
      }
      if (shadowStyle.dataset.cbgRevision !== config.revision) {
        shadowStyle.textContent = reviewShadowCssText;
        shadowStyle.dataset.cbgRevision = config.revision;
      }
      // 始终放到 Shadow DOM 样式末尾，确保覆盖组件稍后同步追加的原生样式。
      shadow.appendChild(shadowStyle);
      return true;
    };

    const suspend = () => {
      observer?.disconnect();
      if (timer) clearInterval(timer);
      if (scheduled) cancelAnimationFrame(scheduled);
      if (shadowPatch?.prototype.attachShadow === shadowPatch.wrapped) {
        shadowPatch.prototype.attachShadow = shadowPatch.original;
      }
      observer = null;
      timer = null;
      scheduled = null;
      shadowPatch = null;
    };

    const cleanup = () => {
      suspend();
      // 已被下一轮接管的旧媒体可能稍后才冒出 error 事件；旧 cleanup
      // 只能收自己的运行时，不能把当前背景层和样式一并删掉。
      if (state && window[STATE] !== state) return true;
      document.getElementById(LAYER_ID)?.remove();
      document.getElementById(STYLE_ID)?.remove();
      document.querySelectorAll("diffs-container").forEach((host) => {
        host.shadowRoot?.getElementById(reviewShadowStyleId)?.remove();
      });
      document.documentElement?.classList.remove(...ROOT_CLASSES);
      for (const property of ROOT_PROPERTIES) document.documentElement?.style.removeProperty(property);
      if (state?.blobUrl) URL.revokeObjectURL(state.blobUrl);
      if (window[STATE] === state) delete window[STATE];
      return true;
    };

    // 页面创建 diff Shadow DOM 时同步接管；微任务和下一帧各补一次，
    // 保证组件无论同步还是异步追加原生样式，我们的覆盖都在首帧绘制前位于末尾。
    const patchAttachShadow = () => {
      const prototype = Element.prototype;
      const original = prototype.attachShadow;
      const wrapped = function(init) {
        const shadow = original.call(this, init);
        if (this.localName === REVIEW_HOST_SELECTOR) {
          queueMicrotask(() => installReviewShadowStyle(this, shadow));
          requestAnimationFrame(() => installReviewShadowStyle(this, shadow));
        }
        return shadow;
      };
      prototype.attachShadow = wrapped;
      return { prototype, original, wrapped };
    };
    // 检测 Codex 原生外观：优先读根节点/滚动容器的计算 color-scheme
    //（跟随应用内主题设置），系统偏好只作兜底
    const detectAppearance = () => {
      const root = document.documentElement;
      const classText = ((root?.className || "") + " " + (document.body?.className || ""))
        .toLowerCase()
        .replace(/\bcodex-background-[a-z-]+\b/g, "");
      if (/\b(?:dark|theme-dark|appearance-dark)\b/.test(classText)) return "dark";
      if (/\b(?:light|theme-light|appearance-light)\b/.test(classText)) return "light";
      const dataTheme = (
        root?.getAttribute("data-theme") || root?.getAttribute("data-appearance") ||
        document.body?.getAttribute("data-theme") || ""
      ).toLowerCase();
      if (dataTheme.includes("dark")) return "dark";
      if (dataTheme.includes("light")) return "light";
      try {
        const scheme = getComputedStyle(root).colorScheme || "";
        if (scheme.includes("dark") && !scheme.includes("light")) return "dark";
        if (scheme.includes("light") && !scheme.includes("dark")) return "light";
      } catch {}
      try {
        return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
      } catch {}
      return "light";
    };

    const install = () => {
      const root = document.documentElement;
      if (!root) return false;

      // 值不同才写入，避免 attribute 观察被自己的写入反复触发
      const setClass = (name, on) => {
        if (root.classList.contains(name) !== on) root.classList.toggle(name, on);
      };
      const setProp = (name, value) => {
        if (root.style.getPropertyValue(name) !== value) root.style.setProperty(name, value);
      };

      const dark = detectAppearance() === "dark";
      setClass("codex-background-dark", dark);
      setProp("--cbg-surface-color", dark ? "#16181c" : "#f6f7f7");

      let style = document.getElementById(STYLE_ID);
      if (!style) {
        style = document.createElement("style");
        style.id = STYLE_ID;
        (document.head || root).appendChild(style);
      }
      // 只在修订号变化时写入样式：无条件写 textContent 会触发 childList
      // 变更，被下面的 MutationObserver 观察到后再次进入 install()，
      // 形成微任务死循环，直接卡死渲染进程
      if (style.dataset.cbgRevision !== config.revision) {
        style.textContent = cssText;
        style.dataset.cbgRevision = config.revision;
      }
      // 审阅 diff 使用 Shadow DOM，普通页面 CSS 无法进入其内部。
      // 对每个已挂载的 diff 宿主注入同一份轻量样式；定时 install 会覆盖后续新建的宿主。
      document.querySelectorAll(
        'main:is(.main-surface, [class*="MainContentSurface"]) aside[class*="z-[41]"] diffs-container'
      ).forEach((host) => {
        installReviewShadowStyle(host);
      });

      layer = document.getElementById(LAYER_ID);
      if (!layer && document.body) {
        layer = document.createElement("div");
        layer.id = LAYER_ID;
        tile = document.createElement("div");
        tile.id = "codex-background-tile";
        overlay = document.createElement("div");
        overlay.id = "codex-background-overlay";
        layer.append(activeMedia, tile, overlay);
        document.body.prepend(layer);
      } else if (layer) {
        if (activeMedia.parentNode !== layer) layer.prepend(activeMedia);
        tile = layer.querySelector("#codex-background-tile") || tile;
        if (!tile) {
          tile = document.createElement("div");
          tile.id = "codex-background-tile";
          layer.appendChild(tile);
        }
        overlay = layer.querySelector("#codex-background-overlay") || overlay;
        if (!overlay) {
          overlay = document.createElement("div");
          overlay.id = "codex-background-overlay";
          layer.appendChild(overlay);
        }
      }
      if (state) state.layer = layer;

      setClass("codex-background-active", true);
      setClass("codex-background-fit-tile", config.display.fit === "tile" && config.mediaKind === "image");
      setClass("codex-background-home-disabled", !config.display.enabledOnHome);
      setClass("codex-background-task-disabled", !config.display.enabledOnTasks);
      setProp("--cbg-opacity", String(config.display.opacity));
      setProp("--cbg-blur", config.display.blur + "px");
      setProp("--cbg-scale", String(config.display.scale));
      setProp("--cbg-fit", config.display.fit === "tile" ? "cover" : config.display.fit);
      setProp("--cbg-position-x", config.display.positionX + "%");
      setProp("--cbg-position-y", config.display.positionY + "%");
      setProp("--cbg-overlay-color", config.display.overlayColor);
      setProp("--cbg-overlay-opacity", String(config.display.overlayOpacity));
      setProp("--cbg-home-intensity", String(config.display.homeIntensity));
      setProp("--cbg-task-intensity", String(config.display.taskIntensity));
      setProp("--cbg-sidebar-opacity", String(config.display.sidebarOpacity));
      setProp("--cbg-surface-opacity", String(config.display.surfaceOpacity));
      setProp("--cbg-composer-opacity", String(config.display.composerOpacity));
      setProp("--cbg-menu-opacity", String(config.display.menuOpacity));
      setProp("--cbg-terminal-opacity", String(config.display.terminalOpacity));
      setProp("--cbg-media-url", 'url("' + String(blobUrl).replace(/["\\\n\r]/g, "") + '")');

      // 登录页无 home-icon，但视觉上更接近落地页；按首页强度控制，避免落到任务页强度 0。
      const login = Boolean(document.querySelector(
        '#root div.fixed.inset-0 > div[class~="flex"][class~="h-full"][class~="w-full"][class~="items-center"][class~="justify-center"][class~="bg-token-main-surface-primary"]'
      ));
      const home = login || Boolean(document.querySelector('[role="main"]:has([data-testid="home-icon"])'));
      setClass("codex-background-home", home);
      setClass("codex-background-task", !home);
      return true;
    };

    const scheduleInstall = () => {
      if (scheduled) return;
      scheduled = requestAnimationFrame(() => { scheduled = null; install(); });
    };
    return mediaReady.then(() => {
      if (superseded()) {
        if (String(blobUrl).startsWith("blob:")) URL.revokeObjectURL(blobUrl);
        return { installed: false, superseded: true, revision: config.revision };
      }
      // 只有新媒体已经可绘制后才停止旧运行时。旧版状态没有 suspend，
      // 此处才调用完整 cleanup；删除与重建发生在同一任务内，不会产生可见空帧。
      if (previous?.suspend) {
        previous.suspend();
      } else if (previous?.cleanup) {
        previous.cleanup();
      } else {
        previous?.observer?.disconnect();
        if (previous?.timer) clearInterval(previous.timer);
      }

      layer = document.getElementById(LAYER_ID);
      const previousMedia = layer?.querySelector("#codex-background-media");
      tile = layer?.querySelector("#codex-background-tile") || null;
      overlay = layer?.querySelector("#codex-background-overlay") || null;
      activeMedia = candidate;
      activeMedia.id = "codex-background-media";
      if (previousMedia) previousMedia.replaceWith(activeMedia);

      state = { revision: config.revision, cleanup, suspend, observer: null, timer: null, layer, blobUrl };
      window[STATE] = state;
      shadowPatch = patchAttachShadow();
      install();

      observer = new MutationObserver(scheduleInstall);
      observer.observe(document.documentElement, {
        childList: true,
        subtree: true,
        attributes: true,
        attributeFilter: ["class", "data-theme", "data-appearance"],
      });
      timer = setInterval(install, 4000);
      state.observer = observer;
      state.timer = timer;
      state.layer = layer;
      activeMedia.addEventListener("error", () => cleanup(), { once: true });
      if (config.mediaKind === "video") activeMedia.play().catch(() => undefined);

      if (previous?.suspend && previous.blobUrl && previous.blobUrl !== blobUrl) {
        URL.revokeObjectURL(previous.blobUrl);
      }
      return { installed: true, revision: config.revision, mediaKind: config.mediaKind };
    }).catch((error) => {
      if (blobUrl && blobUrl !== previous?.blobUrl) URL.revokeObjectURL(blobUrl);
      throw error;
    });
  })(${serialized}, ${css}, ${reviewShadowCss}, ${reviewShadowStyleId})`;
}

export const REMOVE_RENDERER_PAYLOAD = String.raw`(() => {
  window.__CODEX_BACKGROUND_RUN_SEQ__ = (Number(window.__CODEX_BACKGROUND_RUN_SEQ__) || 0) + 1;
  const state = window.__CODEX_BACKGROUND_STUDIO__;
  if (state?.cleanup) return state.cleanup();
  document.getElementById("codex-background-layer")?.remove();
  document.getElementById("codex-background-style")?.remove();
  document.documentElement?.classList.remove(
    "codex-background-active", "codex-background-home", "codex-background-task",
    "codex-background-home-disabled", "codex-background-task-disabled", "codex-background-fit-tile"
  );
  delete window.__CODEX_BACKGROUND_STUDIO__;
  return true;
})()`;

export function earlyPayloadFor(payload: string, revision: string) {
  const safeRevision = JSON.stringify(revision);
  return String.raw`(() => {
    const revision = ${safeRevision};
    const run = () => {
      if (!document.documentElement) return false;
      try { ${payload}; return true; } catch { return false; }
    };
    if (!run()) {
      const observer = new MutationObserver(() => {
        if (run()) observer.disconnect();
      });
      observer.observe(document.documentElement || document, { childList: true, subtree: true });
      setTimeout(() => observer.disconnect(), 30000);
    }
    return revision;
  })()`;
}
