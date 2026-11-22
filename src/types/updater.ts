export type UpdateCheckStatus =
  | "idle"
  | "checking"
  | "update-available"
  | "up-to-date"
  | "offline-or-unreleased";

export type UpdateInstallStage =
  | "idle"
  | "downloading"
  | "installing"
  | "restarting"
  | "error";

export interface UpdateCheckResult {
  available: boolean;
  currentVersion: string;
  latestVersion: string | null;
  releaseNotes: string | null;
  releaseDate: string | null;
  releaseUrl: string;
  endpoint: string;
  status: UpdateCheckStatus;
  message: string;
}

export interface UpdateProgressEvent {
  stage: UpdateInstallStage;
  downloadedBytes: number;
  totalBytes: number | null;
  percent: number;
  message: string;
}

export interface PlatformBuildInfo {
  appVersion: string;
  os: string;
  arch: string;
  targetTriple: string;
  displayServer: string;
  overlayBackend: string;
  packagingTargets: string[];
  updaterEndpoint: string;
}
