import { invoke } from "@tauri-apps/api/core";

export interface AppDto {
  name: string;
  bundle_id: string;
  extensions: string[];
  url_schemes: string[];
}

export interface AssociationDto {
  ext: string;
  app_name: string | null;
  bundle_id: string | null;
  conflict: boolean;
  siblings: string[];
}

export interface SchemeDto {
  scheme: string;
  app_name: string | null;
  bundle_id: string | null;
}

export interface SnapshotDto {
  apps: AppDto[];
  associations: AssociationDto[];
  schemes: SchemeDto[];
}

export interface SetResultDto {
  key: string;
  kind: "set" | "set_scheme";
  app_name: string;
  bundle_id: string;
  previous_app_name: string | null;
  unchanged: boolean;
  siblings: string[];
  timestamp: number;
}

export interface ExportResultDto {
  toml: string;
  association_count: number;
  scheme_count: number;
}

export interface ImportAppliedDto {
  key: string;
  app_name: string;
  previous_app_name: string | null;
}

export interface ImportSkippedDto {
  key: string;
  app_name: string;
  reason: string;
}

export interface ImportPreviewDto {
  applied: ImportAppliedDto[];
  unchanged: number;
  skipped: ImportSkippedDto[];
}

export interface ExtMatchDto {
  ext: string;
  app_name: string | null;
  bundle_id: string | null;
}

export interface PickerAppDto {
  name: string;
  bundle_id: string;
  current: boolean;
}

export interface RecentChangeDto {
  kind: "set" | "set_scheme";
  key: string;
  app_name: string;
  old_bundle_id: string | null;
  timestamp: number;
}

export interface HistoryEventDto {
  kind: "set" | "set_scheme" | "export" | "import";
  key: string;
  old_name: string | null;
  new_name: string | null;
  detail: string | null;
  timestamp: number;
  source: string;
  undone: boolean;
  is_undo: boolean;
}

/** The shared answer of the release check (updates.rs). */
export interface UpdateStatusDto {
  running: string;
  /** Newer version waiting on this channel, without the leading v. */
  pending: string | null;
  /** Unix seconds of the last check that got an answer. */
  checked_at: number | null;
  checking: boolean;
  error: string | null;
  /** A scheduled check just found `pending`: the once-a-day toast cue. */
  announce: boolean;
}

export const api = {
  detectCli: () => invoke<string | null>("detect_cli"),
  relaunchFinder: () => invoke<void>("relaunch_finder"),
  getSnapshot: () => invoke<SnapshotDto>("get_snapshot"),
  setDefault: (ext: string, app: string) =>
    invoke<SetResultDto>("set_default", { ext, app }),
  setSchemeDefault: (scheme: string, app: string) =>
    invoke<SetResultDto>("set_scheme_default", { scheme, app }),
  exportToml: (path: string | null) =>
    invoke<ExportResultDto>("export_toml", { path }),
  importToml: (path: string, dryRun: boolean) =>
    invoke<ImportPreviewDto>("import_toml", { path, dryRun }),
  getHistory: (limit: number, windowDays: number | null) =>
    invoke<HistoryEventDto[]>("get_history", { limit, windowDays }),
  searchExtensions: (query: string) =>
    invoke<ExtMatchDto[]>("search_extensions", { query }),
  getExtPicker: (ext: string) =>
    invoke<PickerAppDto[]>("get_ext_picker", { ext }),
  getRecentChanges: (limit: number, windowDays: number | null) =>
    invoke<RecentChangeDto[]>("get_recent_changes", { limit, windowDays }),
  undoChange: (kind: string, key: string, timestamp: number) =>
    invoke<SetResultDto>("undo_change", { kind, key, timestamp }),
  mainWindowReady: (background: string) => invoke<void>("main_window_ready", { background }),
  showMainWindow: () => invoke<void>("show_main_window"),
  quitApp: () => invoke<void>("quit_app"),
  setTrayEnabled: (enabled: boolean) =>
    invoke<void>("set_tray_enabled", { enabled }),
  setDockVisible: (visible: boolean) =>
    invoke<void>("set_dock_visible", { visible }),
  setDockIconDark: (dark: boolean) =>
    invoke<void>("set_dock_icon_dark", { dark }),
  setPopoverPinned: (pinned: boolean) =>
    invoke<void>("set_popover_pinned", { pinned }),
  setToggleShortcut: (accelerator: string) =>
    invoke<void>("set_toggle_shortcut", { accelerator }),
  updaterAvailable: () => invoke<boolean>("updater_available"),
  openReleasePage: (version: string) =>
    invoke<void>("open_release_page", { version }),
  getUpdateStatus: (channel: string) =>
    invoke<UpdateStatusDto>("get_update_status", { channel }),
  /** `manual` always asks GitHub; otherwise only when a daily check is due
   * and `enabled` ("Check automatically") is on. */
  checkForUpdate: (channel: string, manual: boolean, enabled: boolean) =>
    invoke<UpdateStatusDto>("check_for_update", { channel, manual, enabled }),
};
