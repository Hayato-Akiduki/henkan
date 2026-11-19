use super::{ConvertError, TargetFormat};
use pulldown_cmark::{html, Options, Parser};
use std::fs;
use std::path::Path;

const SUPPORTED_EXTENSIONS: &[&str] = &["md", "markdown"];

pub fn is_supported_extension(ext: &str) -> bool {
    SUPPORTED_EXTENSIONS.contains(&ext)
}

pub fn available_targets(_input_ext: &str) -> Vec<TargetFormat> {
    vec![TargetFormat {
        id: "html".to_string(),
        label: "HTML".to_string(),
        extension: "html".to_string(),
        category: "document".to_string(),
        description: "スタイル付き HTML ドキュメント".to_string(),
    }]
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
}
