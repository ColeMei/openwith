import { getVersion } from "@tauri-apps/api/app";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  ask,
  open as openDialog,
  save as saveDialog,
} from "@tauri-apps/plugin-dialog";

import { api, type AppDto, type SetResultDto } from "./api";
import { avatarColor, initial } from "./colors";
import {
  appStats,
  escapeHtml,
  filteredApps,
  filteredAssociations,
  findApp,
  saveSettings,
  schemeRole,
  state,
  type Tab,
} from "./state";

const root = document.getElementById("app")!;

function avatar(name: string, extraClass = ""): string {
  return `<span class="avatar ${extraClass}" style="background:${avatarColor(name)}">${escapeHtml(initial(name))}</span>`;
}

// ---------- shell ----------

const TABS: { id: Tab; icon: string; label: string }[] = [
  { id: "extensions", icon: "🗂", label: "Extensions" },
  { id: "apps", icon: "📱", label: "Apps" },
  { id: "schemes", icon: "🔗", label: "Schemes" },
  { id: "profiles", icon: "🔄", label: "Profiles" },
];

function renderHeader(): string {
  const tabs = TABS.map(
    (t) => `
    <button class="tab ${!state.settingsOpen && state.tab === t.id ? "active" : ""}" data-action="tab" data-tab="${t.id}">
      <span class="icon">${t.icon}</span>${t.label}
    </button>`,
  ).join("");

  return `
  <div class="header">
    <span style="font-size:13px;font-weight:700">OpenWith</span>
    <div class="tabs">
      ${tabs}
      <button class="gear ${state.settingsOpen ? "active" : ""}" data-action="settings-toggle" title="Settings">⚙</button>
    </div>
  </div>`;
}

// ---------- extensions ----------

function renderExtensions(): string {
  const rows = filteredAssociations();
  const rowsHtml = rows
    .map((r) => {
      const appName = r.app_name ?? "(none)";
      const bid = r.bundle_id ?? "";
      const badge = r.conflict
        ? `<span class="badge-conflict" title="Shares a UTI with sibling extensions">UTI ⚠</span>`
        : "";
      return `
      <div class="ext-row" data-action="open-ext-sheet" data-ext="${escapeHtml(r.ext)}">
        <span class="ext-ext">.${escapeHtml(r.ext)}</span>
        <span class="ext-app">
          ${avatar(appName, "")}
          <span class="name">${escapeHtml(appName)}</span>
        </span>
        <span class="ext-bid">${escapeHtml(bid)}</span>
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
          <input id="ext-search-input" data-action="ext-query" placeholder="Search extensions or apps" value="${escapeHtml(state.extQuery)}">
        </div>
        <span class="count">${rows.length} extensions</span>
      </div>
      <div class="ext-head">
        <span>EXT</span><span>DEFAULT APP</span><span>BUNDLE ID</span><span></span>
      </div>
      <div class="ext-rows">${rowsHtml || `<div class="footer-hint" style="padding:16px">No extensions match.</div>`}</div>
      <div class="footer-hint">Click a row to change its default · drop any file on this window to look it up</div>
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
      ${avatar(app.name)}
      <div>
        <div class="app-detail-name">${escapeHtml(app.name)}</div>
        <div class="app-detail-bid mono">${escapeHtml(app.bundle_id)}</div>
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
          ${avatar(appName, "")}
          <span>${escapeHtml(appName)}</span>
        </span>
        <span class="scheme-change">Change</span>
      </div>`;
    })
    .join("");

  return `<div class="view schemes-view panel">${rows}</div>`;
}

// ---------- profiles (export/import) ----------

function renderProfiles(): string {
  const totalExt = state.snapshot?.associations.length ?? 0;
  const totalSchemes = state.snapshot?.schemes.length ?? 0;

  const importCard = state.importPending
    ? renderImportPreview()
    : `
    <div class="profile-card" style="display:flex;flex-direction:column">
      <div class="title">Import</div>
      <div class="desc">Idempotent — correct entries skipped, missing apps ignored.</div>
      <div class="dropzone ${state.windowDragOver ? "dragover" : ""}" data-action="import-choose">
        Drop a .toml here, or <span class="link">choose file…</span>
      </div>
    </div>`;

  return `
  <div class="view profiles-view">
    <div class="profiles-grid">
      <div class="profile-card">
        <div class="title">Export</div>
        <div class="desc">Save all associations to a portable TOML file — like a dotfile.</div>
        <div class="meta">${totalExt} extensions · ${totalSchemes} schemes</div>
        <button class="btn-primary" data-action="export">Export openwith.toml…</button>
      </div>
      ${importCard}
    </div>
  </div>`;
}

function renderImportPreview(): string {
  const pending = state.importPending!;
  const preview = pending.preview;
  const lines: string[] = [];
  for (const a of preview.applied) {
    const was = a.previous_app_name ? ` (was ${escapeHtml(a.previous_app_name)})` : "";
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
  <div class="profile-card" style="display:flex;flex-direction:column">
    <div class="title">Import</div>
    <div class="desc">Idempotent — correct entries skipped, missing apps ignored.</div>
    <div class="panel-block" style="margin-top:0">
      <div class="panel-block-head">
        <span>DRY-RUN PREVIEW — ${escapeHtml(pending.fileName)}</span>
        <button class="apply" data-action="import-apply" ${preview.applied.length === 0 ? "disabled" : ""}>Apply ${preview.applied.length} changes</button>
      </div>
      <div class="preview-body">${lines.join("") || "No changes."}</div>
    </div>
    <button class="btn-pill" style="margin:10px 0 0" data-action="import-cancel">Cancel</button>
  </div>`;
}

// ---------- settings ----------

function toggleRow(id: string, on: boolean, label: string, desc: string): string {
  return `
  <div class="settings-row">
    <span>
      <span class="label">${escapeHtml(label)}</span>
      <span class="desc">${escapeHtml(desc)}</span>
    </span>
    <button class="toggle ${on ? "on" : ""}" data-action="toggle" data-toggle="${id}"><span class="knob"></span></button>
  </div>`;
}

function renderSettings(): string {
  const s = state.settings;

  const startTabs = (["extensions", "apps"] as Tab[])
    .map(
      (t) =>
        `<button class="option ${s.openOnTab === t ? "active" : ""}" data-action="set-open-tab" data-tab="${t}">${t === "extensions" ? "Extensions" : "Apps"}</button>`,
    )
    .join("");

  const cliStatus = state.cliVersion
    ? `<span class="desc ok">✓ Installed ${escapeHtml(state.cliVersion)} — GUI and CLI share the same engine</span>`
    : `<span class="desc">Not found — install with <span class="mono">brew install openwith</span></span>`;

  return `
  <div class="view settings-view">
    <div class="settings-grid">
      <div class="settings-card">
        <div class="settings-card-head">GENERAL</div>
        <div class="settings-row">
          <span><span class="label">Open on tab</span><span class="desc">Which view the main window starts on</span></span>
          <span class="segmented">${startTabs}</span>
        </div>
      </div>
      <div class="settings-card">
        <div class="settings-card-head">BEHAVIOR</div>
        ${toggleRow("warnSharedTypes", s.warnSharedTypes, "Warn on shared file types", "Show a heads-up when a change affects sibling extensions")}
        ${toggleRow("confirmBrowserChange", s.confirmBrowserChange, "Confirm before changing browser", "Ask before setting a new default for http/https")}
      </div>
      <div class="settings-card">
        <div class="settings-card-head">UPDATES</div>
        <div class="settings-row">
          <span>
            <span class="label">OpenWith ${escapeHtml(state.appVersion ?? "…")}</span>
            <span class="desc">Updates ship via Homebrew — <span class="mono">brew upgrade --cask openwith</span></span>
          </span>
        </div>
      </div>
      <div class="settings-card">
        <div class="settings-card-head">COMMAND LINE</div>
        <div class="settings-row">
          <span><span class="label mono">openwith</span>${cliStatus}</span>
        </div>
      </div>
    </div>
  </div>`;
}

// ---------- sheet + toast ----------

function renderSheet(): string {
  if (!state.sheet) return "";
  const sheet = state.sheet;
  const apps = state.snapshot?.apps ?? [];
  const eligible = apps
    .filter((a) =>
      sheet.kind === "ext"
        ? a.extensions.includes(sheet.key)
        : a.url_schemes.includes(sheet.key),
    )
    .sort((a, b) => a.name.localeCompare(b.name));

  const target = sheet.kind === "ext" ? `.${sheet.key}` : `${sheet.key}://`;

  const warning =
    sheet.kind === "ext" && sheet.conflict && state.settings.warnSharedTypes
      ? `<div class="sheet-warning">⚠ Shares a file type — also affects <span class="mono">${sheet.siblings
          .slice(0, 6)
          .map((s) => `.${escapeHtml(s)}`)
          .join(", ")}${sheet.siblings.length > 6 ? ` +${sheet.siblings.length - 6}` : ""}</span></div>`
      : "";

  const appsHtml = eligible
    .map(
      (a) => `
      <div class="sheet-app" data-action="choose-app" data-bundle-id="${escapeHtml(a.bundle_id)}">
        ${avatar(a.name)}
        <span class="name">${escapeHtml(a.name)}</span>
      </div>`,
    )
    .join("");

  return `
  <div class="sheet-overlay" data-action="close-sheet">
    <div class="sheet" data-action="swallow">
      <div class="sheet-handle"></div>
      <div class="sheet-title">Open <span class="target mono">${target}</span> with…</div>
      ${warning}
      <div class="sheet-apps">${appsHtml || `<span class="italic-muted">No installed app declares support for this.</span>`}</div>
    </div>
  </div>`;
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
}

// ---------- mutation helpers ----------

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
    state.toast = buildToast(result);
  }
}

function buildToast(result: SetResultDto) {
  if (result.unchanged) {
    return { text: `${result.key} is already ${result.app_name}` };
  }
  const was = result.previous_app_name ? ` (was ${result.previous_app_name})` : "";
  const extra =
    result.siblings.length > 0
      ? ` · also affects ${result.siblings
          .slice(0, 3)
          .map((s) => `.${s}`)
          .join(", ")}${result.siblings.length > 3 ? ` +${result.siblings.length - 3}` : ""}`
      : "";
  return {
    text: `Set ${result.key} → ${result.app_name}${was}${extra}`,
    undo: result.previous_app_name
      ? () => undoSet(result.key, result.previous_app_name!)
      : undefined,
  };
}

async function undoSet(key: string, previousAppName: string) {
  try {
    const isScheme = key.endsWith("://");
    const result = isScheme
      ? await api.setSchemeDefault(key.slice(0, -3), previousAppName)
      : await api.setDefault(key.slice(1), previousAppName);
    applySetResult(result, false);
    state.toast = { text: `Reverted ${result.key} → ${result.app_name}` };
  } catch (e) {
    state.toast = { text: `Undo failed: ${e}` };
  }
  render();
}

const CONFIRMED_SCHEMES = new Set(["http", "https", "mailto"]);

async function chooseApp(bundleId: string) {
  const sheet = state.sheet;
  state.sheet = null;
  if (!sheet) {
    render();
    return;
  }
  if (
    sheet.kind === "scheme" &&
    CONFIRMED_SCHEMES.has(sheet.key) &&
    state.settings.confirmBrowserChange
  ) {
    const appName = findApp(bundleId)?.name ?? bundleId;
    const role = sheet.key === "mailto" ? "mail client" : "browser";
    const confirmed = await ask(
      `Make ${appName} your default ${role} (${sheet.key}://)?`,
      { title: "OpenWith", kind: "warning" },
    );
    if (!confirmed) {
      render();
      return;
    }
  }
  try {
    const result =
      sheet.kind === "ext"
        ? await api.setDefault(sheet.key, bundleId)
        : await api.setSchemeDefault(sheet.key, bundleId);
    applySetResult(result);
  } catch (e) {
    state.toast = { text: `Failed: ${e}` };
  }
  render();
}

async function claimExt(ext: string) {
  const app = findApp(state.selectedBundleId);
  if (!app) return;
  try {
    const result = await api.setDefault(ext, app.bundle_id);
    applySetResult(result);
  } catch (e) {
    state.toast = { text: `Failed: ${e}` };
  }
  render();
}

async function claimAll() {
  const app = findApp(state.selectedBundleId);
  if (!app) return;
  const stats = appStats(app);
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
  state.toast = { text: `Claimed ${count} extension${count === 1 ? "" : "s"} for ${app.name}` };
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
    state.toast = { text: `Export failed: ${e}` };
    render();
    return;
  }
  if (!path) return;
  try {
    const result = await api.exportToml(path);
    state.toast = {
      text: `Exported ${result.association_count} associations and ${result.scheme_count} schemes`,
    };
  } catch (e) {
    state.toast = { text: `Export failed: ${e}` };
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
    state.toast = { text: `Import failed: ${e}` };
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
    state.toast = { text: `Import failed: ${e}` };
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
  const path = state.importPending.path;
  state.loading = true;
  render();
  try {
    const result = await api.importToml(path, false);
    state.importPending = null;
    state.snapshot = await api.getSnapshot();
    state.toast = {
      text: `Applied ${result.applied.length}, unchanged ${result.unchanged}, skipped ${result.skipped.length}`,
    };
  } catch (e) {
    state.toast = { text: `Import failed: ${e}` };
  } finally {
    state.loading = false;
    render();
  }
}

function lookupDroppedFile(path: string) {
  const filename = path.split("/").pop() ?? path;
  const dot = filename.lastIndexOf(".");
  if (dot <= 0) {
    state.toast = { text: `${filename} has no file extension` };
    render();
    return;
  }
  const ext = filename.slice(dot + 1).toLowerCase();
  state.settingsOpen = false;
  state.tab = "extensions";
  const assoc = state.snapshot?.associations.find((a) => a.ext === ext);
  state.sheet = {
    kind: "ext",
    key: ext,
    conflict: assoc?.conflict ?? false,
    siblings: assoc?.siblings ?? [],
  };
  render();
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
      render();
      break;
    case "open-ext-sheet": {
      const ext = target.dataset.ext!;
      const assoc = state.snapshot?.associations.find((a) => a.ext === ext);
      state.sheet = {
        kind: "ext",
        key: ext,
        conflict: assoc?.conflict ?? false,
        siblings: assoc?.siblings ?? [],
      };
      render();
      break;
    }
    case "open-scheme-sheet":
      state.sheet = { kind: "scheme", key: target.dataset.scheme!, conflict: false, siblings: [] };
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
      state.toast = null;
      render();
      break;
    case "toggle": {
      const key = target.dataset.toggle as
        | "warnSharedTypes"
        | "confirmBrowserChange";
      state.settings[key] = !state.settings[key];
      saveSettings();
      render();
      break;
    }
    case "set-open-tab":
      state.settings.openOnTab = target.dataset.tab as Tab;
      saveSettings();
      render();
      break;
  }
});

root.addEventListener("input", (e) => {
  const target = e.target as HTMLInputElement;
  if (target.dataset.action === "ext-query") {
    state.extQuery = target.value;
    render();
  } else if (target.dataset.action === "apps-query") {
    state.appsQuery = target.value;
    render();
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

async function bootstrap() {
  render();

  getVersion().then((v) => {
    state.appVersion = v;
    render();
  });
  api.detectCli().then((v) => {
    state.cliVersion = v;
    render();
  });

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
