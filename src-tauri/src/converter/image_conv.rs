use super::pdf_conv;
use super::{ConvertError, ConvertOptions, TargetFormat};
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

const SUPPORTED_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "gif", "bmp", "ico", "tif", "tiff",
];

pub fn is_supported_extension(ext: &str) -> bool {
    SUPPORTED_EXTENSIONS.contains(&ext)
}

pub fn available_targets(input_ext: &str) -> Vec<TargetFormat> {
    let normalized = match input_ext {
        "jpeg" => "jpg",
        "tif" => "tiff",
        other => other,
    };

    let all = vec![
        TargetFormat {
            id: "png".to_string(),
            label: "PNG".to_string(),
            extension: "png".to_string(),
            category: "image".to_string(),
            description: "可逆圧縮・透過対応".to_string(),
        },
        TargetFormat {
            id: "jpg".to_string(),
            label: "JPEG".to_string(),
            extension: "jpg".to_string(),
            category: "image".to_string(),
            description: "写真向け・軽量（品質調整可）".to_string(),
        },
        TargetFormat {
            id: "webp".to_string(),
            label: "WebP".to_string(),
            extension: "webp".to_string(),
            category: "image".to_string(),
            description: "Web向け・高圧縮".to_string(),
        },
        TargetFormat {
            id: "pdf".to_string(),
            label: "PDF".to_string(),
            extension: "pdf".to_string(),
            category: "document".to_string(),
            description: "画像を PDF ページとして保存（内蔵）".to_string(),
        },
        TargetFormat {
            id: "gif".to_string(),
            label: "GIF".to_string(),
            extension: "gif".to_string(),
            category: "image".to_string(),
            description: "256色・互換性高".to_string(),
        },
        TargetFormat {
            id: "bmp".to_string(),
            label: "BMP".to_string(),
            extension: "bmp".to_string(),
            category: "image".to_string(),
            description: "無圧縮ビットマップ".to_string(),
        },
        TargetFormat {
            id: "ico".to_string(),
            label: "ICO".to_string(),
            extension: "ico".to_string(),
            category: "image".to_string(),
            description: "アイコン（最大256px）".to_string(),
        },
        TargetFormat {
            id: "tiff".to_string(),
            label: "TIFF".to_string(),
            extension: "tiff".to_string(),
            category: "image".to_string(),
            description: "印刷・高画質向け".to_string(),
        },
    ];

    all.into_iter().filter(|t| t.id != normalized).collect()
}

/// 透過部分を白背景に合成して RGB8 に変換（JPEG 出力時に黒背景になるのを防ぐ）
pub fn flatten_to_white_rgb8(img: &DynamicImage) -> DynamicImage {
    let rgba: RgbaImage = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut out = image::RgbImage::new(w, h);

    for (x, y, Rgba([r, g, b, a])) in rgba.enumerate_pixels() {
        let alpha = *a as f32 / 255.0;
        let inv = 1.0 - alpha;
        let nr = (*r as f32 * alpha + 255.0 * inv).round() as u8;
        let ng = (*g as f32 * alpha + 255.0 * inv).round() as u8;
        let nb = (*b as f32 * alpha + 255.0 * inv).round() as u8;
        out.put_pixel(x, y, image::Rgb([nr, ng, nb]));
    }

    DynamicImage::ImageRgb8(out)
}

fn apply_resize_if_needed(img: DynamicImage, options: Option<&ConvertOptions>) -> DynamicImage {
    if let Some(max_w) = options.and_then(|o| o.image_max_width) {
        if max_w > 0 && img.width() > max_w {
            return img.resize(max_w, u32::MAX, image::imageops::FilterType::Lanczos3);
        }
    }
    img
}

pub fn save_dynamic_image(
    img: &DynamicImage,
    output_path: &Path,
    target_format: &str,
    options: Option<&ConvertOptions>,
) -> Result<(), ConvertError> {
    let img = apply_resize_if_needed(img.clone(), options);

    if target_format == "pdf" {
        let quality = options.and_then(|o| o.image_quality).unwrap_or(85);
        return pdf_conv::images_to_pdf(&[img], output_path, quality);
    }

    if target_format == "jpg" || target_format == "jpeg" {
        let rgb = flatten_to_white_rgb8(&img);
        let quality = options
            .and_then(|o| o.image_quality)
            .unwrap_or(85)
            .clamp(10, 100);
        let file = File::create(output_path)?;
        let writer = BufWriter::new(file);
        let mut encoder = JpegEncoder::new_with_quality(writer, quality);
        encoder
            .encode_image(&rgb)
            .map_err(|e| ConvertError::ConversionFailed(format!("JPEG の保存に失敗: {e}")))?;
        return Ok(());
    }

    let (processed, fmt) = match target_format {
        "png" => (img, ImageFormat::Png),
        "webp" => (img, ImageFormat::WebP),
        "gif" => (img, ImageFormat::Gif),
        "bmp" => (flatten_to_white_rgb8(&img), ImageFormat::Bmp),
        "ico" => {
            let resized = if img.width() > 256 || img.height() > 256 {
                img.thumbnail(256, 256)
            } else {
                img
            };
            (resized, ImageFormat::Ico)
        }
        "tiff" | "tif" => (img, ImageFormat::Tiff),
        other => return Err(ConvertError::UnsupportedTarget(other.to_string())),
    };

    processed
        .save_with_format(output_path, fmt)
        .map_err(|e| ConvertError::ConversionFailed(format!("画像の保存に失敗: {e}")))?;

    Ok(())
}

pub fn convert_image(
    input_path: &Path,
    output_path: &Path,
    target_format: &str,
    options: Option<&ConvertOptions>,
) -> Result<(), ConvertError> {
    let img = image::open(input_path)
        .map_err(|e| ConvertError::ConversionFailed(format!("画像の読み込みに失敗: {e}")))?;

    save_dynamic_image(&img, output_path, target_format, options)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_png_to_jpg_ico_and_pdf_with_resize() {
        let dir = std::env::temp_dir().join("henkan_test_img");
        let _ = fs::create_dir_all(&dir);
        let src = dir.join("sample.png");
        let dst_jpg = dir.join("sample.jpg");
        let dst_ico = dir.join("sample.ico");
        let dst_pdf = dir.join("sample.pdf");

        let img =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(400, 200, Rgba([50, 120, 220, 128])));
        img.save_with_format(&src, ImageFormat::Png).unwrap();

        let opts = ConvertOptions {
            image_quality: Some(75),
            image_max_width: Some(200),
            video_preset: None,
        };
        convert_image(&src, &dst_jpg, "jpg", Some(&opts)).unwrap();
        let loaded_jpg = image::open(&dst_jpg).unwrap();
        assert_eq!(loaded_jpg.width(), 200);

        convert_image(&src, &dst_ico, "ico", None).unwrap();
        let loaded_ico = image::open(&dst_ico).unwrap();
        assert!(loaded_ico.width() <= 256 && loaded_ico.height() <= 256);

        convert_image(&src, &dst_pdf, "pdf", None).unwrap();
        let pdf_bytes = fs::read(&dst_pdf).unwrap();
        assert!(pdf_bytes.starts_with(b"%PDF-"));

        let _ = fs::remove_dir_all(&dir);
    }
}
