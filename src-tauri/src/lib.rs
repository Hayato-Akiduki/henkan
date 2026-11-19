mod converter;

use converter::tools::ToolsOverview;
use converter::{ConvertRequest, ConvertResult, FileInspectResult};
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
fn inspect_files(paths: Vec<String>) -> Vec<Result<FileInspectResult, String>> {
    paths
        .iter()
        .map(|p| converter::inspect_file(p).map_err(|e| e.to_string()))
        .collect()
}

#[tauri::command]
async fn convert_file(request: ConvertRequest) -> Result<ConvertResult, String> {
    // 重い変換処理で UI スレッドを止めないようブロッキングスレッドプールで実行
    tauri::async_runtime::spawn_blocking(move || {
        converter::convert_file(&request).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("変換タスクの実行に失敗しました: {e}"))?
}

#[tauri::command]
async fn check_external_tools(
    ffmpeg_path: Option<String>,
    pandoc_path: Option<String>,
) -> Result<ToolsOverview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        converter::tools::check_all_tools(ffmpeg_path.as_deref(), pandoc_path.as_deref())
    })
    .await
    .map_err(|e| format!("外部ツールの確認に失敗しました: {e}"))
}

#[tauri::command]
fn reveal_file(app: tauri::AppHandle, path: String) -> Result<(), String> {
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|e| format!("フォルダを開けませんでした: {e}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            inspect_files,
            convert_file,
            check_external_tools,
            reveal_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
