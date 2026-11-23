mod converter;

use converter::tools::ToolsOverview;
use converter::{ConvertRequest, ConvertResult, FileInspectResult, MergePdfRequest};
use std::collections::HashMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use tauri::{Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

#[derive(Default)]
struct ConversionTasks(Mutex<HashMap<String, Arc<AtomicBool>>>);

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ConversionProgress {
    task_id: String,
    percent: f64,
}

#[tauri::command]
fn cancel_conversion(
    tasks: tauri::State<'_, ConversionTasks>,
    task_id: String,
) -> Result<(), String> {
    let tasks = tasks.0.lock().map_err(|e| e.to_string())?;
    if let Some(cancelled) = tasks.get(&task_id) {
        cancelled.store(true, Ordering::SeqCst);
    }
    Ok(())
}

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
    task_id: String,
) -> Result<ConvertResult, String> {
    let app_data_dir = app.path().app_data_dir().ok();
    let cancelled = Arc::new(AtomicBool::new(false));
    app.state::<ConversionTasks>()
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .insert(task_id.clone(), cancelled.clone());
    let emit_app = app.clone();
    let emit_id = task_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let control = converter::media_conv::ConversionControl {
            cancelled,
            progress: Box::new(move |percent| {
                let _ = emit_app.emit(
                    "conversion-progress",
                    ConversionProgress {
                        task_id: emit_id.clone(),
                        percent,
                    },
                );
            }),
        };
        converter::convert_file_with_control(&request, app_data_dir.as_deref(), Some(&control))
            .map_err(|e| e.to_string())
    })
    .await;
    app.state::<ConversionTasks>()
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&task_id);
    result.map_err(|e| format!("変換タスクの実行に失敗しました: {e}"))?
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
        .manage(ConversionTasks::default())
        .invoke_handler(tauri::generate_handler![
            inspect_files,
            convert_file,
            cancel_conversion,
            merge_to_pdf,
            check_external_tools,
            download_ffmpeg,
            reveal_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
