import { getVersion } from "@tauri-apps/api/app";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  disable as disableAutostart,
  enable as enableAutostart,
  isEnabled as autostartEnabled,
} from "@tauri-apps/plugin-autostart";
import {
  ask,
  open as openDialog,
  save as saveDialog,
} from "@tauri-apps/plugin-dialog";

import { api, type AppDto, type SetResultDto } from "./api";
import { avatarColor, initials } from "./colors";
import {
  appStats,
  escapeHtml,
  filteredApps,
  filteredAssociations,
  findApp,
  saveSettings,
  schemeRole,
  sheetApps,
  shortcutGlyphs,
  state,
  type Tab,
  type ToastState,
} from "./state";
import { applyTheme, type Appearance } from "./theme";

const root = document.getElementById("app")!;

function avatar(name: string, extraClass = ""): string {
  return `<span class="avatar ${extraClass}" style="background:${avatarColor(name)}">${escapeHtml(initials(name))}</span>`;
}

// ---------- shell ----------

// Glyphs from the design prototype — monochrome text, not emoji.
const TABS: { id: Tab; icon: string; label: string }[] = [
  { id: "extensions", icon: "⌸", label: "Extensions" },
  { id: "apps", icon: "⊞", label: "Apps" },
  { id: "schemes", icon: "⤴", label: "Schemes" },
  { id: "profiles", icon: "⇅", label: "Profiles" },
];

function renderHeader(): string {
  const tabs = TABS.map(
    (t) => `
    <button class="tab ${!state.settingsOpen && state.tab === t.id ? "active" : ""}" data-action="tab" data-tab="${t.id}">
      <span class="icon">${t.icon}</span>${t.label}
    </button>`,
  ).join("");

  // data-tauri-drag-region only fires on the element itself, not children:
  // it must sit on every surface that should drag the window (bar + wordmark),
  // while the tab buttons stay grabbable-free by simply not carrying it.
  return `
  <div class="header" data-tauri-drag-region>
    <span data-tauri-drag-region style="font-size:13px;font-weight:700">OpenWith</span>
    <div class="tabs">
      ${tabs}
      <button class="gear ${state.settingsOpen ? "active" : ""}" data-action="settings-toggle" title="Settings">⚙</button>
    </div>
  </div>`;
}

// ---------- extensions ----------

function renderExtensions(): string {
  const rows = filteredAssociations();
  const showBids = state.settings.showBundleIds;
  const gridClass = showBids ? "" : "no-bids";
  const cursor = clampCursor(state.extCursor, rows.length);
  const rowsHtml = rows
    .map((r, i) => {
      const appName = r.app_name ?? "(none)";
      const bid = r.bundle_id ?? "";
      const badge =
        r.conflict && state.settings.warnUtiConflicts
          ? `<span class="badge-conflict" title="Shares a UTI with sibling extensions">UTI ⚠</span>`
          : "";
      return `
      <div class="ext-row ${gridClass} ${i === cursor ? "active" : ""}" data-action="open-ext-sheet" data-ext="${escapeHtml(r.ext)}" data-idx="${i}">
        <span class="ext-ext">.${escapeHtml(r.ext)}</span>
        <span class="ext-app">
          ${avatar(appName)}
          <span class="name">${escapeHtml(appName)}</span>
        </span>
        ${showBids ? `<span class="ext-bid">${escapeHtml(bid)}</span>` : ""}
        <span class="ext-flags">${badge}</span>
      </div>`;
    })
    .join("");

  return `
  <div class="view">
    <div class="panel ext-table">
      <div class="search-bar">
        <div class="search-field">
          <span class="icon">⌕</span>
          <input id="ext-search-input" data-action="ext-query" placeholder="Search extensions or apps (⌘F)" value="${escapeHtml(state.extQuery)}">
        </div>
        <span class="count">${rows.length} extensions</span>
      </div>
      <div class="ext-head ${gridClass}">
        <span>EXT</span><span>DEFAULT APP</span>${showBids ? "<span>BUNDLE ID</span>" : ""}<span></span>
      </div>
      <div class="ext-rows">${rowsHtml || `<div class="footer-hint" style="padding:16px">No extensions match.</div>`}</div>
      <div class="footer-hint">↑↓ to move · ↵ to change its default · drop any file on this window to look it up</div>
    </div>
  </div>`;
}

// ---------- apps ----------

function renderApps(): string {
  const apps = filteredApps();
  if (!state.selectedBundleId && apps.length > 0) {
    state.selectedBundleId = apps[0].bundle_id;
  }
  const selected = findApp(state.selectedBundleId) ?? apps[0] ?? null;

  const rowsHtml = apps
    .map((a) => {
      const stats = appStats(a);
      const selectedClass =
        selected && a.bundle_id === selected.bundle_id ? "selected" : "";
      return `
      <div class="app-row ${selectedClass}" data-action="select-app" data-bundle-id="${escapeHtml(a.bundle_id)}">
        ${avatar(a.name)}
        <span class="name">${escapeHtml(a.name)}</span>
        <span class="defcount">${stats.defCount}</span>
      </div>`;
    })
    .join("");

  const detail = selected ? renderAppDetail(selected) : `<div class="app-detail"></div>`;

  return `
  <div class="view apps-view panel">
    <div class="apps-list">
      <div class="search-field-wrap">
        <div class="search-field">
          <span class="icon">⌕</span>
          <input id="apps-search-input" data-action="apps-query" placeholder="Search apps…" value="${escapeHtml(state.appsQuery)}">
        </div>
      </div>
      <div class="apps-list-rows">${rowsHtml}</div>
    </div>
    ${detail}
  </div>`;
}

function renderAppDetail(app: AppDto): string {
  const stats = appStats(app);
  const defaultsHtml =
    stats.defaults.length > 0
      ? stats.defaults
          .map((e) => `<span class="chip-default">.${escapeHtml(e)}</span>`)
          .join("")
      : `<span class="italic-muted">not the default for anything yet</span>`;

  const claimsHtml =
    stats.claimable.length > 0
      ? stats.claimable
          .map(
            (c) =>
              `<span class="chip-claim" data-action="claim-ext" data-ext="${escapeHtml(c.ext)}" title="${c.currentApp ? `currently ${escapeHtml(c.currentApp)}` : "no default set"}">.${escapeHtml(c.ext)} +</span>`,
          )
          .join("")
      : `<span class="italic-muted">already the default for everything it supports</span>`;

  return `
  <div class="app-detail">
    <div class="app-detail-head">
      ${avatar(app.name, "avatar-lg")}
      <div style="min-width:0">
        <div class="app-detail-name">${escapeHtml(app.name)}</div>
        ${state.settings.showBundleIds ? `<div class="app-detail-bid mono">${escapeHtml(app.bundle_id)}</div>` : ""}
      </div>
      <button class="btn-claim-all" data-action="claim-all" ${stats.claimable.length === 0 ? "disabled" : ""}>Claim all supported (${stats.claimable.length})</button>
    </div>
    <div class="app-stats">
      <div><div class="num">${stats.defCount}</div><div class="label">default for</div></div>
      <div><div class="num">${stats.supCount}</div><div class="label">supported</div></div>
    </div>
    <div class="section-label">DEFAULT FOR</div>
    <div class="chip-row">${defaultsHtml}</div>
    <div class="section-label">ALSO SUPPORTS <span class="hint">— click a chip to make ${escapeHtml(app.name)} its default</span></div>
    <div class="chip-row">${claimsHtml}</div>
  </div>`;
}

// ---------- schemes ----------

function renderSchemes(): string {
  const schemes = state.snapshot?.schemes ?? [];
  const rows = schemes
    .map((s) => {
      const appName = s.app_name ?? "(none)";
      return `
      <div class="scheme-row" data-action="open-scheme-sheet" data-scheme="${escapeHtml(s.scheme)}">
        <span>
          <span class="scheme-name">${escapeHtml(s.scheme)}://</span>
          <span class="scheme-role">${escapeHtml(schemeRole(s))}</span>
        </span>
        <span class="scheme-app">
          ${avatar(appName, "avatar-sm")}
          <span>${escapeHtml(appName)}</span>
        </span>
        <span class="scheme-change">Change</span>
      </div>`;
    })
    .join("");

  return `<div class="view schemes-view panel">${rows}</div>`;
}

// ---------- profiles (export/import) ----------

function historyDate(timestamp: number): string {
  if (!timestamp) return "";
  const d = new Date(timestamp * 1000);
  const today = new Date();
  if (d.toDateString() === today.toDateString()) {
    return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }
  return d.toLocaleDateString([], { month: "short", day: "numeric" });
}

function historyRow(e: import("./api").HistoryEventDto): string {
  let icon: string;
  let iconClass = "";
  let text: string;
  if (e.kind === "export") {
    icon = "↓";
    text = `Exported <span class="mono">${escapeHtml(e.key)}</span>`;
  } else if (e.kind === "import") {
    icon = "✓";
    iconClass = "ok";
    text = `Imported <span class="mono">${escapeHtml(e.key)}</span>`;
  } else if (e.is_undo) {
    icon = "↩";
    text = `Undid <span class="mono">${escapeHtml(e.key)}</span> → ${escapeHtml(e.new_name ?? "?")}`;
  } else {
    icon = "→";
    iconClass = e.undone ? "" : "ok";
    const was = e.old_name ? ` <span class="history-detail">(was ${escapeHtml(e.old_name)})</span>` : "";
    text = `Set <span class="mono">${escapeHtml(e.key)}</span> → ${escapeHtml(e.new_name ?? "?")}${was}`;
  }
  const detail = e.detail ? escapeHtml(e.detail) : e.undone ? "reverted" : "";
  return `
  <div class="history-row ${e.undone ? "undone" : ""}">
    <span class="history-icon ${iconClass}">${icon}</span>
    <span class="history-text">${text}</span>
    <span class="history-detail">${detail}</span>
    <span class="history-date">${escapeHtml(historyDate(e.timestamp))}</span>
  </div>`;
}

/** The window actually in force: the persisted setting, unless the panel's
 * session-only "Show all" override is on. `null` means no filter. */
export function effectiveHistoryWindow(): number | null {
  return state.historyShowAll ? null : state.settings.historyWindowDays;
}

/** "Last 7 days" / "Last 30 days" / "All changes" — used in the panel head. */
function historyWindowLabel(days: number | null): string {
  if (days === null) return "All changes";
  if (days === 1) return "Last 24 hours";
  return `Last ${days} days`;
}

function renderHistory(): string {
  const windowDays = effectiveHistoryWindow();
  const empty =
    windowDays === null
      ? "Changes, exports, and imports will appear here."
      : `No changes in the ${historyWindowLabel(windowDays).toLowerCase()} — older changes are still kept.`;
  const rows =
    state.history.length > 0
      ? state.history.map(historyRow).join("")
      : `<div class="history-row"><span class="italic-muted">${escapeHtml(empty)}</span></div>`;

  // Older events are hidden, never deleted — offer the way back to them
  // whenever the setting is narrowing the view.
  const canWiden = state.settings.historyWindowDays !== null;
  const widen = canWiden
    ? `<button class="panel-block-action" data-action="toggle-history-all">${
        state.historyShowAll
          ? `Show ${historyWindowLabel(state.settings.historyWindowDays).toLowerCase()}`
          : "Show all"
      }</button>`
    : "";

  return `
  <div class="panel-block">
    <div class="panel-block-head">
      <span>HISTORY</span>
      <span class="panel-block-note">${escapeHtml(historyWindowLabel(windowDays))}</span>
      ${widen}
    </div>
    <div class="history-body">${rows}</div>
  </div>`;
}

function renderProfiles(): string {
  const totalExt = state.snapshot?.associations.length ?? 0;
  const totalSchemes = state.snapshot?.schemes.length ?? 0;

  return `
  <div class="view profiles-view">
    <div class="profiles-grid">
      <div class="profile-card">
        <div class="title">Export</div>
        <div class="desc">Save all associations to a portable TOML file — like a dotfile.</div>
        <div class="meta">${totalExt} extensions · ${totalSchemes} schemes</div>
        <button class="btn-primary" data-action="export">Export openwith.toml…</button>
      </div>
      <div class="profile-card" style="display:flex;flex-direction:column">
        <div class="title">Import</div>
        <div class="desc">Idempotent — correct entries skipped, missing apps ignored.</div>
        <div class="dropzone ${state.windowDragOver ? "dragover" : ""}" data-action="import-choose">
          Drop a .toml here, or <span class="link">choose file…</span>
        </div>
      </div>
    </div>
    ${renderImportPreview()}
    ${renderHistory()}
  </div>`;
}

function renderImportPreview(): string {
  const pending = state.importPending;
  // The panel is a fixture of the view (like the prototype): it explains the
  // import flow even when no file is staged.
  if (!pending) {
    return `
  <div class="panel-block">
    <div class="panel-block-head"><span>DRY-RUN PREVIEW</span></div>
    <div class="preview-body"><span class="italic-muted">Drop or choose a .toml to preview changes before applying.</span></div>
  </div>`;
  }
  const preview = pending.preview;
  const lines: string[] = [];
  for (const a of preview.applied) {
    const was = a.previous_app_name
      ? ` <span class="skip">(was ${escapeHtml(a.previous_app_name)})</span>`
      : "";
    lines.push(
      `<div><span class="ok">✓ set</span> ${escapeHtml(a.key)} → ${escapeHtml(a.app_name)}${was}</div>`,
    );
  }
  if (preview.unchanged > 0) {
    lines.push(`<div><span class="skip">− ${preview.unchanged} already set correctly</span></div>`);
  }
  for (const s of preview.skipped) {
    lines.push(
      `<div><span class="warn">! skip</span> <span class="skip">${escapeHtml(s.key)} → ${escapeHtml(s.app_name)}: ${escapeHtml(s.reason)}</span></div>`,
    );
  }

  return `
  <div class="panel-block">
    <div class="panel-block-head">
      <span>DRY-RUN PREVIEW — ${escapeHtml(pending.fileName)}</span>
      <button class="dismiss" data-action="import-cancel">Dismiss</button>
      <button class="apply" data-action="import-apply" ${preview.applied.length === 0 ? "disabled" : ""}>Apply ${preview.applied.length} change${preview.applied.length === 1 ? "" : "s"}</button>
    </div>
    <div class="preview-body">${lines.join("") || "No changes."}</div>
  </div>`;
}

// ---------- settings ----------

function toggleRow(
  id: string,
  on: boolean,
  label: string,
  desc: string,
  disabled = false,
): string {
  return `
  <div class="settings-row ${disabled ? "pending" : ""}">
    <span style="min-width:0">
      <span class="label">${escapeHtml(label)}</span>
      <span class="desc">${escapeHtml(desc)}</span>
    </span>
    <button class="toggle ${on ? "on" : ""}" data-action="toggle" data-toggle="${id}" ${disabled ? "disabled" : ""}><span class="knob"></span></button>
  </div>`;
}

function segmented(
  action: string,
  dataKey: string,
  options: { key: string; label: string }[],
  current: string,
): string {
  const buttons = options
    .map(
      (o) =>
        `<button class="option ${current === o.key ? "active" : ""}" data-action="${action}" data-${dataKey}="${o.key}">${o.label}</button>`,
    )
    .join("");
  return `<span class="segmented">${buttons}</span>`;
}

function updateStatusLine(): string {
  const u = state.updateStatus;
  const v = state.appVersion;
  if (u.error) return `<span class="desc warn-text">Check failed — ${escapeHtml(u.error)}</span>`;
  if (u.latest && v && u.latest !== v)
    return `<span class="desc warn-text">Update ${escapeHtml(u.latest)} available — <span class="mono">brew upgrade --cask openwith-gui</span></span>`;
  if (u.latest && u.checkedAt)
    return `<span class="desc ok">✓ Up to date · last checked ${escapeHtml(u.checkedAt)}</span>`;
  return `<span class="desc">Updates ship via Homebrew</span>`;
}

function renderSettings(): string {
  const s = state.settings;

  const cliStatus = state.cliVersion
    ? `<span class="desc ok">✓ Installed ${escapeHtml(state.cliVersion)} — GUI and CLI share the same engine</span>`
    : `<span class="desc">Not found — install with <span class="mono">brew install ColeMei/openwith/openwith</span></span>`;
  const cliPill = state.cliVersion
    ? "brew upgrade openwith"
    : "brew install ColeMei/openwith/openwith";

  return `
  <div class="view settings-view">
    <div class="settings-grid">
      <div class="settings-card">
        <div class="settings-card-head">GENERAL</div>
        ${toggleRow("launchAtLogin", s.launchAtLogin, "Launch at login", "Start OpenWith when you log in")}
        ${toggleRow("showMenuBar", s.showMenuBar, "Show in menu bar", `Quick-access panel with ${shortcutGlyphs(s.toggleShortcut)}`)}
        <div class="settings-row">
          <span><span class="label">Popover shortcut</span><span class="desc">${state.recordingShortcut ? "Press the new keys… (Esc cancels)" : "Global shortcut that toggles the quick panel"}</span></span>
          <button class="btn-pill" data-action="record-shortcut">${state.recordingShortcut ? "…" : escapeHtml(shortcutGlyphs(s.toggleShortcut))}</button>
        </div>
        ${toggleRow("hideDockIcon", s.hideDockIcon, "Hide Dock icon", "Menu-bar-only mode — needs the menu bar icon on", !s.showMenuBar)}
        <div class="settings-row">
          <span><span class="label">Appearance</span><span class="desc">System follows your macOS setting</span></span>
          ${segmented("set-appearance", "appearance", [{ key: "system", label: "System" }, { key: "light", label: "Light" }, { key: "dark", label: "Dark" }], s.appearance)}
        </div>
        <div class="settings-row">
          <span><span class="label">Open on tab</span><span class="desc">Which view the main window starts on</span></span>
          ${segmented("set-open-tab", "tab", [{ key: "extensions", label: "Extensions" }, { key: "apps", label: "Apps" }], s.openOnTab)}
        </div>
      </div>
      <div class="settings-card">
        <div class="settings-card-head">BEHAVIOR</div>
        ${toggleRow("confirmBeforeApplying", s.confirmBeforeApplying, "Confirm before applying", "Ask before changing a default")}
        ${toggleRow("warnUtiConflicts", s.warnUtiConflicts, "Warn on UTI conflicts", "Flag changes that affect sibling extensions like .env / .txt")}
        ${toggleRow("showBundleIds", s.showBundleIds, "Show bundle IDs", "Display raw bundle identifiers in lists")}
        ${toggleRow("relaunchFinder", s.relaunchFinder, "Relaunch Finder after changes", "Clears stale icon caches — closes Finder windows")}
        <div class="settings-row">
          <span><span class="label">Show history for</span><span class="desc">How far back the History panel and menu bar look — older changes are kept, just hidden</span></span>
          ${segmented("set-history-window", "days", [{ key: "7", label: "1 week" }, { key: "30", label: "1 month" }, { key: "all", label: "All" }], s.historyWindowDays === null ? "all" : String(s.historyWindowDays))}
        </div>
      </div>
      <div class="settings-card">
        <div class="settings-card-head">UPDATES</div>
        <div class="settings-row">
          <span style="min-width:0">
            <span class="label">OpenWith ${escapeHtml(state.appVersion ?? "…")}</span>
            ${updateStatusLine()}
          </span>
          <button class="btn-pill" data-action="check-updates" ${state.updateStatus.checking ? "disabled" : ""}>${state.updateStatus.checking ? "Checking…" : "Check Now"}</button>
        </div>
        ${toggleRow("autoUpdateCheck", s.autoUpdateCheck, "Check automatically", "Once per launch, in the background")}
        <div class="settings-row">
          <span><span class="label">Channel</span><span class="desc">Beta gets new features earlier</span></span>
          ${segmented("set-channel", "channel", [{ key: "stable", label: "Stable" }, { key: "beta", label: "Beta" }], s.updateChannel)}
        </div>
      </div>
      <div class="settings-card">
        <div class="settings-card-head">COMMAND LINE</div>
        <div class="settings-row">
          <span style="min-width:0"><span class="label mono">openwith</span>${cliStatus}</span>
          <span class="code-pill">${cliPill}</span>
        </div>
      </div>
    </div>
  </div>`;
}

// ---------- sheet + toast ----------

function renderSheet(): string {
  if (!state.sheet) return "";
  const sheet = state.sheet;
  const apps = sheetApps(sheet);
  const declaredCount = (state.snapshot?.apps ?? []).filter((a) =>
    sheet.kind === "ext"
      ? a.extensions.includes(sheet.key)
      : a.url_schemes.includes(sheet.key),
  ).length;

  const target = sheet.kind === "ext" ? `.${sheet.key}` : `${sheet.key}://`;

  const warning =
    sheet.kind === "ext" && sheet.conflict && state.settings.warnUtiConflicts
      ? `<div class="sheet-warning">⚠ Shares a file type — also affects <span class="mono">${sheet.siblings
          .slice(0, 6)
          .map((s) => `.${escapeHtml(s)}`)
          .join(", ")}${sheet.siblings.length > 6 ? ` +${sheet.siblings.length - 6}` : ""}</span></div>`
      : "";

  const sheetCursor = clampCursor(state.sheetCursor, apps.length);
  const appsHtml = apps
    .map((a, i) => {
      const current =
        sheet.currentBundleId !== null &&
        a.bundle_id.toLowerCase() === sheet.currentBundleId.toLowerCase();
      return `
      <div class="sheet-app ${current ? "current" : ""} ${i === sheetCursor ? "active" : ""}" data-action="choose-app" data-bundle-id="${escapeHtml(a.bundle_id)}" data-idx="${i}">
        ${avatar(a.name, "avatar-sm")}
        <span class="name">${escapeHtml(a.name)}</span>
      </div>`;
    })
    .join("");

  const showAllToggle =
    declaredCount > 0
      ? segmented("sheet-scope", "scope", [
          { key: "supporting", label: `Supporting (${declaredCount})` },
          { key: "all", label: "All apps" },
        ], sheet.showAll ? "all" : "supporting")
      : `<span class="italic-muted">no app declares support — showing all</span>`;

  return `
  <div class="sheet-overlay" data-action="close-sheet">
    <div class="sheet" data-action="swallow">
      <div class="sheet-handle"></div>
      <div class="sheet-title">Open <span class="target mono">${target}</span> with…</div>
      ${warning}
      <div class="sheet-controls">
        <div class="search-field">
          <span class="icon">⌕</span>
          <input id="sheet-search-input" data-action="sheet-query" placeholder="Search apps…" value="${escapeHtml(sheet.query)}">
        </div>
        ${showAllToggle}
      </div>
      <div class="sheet-apps">${appsHtml || `<span class="italic-muted">No apps match.</span>`}</div>
    </div>
  </div>`;
}

/** Toasts dismiss themselves; ones carrying an Undo button linger longer. */
let toastTimer: number | undefined;

function showToast(toast: ToastState | null) {
  window.clearTimeout(toastTimer);
  state.toast = toast;
  if (!toast) return;
  toastTimer = window.setTimeout(
    () => {
      state.toast = null;
      render();
    },
    toast.undo ? 8000 : 5000,
  );
}

function renderToast(): string {
  if (!state.toast) return "";
  const undoBtn = state.toast.undo
    ? `<button class="undo" data-action="undo">Undo</button>`
    : "";
  return `
  <div class="toast">
    <span class="msg">${escapeHtml(state.toast.text)}</span>
    ${undoBtn}
  </div>`;
}

function renderWindowDropOverlay(): string {
  if (!state.windowDragOver) return "";
  return `<div class="window-dropzone">Drop to look up or import</div>`;
}

// ---------- root ----------

function renderLoading(): string {
  return `
  <div class="loading">
    <div class="spinner"></div>
    <div class="title">Scanning applications…</div>
  </div>`;
}

function renderError(): string {
  return `<div class="loading"><div class="title">Failed to load: ${escapeHtml(state.error ?? "unknown error")}</div></div>`;
}

function renderMain(): string {
  let body: string;
  if (state.settingsOpen) {
    body = renderSettings();
  } else {
    switch (state.tab) {
      case "extensions":
        body = renderExtensions();
        break;
      case "apps":
        body = renderApps();
        break;
      case "schemes":
        body = renderSchemes();
        break;
      case "profiles":
        body = renderProfiles();
        break;
    }
  }
  return `
  ${renderHeader()}
  ${body}
  ${renderSheet()}
  ${renderToast()}
  ${renderWindowDropOverlay()}`;
}

// ---------- keyboard navigation ----------

/** Lists are re-derived on every keystroke, so a stored index can outlive the
 * rows it pointed at. Every read clamps rather than trusting the stored value. */
function clampCursor(i: number, len: number): number {
  if (len === 0) return 0;
  return Math.min(Math.max(i, 0), len - 1);
}

interface NavList {
  len: number;
  cursor: number;
  setCursor: (i: number) => void;
  activate: () => void;
}

/** Which list the arrow keys drive right now. Order matters: the sheet is
 * modal, and Settings has no list of its own. */
function navList(): NavList | null {
  if (state.sheet) {
    const apps = sheetApps(state.sheet);
    return {
      len: apps.length,
      cursor: clampCursor(state.sheetCursor, apps.length),
      setCursor: (i) => (state.sheetCursor = i),
      activate: () => {
        const a = apps[clampCursor(state.sheetCursor, apps.length)];
        if (a) void chooseApp(a.bundle_id);
      },
    };
  }
  if (state.settingsOpen) return null;
  if (state.tab === "extensions") {
    const rows = filteredAssociations();
    return {
      len: rows.length,
      cursor: clampCursor(state.extCursor, rows.length),
      setCursor: (i) => (state.extCursor = i),
      activate: () => {
        const r = rows[clampCursor(state.extCursor, rows.length)];
        if (r) {
          openSheet("ext", r.ext);
          state.sheetCursor = 0;
        }
      },
    };
  }
  if (state.tab === "apps") {
    const apps = filteredApps();
    const cur = Math.max(
      0,
      apps.findIndex((a) => a.bundle_id === state.selectedBundleId),
    );
    return {
      len: apps.length,
      cursor: cur,
      // The Apps tab already tracks its selection by bundle ID for the detail
      // pane, so drive that rather than introducing a second source of truth.
      setCursor: (i) => {
        const a = apps[i];
        if (a) state.selectedBundleId = a.bundle_id;
      },
      activate: () => {},
    };
  }
  return null;
}

/** True while the arrow keys are driving. Toggles a class on `root` (which
 * survives render(), since only its innerHTML is replaced) so CSS can mute the
 * pointer's hover — otherwise the cursor row and the hovered row are both lit
 * and read as two selections. */
let keyboardNav = false;

function setKeyboardNav(on: boolean): void {
  if (keyboardNav === on) return;
  keyboardNav = on;
  root.classList.toggle("kbd-nav", on);
}

/** Last pointer position actually observed. Scrolling the list under a
 * stationary pointer makes WebKit synthesize a mousemove at the *same*
 * coordinates; treating that as user input let arrow-key scrolling hand
 * control straight back to hover, which then re-seated the cursor on whatever
 * row had slid underneath — three keypresses walked eleven rows. */
let lastPointer = { x: -1, y: -1 };

// Only a move that changes position hands control back to the mouse. Passive
// listener touching one class, so it never triggers a re-render.
document.addEventListener(
  "mousemove",
  (e) => {
    if (e.clientX === lastPointer.x && e.clientY === lastPointer.y) return;
    lastPointer = { x: e.clientX, y: e.clientY };
    setKeyboardNav(false);
  },
  { passive: true },
);

/** Move the cursor by swapping the class directly instead of re-rendering.
 * Hover fires per row crossed, and rebuilding 369 rows of innerHTML on each
 * one would be visibly janky — and would fight scrollIntoView besides. */
function moveActiveClass(selector: string, idx: number): void {
  root
    .querySelectorAll(`${selector}.active`)
    .forEach((el) => el.classList.remove("active"));
  root.querySelector(`${selector}[data-idx="${idx}"]`)?.classList.add("active");
}

// Hovering a row *is* moving the cursor — one highlight, driven by whichever
// input the user last touched. Ignored while the keyboard is driving, so the
// pointer resting over the list can't drag the cursor back.
root.addEventListener("mouseover", (e) => {
  if (keyboardNav) return;
  const el = (e.target as HTMLElement).closest?.("[data-idx]") as HTMLElement | null;
  if (!el) return;
  const idx = Number(el.dataset.idx);
  if (Number.isNaN(idx)) return;
  if (el.classList.contains("ext-row")) {
    state.extCursor = idx;
    moveActiveClass(".ext-row", idx);
  } else if (el.classList.contains("sheet-app")) {
    state.sheetCursor = idx;
    moveActiveClass(".sheet-app", idx);
  }
});

function moveCursor(delta: number | "home" | "end"): boolean {
  const nav = navList();
  if (!nav || nav.len === 0) return false;
  const next =
    delta === "home"
      ? 0
      : delta === "end"
        ? nav.len - 1
        : clampCursor(nav.cursor + delta, nav.len);
  nav.setCursor(next);
  render();
  return true;
}

function render() {
  const active = document.activeElement as HTMLInputElement | null;
  const focusId = active?.id || null;
  const selStart = active?.selectionStart ?? null;
  const selEnd = active?.selectionEnd ?? null;

  root.innerHTML = `<div style="height:100%;display:flex;flex-direction:column;position:relative">${
    state.loading ? renderLoading() : state.error ? renderError() : renderMain()
  }</div>`;

  if (focusId) {
    const el = document.getElementById(focusId) as HTMLInputElement | null;
    if (el) {
      el.focus();
      if (selStart !== null && selEnd !== null && "setSelectionRange" in el) {
        el.setSelectionRange(selStart, selEnd);
      }
    }
  }

  // innerHTML is rebuilt wholesale, so the keyboard cursor has to be scrolled
  // back into view every time. "nearest" keeps it from yanking the list when
  // the row is already visible.
  root
    .querySelector(".ext-row.active, .sheet-app.active, .app-row.selected")
    ?.scrollIntoView({ block: "nearest" });
}

// ---------- mutation helpers ----------

function refreshHistory() {
  api
    .getHistory(50, effectiveHistoryWindow())
    .then((events) => {
      state.history = events;
      render();
    })
    .catch(() => {
      // history is display-only; a read failure just leaves the panel stale
    });
}

function afterApply() {
  refreshHistory();
  if (state.settings.relaunchFinder) {
    api.relaunchFinder().catch(() => {
      // Finder relaunch is best-effort; the association change already applied
    });
  }
}

function applySetResult(result: SetResultDto, announce = true) {
  if (!state.snapshot) return;
  const isScheme = result.key.endsWith("://");
  if (isScheme) {
    const bare = result.key.slice(0, -3);
    const s = state.snapshot.schemes.find((s) => s.scheme === bare);
    if (s) {
      s.bundle_id = result.bundle_id;
      s.app_name = result.app_name;
    }
  } else {
    const bare = result.key.slice(1);
    const patchOne = (ext: string) => {
      const a = state.snapshot!.associations.find((a) => a.ext === ext);
      if (a) {
        a.bundle_id = result.bundle_id;
        a.app_name = result.app_name;
      }
    };
    patchOne(bare);
    result.siblings.forEach(patchOne);
  }
  if (announce) {
    showToast(buildToast(result));
  }
}

function buildToast(result: SetResultDto) {
  if (result.unchanged) {
    return { text: `${result.key} is already ${result.app_name}` };
  }
  const was = result.previous_app_name ? ` (was ${result.previous_app_name})` : "";
  const extra =
    result.siblings.length > 0 && state.settings.warnUtiConflicts
      ? ` · also affects ${result.siblings
          .slice(0, 3)
          .map((s) => `.${s}`)
          .join(", ")}${result.siblings.length > 3 ? ` +${result.siblings.length - 3}` : ""}`
      : "";
  return {
    text: `Set ${result.key} → ${result.app_name}${was}${extra}`,
    undo:
      result.previous_app_name && result.timestamp > 0
        ? () => undoSet(result)
        : undefined,
  };
}

async function undoSet(setResult: SetResultDto) {
  try {
    const result = await api.undoChange(
      setResult.kind,
      setResult.key,
      setResult.timestamp,
    );
    applySetResult(result, false);
    showToast({ text: `Reverted ${result.key} → ${result.app_name}` });
    afterApply();
  } catch (e) {
    showToast({ text: `Undo failed: ${e}` });
  }
  render();
}

async function confirmApply(target: string, appName: string): Promise<boolean> {
  if (!state.settings.confirmBeforeApplying) return true;
  return ask(`Set ${target} to open with ${appName}?`, {
    title: "OpenWith",
    kind: "info",
  });
}

async function chooseApp(bundleId: string) {
  const sheet = state.sheet;
  if (!sheet) return;
  const target = sheet.kind === "ext" ? `.${sheet.key}` : `${sheet.key}://`;
  const appName = findApp(bundleId)?.name ?? bundleId;
  if (!(await confirmApply(target, appName))) return;
  state.sheet = null;
  try {
    const result =
      sheet.kind === "ext"
        ? await api.setDefault(sheet.key, bundleId)
        : await api.setSchemeDefault(sheet.key, bundleId);
    applySetResult(result);
    if (!result.unchanged) afterApply();
  } catch (e) {
    showToast({ text: `Failed: ${e}` });
  }
  render();
}

async function claimExt(ext: string) {
  const app = findApp(state.selectedBundleId);
  if (!app) return;
  if (!(await confirmApply(`.${ext}`, app.name))) return;
  try {
    const result = await api.setDefault(ext, app.bundle_id);
    applySetResult(result);
    if (!result.unchanged) afterApply();
  } catch (e) {
    showToast({ text: `Failed: ${e}` });
  }
  render();
}

async function claimAll() {
  const app = findApp(state.selectedBundleId);
  if (!app) return;
  const stats = appStats(app);
  if (!(await confirmApply(`${stats.claimable.length} extensions`, app.name))) return;
  let count = 0;
  for (const c of stats.claimable) {
    try {
      const result = await api.setDefault(c.ext, app.bundle_id);
      applySetResult(result, false);
      count++;
    } catch {
      // skip failures, continue claiming the rest
    }
  }
  showToast({ text: `Claimed ${count} extension${count === 1 ? "" : "s"} for ${app.name}` });
  if (count > 0) afterApply();
  render();
}

async function handleExport() {
  let path: string | null;
  try {
    path = await saveDialog({
      defaultPath: "openwith.toml",
      filters: [{ name: "TOML", extensions: ["toml"] }],
    });
  } catch (e) {
    showToast({ text: `Export failed: ${e}` });
    render();
    return;
  }
  if (!path) return;
  try {
    const result = await api.exportToml(path);
    showToast({
      text: `Exported ${result.association_count} associations and ${result.scheme_count} schemes`,
    });
    refreshHistory();
  } catch (e) {
    showToast({ text: `Export failed: ${e}` });
  }
  render();
}

async function startImportPreview(path: string) {
  try {
    const preview = await api.importToml(path, true);
    state.importPending = {
      path,
      fileName: path.split("/").pop() ?? path,
      preview,
    };
  } catch (e) {
    showToast({ text: `Import failed: ${e}` });
  }
  render();
}

async function handleImportChoose() {
  let selection: string | string[] | null;
  try {
    selection = await openDialog({
      multiple: false,
      filters: [{ name: "TOML", extensions: ["toml"] }],
    });
  } catch (e) {
    showToast({ text: `Import failed: ${e}` });
    render();
    return;
  }
  const path = Array.isArray(selection) ? selection[0] : selection;
  if (!path) return;
  state.tab = "profiles";
  await startImportPreview(path);
}

async function applyImport() {
  if (!state.importPending) return;
  const pending = state.importPending;
  if (!(await confirmApply(`${pending.preview.applied.length} entries from ${pending.fileName}`, "their listed apps"))) return;
  state.loading = true;
  render();
  try {
    const result = await api.importToml(pending.path, false);
    state.importPending = null;
    state.snapshot = await api.getSnapshot();
    showToast({
      text: `Applied ${result.applied.length}, unchanged ${result.unchanged}, skipped ${result.skipped.length}`,
    });
    if (result.applied.length > 0) afterApply();
    else refreshHistory();
  } catch (e) {
    showToast({ text: `Import failed: ${e}` });
  } finally {
    state.loading = false;
    render();
  }
}

function openSheet(kind: "ext" | "scheme", key: string) {
  if (kind === "ext") {
    const assoc = state.snapshot?.associations.find((a) => a.ext === key);
    state.sheet = {
      kind,
      key,
      conflict: assoc?.conflict ?? false,
      siblings: assoc?.siblings ?? [],
      currentBundleId: assoc?.bundle_id ?? null,
      currentAppName: assoc?.app_name ?? null,
      query: "",
      showAll: false,
    };
  } else {
    const scheme = state.snapshot?.schemes.find((s) => s.scheme === key);
    state.sheet = {
      kind,
      key,
      conflict: false,
      siblings: [],
      currentBundleId: scheme?.bundle_id ?? null,
      currentAppName: scheme?.app_name ?? null,
      query: "",
      showAll: false,
    };
  }
}

function lookupDroppedFile(path: string) {
  const filename = path.split("/").pop() ?? path;
  const dot = filename.lastIndexOf(".");
  if (dot <= 0) {
    showToast({ text: `${filename} has no file extension` });
    render();
    return;
  }
  const ext = filename.slice(dot + 1).toLowerCase();
  state.settingsOpen = false;
  state.tab = "extensions";
  openSheet("ext", ext);
  render();
}

// ---------- update check ----------

interface GithubRelease {
  tag_name: string;
  prerelease: boolean;
  draft: boolean;
}

async function checkForUpdates() {
  if (state.updateStatus.checking) return;
  state.updateStatus.checking = true;
  state.updateStatus.error = null;
  render();
  try {
    const resp = await fetch(
      "https://api.github.com/repos/ColeMei/openwith/releases?per_page=15",
      { headers: { Accept: "application/vnd.github+json" } },
    );
    if (!resp.ok) throw new Error(`GitHub returned ${resp.status}`);
    const releases = (await resp.json()) as GithubRelease[];
    const beta = state.settings.updateChannel === "beta";
    const candidate = releases.find(
      (r) => !r.draft && (beta || !r.prerelease),
    );
    if (!candidate) throw new Error("no releases found");
    state.updateStatus.latest = candidate.tag_name.replace(/^v/, "");
    // Seconds matter: repeat "Check Now" clicks within the same minute must
    // still visibly change something.
    state.updateStatus.checkedAt = new Date().toLocaleTimeString([], {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  } catch (e) {
    state.updateStatus.error = e instanceof Error ? e.message : String(e);
  } finally {
    state.updateStatus.checking = false;
    render();
  }
}

// ---------- event delegation ----------

root.addEventListener("click", (e) => {
  const target = (e.target as HTMLElement).closest("[data-action]") as HTMLElement | null;
  if (!target) return;
  const action = target.dataset.action;

  switch (action) {
    case "tab":
      state.settingsOpen = false;
      state.tab = target.dataset.tab as Tab;
      render();
      break;
    case "settings-toggle":
      state.settingsOpen = !state.settingsOpen;
      state.recordingShortcut = false;
      if (state.settingsOpen) {
        // Re-probe so a CLI installed or upgraded since launch shows up.
        api.detectCli().then((v) => {
          if (v !== state.cliVersion) {
            state.cliVersion = v;
            render();
          }
        });
      }
      render();
      break;
    case "record-shortcut":
      state.recordingShortcut = !state.recordingShortcut;
      render();
      break;
    case "open-ext-sheet":
      // Keep the keyboard cursor where the mouse just was, so arrows resume
      // from the clicked row rather than jumping back to the top.
      if (target.dataset.idx) state.extCursor = Number(target.dataset.idx);
      openSheet("ext", target.dataset.ext!);
      state.sheetCursor = 0;
      render();
      break;
    case "open-scheme-sheet":
      openSheet("scheme", target.dataset.scheme!);
      state.sheetCursor = 0;
      render();
      break;
    case "close-sheet":
      state.sheet = null;
      render();
      break;
    case "swallow":
      break;
    case "choose-app":
      void chooseApp(target.dataset.bundleId!);
      break;
    case "select-app":
      state.selectedBundleId = target.dataset.bundleId!;
      render();
      break;
    case "claim-ext":
      void claimExt(target.dataset.ext!);
      break;
    case "claim-all":
      void claimAll();
      break;
    case "export":
      void handleExport();
      break;
    case "import-choose":
      void handleImportChoose();
      break;
    case "import-apply":
      void applyImport();
      break;
    case "import-cancel":
      state.importPending = null;
      render();
      break;
    case "undo":
      if (state.toast?.undo) state.toast.undo();
      showToast(null);
      render();
      break;
    case "sheet-scope":
      if (state.sheet) {
        state.sheet.showAll = target.dataset.scope === "all";
        state.sheetCursor = 0;
        render();
      }
      break;
    case "check-updates":
      void checkForUpdates();
      break;
    case "toggle": {
      const key = target.dataset.toggle as
        | "launchAtLogin"
        | "showMenuBar"
        | "hideDockIcon"
        | "confirmBeforeApplying"
        | "warnUtiConflicts"
        | "showBundleIds"
        | "relaunchFinder"
        | "autoUpdateCheck";
      state.settings[key] = !state.settings[key];
      if (key === "showMenuBar" && !state.settings.showMenuBar) {
        // Never both hidden: losing the tray forces the Dock icon back.
        state.settings.hideDockIcon = false;
        api.setDockVisible(true).catch(() => {});
      }
      saveSettings();
      if (key === "launchAtLogin") {
        void applyLaunchAtLogin(state.settings.launchAtLogin);
      } else if (key === "showMenuBar") {
        api.setTrayEnabled(state.settings.showMenuBar).catch(() => {
          showToast({ text: "Couldn't update the menu bar icon" });
          render();
        });
      } else if (key === "hideDockIcon") {
        api.setDockVisible(!state.settings.hideDockIcon).catch(() => {
          showToast({ text: "Couldn't change the Dock icon" });
          render();
        });
      }
      render();
      break;
    }
    case "set-open-tab":
      state.settings.openOnTab = target.dataset.tab as Tab;
      saveSettings();
      render();
      break;
    case "set-appearance":
      state.settings.appearance = target.dataset.appearance as Appearance;
      saveSettings();
      // Restyles this window now; the popover follows via its storage event.
      applyTheme();
      render();
      break;
    case "set-history-window": {
      const raw = target.dataset.days;
      state.settings.historyWindowDays = raw === "all" ? null : Number(raw);
      // An explicit choice supersedes the panel's one-off override.
      state.historyShowAll = false;
      saveSettings();
      // Refetch rather than filter in place: widening needs rows we never
      // asked the backend for. refreshHistory() re-renders on completion.
      refreshHistory();
      render();
      break;
    }
    case "toggle-history-all":
      state.historyShowAll = !state.historyShowAll;
      refreshHistory();
      render();
      break;
    case "set-channel":
      state.settings.updateChannel = target.dataset.channel as "stable" | "beta";
      saveSettings();
      // Re-check immediately so the status line reflects the new channel.
      void checkForUpdates();
      render();
      break;
  }
});

root.addEventListener("input", (e) => {
  const target = e.target as HTMLInputElement;
  if (target.dataset.action === "ext-query") {
    state.extQuery = target.value;
    // A new result set means the old cursor position is meaningless.
    state.extCursor = 0;
    render();
  } else if (target.dataset.action === "apps-query") {
    state.appsQuery = target.value;
    render();
  } else if (target.dataset.action === "sheet-query") {
    if (state.sheet) {
      state.sheet.query = target.value;
      state.sheetCursor = 0;
      render();
    }
  }
});

// ---------- shortcut recorder ----------

/** Map a KeyboardEvent.code to a token the Rust-side accelerator parser
 * accepts: letters, digits, and function keys. */
function accelKeyFromCode(code: string): string | null {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3).toLowerCase();
  if (/^Digit[0-9]$/.test(code)) return code.slice(5);
  if (/^F([1-9]|1[0-2])$/.test(code)) return code.toLowerCase();
  return null;
}

async function applyShortcut(accel: string) {
  const previous = state.settings.toggleShortcut;
  state.recordingShortcut = false;
  if (accel === previous) {
    render();
    return;
  }
  try {
    await api.setToggleShortcut(accel);
    state.settings.toggleShortcut = accel;
    saveSettings();
    showToast({ text: `Popover shortcut is now ${shortcutGlyphs(accel)}` });
  } catch (e) {
    showToast({ text: `Couldn't set shortcut: ${e}` });
    void api.setToggleShortcut(previous).catch(() => {});
  }
  render();
}

function recordShortcutKey(e: KeyboardEvent): void {
  e.preventDefault();
  e.stopPropagation();
  if (e.key === "Escape") {
    state.recordingShortcut = false;
    render();
    return;
  }
  const key = accelKeyFromCode(e.code);
  // Keep listening through modifier-only or unsupported presses, and require
  // a real modifier so a bare letter can't hijack global typing.
  if (!key || (!e.metaKey && !e.ctrlKey && !e.altKey)) return;
  const accel = [
    e.ctrlKey ? "ctrl" : "",
    e.altKey ? "alt" : "",
    e.shiftKey ? "shift" : "",
    e.metaKey ? "cmd" : "",
    key,
  ]
    .filter(Boolean)
    .join("+");
  void applyShortcut(accel);
}

document.addEventListener("keydown", (e) => {
  if (state.recordingShortcut) {
    recordShortcutKey(e);
    return;
  }

  // Arrows and Enter drive the list even while the search field has focus —
  // that is the whole point, so the caret keys are claimed here.
  if (!e.metaKey && !e.ctrlKey && !e.altKey) {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      if (moveCursor(e.key === "ArrowDown" ? 1 : -1)) {
        e.preventDefault();
        setKeyboardNav(true);
      }
      return;
    }
    if (e.key === "Home" || e.key === "End") {
      if (moveCursor(e.key === "Home" ? "home" : "end")) {
        e.preventDefault();
        setKeyboardNav(true);
      }
      return;
    }
    if (e.key === "Enter") {
      const nav = navList();
      if (nav && nav.len > 0) {
        e.preventDefault();
        nav.activate();
      }
      return;
    }
  }

  if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "f") {
    e.preventDefault();
    let id: string;
    if (state.sheet) {
      id = "sheet-search-input";
    } else {
      state.settingsOpen = false;
      if (state.tab !== "apps") state.tab = "extensions";
      render();
      id = state.tab === "apps" ? "apps-search-input" : "ext-search-input";
    }
    document.getElementById(id)?.focus();
  } else if (e.key === "Escape") {
    if (state.sheet) {
      state.sheet = null;
      render();
      return;
    }
    // Escape in a search field clears it first, and only gives up focus once
    // there is nothing left to clear.
    const el = document.activeElement as HTMLInputElement | null;
    if (el?.dataset.action === "ext-query" && state.extQuery) {
      state.extQuery = "";
      state.extCursor = 0;
      render();
    } else if (el?.dataset.action === "apps-query" && state.appsQuery) {
      state.appsQuery = "";
      render();
    } else {
      el?.blur();
    }
  }
});

// ---------- drag & drop ----------

getCurrentWebview().onDragDropEvent((event) => {
  if (event.payload.type === "over") {
    state.windowDragOver = true;
    render();
  } else if (event.payload.type === "drop") {
    state.windowDragOver = false;
    const path = event.payload.paths[0];
    if (!path) {
      render();
      return;
    }
    if (path.toLowerCase().endsWith(".toml")) {
      state.tab = "profiles";
      state.settingsOpen = false;
      void startImportPreview(path);
    } else {
      lookupDroppedFile(path);
    }
  } else {
    state.windowDragOver = false;
    render();
  }
});

// ---------- bootstrap ----------

async function applyLaunchAtLogin(wanted: boolean) {
  try {
    if (wanted) await enableAutostart();
    else await disableAutostart();
  } catch {
    // reflect reality back into the toggle rather than showing a lie
    state.settings.launchAtLogin = await autostartEnabled().catch(() => false);
    saveSettings();
    render();
  }
}

async function bootstrap() {
  render();

  // Apply persisted preferences that live outside the webview.
  api.setTrayEnabled(state.settings.showMenuBar).catch(() => {});
  // Replace the launch-time default with the saved popover shortcut.
  api.setToggleShortcut(state.settings.toggleShortcut).catch(() => {});
  if (state.settings.hideDockIcon && state.settings.showMenuBar) {
    api.setDockVisible(false).catch(() => {});
  }
  autostartEnabled()
    .then((actual) => {
      if (actual !== state.settings.launchAtLogin) {
        state.settings.launchAtLogin = actual;
        saveSettings();
        render();
      }
    })
    .catch(() => {});

  getVersion().then((v) => {
    state.appVersion = v;
    if (state.settings.autoUpdateCheck) void checkForUpdates();
    else render();
  });
  api.detectCli().then((v) => {
    state.cliVersion = v;
    render();
  });
  refreshHistory();

  try {
    state.snapshot = await api.getSnapshot();
  } catch (e) {
    state.error = String(e);
  } finally {
    state.loading = false;
    render();
  }
}

void bootstrap();
