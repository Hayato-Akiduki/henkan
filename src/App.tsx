import { useCallback, useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import type {
  ConvertResult,
  FileInspectResult,
  QueueItem,
  ToolsOverview,
} from "./types";
import "./App.css";

const STORAGE_FFMPEG_PATH = "henkan.ffmpegPath";
const STORAGE_PANDOC_PATH = "henkan.pandocPath";

const CATEGORY_LABELS: Record<string, { label: string; className: string }> = {
  image: { label: "画像", className: "badge-image" },
  video: { label: "動画", className: "badge-video" },
  audio: { label: "音声", className: "badge-audio" },
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

  const [showToolsPanel, setShowToolsPanel] = useState(false);
  const [ffmpegPath, setFfmpegPath] = useState<string>(
    () => localStorage.getItem(STORAGE_FFMPEG_PATH) ?? ""
  );
  const [pandocPath, setPandocPath] = useState<string>(
    () => localStorage.getItem(STORAGE_PANDOC_PATH) ?? ""
  );
  const [tools, setTools] = useState<ToolsOverview | null>(null);
  const [isCheckingTools, setIsCheckingTools] = useState(false);

  const [appVersion, setAppVersion] = useState<string>("");
  const [availableUpdate, setAvailableUpdate] = useState<Update | null>(null);
  const [updateStatus, setUpdateStatus] = useState<string | null>(null);
  const [isCheckingUpdate, setIsCheckingUpdate] = useState(false);
  const [isInstallingUpdate, setIsInstallingUpdate] = useState(false);
  const [updateProgress, setUpdateProgress] = useState<number | null>(null);

  const handleCheckUpdate = useCallback(async (silent = false) => {
    setIsCheckingUpdate(true);
    if (!silent) setUpdateStatus(null);
    try {
      const update = await check();
      if (update) {
        setAvailableUpdate(update);
        setUpdateStatus(null);
      } else if (!silent) {
        setAvailableUpdate(null);
        setUpdateStatus("最新バージョンを使用しています。");
      }
    } catch (err) {
      if (!silent) {
        setUpdateStatus(
          `更新確認に失敗しました（リリース先リポジトリ未設定またはオフラインの可能性があります）: ${String(err)}`
        );
      }
    } finally {
      setIsCheckingUpdate(false);
    }
  }, []);

  const handleInstallUpdate = async () => {
    if (!availableUpdate) return;
    setIsInstallingUpdate(true);
    setUpdateProgress(0);
    setUpdateStatus("アップデートをダウンロード中...");

    try {
      let totalBytes = 0;
      let downloadedBytes = 0;

      await availableUpdate.downloadAndInstall((event) => {
        if (event.event === "Started") {
          totalBytes = event.data.contentLength ?? 0;
        } else if (event.event === "Progress") {
          downloadedBytes += event.data.chunkLength;
          if (totalBytes > 0) {
            setUpdateProgress(
              Math.min(100, Math.round((downloadedBytes / totalBytes) * 100))
            );
          }
        } else if (event.event === "Finished") {
          setUpdateProgress(100);
          setUpdateStatus("インストール完了。アプリを再起動しています...");
        }
      });

      await relaunch();
    } catch (err) {
      setUpdateStatus(`アップデートの適用に失敗しました: ${String(err)}`);
      setIsInstallingUpdate(false);
    }
  };

  useEffect(() => {
    getVersion()
      .then((v) => setAppVersion(v))
      .catch(() => {});
    // 起動時にバックグラウンドでサイレント更新チェック
    handleCheckUpdate(true);
  }, [handleCheckUpdate]);

  const refreshTools = useCallback(async (ffPath: string, pdPath: string) => {
    setIsCheckingTools(true);
    try {
      const overview = await invoke<ToolsOverview>("check_external_tools", {
        ffmpegPath: ffPath || null,
        pandocPath: pdPath || null,
      });
      setTools(overview);
    } catch {
      // ブラウザプレビュー時など Tauri 環境外では無視
    } finally {
      setIsCheckingTools(false);
    }
  }, []);

  useEffect(() => {
    refreshTools(ffmpegPath, pandocPath);
  }, [ffmpegPath, pandocPath, refreshTools]);

  const handleSelectToolBinary = async (tool: "ffmpeg" | "pandoc") => {
    try {
      const selected = await open({
        multiple: false,
        title: `${tool === "ffmpeg" ? "FFmpeg" : "Pandoc"} 実行ファイルを選択`,
      });
      if (typeof selected !== "string") return;

      if (tool === "ffmpeg") {
        setFfmpegPath(selected);
        localStorage.setItem(STORAGE_FFMPEG_PATH, selected);
      } else {
        setPandocPath(selected);
        localStorage.setItem(STORAGE_PANDOC_PATH, selected);
      }
    } catch (err) {
      setNotice(`ファイル選択を開けませんでした: ${String(err)}`);
    }
  };

  const handleResetToolBinary = (tool: "ffmpeg" | "pandoc") => {
    if (tool === "ffmpeg") {
      setFfmpegPath("");
      localStorage.removeItem(STORAGE_FFMPEG_PATH);
    } else {
      setPandocPath("");
      localStorage.removeItem(STORAGE_PANDOC_PATH);
    }
  };

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
          ffmpegPath: ffmpegPath || null,
          pandocPath: pandocPath || null,
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
            <div className="brand-title-row">
              <h1>Henkan</h1>
              {appVersion && <span className="version-tag">v{appVersion}</span>}
            </div>
            <p className="subtitle">マルチフォーマット・ファイル変換ツール</p>
          </div>
        </div>

        <div className="header-right">
          <button
            className="btn btn-secondary btn-update-check"
            onClick={() => handleCheckUpdate(false)}
            disabled={isCheckingUpdate || isInstallingUpdate}
            type="button"
          >
            {isCheckingUpdate ? "確認中..." : "更新確認"}
          </button>
          <button
            className={`btn btn-tool-toggle ${showToolsPanel ? "active" : ""}`}
            onClick={() => setShowToolsPanel((v) => !v)}
            type="button"
          >
            <span
              className={`status-dot ${
                tools?.ffmpeg.available ? "dot-ok" : "dot-warn"
              }`}
              title={tools?.ffmpeg.available ? "FFmpeg 利用可能" : "FFmpeg 未検出"}
            />
            <span>FFmpeg</span>
            <span
              className={`status-dot ${
                tools?.pandoc.available ? "dot-ok" : "dot-warn"
              }`}
              title={tools?.pandoc.available ? "Pandoc 利用可能" : "Pandoc 未検出"}
            />
            <span>Pandoc</span>
            <span className="toggle-caret">{showToolsPanel ? "▲" : "▼"}</span>
          </button>
        </div>
      </header>

      {/* アップデート通知バナー */}
      {availableUpdate && (
        <section className="update-banner">
          <div className="update-banner-info">
            <strong>
              新しいバージョン v{availableUpdate.version} が利用可能です
            </strong>
            {availableUpdate.body && (
              <p className="update-notes">{availableUpdate.body}</p>
            )}
            {updateProgress !== null && (
              <div className="update-progress-wrap">
                <div
                  className="update-progress-bar"
                  style={{ width: `${updateProgress}%` }}
                />
              </div>
            )}
          </div>
          <div className="update-banner-actions">
            <button
              className="btn btn-primary"
              onClick={handleInstallUpdate}
              disabled={isInstallingUpdate}
              type="button"
            >
              {isInstallingUpdate
                ? `更新中 (${updateProgress ?? 0}%)`
                : "今すぐアップデート"}
            </button>
            {!isInstallingUpdate && (
              <button
                className="btn btn-ghost"
                onClick={() => setAvailableUpdate(null)}
                type="button"
              >
                あとで
              </button>
            )}
          </div>
        </section>
      )}

      {updateStatus && !availableUpdate && (
        <div className="info-banner">
          <span>{updateStatus}</span>
          <button
            className="btn-link"
            onClick={() => setUpdateStatus(null)}
            type="button"
          >
            閉じる
          </button>
        </div>
      )}

      {/* 外部ツール設定パネル */}
      {showToolsPanel && (
        <section className="tools-panel">
          <div className="tools-panel-header">
            <span className="tools-panel-title">
              外部変換エンジン設定（動画・音声 / 文書変換で使用）
            </span>
            <button
              className="btn btn-secondary"
              onClick={() => refreshTools(ffmpegPath, pandocPath)}
              disabled={isCheckingTools}
              type="button"
            >
              {isCheckingTools ? "確認中..." : "再チェック"}
            </button>
          </div>

          <div className="tools-grid">
            <div className="tool-card">
              <div className="tool-card-top">
                <div className="tool-name-row">
                  <span
                    className={`status-dot ${
                      tools?.ffmpeg.available ? "dot-ok" : "dot-warn"
                    }`}
                  />
                  <strong>FFmpeg</strong>
                  <span className="tool-role">動画・音声変換</span>
                </div>
                <div className="tool-card-actions">
                  <button
                    className="btn btn-secondary btn-sm"
                    onClick={() => handleSelectToolBinary("ffmpeg")}
                    type="button"
                  >
                    実行ファイル指定
                  </button>
                  {ffmpegPath && (
                    <button
                      className="btn btn-ghost btn-sm"
                      onClick={() => handleResetToolBinary("ffmpeg")}
                      type="button"
                    >
                      自動に戻す
                    </button>
                  )}
                </div>
              </div>
              <p className="tool-detail">
                {tools?.ffmpeg.available
                  ? tools.ffmpeg.version ?? "検出済み"
                  : "未検出（Windows: winget install Gyan.FFmpeg / macOS: brew install ffmpeg）"}
              </p>
            </div>

            <div className="tool-card">
              <div className="tool-card-top">
                <div className="tool-name-row">
                  <span
                    className={`status-dot ${
                      tools?.pandoc.available ? "dot-ok" : "dot-warn"
                    }`}
                  />
                  <strong>Pandoc</strong>
                  <span className="tool-role">DOCX / EPUB / PDF 変換</span>
                </div>
                <div className="tool-card-actions">
                  <button
                    className="btn btn-secondary btn-sm"
                    onClick={() => handleSelectToolBinary("pandoc")}
                    type="button"
                  >
                    実行ファイル指定
                  </button>
                  {pandocPath && (
                    <button
                      className="btn btn-ghost btn-sm"
                      onClick={() => handleResetToolBinary("pandoc")}
                      type="button"
                    >
                      自動に戻す
                    </button>
                  )}
                </div>
              </div>
              <p className="tool-detail">
                {tools?.pandoc.available
                  ? tools.pandoc.version ?? "検出済み"
                  : "未検出（Windows: winget install JohnMacFarlane.Pandoc / macOS: brew install pandoc）"}
              </p>
            </div>
          </div>
        </section>
      )}

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
            画像・動画・音声・JSON/YAML/TOML/CSV・テキスト・PDF/Markdown/Word など（複数選択OK）
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
            <span
              className="output-path"
              title={outputDir || "元のファイルと同じフォルダ"}
            >
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
              {isConvertingAll ? "変換中..." : `すべて変換 (${pendingCount})`}
            </button>
          </div>
        </section>

        {/* ファイルキュー */}
        <section className="queue-section">
          {items.length === 0 ? (
            <div className="empty-state">
              <p>まだファイルが追加されていません。</p>
              <p className="empty-hint">
                画像（PNG/JPG/WebP 等）、動画・音声（MP4/MP3/WAV 等）、データ（JSON/YAML/TOML/CSV）、文書（Markdown/DOCX/HTML 等）をドロップしてみてください。
              </p>
            </div>
          ) : (
            <ul className="queue-list">
              {items.map((item) => {
                const badge =
                  CATEGORY_LABELS[item.file.category] ??
                  CATEGORY_LABELS.unsupported;

                return (
                  <li
                    key={item.id}
                    className={`queue-item status-${item.status}`}
                  >
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
