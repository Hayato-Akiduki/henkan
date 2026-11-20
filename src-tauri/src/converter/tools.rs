use futures_util::StreamExt;
use serde::Serialize;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use zip::ZipArchive;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    pub available: bool,
    pub command: String,
    pub version: Option<String>,
    pub managed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsOverview {
    pub ffmpeg: ToolStatus,
    pub pandoc: ToolStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgressEvent {
    pub stage: String,
    pub percent: u8,
}

/// Windows でコンソールウィンドウが一瞬表示されるのを防ぐ Command 生成ヘルパー
pub fn new_hidden_command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

pub fn managed_ffmpeg_binary_path(app_data_dir: &Path) -> PathBuf {
    let bin_name = if cfg!(target_os = "windows") {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    app_data_dir.join("tools").join("ffmpeg").join(bin_name)
}

pub fn resolve_program(custom_path: Option<&str>, default_name: &str) -> String {
    match custom_path {
        Some(p) if !p.trim().is_empty() => p.trim().to_string(),
        _ => default_name.to_string(),
    }
}

/// FFmpeg の解決優先順位: 1. ユーザー手動指定 -> 2. アプリ管理ダウンロード済み -> 3. システム PATH
pub fn resolve_ffmpeg_program(custom_path: Option<&str>, app_data_dir: Option<&Path>) -> String {
    if let Some(p) = custom_path {
        if !p.trim().is_empty() {
            return p.trim().to_string();
        }
    }
    if let Some(dir) = app_data_dir {
        let managed = managed_ffmpeg_binary_path(dir);
        if managed.exists() {
            return managed.to_string_lossy().to_string();
        }
    }
    "ffmpeg".to_string()
}

pub fn detect_tool(custom_path: Option<&str>, default_name: &str, version_arg: &str) -> ToolStatus {
    let program = resolve_program(custom_path, default_name);
    probe_program(&program, version_arg, false)
}

fn probe_program(program: &str, version_arg: &str, managed: bool) -> ToolStatus {
    let output = new_hidden_command(program).arg(version_arg).output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let first_line = stdout
                .lines()
                .next()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            ToolStatus {
                available: true,
                command: program.to_string(),
                version: first_line,
                managed,
            }
        }
        _ => ToolStatus {
            available: false,
            command: program.to_string(),
            version: None,
            managed,
        },
    }
}

pub fn check_all_tools(
    ffmpeg_path: Option<&str>,
    pandoc_path: Option<&str>,
    app_data_dir: Option<&Path>,
) -> ToolsOverview {
    let managed_path = app_data_dir.map(managed_ffmpeg_binary_path);
    let using_managed = ffmpeg_path.map(|s| s.trim().is_empty()).unwrap_or(true)
        && managed_path.as_ref().map(|p| p.exists()).unwrap_or(false);

    let ffmpeg_prog = resolve_ffmpeg_program(ffmpeg_path, app_data_dir);
    let ffmpeg = probe_program(&ffmpeg_prog, "-version", using_managed);
    let pandoc = detect_tool(pandoc_path, "pandoc", "--version");

    ToolsOverview { ffmpeg, pandoc }
}

/// OS に応じた FFmpeg 配布 ZIP をダウンロードしてアプリ専用フォルダへ展開する
pub async fn download_ffmpeg_to_dir<F>(app_data_dir: &Path, mut on_progress: F) -> Result<PathBuf, String>
where
    F: FnMut(DownloadProgressEvent),
{
    let download_url = if cfg!(target_os = "windows") {
        "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip"
    } else if cfg!(target_os = "macos") {
        "https://evermeet.cx/ffmpeg/getrelease/zip"
    } else {
        return Err(
            "Linux 環境ではパッケージマネージャ（例: sudo apt install ffmpeg）での導入をおすすめします。"
                .to_string(),
        );
    };

    let target_bin = managed_ffmpeg_binary_path(app_data_dir);
    let target_dir = target_bin
        .parent()
        .ok_or_else(|| "保存先ディレクトリの解決に失敗しました".to_string())?;
    fs::create_dir_all(target_dir).map_err(|e| format!("フォルダ作成に失敗しました: {e}"))?;

    on_progress(DownloadProgressEvent {
        stage: "FFmpeg をダウンロード中...".to_string(),
        percent: 2,
    });

    let client = reqwest::Client::builder()
        .user_agent("Henkan-File-Converter/0.1")
        .build()
        .map_err(|e| format!("HTTP クライアント初期化に失敗しました: {e}"))?;

    let response = client
        .get(download_url)
        .send()
        .await
        .map_err(|e| format!("ダウンロードに接続できませんでした: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "ダウンロードサーバーがエラーを返しました (HTTP {})",
            response.status()
        ));
    }

    let total_size = response.content_length().unwrap_or(0);
    let tmp_zip = target_dir.join("ffmpeg_download.zip");
    let mut zip_file = File::create(&tmp_zip)
        .map_err(|e| format!("一時ファイルの作成に失敗しました: {e}"))?;

    let mut downloaded: u64 = 0;
    let mut last_percent: u8 = 2;
    let mut stream = response.bytes_stream();

    while let Some(chunk_res) = stream.next().await {
        let chunk = chunk_res.map_err(|e| format!("ダウンロード中に通信が途切れました: {e}"))?;
        zip_file
            .write_all(&chunk)
            .map_err(|e| format!("ファイル書き込みに失敗しました: {e}"))?;
        downloaded += chunk.len() as u64;

        if total_size > 0 {
            let pct = (2 + ((downloaded as f64 / total_size as f64) * 86.0) as u8).min(88);
            if pct > last_percent {
                last_percent = pct;
                on_progress(DownloadProgressEvent {
                    stage: format!(
                        "FFmpeg をダウンロード中... ({:.1} MB / {:.1} MB)",
                        downloaded as f64 / 1_048_576.0,
                        total_size as f64 / 1_048_576.0
                    ),
                    percent: pct,
                });
            }
        }
    }
    drop(zip_file);

    on_progress(DownloadProgressEvent {
        stage: "FFmpeg を展開中...".to_string(),
        percent: 92,
    });

    // ZIP 内から ffmpeg(.exe) を抽出
    let extract_result = (|| -> Result<(), String> {
        let file = File::open(&tmp_zip).map_err(|e| format!("ZIP を開けませんでした: {e}"))?;
        let mut archive =
            ZipArchive::new(file).map_err(|e| format!("ZIP の解析に失敗しました: {e}"))?;

        let target_name = if cfg!(target_os = "windows") {
            "ffmpeg.exe"
        } else {
            "ffmpeg"
        };

        let mut found = false;
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| format!("ZIP エントリの読み込みに失敗しました: {e}"))?;
            if entry.is_dir() {
                continue;
            }
            let entry_name = entry.name().replace('\\', "/");
            let file_name = entry_name.rsplit('/').next().unwrap_or("");
            if file_name.eq_ignore_ascii_case(target_name) {
                let mut out_file = File::create(&target_bin)
                    .map_err(|e| format!("FFmpeg の書き出しに失敗しました: {e}"))?;
                io::copy(&mut entry, &mut out_file)
                    .map_err(|e| format!("FFmpeg の展開に失敗しました: {e}"))?;
                found = true;
                break;
            }
        }

        if !found {
            return Err("ZIP アーカイブ内に FFmpeg 実行ファイルが見つかりませんでした".to_string());
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&target_bin)
                .map_err(|e| format!("権限取得に失敗しました: {e}"))?
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&target_bin, perms)
                .map_err(|e| format!("実行権限の付与に失敗しました: {e}"))?;
        }

        Ok(())
    })();

    let _ = fs::remove_file(&tmp_zip);
    extract_result?;

    on_progress(DownloadProgressEvent {
        stage: "FFmpeg のセットアップが完了しました！".to_string(),
        percent: 100,
    });

    Ok(target_bin)
}
