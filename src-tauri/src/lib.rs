mod converter;

use converter::tools::ToolsOverview;
use converter::{ConvertRequest, ConvertResult, FileInspectResult, MergePdfRequest};
use tauri::{Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
fn inspect_files(paths: Vec<String>) -> Vec<Result<FileInspectResult, String>> {
    paths
        .iter()
        .map(|p| converter::inspect_file(p).map_err(|e| e.to_string()))
        .collect()
}

#[tauri::command]
async fn convert_file(
    app: tauri::AppHandle,
    request: ConvertRequest,
) -> Result<ConvertResult, String> {
    let app_data_dir = app.path().app_data_dir().ok();
    tauri::async_runtime::spawn_blocking(move || {
        converter::convert_file(&request, app_data_dir.as_deref()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("変換タスクの実行に失敗しました: {e}"))?
}

#[tauri::command]
async fn merge_to_pdf(request: MergePdfRequest) -> Result<ConvertResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        converter::merge_files_to_pdf(&request).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("PDF 結合タスクの実行に失敗しました: {e}"))?
}

#[tauri::command]
async fn check_external_tools(
    app: tauri::AppHandle,
    ffmpeg_path: Option<String>,
    pandoc_path: Option<String>,
) -> Result<ToolsOverview, String> {
    let app_data_dir = app.path().app_data_dir().ok();
    tauri::async_runtime::spawn_blocking(move || {
        converter::tools::check_all_tools(
            ffmpeg_path.as_deref(),
            pandoc_path.as_deref(),
            app_data_dir.as_deref(),
        )
    })
    .await
    .map_err(|e| format!("外部ツールの確認に失敗しました: {e}"))
}

#[tauri::command]
async fn download_ffmpeg(app: tauri::AppHandle) -> Result<String, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("アプリデータフォルダの取得に失敗しました: {e}"))?;

    let app_for_emit = app.clone();
    let bin_path = converter::tools::download_ffmpeg_to_dir(&app_data_dir, move |progress| {
        let _ = app_for_emit.emit("ffmpeg-download-progress", progress);
    })
    .await?;

    Ok(bin_path.to_string_lossy().to_string())
}

#[tauri::command]
fn reveal_file(app: tauri::AppHandle, path: String) -> Result<(), String> {
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|e| format!("フォルダを開けませんでした: {e}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init());

    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());

    builder
        .invoke_handler(tauri::generate_handler![
            inspect_files,
            convert_file,
            merge_to_pdf,
            check_external_tools,
            download_ffmpeg,
            reveal_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
