use super::image_conv;
use super::{ConvertError, TargetFormat};
use docx_rs::{Docx, Paragraph, Run};
use image::codecs::jpeg::JpegEncoder;
use image::DynamicImage;
use lopdf::{dictionary, Document, Object, ObjectId, Stream};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

pub fn is_supported_extension(ext: &str) -> bool {
    ext == "pdf"
}

pub fn available_targets() -> Vec<TargetFormat> {
    vec![
        TargetFormat {
            id: "docx".to_string(),
            label: "Word (DOCX)".to_string(),
            extension: "docx".to_string(),
            category: "document".to_string(),
            description: "テキスト抽出して Word 化（内蔵エンジン）".to_string(),
        },
        TargetFormat {
            id: "txt".to_string(),
            label: "プレーンテキスト".to_string(),
            extension: "txt".to_string(),
            category: "document".to_string(),
            description: "PDF からテキストを抽出（内蔵エンジン）".to_string(),
        },
        TargetFormat {
            id: "md".to_string(),
            label: "Markdown".to_string(),
            extension: "md".to_string(),
            category: "document".to_string(),
            description: "PDF からテキストを抽出して Markdown 化（内蔵）".to_string(),
        },
        TargetFormat {
            id: "html".to_string(),
            label: "HTML".to_string(),
            extension: "html".to_string(),
            category: "document".to_string(),
            description: "PDF のテキストを HTML 化（内蔵エンジン）".to_string(),
        },
        TargetFormat {
            id: "split-pdf".to_string(),
            label: "ページ分割（PDF）".to_string(),
            extension: "".to_string(),
            category: "document".to_string(),
            description: "1ページずつ個別の PDF に分割（内蔵）".to_string(),
        },
    ]
}

fn extract_pdf_text(input_path: &Path) -> Result<String, ConvertError> {
    let bytes = fs::read(input_path)?;
    let extracted = std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem(&bytes))
        .map_err(|_| {
            ConvertError::ConversionFailed(
                "PDF の解析中にエラーが発生しました（暗号化または特殊な形式の可能性があります）"
                    .to_string(),
            )
        })?
        .map_err(|e| {
            ConvertError::ConversionFailed(format!("PDF からのテキスト抽出に失敗しました: {e}"))
        })?;

    Ok(extracted.replace("\r\n", "\n"))
}

pub fn convert_pdf(
    input_path: &Path,
    output_path: &Path,
    target_format: &str,
) -> Result<(), ConvertError> {
    if target_format == "split-pdf" {
        return split_pdf_pages(input_path, output_path);
    }

    let text = extract_pdf_text(input_path)?;

    match target_format {
        "docx" => write_text_as_docx(&text, output_path),
        "txt" | "md" => {
            fs::write(output_path, text.as_bytes())?;
            Ok(())
        }
        "html" => write_text_as_html(input_path, &text, output_path),
        other => Err(ConvertError::UnsupportedTarget(other.to_string())),
    }
}

/// 1枚以上の画像を各ページに配置した PDF を生成する（JPEG DCTDecode 埋め込み）
pub fn images_to_pdf(
    images: &[DynamicImage],
    output_path: &Path,
    jpeg_quality: u8,
) -> Result<(), ConvertError> {
    if images.is_empty() {
        return Err(ConvertError::ConversionFailed(
            "PDF に変換する画像がありません".to_string(),
        ));
    }

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut page_ids: Vec<Object> = Vec::new();
    let quality = jpeg_quality.clamp(10, 100);

    for img in images {
        let rgb = image_conv::flatten_to_white_rgb8(img);
        let w = rgb.width() as f32;
        let h = rgb.height() as f32;

        let mut jpeg_buf: Vec<u8> = Vec::new();
        {
            let mut encoder = JpegEncoder::new_with_quality(&mut jpeg_buf, quality);
            encoder
                .encode_image(&rgb)
                .map_err(|e| ConvertError::ConversionFailed(format!("PDF 画像圧縮に失敗: {e}")))?;
        }

        // ページサイズ（ポイント単位：72dpi 基準、最大幅 595pt = A4 幅にスケーリング）
        let max_pt = 595.0_f32;
        let scale = if w > max_pt { max_pt / w } else { 1.0 };
        let pw = (w * scale).max(1.0);
        let ph = (h * scale).max(1.0);

        let image_stream = Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => rgb.width() as i64,
                "Height" => rgb.height() as i64,
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8,
                "Filter" => "DCTDecode",
            },
            jpeg_buf,
        );
        let image_id = doc.add_object(image_stream);

        let content_ops = format!("q\n{pw:.2} 0 0 {ph:.2} 0 0 cm\n/Im0 Do\nQ\n");
        let content_id = doc.add_object(Stream::new(dictionary! {}, content_ops.into_bytes()));

        let resources = dictionary! {
            "XObject" => dictionary! {
                "Im0" => image_id,
            },
        };

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), pw.into(), ph.into()],
            "Resources" => resources,
            "Contents" => content_id,
        });
        page_ids.push(page_id.into());
    }

    let pages = dictionary! {
        "Type" => "Pages",
        "Kids" => page_ids.clone(),
        "Count" => page_ids.len() as i64,
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    doc.save(output_path)
        .map_err(|e| ConvertError::ConversionFailed(format!("PDF の保存に失敗しました: {e}")))?;

    Ok(())
}

/// PDF を 1 ページずつ個別の PDF ファイルに分割して出力フォルダへ保存する
pub fn split_pdf_pages(input_path: &Path, output_dir: &Path) -> Result<(), ConvertError> {
    let source = Document::load(input_path)
        .map_err(|e| ConvertError::ConversionFailed(format!("PDF の読み込みに失敗しました: {e}")))?;
    let total_pages = source.get_pages().len() as u32;
    if total_pages == 0 {
        return Err(ConvertError::ConversionFailed(
            "PDF にページが含まれていません".to_string(),
        ));
    }

    fs::create_dir_all(output_dir)?;

    for page_num in 1..=total_pages {
        let mut single = source.clone();
        let to_delete: Vec<u32> = (1..=total_pages).filter(|&p| p != page_num).collect();
        if !to_delete.is_empty() {
            single.delete_pages(&to_delete);
        }
        single.prune_objects();
        let out_file = output_dir.join(format!("page_{page_num:03}.pdf"));
        single.save(&out_file).map_err(|e| {
            ConvertError::ConversionFailed(format!("ページ {page_num} の保存に失敗: {e}"))
        })?;
    }

    Ok(())
}

/// 複数の PDF ファイルを 1 冊の PDF に結合する
pub fn merge_pdfs(input_paths: &[PathBuf], output_path: &Path) -> Result<(), ConvertError> {
    if input_paths.is_empty() {
        return Err(ConvertError::ConversionFailed(
            "結合する PDF ファイルがありません".to_string(),
        ));
    }

    let mut max_id = 1u32;
    let mut documents_pages: BTreeMap<ObjectId, Object> = BTreeMap::new();
    let mut documents_objects: BTreeMap<ObjectId, Object> = BTreeMap::new();
    let mut merged = Document::with_version("1.5");

    for path in input_paths {
        let mut doc = Document::load(path).map_err(|e| {
            ConvertError::ConversionFailed(format!(
                "PDF の読み込みに失敗しました ({}): {e}",
                path.display()
            ))
        })?;
        doc.renumber_objects_with(max_id);
        max_id = doc.max_id + 1;

        for (_, object_id) in doc.get_pages() {
            if let Ok(obj) = doc.get_object(object_id) {
                documents_pages.insert(object_id, obj.to_owned());
            }
        }
        documents_objects.extend(doc.objects);
    }

    let mut catalog_object: Option<(ObjectId, Object)> = None;
    let mut pages_object: Option<(ObjectId, Object)> = None;

    for (object_id, object) in documents_objects {
        match object.type_name().unwrap_or("") {
            "Catalog" => {
                catalog_object = Some((
                    if let Some((id, _)) = catalog_object {
                        id
                    } else {
                        object_id
                    },
                    object,
                ));
            }
            "Pages" => {
                if let Ok(dictionary) = object.as_dict() {
                    let mut dictionary = dictionary.clone();
                    if let Some((_, ref existing)) = pages_object {
                        if let Ok(old_dict) = existing.as_dict() {
                            dictionary.extend(old_dict);
                        }
                    }
                    pages_object = Some((
                        if let Some((id, _)) = pages_object {
                            id
                        } else {
                            object_id
                        },
                        Object::Dictionary(dictionary),
                    ));
                }
            }
            "Page" | "Outlines" | "Outline" => {}
            _ => {
                merged.objects.insert(object_id, object);
            }
        }
    }

    let (pages_id, pages_obj) = pages_object.ok_or_else(|| {
        ConvertError::ConversionFailed("PDF のページ構造の構築に失敗しました".to_string())
    })?;
    let (catalog_id, catalog_obj) = catalog_object.ok_or_else(|| {
        ConvertError::ConversionFailed("PDF のカタログ構造の構築に失敗しました".to_string())
    })?;

    for (object_id, object) in &documents_pages {
        if let Ok(dictionary) = object.as_dict() {
            let mut dictionary = dictionary.clone();
            dictionary.set("Parent", pages_id);
            merged
                .objects
                .insert(*object_id, Object::Dictionary(dictionary));
        }
    }

    if let Ok(dictionary) = pages_obj.as_dict() {
        let mut dictionary = dictionary.clone();
        dictionary.set("Count", documents_pages.len() as u32);
        dictionary.set(
            "Kids",
            documents_pages
                .keys()
                .map(|&id| Object::Reference(id))
                .collect::<Vec<_>>(),
        );
        merged
            .objects
            .insert(pages_id, Object::Dictionary(dictionary));
    }

    if let Ok(dictionary) = catalog_obj.as_dict() {
        let mut dictionary = dictionary.clone();
        dictionary.set("Pages", pages_id);
        dictionary.remove(b"Outlines");
        merged
            .objects
            .insert(catalog_id, Object::Dictionary(dictionary));
    }

    merged.trailer.set("Root", catalog_id);
    merged.max_id = merged.objects.len() as u32;
    merged.renumber_objects();
    merged.compress();

    merged
        .save(output_path)
        .map_err(|e| ConvertError::ConversionFailed(format!("結合 PDF の保存に失敗しました: {e}")))?;

    Ok(())
}

fn write_text_as_docx(text: &str, output_path: &Path) -> Result<(), ConvertError> {
    let mut docx = Docx::new();

    for line in text.lines() {
        let trimmed = line.trim_end();
        let paragraph = if trimmed.is_empty() {
            Paragraph::new()
        } else {
            Paragraph::new().add_run(Run::new().add_text(trimmed))
        };
        docx = docx.add_paragraph(paragraph);
    }

    let file = File::create(output_path)?;
    docx.build().pack(file).map_err(|e| {
        ConvertError::ConversionFailed(format!("Word (.docx) の書き出しに失敗しました: {e}"))
    })?;

    Ok(())
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn write_text_as_html(
    input_path: &Path,
    text: &str,
    output_path: &Path,
) -> Result<(), ConvertError> {
    let title = input_path
        .file_stem()
        .map(|s| escape_html(&s.to_string_lossy()))
        .unwrap_or_else(|| "PDF Document".to_string());

    let paragraphs: String = text
        .split("\n\n")
        .map(|block| {
            let escaped = escape_html(block.trim()).replace('\n', "<br />");
            format!("  <p>{escaped}</p>\n")
        })
        .collect();

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="ja">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>{title}</title>
  <style>
    body {{
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "Hiragino Kaku Gothic ProN", "Yu Gothic", sans-serif;
      max-width: 820px;
      margin: 2rem auto;
      padding: 0 1.25rem;
      line-height: 1.8;
      color: #1f2937;
    }}
    p {{ margin: 0.75rem 0; }}
  </style>
</head>
<body>
{paragraphs}
</body>
</html>
"#
    );

    fs::write(output_path, html.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn build_minimal_pdf() -> Vec<u8> {
        let mut pdf = Vec::new();
        let mut offsets = Vec::new();

        pdf.extend_from_slice(b"%PDF-1.4\n");

        let stream_body = b"BT /F1 18 Tf 72 720 Td (Hello Henkan PDF) Tj ET";
        let obj4 = format!(
            "4 0 obj\n<< /Length {} >>\nstream\n{}\nendstream\nendobj\n",
            stream_body.len(),
            std::str::from_utf8(stream_body).unwrap()
        );

        let objects: Vec<&[u8]> = vec![
            b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
            b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n",
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n",
            obj4.as_bytes(),
            b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n",
        ];

        for obj in &objects {
            offsets.push(pdf.len());
            pdf.extend_from_slice(obj);
        }

        let xref_pos = pdf.len();
        pdf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
        pdf.extend_from_slice(b"0000000000 65535 f \n");
        for off in offsets {
            pdf.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        pdf.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_pos}\n%%EOF\n",
                objects.len() + 1
            )
            .as_bytes(),
        );
        pdf
    }

    #[test]
    fn test_pdf_to_docx_and_txt() {
        let dir = std::env::temp_dir().join("henkan_test_pdf");
        let _ = fs::create_dir_all(&dir);
        let src_pdf = dir.join("sample.pdf");
        let dst_docx = dir.join("sample.docx");
        let dst_txt = dir.join("sample.txt");

        fs::write(&src_pdf, build_minimal_pdf()).unwrap();

        convert_pdf(&src_pdf, &dst_txt, "txt").unwrap();
        let extracted = fs::read_to_string(&dst_txt).unwrap();
        assert!(
            extracted.contains("Hello Henkan PDF"),
            "extracted was: {extracted:?}"
        );

        convert_pdf(&src_pdf, &dst_docx, "docx").unwrap();
        let docx_bytes = fs::read(&dst_docx).unwrap();
        assert!(docx_bytes.len() > 100);
        assert_eq!(&docx_bytes[0..2], b"PK");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_images_to_pdf_merge_and_split() {
        let dir = std::env::temp_dir().join("henkan_test_pdf_ops");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let img1 = DynamicImage::ImageRgba8(RgbaImage::from_pixel(100, 80, Rgba([255, 0, 0, 255])));
        let img2 = DynamicImage::ImageRgba8(RgbaImage::from_pixel(100, 80, Rgba([0, 0, 255, 255])));

        let pdf1 = dir.join("one.pdf");
        let pdf2 = dir.join("two.pdf");
        let merged = dir.join("merged.pdf");
        let split_dir = dir.join("pages");

        images_to_pdf(&[img1], &pdf1, 80).unwrap();
        images_to_pdf(&[img2], &pdf2, 80).unwrap();

        merge_pdfs(&[pdf1, pdf2], &merged).unwrap();
        let loaded = Document::load(&merged).unwrap();
        assert_eq!(loaded.get_pages().len(), 2);

        split_pdf_pages(&merged, &split_dir).unwrap();
        assert!(split_dir.join("page_001.pdf").exists());
        assert!(split_dir.join("page_002.pdf").exists());

        let _ = fs::remove_dir_all(&dir);
    }
}
