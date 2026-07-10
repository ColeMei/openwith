import type {
  AppDto,
  AssociationDto,
  HistoryEventDto,
  SchemeDto,
  SnapshotDto,
} from "./api";

export type Tab = "extensions" | "apps" | "schemes" | "profiles";

export interface SheetState {
  kind: "ext" | "scheme";
  key: string; // bare ext (no dot) or bare scheme
  conflict: boolean;
  siblings: string[];
  currentBundleId: string | null;
  currentAppName: string | null;
  query: string;
  showAll: boolean;
}

export interface ToastState {
  text: string;
  undo?: () => void;
}

export interface ImportPending {
  path: string;
  fileName: string;
  preview: import("./api").ImportPreviewDto;
}

export interface UpdateStatus {
  checking: boolean;
  latest: string | null; // newest version seen on GitHub, without the leading v
  checkedAt: string | null; // human time of the last successful check
  error: string | null;
}

/** User preferences, persisted to localStorage. Mirrors the prototype's
 * Settings pane. `launchAtLogin` mirrors the autostart plugin's real state
 * (synced at bootstrap); `showMenuBar` drives tray creation. */
export interface SettingsState {
  launchAtLogin: boolean;
  showMenuBar: boolean;
  /** Menu-bar-only mode. Only honored while showMenuBar is on — the app must
   * stay reachable somewhere. */
  hideDockIcon: boolean;
  appearance: import("./theme").Appearance;
  confirmBeforeApplying: boolean;
  warnUtiConflicts: boolean;
  showBundleIds: boolean;
  relaunchFinder: boolean;
  autoUpdateCheck: boolean;
  updateChannel: "stable" | "beta";
  openOnTab: Tab;
  /** Global popover toggle, in Tauri accelerator form (e.g. "alt+cmd+o"). */
  toggleShortcut: string;
}

const SETTINGS_KEY = "openwith.settings";

const DEFAULT_SETTINGS: SettingsState = {
  launchAtLogin: false,
  showMenuBar: true,
  hideDockIcon: false,
  appearance: "system",
  confirmBeforeApplying: false,
  warnUtiConflicts: true,
  showBundleIds: true,
  relaunchFinder: false,
  autoUpdateCheck: true,
  updateChannel: "stable",
  openOnTab: "extensions",
  toggleShortcut: "alt+cmd+o",
};

function loadSettings(): SettingsState {
  try {
    const raw = localStorage.getItem(SETTINGS_KEY);
    if (!raw) return { ...DEFAULT_SETTINGS };
    return { ...DEFAULT_SETTINGS, ...JSON.parse(raw) };
  } catch {
    return { ...DEFAULT_SETTINGS };
  }
}

export function saveSettings(): void {
  try {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(state.settings));
  } catch {
    // localStorage unavailable — settings just won't survive relaunch
  }
}

/** Re-read settings from localStorage. The popover calls this on `storage`
 * events so main-window changes (shortcut, bundle IDs…) reach it live. */
export function reloadSettings(): void {
  state.settings = loadSettings();
}

/** "alt+cmd+o" → "⌥⌘O" for display, in canonical macOS modifier order. */
export function shortcutGlyphs(accelerator: string): string {
  const parts = accelerator.split("+").map((p) => p.trim().toLowerCase());
  const has = (...names: string[]) => parts.some((p) => names.includes(p));
  const key = parts[parts.length - 1] ?? "";
  return [
    has("ctrl", "control") ? "⌃" : "",
    has("alt", "option") ? "⌥" : "",
    has("shift") ? "⇧" : "",
    has("cmd", "command", "super", "meta") ? "⌘" : "",
    key.toUpperCase(),
  ].join("");
}

export interface State {
  snapshot: SnapshotDto | null;
  loading: boolean;
  error: string | null;

  tab: Tab;
  settingsOpen: boolean;
  /** Settings shortcut row is capturing the next key combo. */
  recordingShortcut: boolean;

  extQuery: string;
  appsQuery: string;
  selectedBundleId: string | null;

  sheet: SheetState | null;
  toast: ToastState | null;
  importPending: ImportPending | null;
  windowDragOver: boolean;
  history: HistoryEventDto[];

  settings: SettingsState;

  /** Runtime facts shown in Settings, not preferences. */
  appVersion: string | null;
  cliVersion: string | null;
  updateStatus: UpdateStatus;
}

const initialSettings = loadSettings();

export const state: State = {
  snapshot: null,
  loading: true,
  error: null,

  tab: initialSettings.openOnTab,
  settingsOpen: false,
  recordingShortcut: false,

  extQuery: "",
  appsQuery: "",
  selectedBundleId: null,

  sheet: null,
  toast: null,
  importPending: null,
  windowDragOver: false,
  history: [],

  settings: initialSettings,

  appVersion: null,
  cliVersion: null,
  updateStatus: { checking: false, latest: null, checkedAt: null, error: null },
};

export function escapeHtml(input: string): string {
  return input
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

export function findApp(bundleId: string | null | undefined): AppDto | null {
  if (!bundleId || !state.snapshot) return null;
  return (
    state.snapshot.apps.find(
      (a) => a.bundle_id.toLowerCase() === bundleId.toLowerCase(),
    ) ?? null
  );
}

export function filteredAssociations(): AssociationDto[] {
  const q = state.extQuery.trim().toLowerCase();
  const rows = state.snapshot?.associations ?? [];
  if (!q) return rows;
  return rows.filter(
    (r) =>
      r.ext.toLowerCase().includes(q) ||
      (r.app_name?.toLowerCase().includes(q) ?? false),
  );
}

export interface AppStats {
  app: AppDto;
  defCount: number;
  supCount: number;
  defaults: string[];
  claimable: { ext: string; currentApp: string | null }[];
}

export function appStats(app: AppDto): AppStats {
  const associations = state.snapshot?.associations ?? [];
  const byExt = new Map(associations.map((a) => [a.ext, a]));

  const defaults = associations
    .filter((a) => a.bundle_id?.toLowerCase() === app.bundle_id.toLowerCase())
    .map((a) => a.ext)
    .sort();

  const supported = [...new Set(app.extensions)].sort();
  const claimable = supported
    .filter((ext) => {
      const current = byExt.get(ext);
      return current?.bundle_id?.toLowerCase() !== app.bundle_id.toLowerCase();
    })
    .map((ext) => ({
      ext,
      currentApp: byExt.get(ext)?.app_name ?? null,
    }));

  return {
    app,
    defCount: defaults.length,
    supCount: supported.length,
    defaults,
    claimable,
  };
}

export function filteredApps(): AppDto[] {
  const q = state.appsQuery.trim().toLowerCase();
  const apps = state.snapshot?.apps ?? [];
  const sorted = [...apps].sort((a, b) => a.name.localeCompare(b.name));
  if (!q) return sorted;
  return sorted.filter((a) => a.name.toLowerCase().includes(q));
}

/** Apps offered in the "Open with…" sheet. Declared supporters by default;
 * `showAll` (or no declared supporter at all — the prototype's fallback)
 * widens to every installed app, and the query filters either set. */
export function sheetApps(sheet: SheetState): AppDto[] {
  const apps = state.snapshot?.apps ?? [];
  let source = apps.filter((a) =>
    sheet.kind === "ext"
      ? a.extensions.includes(sheet.key)
      : a.url_schemes.includes(sheet.key),
  );
  if (sheet.showAll || source.length === 0) source = apps;
  const q = sheet.query.trim().toLowerCase();
  if (q) source = source.filter((a) => a.name.toLowerCase().includes(q));
  return [...source].sort((a, b) => a.name.localeCompare(b.name));
}

export function schemeRole(scheme: SchemeDto): string {
  switch (scheme.scheme) {
    case "http":
    case "https":
      return "Web browser";
    case "mailto":
      return "Email client";
    case "ftp":
    case "sftp":
      return "File transfer";
    default:
      return "URL scheme";
  }
}
