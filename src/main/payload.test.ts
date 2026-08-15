import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS } from "../shared/contracts.js";
import { buildRendererPayload, earlyPayloadFor, REMOVE_RENDERER_PAYLOAD } from "./payload.js";

describe("renderer payload", () => {
  it("contains route-aware background controls and an inert decorative layer", () => {
    const payload = buildRendererPayload({
      mediaUrl: "http://127.0.0.1:9444/token/media/id",
      mediaKind: "video",
      display: DEFAULT_SETTINGS.display,
      revision: "revision-1",
    });
    expect(payload).toContain("codex-background-layer");
    expect(payload).toContain("pointer-events: none");
    expect(payload).toContain("body > #root");
    expect(payload).toContain("body > #root > div.relative.flex.flex-col");
    expect(payload).toContain("max-height: 100%");
    expect(payload).not.toContain("body > :not(#codex-background-layer)");
    expect(payload).toContain('main:is(.main-surface, [class*=\\"MainContentSurface\\"])');
    expect(payload).toContain('body > [role=\\"dialog\\"]');
    expect(payload).toContain("pointer-events: auto !important");
    expect(payload).toContain("codex-background-home");
    expect(payload).toContain("codex-background-task");
    expect(payload).toContain(
      '#root div.fixed.inset-0 > div[class~=\\"flex\\"][class~=\\"h-full\\"][class~=\\"w-full\\"][class~=\\"items-center\\"][class~=\\"justify-center\\"][class~=\\"bg-token-main-surface-primary\\"]',
    );
    expect(payload).toContain("media.playbackRate");
    expect(payload).toContain("MainContentViewport");
    expect(payload).toContain(".app-shell-main-content-viewport");
    expect(payload).toContain("ApplicationMenuTopBar");
    expect(payload).toContain("MainContentFrame");
    expect(payload).toContain("MainContentTopFade");
    expect(payload).toContain(".home-banners");
    expect(payload).toContain("bg-token-dropdown-background");
    expect(payload).toContain("box-shadow: none !important");
    expect(payload).toContain("border-token-border");
    expect(payload).toContain("border-width: 0 !important");
    expect(payload).toContain(
      'bg-gradient-to-t\\"][class*=\\"from-token-main-surface-primary\\"]',
    );
    expect(payload).toContain(
      'bg-linear-to-t\\"][class*=\\"from-token-main-surface-primary\\"]',
    );
    expect(payload).not.toContain("via-token-main-surface-primary");
    expect(payload).toContain("activity-header");
    expect(payload).toContain("bg-token-main-surface-primary");
    expect(payload).toContain(':is(div, section, aside)[class~=\\"bg-token-main-surface-primary\\"]');
    expect(payload).toContain("turn-diff-header");
    expect(payload).toContain("bg-token-bg-secondary");
    expect(payload).toContain("DndDescribedBy-");
    expect(payload).toContain("size-token-button-composer");
    expect(payload).toContain("text-token-dropdown-background");
    expect(payload).toContain('[class~=\\"sticky\\"][class*=\\"bg-token-main-surface-primary\\"]:has(input[type=\\"text\\"])');
    expect(payload).toContain('[class~=\\"h-full\\"][class~=\\"min-h-0\\"][class~=\\"flex-col\\"]');
    expect(payload).toContain('aside[class*=\\"z-[41]\\"]');
    expect(payload).toContain('aside[class*=\\"z-[41]\\"] [class*=\\"bg-token-main-surface-primary\\"]');
    expect(payload).toContain("diffs-container");
    expect(payload).toContain("file-tree-container");
    expect(payload).toContain("group/file-diff");
    expect(payload).toContain("group/diff-header");
    expect(payload).toContain("codex-background-review-shadow-style");
    expect(payload).toContain(":host,");
    expect(payload).toContain("--diffs-bg: transparent !important");
    expect(payload).toContain("--diffs-bg-separator-override: transparent !important");
    expect(payload).toContain("--diffs-bg-addition");
    expect(payload).toContain("attachShadow");
    expect(payload).toContain("requestAnimationFrame");
    expect(payload).not.toContain("}, 200)");
    expect(payload).not.toContain(':has(ul button[class*=\\"bg-token-bg-fog\\"])');
    expect(payload).not.toContain("backdrop-filter: blur");
    expect(payload).not.toContain("__DREAM_");
    expect(payload).not.toContain("main.main-surface ");
    expect(payload).not.toContain("main.main-surface>");
    expect(payload).not.toContain("main.main-surface.");
  });

  it("serializes media URLs instead of interpolating executable source", () => {
    const payload = buildRendererPayload({
      mediaUrl: "http://127.0.0.1/media/\";window.pwned=true;//",
      mediaKind: "image",
      display: DEFAULT_SETTINGS.display,
      revision: "safe",
    });
    expect(payload).toContain("\\\"");
    expect(payload).not.toContain('mediaUrl: "http');
  });

  it("provides early-document installation and reversible removal", () => {
    const early = earlyPayloadFor("(() => true)()", "abc");
    expect(early).toContain("MutationObserver");
    expect(early).toContain("abc");
    expect(REMOVE_RENDERER_PAYLOAD).toContain("cleanup");
  });
});

