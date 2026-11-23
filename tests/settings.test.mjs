import { test } from "node:test";
import assert from "node:assert/strict";
import { DEFAULT_OPTIONS, readSettings, saveSettings } from "../src/settings.ts";

test("保存先と全変換設定を保存・復元する", () => {
  let stored = null;
  const storage = { getItem: () => stored, setItem: (_, value) => { stored = value; } };
  const options = { imageQuality: 70, imageMaxWidth: 1280, videoPreset: "compact" };
  saveSettings(storage, "C:\\変換結果", options);
  assert.deepEqual(readSettings(storage), { outputDir: "C:\\変換結果", options });
  saveSettings(storage, "", DEFAULT_OPTIONS);
  assert.deepEqual(readSettings(storage), { outputDir: "", options: DEFAULT_OPTIONS });
});

test("保存内容が壊れている場合や読めない場合は既定値に戻る", () => {
  for (const value of [null, "{broken", "null", "42", "[]"]) {
    assert.deepEqual(readSettings({ getItem: () => value }), { outputDir: "", options: DEFAULT_OPTIONS });
  }
  assert.deepEqual(readSettings({ getItem: () => { throw Error("blocked"); } }),
    { outputDir: "", options: DEFAULT_OPTIONS });
});

test("無効な値だけ既定値に戻し、有効な設定は保持する", () => {
  const storage = { getItem: () => JSON.stringify({
    outputDir: "保存先", options: { imageQuality: 200, imageMaxWidth: -1, videoPreset: "high" },
  }) };
  assert.deepEqual(readSettings(storage), {
    outputDir: "保存先", options: { imageQuality: 85, imageMaxWidth: null, videoPreset: "high" },
  });
});
