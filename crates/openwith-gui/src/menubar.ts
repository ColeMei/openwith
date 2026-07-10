import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

import {
  api,
  type ExtMatchDto,
  type PickerAppDto,
  type RecentChangeDto,
} from "./api";
import { avatarColor, initials } from "./colors";
import { escapeHtml } from "./state";

const root = document.getElementById("app")!;
const popoverWindow = getCurrentWebviewWindow();

interface PopoverState {
  query: string;
  matches: ExtMatchDto[];
  recent: RecentChangeDto[];
  picker: { ext: string; apps: PickerAppDto[] } | null;
}

const state: PopoverState = {
  query: "",
  matches: [],
  recent: [],
  picker: null,
};

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
      return `
      <div class="pop-row ${i === 0 ? "first" : ""}">
        ${chip(app)}
        <span class="pop-row-text">
          <span class="line">.${escapeHtml(m.ext)} → ${escapeHtml(app)}</span>
          <span class="bid">${escapeHtml(m.bundle_id ?? "no default set")}</span>
        </span>
        <button class="pop-link" data-action="change" data-ext="${escapeHtml(m.ext)}">Change</button>
      </div>`;
    })
    .join("");
}

function renderRecent(): string {
  if (state.recent.length === 0) {
    return `<div class="pop-recent-row"><span class="pop-muted italic">No changes recorded yet.</span></div>`;
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
  const rows = state.picker.apps
    .map(
      (a) => `
      <div class="pop-picker-row" data-action="choose" data-bundle-id="${escapeHtml(a.bundle_id)}">
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
      <span class="hotkey">⌥⌘O</span>
    </div>
    <div class="pop-dropzone">
      <div class="line1">Drop a file to look up its default</div>
      <div class="line2">or type an extension below</div>
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
  render();
}

async function refreshRecent() {
  try {
    state.recent = await api.getRecentChanges(4);
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
      render();
    } else {
      void popoverWindow.hide();
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
