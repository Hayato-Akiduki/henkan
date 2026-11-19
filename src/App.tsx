import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  ConvertResult,
  FileInspectResult,
  QueueItem,
} from "./types";
import "./App.css";

const CATEGORY_LABELS: Record<string, { label: string; className: string }> = {
  image: { label: "画像", className: "badge-image" },
  data: { label: "データ", className: "badge-data" },
  text: { label: "テキスト", className: "badge-text" },
  document: { label: "文書", className: "badge-doc" },
  unsupported: { label: "未対応", className: "badge-unsupported" },
};

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

export function App() {
  const [items, setItems] = useState<QueueItem[]>([]);
  const [outputDir, setOutputDir] = useState<string>("");
  const [isDragging, setIsDragging] = useState(false);
  const [isConvertingAll, setIsConvertingAll] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  const addFilePaths = useCallback(async (paths: string[]) => {
    if (paths.length === 0) return;
    setNotice(null);

    try {
      const results = await invoke<
        Array<{ Ok?: FileInspectResult; Err?: string } | FileInspectResult>
      >("inspect_files", { paths });

      const newItems: QueueItem[] = [];
      const errors: string[] = [];

      for (const entry of results) {
        if ("Ok" in entry && entry.Ok) {
          const file = entry.Ok;
          newItems.push({
            id: `${file.path}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
            file,
            targetFormat: file.availableTargets[0]?.id ?? "",
            status: "idle",
          });
        } else if ("Err" in entry && entry.Err) {
          errors.push(entry.Err);
        } else if ("path" in entry) {
          const file = entry as FileInspectResult;
          newItems.push({
            id: `${file.path}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
            file,
            targetFormat: file.availableTargets[0]?.id ?? "",
            status: "idle",
          });
        }
      }

      if (newItems.length > 0) {
        setItems((prev) => {
          const existingPaths = new Set(prev.map((p) => p.file.path));
          const unique = newItems.filter((n) => !existingPaths.has(n.file.path));
          return [...prev, ...unique];
        });
      }

      if (errors.length > 0) {
        setNotice(errors.join(" / "));
      }
    } catch (err) {
      setNotice(`ファイルの読み込みに失敗しました: ${String(err)}`);
    }
  }, []);

  // Tauri v2 のネイティブドラッグ＆ドロップイベントを購読
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "over" || event.payload.type === "enter") {
          setIsDragging(true);
        } else if (event.payload.type === "drop") {
          setIsDragging(false);
          addFilePaths(event.payload.paths);
        } else {
          setIsDragging(false);
        }
      })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {
        // ブラウザプレビュー時など Tauri 環境外では無視
      });

    return () => {
      if (unlisten) unlisten();
    };
  }, [addFilePaths]);

  const handleSelectFiles = async () => {
    try {
      const selected = await open({
        multiple: true,
        title: "変換するファイルを選択",
      });
      if (!selected) return;
      const paths = Array.isArray(selected) ? selected : [selected];
      await addFilePaths(paths);
    } catch (err) {
      setNotice(`ファイル選択を開けませんでした: ${String(err)}`);
    }
  };

  const handleSelectOutputDir = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "出力先フォルダを選択",
      });
      if (typeof selected === "string") {
        setOutputDir(selected);
      }
    } catch (err) {
      setNotice(`フォルダ選択を開けませんでした: ${String(err)}`);
    }
  };

  const handleChangeTarget = (id: string, targetFormat: string) => {
    setItems((prev) =>
      prev.map((item) =>
        item.id === id
          ? { ...item, targetFormat, status: "idle", error: undefined }
          : item
      )
    );
  };

  const convertSingleItem = async (item: QueueItem) => {
    if (!item.targetFormat) return;

    setItems((prev) =>
      prev.map((it) =>
        it.id === item.id
          ? { ...it, status: "converting", error: undefined }
          : it
      )
    );

    try {
      const result = await invoke<ConvertResult>("convert_file", {
        request: {
          inputPath: item.file.path,
          targetFormat: item.targetFormat,
          outputDir: outputDir || null,
        },
      });

      setItems((prev) =>
        prev.map((it) =>
          it.id === item.id ? { ...it, status: "done", result } : it
        )
      );
    } catch (err) {
      setItems((prev) =>
        prev.map((it) =>
          it.id === item.id
            ? { ...it, status: "error", error: String(err) }
            : it
        )
      );
    }
  };

  const handleConvertAll = async () => {
    const targets = items.filter(
      (it) =>
        it.targetFormat && (it.status === "idle" || it.status === "error")
    );
    if (targets.length === 0) return;

    setIsConvertingAll(true);
    try {
      for (const item of targets) {
        await convertSingleItem(item);
      }
    } finally {
      setIsConvertingAll(false);
    }
  };

  const handleRevealFile = async (path: string) => {
    try {
      await invoke("reveal_file", { path });
    } catch (err) {
      setNotice(String(err));
    }
  };

  const handleRemoveItem = (id: string) => {
    setItems((prev) => prev.filter((it) => it.id !== id));
  };

  const handleClearCompleted = () => {
    setItems((prev) => prev.filter((it) => it.status !== "done"));
  };

  const handleClearAll = () => {
    setItems([]);
    setNotice(null);
  };

  const pendingCount = items.filter(
    (it) => it.targetFormat && (it.status === "idle" || it.status === "error")
  ).length;
  const doneCount = items.filter((it) => it.status === "done").length;

  return (
    <div className="app-shell">
      <header className="app-header">
        <div className="brand">
          <div className="brand-icon">H</div>
          <div>
            <h1>Henkan</h1>
            <p className="subtitle">マルチフォーマット・ファイル変換ツール</p>
          </div>
        </div>
        <div className="supported-chips">
          <span className="chip">画像 (PNG/JPG/WebP/ICO...)</span>
          <span className="chip">データ (JSON/YAML/TOML/CSV)</span>
          <span className="chip">文字コード / Markdown</span>
        </div>
      </header>

      <main className="app-main">
        {/* ドロップゾーン */}
        <section
          className={`dropzone ${isDragging ? "dropzone-active" : ""}`}
          onClick={handleSelectFiles}
          role="button"
          tabIndex={0}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") handleSelectFiles();
          }}
        >
          <div className="dropzone-icon">+</div>
          <p className="dropzone-title">
            {isDragging
              ? "ここにドロップして追加"
              : "ファイルをここにドラッグ＆ドロップ"}
          </p>
          <p className="dropzone-sub">
            またはクリックしてファイルを選択（複数選択OK）
          </p>
        </section>

        {notice && (
          <div className="notice-banner">
            <span>{notice}</span>
            <button
              className="btn-link"
              onClick={() => setNotice(null)}
              type="button"
            >
              閉じる
            </button>
          </div>
        )}

        {/* ツールバー */}
        <section className="toolbar">
          <div className="output-dir-box">
            <span className="output-label">出力先:</span>
            <span className="output-path" title={outputDir || "元のファイルと同じフォルダ"}>
              {outputDir || "元のファイルと同じフォルダ"}
            </span>
            <button
              className="btn btn-secondary"
              onClick={handleSelectOutputDir}
              type="button"
            >
              変更
            </button>
            {outputDir && (
              <button
                className="btn btn-ghost"
                onClick={() => setOutputDir("")}
                type="button"
              >
                リセット
              </button>
            )}
          </div>

          <div className="toolbar-actions">
            {doneCount > 0 && (
              <button
                className="btn btn-ghost"
                onClick={handleClearCompleted}
                disabled={isConvertingAll}
                type="button"
              >
                完了をクリア
              </button>
            )}
            {items.length > 0 && (
              <button
                className="btn btn-ghost"
                onClick={handleClearAll}
                disabled={isConvertingAll}
                type="button"
              >
                すべてクリア
              </button>
            )}
            <button
              className="btn btn-primary"
              onClick={handleConvertAll}
              disabled={pendingCount === 0 || isConvertingAll}
              type="button"
            >
              {isConvertingAll
                ? "変換中..."
                : `すべて変換 (${pendingCount})`}
            </button>
          </div>
        </section>

        {/* ファイルキュー */}
        <section className="queue-section">
          {items.length === 0 ? (
            <div className="empty-state">
              <p>まだファイルが追加されていません。</p>
              <p className="empty-hint">
                画像（PNG/JPG/WebP 等）、データ（JSON/YAML/TOML/CSV）、テキスト、Markdown ファイルをドロップしてみてください。
              </p>
            </div>
          ) : (
            <ul className="queue-list">
              {items.map((item) => {
                const badge =
                  CATEGORY_LABELS[item.file.category] ??
                  CATEGORY_LABELS.unsupported;

                return (
                  <li key={item.id} className={`queue-item status-${item.status}`}>
                    <div className="item-main">
                      <div className="item-header">
                        <span className={`badge ${badge.className}`}>
                          {badge.label}
                        </span>
                        <span className="item-name" title={item.file.path}>
                          {item.file.fileName}
                        </span>
                        <span className="item-meta">
                          .{item.file.extension || "なし"} ·{" "}
                          {formatBytes(item.file.size)}
                        </span>
                      </div>

                      {item.status === "done" && item.result && (
                        <div className="item-result">
                          <span className="result-ok">
                            ✓ 完了 ({formatBytes(item.result.outputSize)} /{" "}
                            {item.result.durationMs}ms)
                          </span>
                          <button
                            className="btn-link"
                            onClick={() =>
                              handleRevealFile(item.result!.outputPath)
                            }
                            type="button"
                          >
                            フォルダを開く
                          </button>
                        </div>
                      )}

                      {item.status === "error" && item.error && (
                        <div className="item-error">{item.error}</div>
                      )}
                    </div>

                    <div className="item-controls">
                      {item.file.availableTargets.length > 0 ? (
                        <>
                          <span className="arrow">→</span>
                          <select
                            className="format-select"
                            value={item.targetFormat}
                            disabled={item.status === "converting"}
                            onChange={(e) =>
                              handleChangeTarget(item.id, e.target.value)
                            }
                          >
                            {item.file.availableTargets.map((t) => (
                              <option key={t.id} value={t.id}>
                                {t.label} ({t.description})
                              </option>
                            ))}
                          </select>
                          <button
                            className="btn btn-secondary"
                            onClick={() => convertSingleItem(item)}
                            disabled={
                              item.status === "converting" || isConvertingAll
                            }
                            type="button"
                          >
                            {item.status === "converting"
                              ? "変換中"
                              : item.status === "done"
                              ? "再変換"
                              : "変換"}
                          </button>
                        </>
                      ) : (
                        <span className="unsupported-text">
                          現在のバージョンでは未対応です
                        </span>
                      )}

                      <button
                        className="btn-icon-remove"
                        onClick={() => handleRemoveItem(item.id)}
                        disabled={item.status === "converting"}
                        title="リストから削除"
                        type="button"
                      >
                        ×
                      </button>
                    </div>
                  </li>
                );
              })}
            </ul>
          )}
        </section>
      </main>
    </div>
  );
}

export default App;
