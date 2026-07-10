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
  app_name: string;
  bundle_id: string;
  previous_app_name: string | null;
  unchanged: boolean;
  siblings: string[];
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

export const api = {
  detectCli: () => invoke<string | null>("detect_cli"),
  getSnapshot: () => invoke<SnapshotDto>("get_snapshot"),
  setDefault: (ext: string, app: string) =>
    invoke<SetResultDto>("set_default", { ext, app }),
  setSchemeDefault: (scheme: string, app: string) =>
    invoke<SetResultDto>("set_scheme_default", { scheme, app }),
  exportToml: (path: string | null) =>
    invoke<ExportResultDto>("export_toml", { path }),
  importToml: (path: string, dryRun: boolean) =>
    invoke<ImportPreviewDto>("import_toml", { path, dryRun }),
};
