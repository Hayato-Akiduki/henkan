export interface TargetFormat {
  id: string;
  label: string;
  extension: string;
  category: string;
  description: string;
}

export interface FileInspectResult {
  path: string;
  fileName: string;
  extension: string;
  size: number;
  category:
    | "image"
    | "video"
    | "audio"
    | "excel"
    | "archive"
    | "data"
    | "text"
    | "document"
    | "unsupported"
    | string;
  availableTargets: TargetFormat[];
}

export interface ConvertOptions {
  imageQuality: number;
  imageMaxWidth: number | null;
  videoPreset: "default" | "compact" | "high";
}

export interface ConvertResult {
  inputPath: string;
  outputPath: string;
  outputSize: number;
  durationMs: number;
}

export interface ToolStatus {
  available: boolean;
  command: string;
  version: string | null;
  managed: boolean;
}

export interface ToolsOverview {
  ffmpeg: ToolStatus;
  pandoc: ToolStatus;
}

export interface DownloadProgressEvent {
  stage: string;
  percent: number;
}

export type ItemStatus = "idle" | "converting" | "done" | "error" | "cancelled";

export interface QueueItem {
  id: string;
  file: FileInspectResult;
  targetFormat: string;
  status: ItemStatus;
  result?: ConvertResult;
  error?: string;
  progress?: number;
  taskId?: string;
}
