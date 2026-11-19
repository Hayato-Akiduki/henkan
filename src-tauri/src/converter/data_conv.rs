use super::{ConvertError, TargetFormat};
use encoding_rs::{EUC_JP, SHIFT_JIS, UTF_8};
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const STRUCTURED_EXTENSIONS: &[&str] = &["json", "yaml", "yml", "toml", "csv"];
const TEXT_EXTENSIONS: &[&str] = &[
    "txt", "log", "ini", "cfg", "conf", "xml", "html", "htm", "css", "js", "ts", "sql", "tsv",
];

pub fn is_structured_data_extension(ext: &str) -> bool {
    STRUCTURED_EXTENSIONS.contains(&ext)
}

pub fn is_text_extension(ext: &str) -> bool {
    TEXT_EXTENSIONS.contains(&ext)
}

pub fn structured_targets(input_ext: &str) -> Vec<TargetFormat> {
    let normalized = match input_ext {
        "yml" => "yaml",
        other => other,
    };

    let all = vec![
        TargetFormat {
            id: "json".to_string(),
            label: "JSON".to_string(),
            extension: "json".to_string(),
            category: "data".to_string(),
            description: "整形済み JSON".to_string(),
        },
        TargetFormat {
            id: "yaml".to_string(),
            label: "YAML".to_string(),
            extension: "yaml".to_string(),
            category: "data".to_string(),
            description: "設定ファイル向け YAML".to_string(),
        },
        TargetFormat {
            id: "toml".to_string(),
            label: "TOML".to_string(),
            extension: "toml".to_string(),
            category: "data".to_string(),
            description: "シンプルな TOML 形式".to_string(),
        },
        TargetFormat {
            id: "csv".to_string(),
            label: "CSV".to_string(),
            extension: "csv".to_string(),
            category: "data".to_string(),
            description: "表形式データ（配列データ向け）".to_string(),
        },
    ];

    all.into_iter().filter(|t| t.id != normalized).collect()
}

pub fn encoding_targets() -> Vec<TargetFormat> {
    vec![
        TargetFormat {
            id: "enc:utf8".to_string(),
            label: "UTF-8（BOMなし）".to_string(),
            extension: "txt".to_string(),
            category: "encoding".to_string(),
            description: "標準的な UTF-8 に変換".to_string(),
        },
        TargetFormat {
            id: "enc:utf8-bom".to_string(),
            label: "UTF-8（BOMあり）".to_string(),
            extension: "txt".to_string(),
            category: "encoding".to_string(),
            description: "Excel で文字化けしにくい UTF-8".to_string(),
        },
        TargetFormat {
            id: "enc:shift_jis".to_string(),
            label: "Shift_JIS".to_string(),
            extension: "txt".to_string(),
            category: "encoding".to_string(),
            description: "Windows 従来形式の日本語".to_string(),
        },
        TargetFormat {
            id: "enc:euc-jp".to_string(),
            label: "EUC-JP".to_string(),
            extension: "txt".to_string(),
            category: "encoding".to_string(),
            description: "UNIX 系従来形式の日本語".to_string(),
        },
    ]
}

/// バイト列から文字コードを自動判別して UTF-8 文字列へデコード
fn decode_auto(bytes: &[u8]) -> String {
    // UTF-8 BOM チェック
    if let Some(stripped) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        if let Ok(s) = std::str::from_utf8(stripped) {
            return s.to_string();
        }
    }
    // そのまま有効な UTF-8 なら採用
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    // Shift_JIS でデコードを試す
    let (sjis_cow, _, sjis_errors) = SHIFT_JIS.decode(bytes);
    if !sjis_errors {
        return sjis_cow.into_owned();
    }
    // EUC-JP でデコードを試す
    let (euc_cow, _, euc_errors) = EUC_JP.decode(bytes);
    if !euc_errors {
        return euc_cow.into_owned();
    }
    // フォールバック: Shift_JIS
    sjis_cow.into_owned()
}

pub fn convert_encoding(
    input_path: &Path,
    output_path: &Path,
    target_encoding: &str,
) -> Result<(), ConvertError> {
    let bytes = fs::read(input_path)?;
    let text = decode_auto(&bytes);

    let out_bytes: Vec<u8> = match target_encoding {
        "utf8" => UTF_8.encode(&text).0.into_owned(),
        "utf8-bom" => {
            let mut v = vec![0xEF, 0xBB, 0xBF];
            v.extend_from_slice(text.as_bytes());
            v
        }
        "shift_jis" => SHIFT_JIS.encode(&text).0.into_owned(),
        "euc-jp" => EUC_JP.encode(&text).0.into_owned(),
        other => return Err(ConvertError::UnsupportedTarget(other.to_string())),
    };

    fs::write(output_path, out_bytes)?;
    Ok(())
}

pub fn convert_structured_data(
    input_path: &Path,
    output_path: &Path,
    input_ext: &str,
    target_format: &str,
) -> Result<(), ConvertError> {
    let raw_bytes = fs::read(input_path)?;
    let content = decode_auto(&raw_bytes);

    let value = parse_data(&content, input_ext)?;
    let serialized = serialize_data(&value, target_format)?;

    fs::write(output_path, serialized.as_bytes())?;
    Ok(())
}

fn parse_data(content: &str, ext: &str) -> Result<Value, ConvertError> {
    match ext {
        "json" => serde_json::from_str(content)
            .map_err(|e| ConvertError::ConversionFailed(format!("JSON の解析に失敗: {e}"))),
        "yaml" | "yml" => serde_yaml::from_str(content)
            .map_err(|e| ConvertError::ConversionFailed(format!("YAML の解析に失敗: {e}"))),
        "toml" => {
            let tv: toml::Value = toml::from_str(content)
                .map_err(|e| ConvertError::ConversionFailed(format!("TOML の解析に失敗: {e}")))?;
            serde_json::to_value(tv)
                .map_err(|e| ConvertError::ConversionFailed(format!("TOML 値の変換に失敗: {e}")))
        }
        "csv" => parse_csv_to_json(content),
        other => Err(ConvertError::UnsupportedInput(other.to_string())),
    }
}

fn parse_csv_to_json(content: &str) -> Result<Value, ConvertError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(content.as_bytes());

    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| ConvertError::ConversionFailed(format!("CSV ヘッダーの解析に失敗: {e}")))?
        .iter()
        .map(|s| s.to_string())
        .collect();

    let mut rows = Vec::new();
    for result in reader.records() {
        let record = result
            .map_err(|e| ConvertError::ConversionFailed(format!("CSV 行の解析に失敗: {e}")))?;
        let mut obj = Map::new();
        for (i, field) in record.iter().enumerate() {
            let key = headers
                .get(i)
                .cloned()
                .unwrap_or_else(|| format!("col_{i}"));
            obj.insert(key, Value::String(field.to_string()));
        }
        rows.push(Value::Object(obj));
    }

    Ok(Value::Array(rows))
}

fn serialize_data(value: &Value, target_format: &str) -> Result<String, ConvertError> {
    match target_format {
        "json" => serde_json::to_string_pretty(value)
            .map_err(|e| ConvertError::ConversionFailed(format!("JSON の生成に失敗: {e}"))),
        "yaml" | "yml" => serde_yaml::to_string(value)
            .map_err(|e| ConvertError::ConversionFailed(format!("YAML の生成に失敗: {e}"))),
        "toml" => {
            // TOML のルートはテーブル（オブジェクト）である必要があるため、配列の場合は items でラップする
            let wrapped = match value {
                Value::Object(_) => value.clone(),
                other => {
                    let mut m = Map::new();
                    m.insert("items".to_string(), other.clone());
                    Value::Object(m)
                }
            };
            let tv: toml::Value = serde_json::from_value(wrapped).map_err(|e| {
                ConvertError::ConversionFailed(format!("TOML 形式への変換に失敗: {e}"))
            })?;
            toml::to_string_pretty(&tv)
                .map_err(|e| ConvertError::ConversionFailed(format!("TOML の生成に失敗: {e}")))
        }
        "csv" => serialize_json_to_csv(value),
        other => Err(ConvertError::UnsupportedTarget(other.to_string())),
    }
}

fn serialize_json_to_csv(value: &Value) -> Result<String, ConvertError> {
    let rows: Vec<&Map<String, Value>> = match value {
        Value::Array(arr) => arr.iter().filter_map(|v| v.as_object()).collect(),
        Value::Object(obj) => vec![obj],
        _ => {
            return Err(ConvertError::ConversionFailed(
                "CSV へ変換できるのはオブジェクトまたはオブジェクトの配列のみです".to_string(),
            ))
        }
    };

    if rows.is_empty() {
        return Err(ConvertError::ConversionFailed(
            "CSV に書き出す行データが見つかりませんでした".to_string(),
        ));
    }

    let mut header_set = BTreeSet::new();
    for row in &rows {
        for key in row.keys() {
            header_set.insert(key.clone());
        }
    }
    let headers: Vec<String> = header_set.into_iter().collect();

    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record(&headers)
        .map_err(|e| ConvertError::ConversionFailed(format!("CSV ヘッダー書き込みに失敗: {e}")))?;

    for row in rows {
        let record: Vec<String> = headers
            .iter()
            .map(|h| match row.get(h) {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Null) | None => String::new(),
                Some(other) => other.to_string(),
            })
            .collect();
        wtr.write_record(&record)
            .map_err(|e| ConvertError::ConversionFailed(format!("CSV 行書き込みに失敗: {e}")))?;
    }

    let bytes = wtr
        .into_inner()
        .map_err(|e| ConvertError::ConversionFailed(format!("CSV 出力に失敗: {e}")))?;
    String::from_utf8(bytes)
        .map_err(|e| ConvertError::ConversionFailed(format!("CSV UTF-8 変換に失敗: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_to_yaml_and_csv() {
        let dir = std::env::temp_dir().join("henkan_test_data");
        let _ = fs::create_dir_all(&dir);
        let src = dir.join("users.json");
        let dst_yaml = dir.join("users.yaml");
        let dst_csv = dir.join("users.csv");

        fs::write(&src, r#"[{"name":"Taro","age":20},{"name":"Hanako","age":25}]"#).unwrap();

        convert_structured_data(&src, &dst_yaml, "json", "yaml").unwrap();
        let yaml_text = fs::read_to_string(&dst_yaml).unwrap();
        assert!(yaml_text.contains("Taro"));

        convert_structured_data(&src, &dst_csv, "json", "csv").unwrap();
        let csv_text = fs::read_to_string(&dst_csv).unwrap();
        assert!(csv_text.contains("age,name"));
        assert!(csv_text.contains("20,Taro"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_encoding_utf8_to_sjis_and_back() {
        let dir = std::env::temp_dir().join("henkan_test_enc");
        let _ = fs::create_dir_all(&dir);
        let src = dir.join("note.txt");
        let sjis = dir.join("note_sjis.txt");
        let utf8 = dir.join("note_utf8.txt");

        fs::write(&src, "こんにちは、Henkan！").unwrap();
        convert_encoding(&src, &sjis, "shift_jis").unwrap();
        convert_encoding(&sjis, &utf8, "utf8").unwrap();

        assert_eq!(fs::read_to_string(&utf8).unwrap(), "こんにちは、Henkan！");
        let _ = fs::remove_dir_all(&dir);
    }
}
