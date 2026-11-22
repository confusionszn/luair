import type { LuairState } from "./crosshair";

export interface AiModelInfo {
  id: string;
  name: string;
  architecture: string;
  quantization: string;
  approxSizeMb: number;
  description: string;
  downloaded: boolean;
  loaded: boolean;
  localSizeBytes: number;
}

export interface AiEngineStatus {
  models: AiModelInfo[];
  loadedModelId: string | null;
  downloadingModelId: string | null;
  activeBackend: string;
}

export interface AiDownloadProgress {
  modelId: string;
  stage: string;
  downloadedBytes: number;
  totalBytes: number | null;
  percent: number;
  message: string;
}

export interface ChatMessage {
  role: "user" | "assistant";
  content: string;
}

export interface AiChatResponse {
  reply: string;
  engineUsed: string;
  appliedAction: string | null;
  updatedState: LuairState | null;
}

export interface AiApplyResult {
  summary: string;
  engineUsed: string;
  boundHotkey: string | null;
  state: LuairState;
}
