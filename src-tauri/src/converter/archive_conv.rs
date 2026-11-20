use super::{ConvertError, TargetFormat};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::{self, File};
use std::io;
use std::path::{Component, Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::ZipArchive;

const ARCHIVE_EXTENSIONS: &[&str] = &["zip", "tar", "gz", "tgz", "7z"];

pub fn is_supported_extension(ext: &str) -> bool {
    ARCHIVE_EXTENSIONS.contains(&ext)
}

pub fn available_targets(input_ext: &str) -> Vec<TargetFormat> {
    let mut targets = Vec::new();
    if input_ext != "zip" {
        targets.push(TargetFormat {
            id: "zip".to_string(),
            label: "ZIP".to_string(),
            extension: "zip".to_string(),
            category: "archive".to_string(),
            description: "標準 ZIP 形式に変換（内蔵）".to_string(),
        });
    }
    if input_ext != "gz" && input_ext != "tgz" {
        targets.push(TargetFormat {
            id: "tar.gz".to_string(),
            label: "TAR.GZ".to_string(),
            extension: "tar.gz".to_string(),
            category: "archive".to_string(),
            description: "Linux / macOS 向け圧縮形式に変換（内蔵）".to_string(),
        });
    }
    targets.push(TargetFormat {
        id: "extract".to_string(),
        label: "フォルダに解凍".to_string(),
        extension: "".to_string(),
        category: "archive".to_string(),
        description: "アーカイブの中身をフォルダに展開（内蔵）".to_string(),
    });
    targets
}

/// パストラバーサル（Zip Slip）を防ぐ安全な相対パス結合
fn safe_join(base: &Path, rel: &Path) -> Option<PathBuf> {
    let mut out = base.to_path_buf();
    for comp in rel.components() {
        match comp {
            Component::Normal(c) => out.push(c),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(out)
}

fn extract_to_dir(input_path: &Path, input_ext: &str, dest_dir: &Path) -> Result<(), ConvertError> {
    fs::create_dir_all(dest_dir)?;

    match input_ext {
        "zip" => {
            let file = File::open(input_path)?;
            let mut archive = ZipArchive::new(file).map_err(|e| {
                ConvertError::ConversionFailed(format!("ZIP の読み込みに失敗: {e}"))
            })?;
            for i in 0..archive.len() {
                let mut entry = archive.by_index(i).map_err(|e| {
                    ConvertError::ConversionFailed(format!("ZIP エントリの読み込みに失敗: {e}"))
                })?;
                let Some(enclosed) = entry.enclosed_name() else {
                    continue;
                };
                let Some(out_path) = safe_join(dest_dir, &enclosed) else {
                    continue;
                };
                if entry.is_dir() {
                    fs::create_dir_all(&out_path)?;
                } else {
                    if let Some(parent) = out_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    let mut outfile = File::create(&out_path)?;
                    io::copy(&mut entry, &mut outfile)?;
                }
            }
            Ok(())
        }
        "tar" => {
            let file = File::open(input_path)?;
            extract_tar_reader(file, dest_dir)
        }
        "tgz" | "gz" => {
            let name_lower = input_path
                .file_name()
                .map(|s| s.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if input_ext == "tgz" || name_lower.ends_with(".tar.gz") {
                let file = File::open(input_path)?;
                let gz = GzDecoder::new(file);
                extract_tar_reader(gz, dest_dir)
            } else {
                // 単体 .gz ファイルの展開
                let file = File::open(input_path)?;
                let mut gz = GzDecoder::new(file);
                let out_name = input_path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "extracted".to_string());
                let mut out_file = File::create(dest_dir.join(out_name))?;
                io::copy(&mut gz, &mut out_file)?;
                Ok(())
            }
        }
        "7z" => {
            sevenz_rust::decompress_file(input_path, dest_dir).map_err(|e| {
                ConvertError::ConversionFailed(format!("7z の展開に失敗しました: {e}"))
            })?;
            Ok(())
        }
        other => Err(ConvertError::UnsupportedInput(other.to_string())),
    }
}

fn extract_tar_reader<R: io::Read>(reader: R, dest_dir: &Path) -> Result<(), ConvertError> {
    let mut archive = tar::Archive::new(reader);
    let entries = archive
        .entries()
        .map_err(|e| ConvertError::ConversionFailed(format!("TAR の読み込みに失敗: {e}")))?;

    for entry_res in entries {
        let mut entry = entry_res
            .map_err(|e| ConvertError::ConversionFailed(format!("TAR エントリの読み込みに失敗: {e}")))?;
        let path = entry
            .path()
            .map_err(|e| ConvertError::ConversionFailed(format!("TAR パス解析に失敗: {e}")))?
            .into_owned();
        let Some(out_path) = safe_join(dest_dir, &path) else {
            continue;
        };
        if entry.header().entry_type().is_dir() {
            fs::create_dir_all(&out_path)?;
        } else if entry.header().entry_type().is_file() {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut outfile = File::create(&out_path)?;
            io::copy(&mut entry, &mut outfile)?;
        }
    }
    Ok(())
}

fn collect_files_recursive(root: &Path, current: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(root, &path, out)?;
        } else if path.is_file() {
            if let Ok(rel) = path.strip_prefix(root) {
                out.push(rel.to_path_buf());
            }
        }
    }
    Ok(())
}

fn pack_dir_to_zip(src_dir: &Path, output_path: &Path) -> Result<(), ConvertError> {
    let file = File::create(output_path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let mut rel_files = Vec::new();
    collect_files_recursive(src_dir, src_dir, &mut rel_files)?;

    for rel in rel_files {
        let name = rel.to_string_lossy().replace('\\', "/");
        zip.start_file(name, options)
            .map_err(|e| ConvertError::ConversionFailed(format!("ZIP 作成に失敗: {e}")))?;
        let mut f = File::open(src_dir.join(&rel))?;
        io::copy(&mut f, &mut zip)?;
    }

    zip.finish()
        .map_err(|e| ConvertError::ConversionFailed(format!("ZIP 完了処理に失敗: {e}")))?;
    Ok(())
}

fn pack_dir_to_targz(src_dir: &Path, output_path: &Path) -> Result<(), ConvertError> {
    let file = File::create(output_path)?;
    let enc = GzEncoder::new(file, Compression::default());
    let mut tar_builder = tar::Builder::new(enc);

    let mut rel_files = Vec::new();
    collect_files_recursive(src_dir, src_dir, &mut rel_files)?;

    for rel in rel_files {
        let full = src_dir.join(&rel);
        let mut f = File::open(&full)?;
        tar_builder
            .append_file(&rel, &mut f)
            .map_err(|e| ConvertError::ConversionFailed(format!("TAR.GZ 追加に失敗: {e}")))?;
    }

    tar_builder
        .finish()
        .map_err(|e| ConvertError::ConversionFailed(format!("TAR.GZ 完了処理に失敗: {e}")))?;
    Ok(())
}

pub fn convert_archive(
    input_path: &Path,
    output_path: &Path,
    input_ext: &str,
    target_format: &str,
) -> Result<(), ConvertError> {
    if target_format == "extract" {
        return extract_to_dir(input_path, input_ext, output_path);
    }

    let tmp_dir = std::env::temp_dir().join(format!(
        "henkan_arc_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));

    let res = (|| {
        extract_to_dir(input_path, input_ext, &tmp_dir)?;
        match target_format {
            "zip" => pack_dir_to_zip(&tmp_dir, output_path),
            "tar.gz" => pack_dir_to_targz(&tmp_dir, output_path),
            other => Err(ConvertError::UnsupportedTarget(other.to_string())),
        }
    })();

    let _ = fs::remove_dir_all(&tmp_dir);
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zip_to_targz_and_extract() {
        let dir = std::env::temp_dir().join("henkan_test_archive");
        let _ = fs::remove_dir_all(&dir);
        let src_folder = dir.join("src");
        fs::create_dir_all(&src_folder).unwrap();
        fs::write(src_folder.join("hello.txt"), "Henkan Archive Test").unwrap();

        let zip_path = dir.join("sample.zip");
        let targz_path = dir.join("sample.tar.gz");
        let extracted_dir = dir.join("out");

        pack_dir_to_zip(&src_folder, &zip_path).unwrap();
        convert_archive(&zip_path, &targz_path, "zip", "tar.gz").unwrap();
        assert!(targz_path.exists());

        convert_archive(&targz_path, &extracted_dir, "gz", "extract").unwrap();
        let content = fs::read_to_string(extracted_dir.join("hello.txt")).unwrap();
        assert_eq!(content, "Henkan Archive Test");

        let _ = fs::remove_dir_all(&dir);
    }
}
