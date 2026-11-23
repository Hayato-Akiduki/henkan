pub mod archive_conv;
pub mod data_conv;
pub mod doc_conv;
pub mod excel_conv;
pub mod image_conv;
pub mod media_conv;
pub mod pdf_conv;
pub mod svg_conv;
pub mod tools;

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConvertError {
    #[error("ファイルが見つかりません: {0}")]
    FileNotFound(String),
    #[error("未対応の入力形式です: {0}")]
    UnsupportedInput(String),
    #[error("未対応の出力形式です: {0}")]
    UnsupportedTarget(String),
    #[error("入出力エラー: {0}")]
    Io(#[from] std::io::Error),
    #[error("変換処理に失敗しました: {0}")]
    ConversionFailed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetFormat {
    pub id: String,
    pub label: String,
    pub extension: String,
    pub category: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInspectResult {
    pub path: String,
    pub file_name: String,
    pub extension: String,
    pub size: u64,
    pub category: String,
    pub available_targets: Vec<TargetFormat>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertOptions {
    pub image_quality: Option<u8>,
    pub image_max_width: Option<u32>,
    pub video_preset: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertRequest {
    pub input_path: String,
    pub target_format: String,
    pub output_dir: Option<String>,
    pub ffmpeg_path: Option<String>,
    pub pandoc_path: Option<String>,
    pub options: Option<ConvertOptions>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergePdfRequest {
    pub input_paths: Vec<String>,
    pub output_dir: Option<String>,
    pub image_quality: Option<u8>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertResult {
    pub input_path: String,
    pub output_path: String,
    pub output_size: u64,
    pub duration_ms: u128,
}

/// ファイルパスからカテゴリと変換候補一覧を調べる
pub fn inspect_file(path_str: &str) -> Result<FileInspectResult, ConvertError> {
    let path = Path::new(path_str);
    if !path.exists() {
        return Err(ConvertError::FileNotFound(path_str.to_string()));
    }

    let metadata = fs::metadata(path)?;
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path_str.to_string());

    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    let (category, available_targets) = detect_category_and_targets(&ext);

    Ok(FileInspectResult {
        path: path_str.to_string(),
        file_name,
        extension: ext,
        size: metadata.len(),
        category,
        available_targets,
    })
}

fn detect_category_and_targets(ext: &str) -> (String, Vec<TargetFormat>) {
    if svg_conv::is_supported_extension(ext) {
        return ("image".to_string(), svg_conv::available_targets());
    }
    if image_conv::is_supported_extension(ext) {
        return ("image".to_string(), image_conv::available_targets(ext));
    }
    if media_conv::is_video_extension(ext) {
        return ("video".to_string(), media_conv::video_targets(ext));
    }
    if media_conv::is_audio_extension(ext) {
        return ("audio".to_string(), media_conv::audio_targets(ext));
    }
    if excel_conv::is_supported_extension(ext) {
        return ("excel".to_string(), excel_conv::available_targets());
    }
    if archive_conv::is_supported_extension(ext) {
        return ("archive".to_string(), archive_conv::available_targets(ext));
    }
    if pdf_conv::is_supported_extension(ext) {
        return ("document".to_string(), pdf_conv::available_targets());
    }
    if doc_conv::is_supported_extension(ext) {
        let mut targets = doc_conv::available_targets(ext);
        if matches!(
            ext,
            "md" | "markdown" | "html" | "htm" | "rst" | "tex" | "org"
        ) {
            targets.extend(data_conv::encoding_targets());
        }
        return ("document".to_string(), targets);
    }
    if data_conv::is_structured_data_extension(ext) {
        let mut targets = data_conv::structured_targets(ext);
        targets.extend(data_conv::encoding_targets());
        return ("data".to_string(), targets);
    }
    if data_conv::is_text_extension(ext) {
        return ("text".to_string(), data_conv::encoding_targets());
    }

    ("unsupported".to_string(), vec![])
}

/// 出力ファイルまたは出力フォルダのパスを決定する（同名が存在する場合は連番を付けて上書きを防ぐ）
pub fn resolve_output_path(
    input_path: &Path,
    output_dir: Option<&str>,
    target_ext: &str,
    suffix: Option<&str>,
) -> Result<PathBuf, ConvertError> {
    let base_dir = match output_dir {
        Some(dir) if !dir.trim().is_empty() => {
            let p = PathBuf::from(dir);
            if !p.exists() {
                fs::create_dir_all(&p)?;
            }
            p
        }
        _ => input_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(".")),
    };

    let stem = input_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".to_string());

    let stem_with_suffix = match suffix {
        Some(s) if !s.is_empty() => format!("{stem}_{s}"),
        _ => stem,
    };

    let build_name = |idx: Option<u32>| -> String {
        let numbered = match idx {
            Some(i) => format!("{stem_with_suffix}_{i}"),
            None => stem_with_suffix.clone(),
        };
        if target_ext.is_empty() {
            numbered
        } else {
            format!("{numbered}.{target_ext}")
        }
    };

    let first_candidate = base_dir.join(build_name(None));
    if !first_candidate.exists() {
        return Ok(first_candidate);
    }

    for i in 1..=9999 {
        let candidate = base_dir.join(build_name(Some(i)));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(ConvertError::ConversionFailed(
        "出力パスの生成に失敗しました".to_string(),
    ))
}

/// 出力フォルダ配下の総サイズ（ディレクトリ出力用）
fn calc_path_size(path: &Path) -> u64 {
    if path.is_file() {
        return fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    }
    if path.is_dir() {
        let mut total = 0;
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                total += calc_path_size(&entry.path());
            }
        }
        return total;
    }
    0
}

/// 変換を実行する
#[cfg(test)]
pub fn convert_file(
    req: &ConvertRequest,
    app_data_dir: Option<&Path>,
) -> Result<ConvertResult, ConvertError> {
    convert_file_with_control(req, app_data_dir, None)
}

pub fn convert_file_with_control(
    req: &ConvertRequest,
    app_data_dir: Option<&Path>,
    control: Option<&media_conv::ConversionControl>,
) -> Result<ConvertResult, ConvertError> {
    let start = Instant::now();
    let input_path = Path::new(&req.input_path);
    if !input_path.exists() {
        return Err(ConvertError::FileNotFound(req.input_path.clone()));
    }

    let input_ext = input_path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    let output_path = if req.target_format.starts_with("enc:") {
        let encoding_name = req.target_format.trim_start_matches("enc:");
        let ext = if input_ext.is_empty() {
            "txt"
        } else {
            &input_ext
        };
        let out = resolve_output_path(
            input_path,
            req.output_dir.as_deref(),
            ext,
            Some(encoding_name),
        )?;
        data_conv::convert_encoding(input_path, &out, encoding_name)?;
        out
    } else if svg_conv::is_supported_extension(&input_ext) {
        let out = resolve_output_path(
            input_path,
            req.output_dir.as_deref(),
            &req.target_format,
            None,
        )?;
        svg_conv::convert_svg(input_path, &out, &req.target_format, req.options.as_ref())?;
        out
    } else if image_conv::is_supported_extension(&input_ext) {
        let out = resolve_output_path(
            input_path,
            req.output_dir.as_deref(),
            &req.target_format,
            None,
        )?;
        image_conv::convert_image(input_path, &out, &req.target_format, req.options.as_ref())?;
        out
    } else if media_conv::is_video_extension(&input_ext)
        || media_conv::is_audio_extension(&input_ext)
    {
        let out = resolve_output_path(
            input_path,
            req.output_dir.as_deref(),
            &req.target_format,
            None,
        )?;
        media_conv::convert_media_with_control(
            input_path,
            &out,
            &req.target_format,
            req.ffmpeg_path.as_deref(),
            app_data_dir,
            req.options.as_ref(),
            control,
        )?;
        out
    } else if excel_conv::is_supported_extension(&input_ext) {
        let out = resolve_output_path(
            input_path,
            req.output_dir.as_deref(),
            &req.target_format,
            None,
        )?;
        excel_conv::convert_excel(input_path, &out, &req.target_format)?;
        out
    } else if archive_conv::is_supported_extension(&input_ext) {
        let out = if req.target_format == "extract" {
            resolve_output_path(input_path, req.output_dir.as_deref(), "", Some("extracted"))?
        } else {
            resolve_output_path(
                input_path,
                req.output_dir.as_deref(),
                &req.target_format,
                None,
            )?
        };
        archive_conv::convert_archive(input_path, &out, &input_ext, &req.target_format)?;
        out
    } else if pdf_conv::is_supported_extension(&input_ext) {
        let out = if req.target_format == "split-pdf" {
            resolve_output_path(input_path, req.output_dir.as_deref(), "", Some("pages"))?
        } else {
            resolve_output_path(
                input_path,
                req.output_dir.as_deref(),
                &req.target_format,
                None,
            )?
        };
        pdf_conv::convert_pdf(input_path, &out, &req.target_format)?;
        out
    } else if doc_conv::is_supported_extension(&input_ext) {
        let out = resolve_output_path(
            input_path,
            req.output_dir.as_deref(),
            &req.target_format,
            None,
        )?;
        doc_conv::convert_document(
            input_path,
            &out,
            &input_ext,
            &req.target_format,
            req.pandoc_path.as_deref(),
        )?;
        out
    } else if data_conv::is_structured_data_extension(&input_ext) {
        let out = resolve_output_path(
            input_path,
            req.output_dir.as_deref(),
            &req.target_format,
            None,
        )?;
        data_conv::convert_structured_data(input_path, &out, &input_ext, &req.target_format)?;
        out
    } else {
        return Err(ConvertError::UnsupportedInput(input_ext));
    };

    let output_size = calc_path_size(&output_path);
    let duration_ms = start.elapsed().as_millis();

    Ok(ConvertResult {
        input_path: req.input_path.clone(),
        output_path: output_path.to_string_lossy().to_string(),
        output_size,
        duration_ms,
    })
}

/// 複数の画像 または 複数の PDF を 1 冊の PDF に結合する
pub fn merge_files_to_pdf(req: &MergePdfRequest) -> Result<ConvertResult, ConvertError> {
    let start = Instant::now();
    if req.input_paths.len() < 2 {
        return Err(ConvertError::ConversionFailed(
            "PDF にまとめるには 2 つ以上のファイルが必要です".to_string(),
        ));
    }

    let first = Path::new(&req.input_paths[0]);
    let out_path = resolve_output_path(first, req.output_dir.as_deref(), "pdf", Some("merged"))?;

    let all_pdf = req.input_paths.iter().all(|p| {
        Path::new(p)
            .extension()
            .map(|e| e.to_string_lossy().eq_ignore_ascii_case("pdf"))
            .unwrap_or(false)
    });

    if all_pdf {
        let paths: Vec<PathBuf> = req.input_paths.iter().map(PathBuf::from).collect();
        pdf_conv::merge_pdfs(&paths, &out_path)?;
    } else {
        // 画像一覧を 1 冊の PDF にまとめる
        let mut images = Vec::new();
        for p in &req.input_paths {
            let img = image::open(p).map_err(|e| {
                ConvertError::ConversionFailed(format!("画像の読み込みに失敗 ({p}): {e}"))
            })?;
            images.push(img);
        }
        let quality = req.image_quality.unwrap_or(85);
        pdf_conv::images_to_pdf(&images, &out_path, quality)?;
    }

    let output_size = calc_path_size(&out_path);
    let duration_ms = start.elapsed().as_millis();

    Ok(ConvertResult {
        input_path: req.input_paths[0].clone(),
        output_path: out_path.to_string_lossy().to_string(),
        output_size,
        duration_ms,
    })
}
