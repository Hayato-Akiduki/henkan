use super::{ConvertError, TargetFormat};
use docx_rs::{Docx, Paragraph, Run};
use std::fs::{self, File};
use std::path::Path;

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
    ]
}

fn extract_pdf_text(input_path: &Path) -> Result<String, ConvertError> {
    let bytes = fs::read(input_path)?;
    // pdf-extract が不正な PDF で panic するケースも安全にキャッチする
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

    /// テスト用の最小限の有効な PDF バイト列を生成する（"Hello Henkan PDF" を含む）
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
        assert!(dst_docx.exists());
        // DOCX は ZIP 形式（先頭 2 バイトが "PK"）であることを確認
        let docx_bytes = fs::read(&dst_docx).unwrap();
        assert!(docx_bytes.len() > 100);
        assert_eq!(&docx_bytes[0..2], b"PK");

        let _ = fs::remove_dir_all(&dir);
    }
}
