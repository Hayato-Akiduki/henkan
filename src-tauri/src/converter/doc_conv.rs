use super::tools::{new_hidden_command, resolve_program};
use super::{ConvertError, TargetFormat};
use pulldown_cmark::{html, Options, Parser};
use std::fs;
use std::path::Path;

const DOCUMENT_EXTENSIONS: &[&str] = &[
    "md", "markdown", "docx", "odt", "epub", "rtf", "rst", "tex", "org", "html", "htm",
];

pub fn is_supported_extension(ext: &str) -> bool {
    DOCUMENT_EXTENSIONS.contains(&ext)
}

pub fn available_targets(input_ext: &str) -> Vec<TargetFormat> {
    let normalized = match input_ext {
        "markdown" => "md",
        "htm" => "html",
        other => other,
    };

    let all = vec![
        TargetFormat {
            id: "html".to_string(),
            label: "HTML".to_string(),
            extension: "html".to_string(),
            category: "document".to_string(),
            description: if normalized == "md" {
                "スタイル付き HTML（内蔵エンジン）".to_string()
            } else {
                "HTML ドキュメント（Pandoc）".to_string()
            },
        },
        TargetFormat {
            id: "md".to_string(),
            label: "Markdown".to_string(),
            extension: "md".to_string(),
            category: "document".to_string(),
            description: "Markdown 形式（Pandoc）".to_string(),
        },
        TargetFormat {
            id: "docx".to_string(),
            label: "Word (DOCX)".to_string(),
            extension: "docx".to_string(),
            category: "document".to_string(),
            description: "Microsoft Word 文書（Pandoc）".to_string(),
        },
        TargetFormat {
            id: "epub".to_string(),
            label: "EPUB".to_string(),
            extension: "epub".to_string(),
            category: "document".to_string(),
            description: "電子書籍形式（Pandoc）".to_string(),
        },
        TargetFormat {
            id: "pdf".to_string(),
            label: "PDF".to_string(),
            extension: "pdf".to_string(),
            category: "document".to_string(),
            description: "PDF 文書（Pandoc + PDFエンジン）".to_string(),
        },
        TargetFormat {
            id: "rtf".to_string(),
            label: "RTF".to_string(),
            extension: "rtf".to_string(),
            category: "document".to_string(),
            description: "リッチテキスト形式（Pandoc）".to_string(),
        },
        TargetFormat {
            id: "txt".to_string(),
            label: "プレーンテキスト".to_string(),
            extension: "txt".to_string(),
            category: "document".to_string(),
            description: "書式なしテキスト抽出（Pandoc）".to_string(),
        },
    ];

    all.into_iter().filter(|t| t.id != normalized).collect()
}

pub fn convert_document(
    input_path: &Path,
    output_path: &Path,
    input_ext: &str,
    target_format: &str,
    pandoc_path: Option<&str>,
) -> Result<(), ConvertError> {
    // Markdown -> HTML は外部ツール不要な Rust 内蔵エンジンで高速に変換
    if (input_ext == "md" || input_ext == "markdown") && target_format == "html" {
        return convert_markdown_to_html(input_path, output_path);
    }

    convert_with_pandoc(input_path, output_path, target_format, pandoc_path)
}

pub fn convert_with_pandoc(
    input_path: &Path,
    output_path: &Path,
    target_format: &str,
    pandoc_path: Option<&str>,
) -> Result<(), ConvertError> {
    let program = resolve_program(pandoc_path, "pandoc");
    let mut cmd = new_hidden_command(&program);

    cmd.arg(input_path).arg("-o").arg(output_path);

    match target_format {
        "html" => {
            cmd.arg("--standalone");
        }
        "txt" => {
            cmd.args(["-t", "plain"]);
        }
        "md" => {
            cmd.args(["-t", "gfm"]);
        }
        _ => {}
    }

    let output = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ConvertError::ConversionFailed(
                "Pandoc が見つかりません。PC に Pandoc をインストールするか、画面上部のツール設定から pandoc 実行ファイルを指定してください。".to_string(),
            )
        } else {
            ConvertError::ConversionFailed(format!("Pandoc の起動に失敗しました: {e}"))
        }
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.trim();
        if target_format == "pdf" && detail.contains("pdflatex") {
            return Err(ConvertError::ConversionFailed(
                "PDF 出力には Pandoc に加えて LaTeX（TeX Live / MiKTeX）または typst 等の PDF エンジンが必要です。".to_string(),
            ));
        }
        return Err(ConvertError::ConversionFailed(format!(
            "Pandoc 変換エラー: {detail}"
        )));
    }

    Ok(())
}

pub fn convert_markdown_to_html(
    input_path: &Path,
    output_path: &Path,
) -> Result<(), ConvertError> {
    let md_content = fs::read_to_string(input_path)?;
    let title = input_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Document".to_string());

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);

    let parser = Parser::new_ext(&md_content, options);
    let mut body_html = String::new();
    html::push_html(&mut body_html, parser);

    let full_html = format!(
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
      line-height: 1.75;
      color: #1f2937;
      background: #ffffff;
    }}
    pre {{
      background: #f3f4f6;
      padding: 1rem;
      border-radius: 8px;
      overflow-x: auto;
    }}
    code {{
      background: #f3f4f6;
      padding: 0.15rem 0.35rem;
      border-radius: 4px;
      font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    }}
    pre code {{ background: none; padding: 0; }}
    table {{ border-collapse: collapse; width: 100%; margin: 1rem 0; }}
    th, td {{ border: 1px solid #d1d5db; padding: 0.5rem 0.75rem; text-align: left; }}
    th {{ background: #f9fafb; }}
    blockquote {{ border-left: 4px solid #6366f1; margin: 1rem 0; padding: 0.25rem 1rem; color: #4b5563; }}
  </style>
</head>
<body>
{body_html}
</body>
</html>
"#
    );

    fs::write(output_path, full_html.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_to_html() {
        let dir = std::env::temp_dir().join("henkan_test_doc");
        let _ = fs::create_dir_all(&dir);
        let src = dir.join("readme.md");
        let dst = dir.join("readme.html");

        fs::write(&src, "# タイトル\n\n- 項目1\n- 項目2\n").unwrap();
        convert_markdown_to_html(&src, &dst).unwrap();

        let html_text = fs::read_to_string(&dst).unwrap();
        assert!(html_text.contains("<h1>タイトル</h1>"));
        assert!(html_text.contains("<li>項目1</li>"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_document_targets() {
        let md_targets = available_targets("md");
        assert!(md_targets.iter().any(|t| t.id == "html"));
        assert!(md_targets.iter().any(|t| t.id == "docx"));
        assert!(!md_targets.iter().any(|t| t.id == "md"));
    }
}
