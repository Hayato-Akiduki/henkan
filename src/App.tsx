import { useCallback, useEffect, useMemo, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import {
  AlertCircleIcon,
  ArrowRightIcon,
  CheckCircleIcon,
  FolderIcon,
  MoonIcon,
  PlayIcon,
  PlusIcon,
  RefreshIcon,
  SunIcon,
  TrashIcon,
  UploadIcon,
  WrenchIcon,
} from "./components/Icons";
import type {
  ConvertOptions,
  ConvertResult,
  DownloadProgressEvent,
  FileInspectResult,
  QueueItem,
  TargetFormat,
  ToolsOverview,
} from "./types";
import "./App.css";

const STORAGE_FFMPEG_PATH = "henkan.ffmpegPath";
const STORAGE_PANDOC_PATH = "henkan.pandocPath";
const STORAGE_THEME = "henkan.theme";

type FilterTab = "all" | "pending" | "done" | "error";

const CATEGORY_LABELS: Record<string, { label: string; className: string }> = {
  image: { label: "画像", className: "badge-image" },
  video: { label: "動画", className: "badge-video" },
  audio: { label: "音声", className: "badge-audio" },
  excel: { label: "Excel", className: "badge-excel" },
  archive: { label: "圧縮", className: "badge-archive" },
  data: { label: "データ", className: "badge-data" },
  text: { label: "テキスト", className: "badge-text" },
  document: { label: "文書", className: "badge-doc" },
  unsupported: { label: "未対応", className: "badge-unsupported" },
};

const SUPPORTED_CARDS = [
  {
    title: "画像・SVG",
    badge: "内蔵・高速",
    className: "badge-image",
    formats: "PNG / JPG / WebP / GIF / ICO / SVG → 画像・PDF（リサイズ対応）",
  },
  {
    title: "Excel・データ",
    badge: "内蔵・高速",
    className: "badge-excel",
    formats: "XLSX ⇔ CSV / JSON / YAML / TOML、文字コード変換",
  },
  {
    title: "PDF・文書",
    badge: "内蔵 + Pandoc",
    className: "badge-doc",
    formats: "PDF → Word / TXT / ページ分割・結合、Markdown / DOCX / EPUB",
  },
  {
    title: "圧縮ファイル",
    badge: "内蔵・高速",
    className: "badge-archive",
    formats: "ZIP / TAR.GZ / 7z の相互変換・フォルダ解凍",
  },
  {
    title: "動画・音声",
    badge: "FFmpeg 自動導入",
    className: "badge-video",
    formats: "MP4 / WebM / MKV / MOV / GIF / MP3 / WAV / FLAC（軽量化可）",
  },
];

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

function formatSizeDelta(before: number, after: number): string | null {
  if (before <= 0) return null;
  const pct = Math.round(((after - before) / before) * 100);
  if (pct === 0) return "±0%";
  return pct > 0 ? `+${pct}%` : `${pct}%`;
}

export function App() {
  const [theme, setTheme] = useState<"dark" | "light">(() => {
    const saved = localStorage.getItem(STORAGE_THEME);
    return saved === "light" ? "light" : "dark";
  });

  const [items, setItems] = useState<QueueItem[]>([]);
  const [filterTab, setFilterTab] = useState<FilterTab>("all");
  const [outputDir, setOutputDir] = useState<string>("");
  const [isDragging, setIsDragging] = useState(false);
  const [isConvertingAll, setIsConvertingAll] = useState(false);
  const [batchProgress, setBatchProgress] = useState<{
    current: number;
    total: number;
  } | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [mergeResult, setMergeResult] = useState<ConvertResult | null>(null);
  const [isMergingPdf, setIsMergingPdf] = useState(false);

  // 変換オプション
  const [showOptions, setShowOptions] = useState(false);
  const [options, setOptions] = useState<ConvertOptions>({
    imageQuality: 85,
    imageMaxWidth: null,
    videoPreset: "default",
  });

  // 外部ツール設定 + FFmpeg 自動ダウンロード
  const [showToolsPanel, setShowToolsPanel] = useState(false);
  const [ffmpegPath, setFfmpegPath] = useState<string>(
    () => localStorage.getItem(STORAGE_FFMPEG_PATH) ?? ""
  );
  const [pandocPath, setPandocPath] = useState<string>(
    () => localStorage.getItem(STORAGE_PANDOC_PATH) ?? ""
  );
  const [tools, setTools] = useState<ToolsOverview | null>(null);
  const [isCheckingTools, setIsCheckingTools] = useState(false);
  const [isDownloadingFfmpeg, setIsDownloadingFfmpeg] = useState(false);
  const [ffmpegDlProgress, setFfmpegDlProgress] =
    useState<DownloadProgressEvent | null>(null);

  // 自動アップデート
  const [appVersion, setAppVersion] = useState<string>("");
  const [availableUpdate, setAvailableUpdate] = useState<Update | null>(null);
  const [updateStatus, setUpdateStatus] = useState<string | null>(null);
  const [isCheckingUpdate, setIsCheckingUpdate] = useState(false);
  const [isInstallingUpdate, setIsInstallingUpdate] = useState(false);
  const [updateProgress, setUpdateProgress] = useState<number | null>(null);

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
    localStorage.setItem(STORAGE_THEME, theme);
  }, [theme]);

  const toggleTheme = () => {
    setTheme((prev) => (prev === "dark" ? "light" : "dark"));
  };

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
      // Tauri 環境外では無視
    } finally {
      setIsCheckingTools(false);
    }
  }, []);

  useEffect(() => {
    refreshTools(ffmpegPath, pandocPath);
  }, [ffmpegPath, pandocPath, refreshTools]);

  // FFmpeg 自動ダウンロード進捗イベント購読
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<DownloadProgressEvent>("ffmpeg-download-progress", (event) => {
      setFfmpegDlProgress(event.payload);
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  const handleDownloadFfmpeg = async () => {
    setIsDownloadingFfmpeg(true);
    setFfmpegDlProgress({ stage: "ダウンロードを準備中...", percent: 1 });
    setNotice(null);
    try {
      await invoke<string>("download_ffmpeg");
      // 手動パスが設定されていた場合は自動管理を優先するためクリア
      if (ffmpegPath) {
        setFfmpegPath("");
        localStorage.removeItem(STORAGE_FFMPEG_PATH);
      }
      await refreshTools("", pandocPath);
    } catch (err) {
      setNotice(`FFmpeg の自動ダウンロードに失敗しました: ${String(err)}`);
    } finally {
      setIsDownloadingFfmpeg(false);
    }
  };

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
    setMergeResult(null);

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
        setFilterTab("all");
      }

      if (errors.length > 0) {
        setNotice(errors.join(" / "));
      }
    } catch (err) {
      setNotice(`ファイルの読み込みに失敗しました: ${String(err)}`);
    }
  }, []);

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
      .catch(() => {});

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

  const bulkCategoryOptions = useMemo(() => {
    const groups = new Map<
      string,
      { count: number; targets: Map<string, TargetFormat> }
    >();

    for (const item of items) {
      if (item.file.availableTargets.length === 0) continue;
      const cat = item.file.category;
      const existing = groups.get(cat) ?? {
        count: 0,
        targets: new Map<string, TargetFormat>(),
      };
      existing.count += 1;
      for (const t of item.file.availableTargets) {
        if (!existing.targets.has(t.id)) {
          existing.targets.set(t.id, t);
        }
      }
      groups.set(cat, existing);
    }

    return Array.from(groups.entries())
      .filter(([, g]) => g.count >= 2)
      .map(([category, g]) => ({
        category,
        label: CATEGORY_LABELS[category]?.label ?? category,
        count: g.count,
        targets: Array.from(g.targets.values()),
      }));
  }, [items]);

  // 複数画像 または 複数 PDF を 1 冊の PDF に結合できるか判定
  const mergeablePdfInfo = useMemo(() => {
    const pdfs = items.filter((it) => it.file.extension === "pdf");
    if (pdfs.length >= 2) {
      return { kind: "pdf" as const, paths: pdfs.map((i) => i.file.path) };
    }
    const rasterImages = items.filter(
      (it) => it.file.category === "image" && it.file.extension !== "svg"
    );
    if (rasterImages.length >= 2) {
      return {
        kind: "image" as const,
        paths: rasterImages.map((i) => i.file.path),
      };
    }
    return null;
  }, [items]);

  const handleMergeToPdf = async () => {
    if (!mergeablePdfInfo) return;
    setIsMergingPdf(true);
    setNotice(null);
    setMergeResult(null);
    try {
      const res = await invoke<ConvertResult>("merge_to_pdf", {
        request: {
          inputPaths: mergeablePdfInfo.paths,
          outputDir: outputDir || null,
          imageQuality: options.imageQuality,
        },
      });
      setMergeResult(res);
    } catch (err) {
      setNotice(`PDF 結合に失敗しました: ${String(err)}`);
    } finally {
      setIsMergingPdf(false);
    }
  };

  const handleBulkFormatChange = (category: string, targetId: string) => {
    if (!targetId) return;
    setItems((prev) =>
      prev.map((item) => {
        if (item.file.category !== category || item.status === "converting") {
          return item;
        }
        const canApply = item.file.availableTargets.some(
          (t) => t.id === targetId
        );
        if (!canApply) return item;
        return {
          ...item,
          targetFormat: targetId,
          status: "idle",
          error: undefined,
        };
      })
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
          options: {
            imageQuality: options.imageQuality,
            imageMaxWidth: options.imageMaxWidth,
            videoPreset: options.videoPreset,
          },
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
    setBatchProgress({ current: 0, total: targets.length });
    try {
      for (let i = 0; i < targets.length; i++) {
        setBatchProgress({ current: i + 1, total: targets.length });
        await convertSingleItem(targets[i]);
      }
    } finally {
      setIsConvertingAll(false);
      setBatchProgress(null);
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
    setMergeResult(null);
    setFilterTab("all");
  };

  const pendingCount = items.filter(
    (it) => it.targetFormat && (it.status === "idle" || it.status === "error")
  ).length;
  const doneCount = items.filter((it) => it.status === "done").length;
  const errorCount = items.filter((it) => it.status === "error").length;

  const filteredItems = useMemo(() => {
    switch (filterTab) {
      case "pending":
        return items.filter(
          (it) => it.status === "idle" || it.status === "converting"
        );
      case "done":
        return items.filter((it) => it.status === "done");
      case "error":
        return items.filter((it) => it.status === "error");
      default:
        return items;
    }
  }, [items, filterTab]);

  return (
    <div className="app-shell">
      {isDragging && (
        <div className="drag-overlay">
          <div className="drag-overlay-box">
            <UploadIcon size={36} />
            <p className="drag-overlay-title">
              ここにドロップしてファイルを追加
            </p>
            <p className="drag-overlay-sub">
              複数ファイル・異なる形式をまとめて追加できます
            </p>
          </div>
        </div>
      )}

      {/* ヘッダー */}
      <header className="app-header">
        <div className="brand">
          <img
            src="/icon.png"
            alt="Henkan"
            className="brand-icon-img"
            width={40}
            height={40}
          />
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
            className="btn btn-icon-only"
            onClick={toggleTheme}
            title={
              theme === "dark" ? "ライトテーマに切替" : "ダークテーマに切替"
            }
            type="button"
          >
            {theme === "dark" ? <SunIcon size={16} /> : <MoonIcon size={16} />}
          </button>

          <button
            className="btn btn-secondary btn-header"
            onClick={() => handleCheckUpdate(false)}
            disabled={isCheckingUpdate || isInstallingUpdate}
            type="button"
          >
            <RefreshIcon
              size={14}
              className={isCheckingUpdate ? "spin" : undefined}
            />
            <span>{isCheckingUpdate ? "確認中" : "更新確認"}</span>
          </button>

          <button
            className={`btn btn-tool-toggle ${showToolsPanel ? "active" : ""}`}
            onClick={() => setShowToolsPanel((v) => !v)}
            type="button"
          >
            <WrenchIcon size={14} />
            <span
              className={`status-dot ${
                tools?.ffmpeg.available ? "dot-ok" : "dot-warn"
              }`}
              title={
                tools?.ffmpeg.available ? "FFmpeg 利用可能" : "FFmpeg 未検出"
              }
            />
            <span>FFmpeg</span>
            <span
              className={`status-dot ${
                tools?.pandoc.available ? "dot-ok" : "dot-warn"
              }`}
              title={
                tools?.pandoc.available ? "Pandoc 利用可能" : "Pandoc 未検出"
              }
            />
            <span>Pandoc</span>
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
              <div className="progress-track">
                <div
                  className="progress-fill"
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

      {/* 外部ツール設定パネル（FFmpeg 自動ダウンロード対応） */}
      {showToolsPanel && (
        <section className="tools-panel">
          <div className="tools-panel-header">
            <span className="tools-panel-title">
              外部変換エンジン設定（動画・音声 / 文書変換で使用）
            </span>
            <button
              className="btn btn-secondary btn-sm"
              onClick={() => refreshTools(ffmpegPath, pandocPath)}
              disabled={isCheckingTools || isDownloadingFfmpeg}
              type="button"
            >
              <RefreshIcon
                size={13}
                className={isCheckingTools ? "spin" : undefined}
              />
              <span>{isCheckingTools ? "確認中..." : "再チェック"}</span>
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
                  <span className="tool-role">
                    {tools?.ffmpeg.managed
                      ? "アプリ内蔵管理済み"
                      : "動画・音声変換"}
                  </span>
                </div>
                <div className="tool-card-actions">
                  <button
                    className={`btn btn-sm ${
                      tools?.ffmpeg.available ? "btn-secondary" : "btn-primary"
                    }`}
                    onClick={handleDownloadFfmpeg}
                    disabled={isDownloadingFfmpeg}
                    type="button"
                  >
                    {isDownloadingFfmpeg
                      ? "導入中..."
                      : tools?.ffmpeg.available
                      ? "再ダウンロード"
                      : "自動ダウンロード"}
                  </button>
                  <button
                    className="btn btn-secondary btn-sm"
                    onClick={() => handleSelectToolBinary("ffmpeg")}
                    disabled={isDownloadingFfmpeg}
                    type="button"
                  >
                    パス指定
                  </button>
                  {ffmpegPath && (
                    <button
                      className="btn btn-ghost btn-sm"
                      onClick={() => handleResetToolBinary("ffmpeg")}
                      type="button"
                    >
                      自動
                    </button>
                  )}
                </div>
              </div>

              {ffmpegDlProgress && (
                <div className="dl-progress-box">
                  <div className="dl-progress-text">
                    <span>{ffmpegDlProgress.stage}</span>
                    <span>{ffmpegDlProgress.percent}%</span>
                  </div>
                  <div className="progress-track">
                    <div
                      className="progress-fill"
                      style={{ width: `${ffmpegDlProgress.percent}%` }}
                    />
                  </div>
                </div>
              )}

              <p className="tool-detail">
                {tools?.ffmpeg.available
                  ? tools.ffmpeg.version ?? "検出済み"
                  : "未検出です。「自動ダウンロード」を押すとアプリ専用フォルダに自動導入します。"}
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
                  <span className="tool-role">DOCX / EPUB 変換</span>
                </div>
                <div className="tool-card-actions">
                  <button
                    className="btn btn-secondary btn-sm"
                    onClick={() => handleSelectToolBinary("pandoc")}
                    type="button"
                  >
                    パス指定
                  </button>
                  {pandocPath && (
                    <button
                      className="btn btn-ghost btn-sm"
                      onClick={() => handleResetToolBinary("pandoc")}
                      type="button"
                    >
                      自動
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

      {notice && (
        <div className="notice-banner">
          <div className="notice-left">
            <AlertCircleIcon size={16} />
            <span>{notice}</span>
          </div>
          <button
            className="btn-link"
            onClick={() => setNotice(null)}
            type="button"
          >
            閉じる
          </button>
        </div>
      )}

      {mergeResult && (
        <div className="success-banner">
          <div className="notice-left">
            <CheckCircleIcon size={16} />
            <span>
              PDF 結合が完了しました ({formatBytes(mergeResult.outputSize)} /{" "}
              {mergeResult.durationMs}ms)
            </span>
          </div>
          <div className="toolbar-actions">
            <button
              className="btn-link"
              onClick={() => handleRevealFile(mergeResult.outputPath)}
              type="button"
            >
              <FolderIcon size={13} />
              <span>フォルダを開く</span>
            </button>
            <button
              className="btn-link"
              onClick={() => setMergeResult(null)}
              type="button"
            >
              閉じる
            </button>
          </div>
        </div>
      )}

      <main className="app-main">
        {items.length === 0 ? (
          <section className="welcome-section">
            <div
              className="dropzone-hero"
              onClick={handleSelectFiles}
              role="button"
              tabIndex={0}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") handleSelectFiles();
              }}
            >
              <div className="dropzone-icon-ring">
                <UploadIcon size={26} />
              </div>
              <h2 className="dropzone-title">
                ファイルをここにドラッグ＆ドロップ
              </h2>
              <p className="dropzone-sub">
                またはクリックしてファイルを選択（複数ファイル・異なる形式をまとめて追加可能）
              </p>
              <span className="btn btn-primary dropzone-cta">
                <PlusIcon size={15} />
                <span>ファイルを選択</span>
              </span>
            </div>

            <div className="supported-grid">
              {SUPPORTED_CARDS.map((card) => (
                <div key={card.title} className="supported-card">
                  <div className="supported-card-head">
                    <strong>{card.title}</strong>
                    <span className={`badge ${card.className}`}>
                      {card.badge}
                    </span>
                  </div>
                  <p className="supported-card-formats">{card.formats}</p>
                </div>
              ))}
            </div>
          </section>
        ) : (
          <>
            {/* 出力先 & アクションバー */}
            <section className="toolbar">
              <button
                className="btn btn-secondary"
                onClick={handleSelectFiles}
                disabled={isConvertingAll}
                type="button"
              >
                <PlusIcon size={15} />
                <span>ファイル追加</span>
              </button>

              <div className="output-dir-box">
                <FolderIcon size={15} className="muted-icon" />
                <span
                  className="output-path"
                  title={outputDir || "元のファイルと同じフォルダに保存"}
                >
                  {outputDir || "保存先: 元のファイルと同じフォルダ"}
                </span>
                <button
                  className="btn btn-ghost btn-sm"
                  onClick={handleSelectOutputDir}
                  disabled={isConvertingAll}
                  type="button"
                >
                  変更
                </button>
                {outputDir && (
                  <button
                    className="btn btn-ghost btn-sm"
                    onClick={() => setOutputDir("")}
                    disabled={isConvertingAll}
                    type="button"
                  >
                    戻す
                  </button>
                )}
              </div>

              <div className="toolbar-actions">
                <button
                  className={`btn btn-secondary btn-sm ${
                    showOptions ? "btn-active-outline" : ""
                  }`}
                  onClick={() => setShowOptions((v) => !v)}
                  type="button"
                >
                  画質・サイズ設定
                </button>

                {mergeablePdfInfo && (
                  <button
                    className="btn btn-secondary btn-sm"
                    onClick={handleMergeToPdf}
                    disabled={isMergingPdf || isConvertingAll}
                    type="button"
                  >
                    {isMergingPdf
                      ? "結合中..."
                      : mergeablePdfInfo.kind === "pdf"
                      ? `PDF を結合 (${mergeablePdfInfo.paths.length})`
                      : `画像を1冊のPDFへ (${mergeablePdfInfo.paths.length})`}
                  </button>
                )}

                <button
                  className="btn btn-primary btn-convert-all"
                  onClick={handleConvertAll}
                  disabled={pendingCount === 0 || isConvertingAll}
                  type="button"
                >
                  <PlayIcon size={13} />
                  <span>
                    {isConvertingAll && batchProgress
                      ? `変換中 (${batchProgress.current}/${batchProgress.total})`
                      : `すべて変換 (${pendingCount})`}
                  </span>
                </button>
              </div>
            </section>

            {/* 変換オプションパネル（画像画質・リサイズ・動画プリセット） */}
            {showOptions && (
              <section className="options-panel">
                <div className="option-item">
                  <label htmlFor="opt-quality">
                    画像画質 (JPEG/PDF): <strong>{options.imageQuality}%</strong>
                  </label>
                  <input
                    id="opt-quality"
                    type="range"
                    min={40}
                    max={100}
                    step={5}
                    value={options.imageQuality}
                    onChange={(e) =>
                      setOptions((prev) => ({
                        ...prev,
                        imageQuality: Number(e.target.value),
                      }))
                    }
                  />
                </div>

                <div className="option-item">
                  <label htmlFor="opt-max-width">画像・SVG の最大幅:</label>
                  <select
                    id="opt-max-width"
                    className="format-select format-select-sm"
                    value={options.imageMaxWidth ?? ""}
                    onChange={(e) =>
                      setOptions((prev) => ({
                        ...prev,
                        imageMaxWidth: e.target.value
                          ? Number(e.target.value)
                          : null,
                      }))
                    }
                  >
                    <option value="">変更しない（原寸）</option>
                    <option value="1920">1920px（フルHD）</option>
                    <option value="1280">1280px（Web向け）</option>
                    <option value="800">800px（軽量）</option>
                    <option value="512">512px（アイコン/サムネ）</option>
                  </select>
                </div>

                <div className="option-item">
                  <label htmlFor="opt-video-preset">動画プリセット:</label>
                  <select
                    id="opt-video-preset"
                    className="format-select format-select-sm"
                    value={options.videoPreset}
                    onChange={(e) =>
                      setOptions((prev) => ({
                        ...prev,
                        videoPreset: e.target.value as ConvertOptions["videoPreset"],
                      }))
                    }
                  >
                    <option value="default">標準バランス</option>
                    <option value="compact">SNS向け軽量化（最大720p）</option>
                    <option value="high">高画質</option>
                  </select>
                </div>
              </section>
            )}

            {/* 一括変換プログレスバー */}
            {batchProgress && (
              <div className="batch-progress">
                <div className="batch-progress-label">
                  <span>
                    変換を実行しています... ({batchProgress.current} /{" "}
                    {batchProgress.total})
                  </span>
                  <span>
                    {Math.round(
                      (batchProgress.current / batchProgress.total) * 100
                    )}
                    %
                  </span>
                </div>
                <div className="progress-track">
                  <div
                    className="progress-fill"
                    style={{
                      width: `${Math.round(
                        (batchProgress.current / batchProgress.total) * 100
                      )}%`,
                    }}
                  />
                </div>
              </div>
            )}

            {/* 同カテゴリの一括フォーマット変更バー */}
            {bulkCategoryOptions.length > 0 && (
              <section className="bulk-bar">
                <span className="bulk-bar-label">まとめて形式を変更:</span>
                <div className="bulk-controls">
                  {bulkCategoryOptions.map((group) => (
                    <div key={group.category} className="bulk-group">
                      <span
                        className={`badge ${
                          CATEGORY_LABELS[group.category]?.className ?? ""
                        }`}
                      >
                        {group.label} ({group.count})
                      </span>
                      <select
                        className="format-select format-select-sm"
                        defaultValue=""
                        disabled={isConvertingAll}
                        onChange={(e) => {
                          handleBulkFormatChange(group.category, e.target.value);
                          e.currentTarget.value = "";
                        }}
                      >
                        <option value="" disabled>
                          変換先を選択...
                        </option>
                        {group.targets.map((t) => (
                          <option key={t.id} value={t.id}>
                            すべて {t.label} にする
                          </option>
                        ))}
                      </select>
                    </div>
                  ))}
                </div>
              </section>
            )}

            {/* キュー領域 */}
            <section className="queue-section">
              <div className="queue-subheader">
                <div className="filter-tabs">
                  <button
                    className={`filter-tab ${filterTab === "all" ? "active" : ""}`}
                    onClick={() => setFilterTab("all")}
                    type="button"
                  >
                    すべて <span className="tab-count">{items.length}</span>
                  </button>
                  <button
                    className={`filter-tab ${filterTab === "pending" ? "active" : ""}`}
                    onClick={() => setFilterTab("pending")}
                    type="button"
                  >
                    待機中{" "}
                    <span className="tab-count">
                      {
                        items.filter(
                          (it) =>
                            it.status === "idle" || it.status === "converting"
                        ).length
                      }
                    </span>
                  </button>
                  <button
                    className={`filter-tab ${filterTab === "done" ? "active" : ""}`}
                    onClick={() => setFilterTab("done")}
                    type="button"
                  >
                    完了 <span className="tab-count">{doneCount}</span>
                  </button>
                  {errorCount > 0 && (
                    <button
                      className={`filter-tab tab-error ${
                        filterTab === "error" ? "active" : ""
                      }`}
                      onClick={() => setFilterTab("error")}
                      type="button"
                    >
                      エラー <span className="tab-count">{errorCount}</span>
                    </button>
                  )}
                </div>

                <div className="queue-clear-actions">
                  {doneCount > 0 && (
                    <button
                      className="btn btn-ghost btn-sm"
                      onClick={handleClearCompleted}
                      disabled={isConvertingAll}
                      type="button"
                    >
                      完了をクリア
                    </button>
                  )}
                  <button
                    className="btn btn-ghost btn-sm"
                    onClick={handleClearAll}
                    disabled={isConvertingAll}
                    type="button"
                  >
                    すべてクリア
                  </button>
                </div>
              </div>

              {filteredItems.length === 0 ? (
                <div className="empty-filter-state">
                  この条件に一致するファイルはありません。
                </div>
              ) : (
                <ul className="queue-list">
                  {filteredItems.map((item) => {
                    const badge =
                      CATEGORY_LABELS[item.file.category] ??
                      CATEGORY_LABELS.unsupported;
                    const delta =
                      item.status === "done" && item.result
                        ? formatSizeDelta(item.file.size, item.result.outputSize)
                        : null;

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

                          {item.status === "converting" && (
                            <div className="item-converting">
                              <span className="mini-spinner" />
                              <span>変換処理を実行中...</span>
                            </div>
                          )}

                          {item.status === "done" && item.result && (
                            <div className="item-result">
                              <span className="result-ok">
                                <CheckCircleIcon size={14} />
                                <span>
                                  完了 · {formatBytes(item.result.outputSize)}
                                  {delta && (
                                    <span className="size-delta">
                                      ({delta})
                                    </span>
                                  )}
                                  {" · "}
                                  {item.result.durationMs}ms
                                </span>
                              </span>
                              <button
                                className="btn-link btn-reveal"
                                onClick={() =>
                                  handleRevealFile(item.result!.outputPath)
                                }
                                type="button"
                              >
                                <FolderIcon size={13} />
                                <span>フォルダを開く</span>
                              </button>
                            </div>
                          )}

                          {item.status === "error" && item.error && (
                            <div className="item-error">
                              <AlertCircleIcon size={14} />
                              <span>{item.error}</span>
                            </div>
                          )}
                        </div>

                        <div className="item-controls">
                          {item.file.availableTargets.length > 0 ? (
                            <>
                              <ArrowRightIcon size={15} className="arrow" />
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
                                    {t.label} — {t.description}
                                  </option>
                                ))}
                              </select>
                              <button
                                className="btn btn-secondary btn-sm"
                                onClick={() => convertSingleItem(item)}
                                disabled={
                                  item.status === "converting" ||
                                  isConvertingAll
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
                            <span className="unsupported-text">未対応形式</span>
                          )}

                          <button
                            className="btn-icon-remove"
                            onClick={() => handleRemoveItem(item.id)}
                            disabled={item.status === "converting"}
                            title="リストから削除"
                            type="button"
                          >
                            <TrashIcon size={15} />
                          </button>
                        </div>
                      </li>
                    );
                  })}
                </ul>
              )}
            </section>
          </>
        )}
      </main>
    </div>
  );
}

export default App;
