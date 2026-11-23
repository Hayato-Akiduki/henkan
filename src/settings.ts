import type { ConvertOptions } from "./types";

const STORAGE_SETTINGS = "henkan.settings";

export const DEFAULT_OPTIONS: ConvertOptions = {
  imageQuality: 85,
  imageMaxWidth: null,
  videoPreset: "default",
};

export function readSettings(storage: Pick<Storage, "getItem">): {
  outputDir: string;
  options: ConvertOptions;
} {
  const defaults = { outputDir: "", options: { ...DEFAULT_OPTIONS } };
  try {
    const value = JSON.parse(storage.getItem(STORAGE_SETTINGS) ?? "null");
    if (!value || typeof value !== "object") return defaults;
    const options = value.options ?? {};
    return {
      outputDir: typeof value.outputDir === "string" ? value.outputDir : "",
      options: {
        imageQuality: Number.isInteger(options.imageQuality) && options.imageQuality >= 40 && options.imageQuality <= 100
          ? options.imageQuality : DEFAULT_OPTIONS.imageQuality,
        imageMaxWidth: [512, 800, 1280, 1920].includes(options.imageMaxWidth)
          ? options.imageMaxWidth : null,
        videoPreset: ["default", "compact", "high"].includes(options.videoPreset)
          ? options.videoPreset : DEFAULT_OPTIONS.videoPreset,
      },
    };
  } catch {
    return defaults;
  }
}

export function saveSettings(storage: Pick<Storage, "setItem">, outputDir: string, options: ConvertOptions): void {
  storage.setItem(STORAGE_SETTINGS, JSON.stringify({ outputDir, options }));
}
