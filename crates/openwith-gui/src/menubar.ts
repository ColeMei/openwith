import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

import {
  api,
  type ExtMatchDto,
  type PickerAppDto,
  type RecentChangeDto,
} from "./api";
import { avatarColor, initials } from "./colors";
import {
  escapeHtml,
  reloadSettings,
  shortcutGlyphs,
  state as shared,
} from "./state";

const root = document.getElementById("app")!;
const popoverWindow = getCurrentWebviewWindow();

interface PopoverState {
  query: string;
  matches: ExtMatchDto[];
  recent: RecentChangeDto[];
  picker: { ext: string; apps: PickerAppDto[] } | null;
  /** Pinned popovers survive losing focus, so files can be dragged in. */
  pinned: boolean;
  /** Keyboard cursor into `matches`, or into `picker.apps` when the picker is
   * open. Clamped on read — both lists change under it as the user types. */
  cursor: number;
}

const state: PopoverState = {
  query: "",
  matches: [],
  recent: [],
  picker: null,
  pinned: false,
  cursor: 0,
};

/** Mirrors the main window: while the arrows are driving, hover is inert. */
let keyboardNav = false;
document.addEventListener(
  "mousemove",
  () => {
    keyboardNav = false;
  },
  { passive: true },
);

function clamp(i: number, len: number): number {
  if (len === 0) return 0;
  return Math.min(Math.max(i, 0), len - 1);
}

function chip(name: string): string {
  return `<span class="avatar avatar-sm" style="background:${avatarColor(name)}">${escapeHtml(initials(name))}</span>`;
}

function relTime(timestamp: number): string {
  const ago = Math.max(0, Math.floor(Date.now() / 1000) - timestamp);
  if (ago < 60) return "now";
  if (ago < 3600) return `${Math.floor(ago / 60)}m ago`;
  if (ago < 86_400) return `${Math.floor(ago / 3600)}h ago`;
  return `${Math.floor(ago / 86_400)}d ago`;
}

function renderMatches(): string {
  if (state.query.trim() === "") {
    return "";
  }
  if (state.matches.length === 0) {
    return `<div class="pop-empty">No extension matches “${escapeHtml(state.query)}”</div>`;
  }
  return state.matches
    .map((m, i) => {
      const app = m.app_name ?? "(none)";
      // "no default set" is state, not a bundle ID — show it regardless.
      const bid =
        shared.settings.showBundleIds || !m.bundle_id
          ? `<span class="bid">${escapeHtml(m.bundle_id ?? "no default set")}</span>`
          : "";
      return `
      <div class="pop-row ${i === clamp(state.cursor, state.matches.length) ? "active" : ""}" data-idx="${i}">
        ${chip(app)}
        <span class="pop-row-text">
          <span class="line">.${escapeHtml(m.ext)} → ${escapeHtml(app)}</span>
          ${bid}
        </span>
        <button class="pop-link" data-action="change" data-ext="${escapeHtml(m.ext)}">Change</button>
      </div>`;
    })
    .join("");
}

function renderRecent(): string {
  if (state.recent.length === 0) {
    const days = shared.settings.historyWindowDays;
    const empty =
      days === null
        ? "No changes recorded yet."
        : `Nothing in the last ${days} days.`;
    return `<div class="pop-recent-row"><span class="pop-muted italic">${escapeHtml(empty)}</span></div>`;
  }
  return state.recent
    .map((e, i) => {
      const right = e.old_bundle_id
        ? `<button class="pop-link small" data-action="undo" data-index="${i}">Undo</button>`
        : `<span class="pop-time">${relTime(e.timestamp)}</span>`;
      return `
      <div class="pop-recent-row">
        <span class="tick">✓</span>
        <span class="mono">${escapeHtml(e.key)}</span>
        <span class="pop-muted">→ ${escapeHtml(e.app_name)}</span>
        ${right}
      </div>`;
    })
    .join("");
}

function renderPicker(): string {
  if (!state.picker) return "";
  const cur = clamp(state.cursor, state.picker.apps.length);
  const rows = state.picker.apps
    .map(
      (a, i) => `
      <div class="pop-picker-row ${i === cur ? "active" : ""}" data-action="choose" data-bundle-id="${escapeHtml(a.bundle_id)}" data-idx="${i}">
        ${chip(a.name)}
        <span class="name ${a.current ? "bold" : ""}">${escapeHtml(a.name)}</span>
        ${a.current ? `<span class="current-tag">CURRENT</span>` : ""}
      </div>`,
    )
    .join("");
  return `
  <div class="pop-picker-overlay" data-action="close-picker">
    <div class="pop-picker" data-action="swallow">
      <div class="pop-picker-title">Open <span class="mono accent">.${escapeHtml(state.picker.ext)}</span> with…</div>
      <div class="pop-picker-rows">${rows}</div>
    </div>
  </div>`;
}

function render() {
  const active = document.activeElement as HTMLInputElement | null;
  const hadFocus = active?.id === "pop-search";
  const selStart = active?.selectionStart ?? null;

  root.innerHTML = `
  <div class="popover">
    <div class="pop-head">
      <span class="title">OpenWith</span>
      <button class="pop-pin ${state.pinned ? "active" : ""}" data-action="pin" title="Keep the panel open while you grab a file from Finder">${state.pinned ? "Pinned ✓" : "Pin"}</button>
      <span class="hotkey">${escapeHtml(shortcutGlyphs(shared.settings.toggleShortcut))}</span>
    </div>
    <div class="pop-dropzone">
      <div class="line1">Drop a file to look up its default</div>
      <div class="line2">${state.pinned ? "pinned — go grab a file, the panel will wait" : "type an extension below · Pin above to drag a file in"}</div>
    </div>
    <div class="pop-search">
      <span class="icon">⌕</span>
      <input id="pop-search" placeholder="Type an extension…" value="${escapeHtml(state.query)}">
    </div>
    <div class="pop-matches">${renderMatches()}</div>
    <div class="pop-recent">
      <div class="pop-section-label">RECENT CHANGES</div>
      <div class="pop-recent-rows">${renderRecent()}</div>
    </div>
    <div class="pop-footer">
      <button data-action="open-main">Open main window</button>
      <button data-action="quit">Quit</button>
    </div>
    ${renderPicker()}
  </div>`;

  if (hadFocus) {
    const el = document.getElementById("pop-search") as HTMLInputElement | null;
    el?.focus();
    if (el && selStart !== null) el.setSelectionRange(selStart, selStart);
  }
}

async function refreshMatches() {
  try {
    state.matches = state.query.trim()
      ? await api.searchExtensions(state.query)
      : [];
  } catch {
    state.matches = [];
  }
  state.cursor = 0;
  render();
}

async function refreshRecent() {
  try {
    // Same window as the main window's HISTORY panel — reloadSettings() on
    // `storage` events keeps this in step when the setting changes there.
    state.recent = await api.getRecentChanges(4, shared.settings.historyWindowDays);
  } catch {
    state.recent = [];
  }
  render();
}

async function openPicker(ext: string) {
  try {
    state.picker = { ext, apps: await api.getExtPicker(ext) };
  } catch {
    state.picker = null;
  }
  state.cursor = 0;
  render();
}

async function choose(bundleId: string) {
  const picker = state.picker;
  state.picker = null;
  if (!picker) return;
  try {
    await api.setDefault(picker.ext, bundleId);
  } catch {
    // surfaced by the refreshed rows showing the unchanged default
  }
  await Promise.all([refreshMatches(), refreshRecent()]);
}

async function undo(index: number) {
  const entry = state.recent[index];
  if (!entry?.old_bundle_id) return;
  try {
    // Consumes the entry: it disappears from this list instead of piling
    // a compensating row on top.
    await api.undoChange(entry.kind, entry.key, entry.timestamp);
  } catch {
    // leave the list as-is; the refresh below shows the real state
  }
  await Promise.all([refreshMatches(), refreshRecent()]);
}

root.addEventListener("click", (e) => {
  const target = (e.target as HTMLElement).closest("[data-action]") as HTMLElement | null;
  if (!target) return;
  switch (target.dataset.action) {
    case "pin":
      state.pinned = !state.pinned;
      void api.setPopoverPinned(state.pinned);
      render();
      break;
    case "change":
      void openPicker(target.dataset.ext!);
      break;
    case "choose":
      void choose(target.dataset.bundleId!);
      break;
    case "close-picker":
      state.picker = null;
      render();
      break;
    case "swallow":
      break;
    case "undo":
      void undo(Number(target.dataset.index));
      break;
    case "open-main":
      void api.showMainWindow();
      break;
    case "quit":
      void api.quitApp();
      break;
  }
});

// Same single-cursor rule as the main window: hovering moves it rather than
// lighting a second row. Class swap only — no re-render per row crossed.
// The popover's lists are short enough not to scroll under the pointer, so
// this needs no equivalent of the main window's synthetic-mousemove guard.
root.addEventListener("mouseover", (e) => {
  if (keyboardNav) return;
  const el = (e.target as HTMLElement).closest?.("[data-idx]") as HTMLElement | null;
  if (!el) return;
  const idx = Number(el.dataset.idx);
  if (Number.isNaN(idx)) return;
  const sel = state.picker ? ".pop-picker-row" : ".pop-row";
  if (!el.matches(sel)) return;
  state.cursor = idx;
  root.querySelectorAll(`${sel}.active`).forEach((n) => n.classList.remove("active"));
  el.classList.add("active");
});

root.addEventListener("input", (e) => {
  const target = e.target as HTMLInputElement;
  if (target.id === "pop-search") {
    state.query = target.value;
    void refreshMatches();
  }
});

document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    if (state.picker) {
      state.picker = null;
      state.cursor = 0;
      render();
    } else {
      state.pinned = false;
      void api.setPopoverPinned(false);
      void popoverWindow.hide();
    }
    return;
  }

  if (e.metaKey || e.ctrlKey || e.altKey) return;

  // The picker is modal over the matches; whichever is showing takes the keys.
  const len = state.picker ? state.picker.apps.length : state.matches.length;
  if (len === 0) return;

  if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    e.preventDefault();
    keyboardNav = true;
    state.cursor = clamp(
      clamp(state.cursor, len) + (e.key === "ArrowDown" ? 1 : -1),
      len,
    );
    render();
  } else if (e.key === "Enter") {
    e.preventDefault();
    const i = clamp(state.cursor, len);
    if (state.picker) {
      const a = state.picker.apps[i];
      if (a) void choose(a.bundle_id);
    } else {
      const m = state.matches[i];
      if (m) void openPicker(m.ext);
    }
  }
});

// Fresh data each time the popover opens; focus the search field.
void popoverWindow.onFocusChanged(({ payload: focused }) => {
  if (focused) {
    void refreshRecent();
    document.getElementById("pop-search")?.focus();
  }
});

// Focus events don't fire reliably for the transparent panel, so the backend
// emits this on every open: refresh data, reset the per-showing pin state.
void popoverWindow.listen("popover-shown", () => {
  state.pinned = false;
  void refreshRecent();
  document.getElementById("pop-search")?.focus();
});

// Settings changed in the main window (shortcut, bundle IDs) reach us here.
// The history window is one of them, and widening it needs rows we never
// fetched — so refetch rather than just re-render.
window.addEventListener("storage", () => {
  reloadSettings();
  void refreshRecent();
  render();
});

getCurrentWebview().onDragDropEvent((event) => {
  if (event.payload.type !== "drop") return;
  const path = event.payload.paths[0];
  if (!path) return;
  const filename = path.split("/").pop() ?? path;
  const dot = filename.lastIndexOf(".");
  if (dot <= 0) return;
  state.query = filename.slice(dot + 1).toLowerCase();
  void refreshMatches();
});

render();
void refreshRecent();
