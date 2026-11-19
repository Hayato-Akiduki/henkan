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
  category: "image" | "data" | "text" | "document" | "unsupported" | string;
  availableTargets: TargetFormat[];
}

export interface ConvertResult {
  inputPath: string;
  outputPath: string;
  outputSize: number;
  durationMs: number;
}

export type ItemStatus = "idle" | "converting" | "done" | "error";

export interface QueueItem {
  id: string;
  file: FileInspectResult;
  targetFormat: string;
  status: ItemStatus;
  result?: ConvertResult;
  error?: string;
}
