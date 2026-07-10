import type { AppDto, AssociationDto, SchemeDto, SnapshotDto } from "./api";

export type Tab = "extensions" | "apps" | "schemes" | "profiles";

export interface SheetState {
  kind: "ext" | "scheme";
  key: string; // bare ext (no dot) or bare scheme
  conflict: boolean;
  siblings: string[];
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

/** User preferences, persisted to localStorage. Phase-2/3 features
 * (launch at login, update channel, notifications) gain their toggles
 * when they land — no dead controls before that. */
export interface SettingsState {
  warnSharedTypes: boolean;
  confirmBrowserChange: boolean;
  openOnTab: Tab;
}

const SETTINGS_KEY = "openwith.settings";

const DEFAULT_SETTINGS: SettingsState = {
  warnSharedTypes: true,
  confirmBrowserChange: true,
  openOnTab: "extensions",
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

export interface State {
  snapshot: SnapshotDto | null;
  loading: boolean;
  error: string | null;

  tab: Tab;
  settingsOpen: boolean;

  extQuery: string;
  appsQuery: string;
  selectedBundleId: string | null;

  sheet: SheetState | null;
  toast: ToastState | null;
  importPending: ImportPending | null;
  windowDragOver: boolean;

  settings: SettingsState;

  /** Runtime facts shown in Settings, not preferences. */
  appVersion: string | null;
  cliVersion: string | null;
}

const initialSettings = loadSettings();

export const state: State = {
  snapshot: null,
  loading: true,
  error: null,

  tab: initialSettings.openOnTab,
  settingsOpen: false,

  extQuery: "",
  appsQuery: "",
  selectedBundleId: null,

  sheet: null,
  toast: null,
  importPending: null,
  windowDragOver: false,

  settings: initialSettings,

  appVersion: null,
  cliVersion: null,
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

export function schemeRole(scheme: SchemeDto): string {
  switch (scheme.scheme) {
    case "http":
    case "https":
      return "Default browser";
    case "mailto":
      return "Default mail client";
    default:
      return "URL scheme handler";
  }
}
