use super::{ConvertError, TargetFormat};
use calamine::{open_workbook_auto, Data, Reader};
use rust_xlsxwriter::Workbook;
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const EXCEL_EXTENSIONS: &[&str] = &["xlsx", "xls", "xlsm", "xlsb", "ods"];

pub fn is_supported_extension(ext: &str) -> bool {
    EXCEL_EXTENSIONS.contains(&ext)
}

pub fn available_targets() -> Vec<TargetFormat> {
    vec![
        TargetFormat {
            id: "csv".to_string(),
            label: "CSV".to_string(),
            extension: "csv".to_string(),
            category: "data".to_string(),
            description: "先頭シートを CSV に変換（内蔵）".to_string(),
        },
        TargetFormat {
            id: "json".to_string(),
            label: "JSON".to_string(),
            extension: "json".to_string(),
            category: "data".to_string(),
            description: "1行目をヘッダーとして JSON 化（内蔵）".to_string(),
        },
        TargetFormat {
            id: "yaml".to_string(),
            label: "YAML".to_string(),
            extension: "yaml".to_string(),
            category: "data".to_string(),
            description: "1行目をヘッダーとして YAML 化（内蔵）".to_string(),
        },
    ]
}

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(f) => {
            if f.fract() == 0.0 && *f >= i64::MIN as f64 && *f <= i64::MAX as f64 {
                (*f as i64).to_string()
            } else {
                f.to_string()
            }
        }
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(dt) => dt.to_string(),
        Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
        Data::Error(e) => format!("{e:?}"),
    }
}

fn read_first_sheet_rows(input_path: &Path) -> Result<Vec<Vec<String>>, ConvertError> {
    let mut workbook = open_workbook_auto(input_path)
        .map_err(|e| ConvertError::ConversionFailed(format!("Excel ファイルの読み込みに失敗: {e}")))?;

    let sheet_names = workbook.sheet_names().to_vec();
    let first_sheet = sheet_names.first().ok_or_else(|| {
        ConvertError::ConversionFailed("ワークシートが見つかりませんでした".to_string())
    })?;

    let range = workbook
        .worksheet_range(first_sheet)
        .map_err(|e| ConvertError::ConversionFailed(format!("シートの読み込みに失敗: {e}")))?;

    let rows: Vec<Vec<String>> = range
        .rows()
        .map(|r| r.iter().map(cell_to_string).collect())
        .collect();

    Ok(rows)
}

pub fn convert_excel(
    input_path: &Path,
    output_path: &Path,
    target_format: &str,
) -> Result<(), ConvertError> {
    let rows = read_first_sheet_rows(input_path)?;

    match target_format {
        "csv" => {
            let mut wtr = csv::Writer::from_path(output_path).map_err(|e| {
                ConvertError::ConversionFailed(format!("CSV の書き出し開始に失敗: {e}"))
            })?;
            for row in &rows {
                wtr.write_record(row).map_err(|e| {
                    ConvertError::ConversionFailed(format!("CSV 行の書き出しに失敗: {e}"))
                })?;
            }
            wtr.flush()?;
            Ok(())
        }
        "json" | "yaml" => {
            let value = rows_to_json_value(&rows);
            let text = if target_format == "json" {
                serde_json::to_string_pretty(&value).map_err(|e| {
                    ConvertError::ConversionFailed(format!("JSON の生成に失敗: {e}"))
                })?
            } else {
                serde_yaml::to_string(&value).map_err(|e| {
                    ConvertError::ConversionFailed(format!("YAML の生成に失敗: {e}"))
                })?
            };
            fs::write(output_path, text.as_bytes())?;
            Ok(())
        }
        other => Err(ConvertError::UnsupportedTarget(other.to_string())),
    }
}

fn rows_to_json_value(rows: &[Vec<String>]) -> Value {
    if rows.is_empty() {
        return Value::Array(vec![]);
    }
    let headers: Vec<String> = rows[0]
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let trimmed = h.trim();
            if trimmed.is_empty() {
                format!("col_{}", i + 1)
            } else {
                trimmed.to_string()
            }
        })
        .collect();

    let mut out = Vec::new();
    for row in rows.iter().skip(1) {
        let mut map = Map::new();
        for (i, header) in headers.iter().enumerate() {
            let val = row.get(i).cloned().unwrap_or_default();
            map.insert(header.clone(), Value::String(val));
        }
        out.push(Value::Object(map));
    }
    Value::Array(out)
}

/// JSON Value（オブジェクト配列 or オブジェクト）を Excel (.xlsx) として書き出す
pub fn write_json_value_as_xlsx(value: &Value, output_path: &Path) -> Result<(), ConvertError> {
    let rows: Vec<&Map<String, Value>> = match value {
        Value::Array(arr) => arr.iter().filter_map(|v| v.as_object()).collect(),
        Value::Object(obj) => vec![obj],
        _ => {
            return Err(ConvertError::ConversionFailed(
                "Excel (.xlsx) へ変換できるのはオブジェクトまたはオブジェクトの配列のみです"
                    .to_string(),
            ))
        }
    };

    if rows.is_empty() {
        return Err(ConvertError::ConversionFailed(
            "Excel に書き出す行データが見つかりませんでした".to_string(),
        ));
    }

    let mut header_set = BTreeSet::new();
    for row in &rows {
        for key in row.keys() {
            header_set.insert(key.clone());
        }
    }
    let headers: Vec<String> = header_set.into_iter().collect();

    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();

    for (col, h) in headers.iter().enumerate() {
        sheet.write_string(0, col as u16, h).map_err(|e| {
            ConvertError::ConversionFailed(format!("Excel ヘッダー書き込みに失敗: {e}"))
        })?;
    }

    for (row_idx, row) in rows.iter().enumerate() {
        let r = (row_idx + 1) as u32;
        for (col_idx, h) in headers.iter().enumerate() {
            let c = col_idx as u16;
            match row.get(h) {
                Some(Value::Number(n)) => {
                    if let Some(f) = n.as_f64() {
                        sheet.write_number(r, c, f).map_err(|e| {
                            ConvertError::ConversionFailed(format!("Excel 数値書き込みに失敗: {e}"))
                        })?;
                    } else {
                        sheet.write_string(r, c, n.to_string()).map_err(|e| {
                            ConvertError::ConversionFailed(format!("Excel 書き込みに失敗: {e}"))
                        })?;
                    }
                }
                Some(Value::Bool(b)) => {
                    sheet.write_boolean(r, c, *b).map_err(|e| {
                        ConvertError::ConversionFailed(format!("Excel 真偽値書き込みに失敗: {e}"))
                    })?;
                }
                Some(Value::String(s)) => {
                    sheet.write_string(r, c, s).map_err(|e| {
                        ConvertError::ConversionFailed(format!("Excel 文字列書き込みに失敗: {e}"))
                    })?;
                }
                Some(Value::Null) | None => {}
                Some(other) => {
                    sheet.write_string(r, c, other.to_string()).map_err(|e| {
                        ConvertError::ConversionFailed(format!("Excel 書き込みに失敗: {e}"))
                    })?;
                }
            }
        }
    }

    workbook
        .save(output_path)
        .map_err(|e| ConvertError::ConversionFailed(format!("Excel ファイルの保存に失敗: {e}")))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_to_xlsx_and_back_to_csv_and_json() {
        let dir = std::env::temp_dir().join("henkan_test_excel");
        let _ = fs::create_dir_all(&dir);
        let xlsx = dir.join("users.xlsx");
        let csv_out = dir.join("users.csv");
        let json_out = dir.join("users.json");

        let data: Value =
            serde_json::from_str(r#"[{"name":"Taro","score":95},{"name":"Hanako","score":88}]"#)
                .unwrap();

        write_json_value_as_xlsx(&data, &xlsx).unwrap();
        assert!(xlsx.exists());

        convert_excel(&xlsx, &csv_out, "csv").unwrap();
        let csv_text = fs::read_to_string(&csv_out).unwrap();
        assert!(csv_text.contains("Taro"));
        assert!(csv_text.contains("95"));

        convert_excel(&xlsx, &json_out, "json").unwrap();
        let json_text = fs::read_to_string(&json_out).unwrap();
        assert!(json_text.contains("Hanako"));

        let _ = fs::remove_dir_all(&dir);
    }
}
