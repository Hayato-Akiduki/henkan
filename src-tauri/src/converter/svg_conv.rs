use super::image_conv;
use super::{ConvertError, ConvertOptions, TargetFormat};
use image::{DynamicImage, RgbaImage};
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree};
use std::fs;
use std::path::Path;

pub fn is_supported_extension(ext: &str) -> bool {
    ext == "svg"
}

pub fn available_targets() -> Vec<TargetFormat> {
    vec![
        TargetFormat {
            id: "png".to_string(),
            label: "PNG".to_string(),
            extension: "png".to_string(),
            category: "image".to_string(),
            description: "透過対応 PNG に書き出し（内蔵）".to_string(),
        },
        TargetFormat {
            id: "jpg".to_string(),
            label: "JPEG".to_string(),
            extension: "jpg".to_string(),
            category: "image".to_string(),
            description: "白背景 JPEG に書き出し（内蔵）".to_string(),
        },
        TargetFormat {
            id: "webp".to_string(),
            label: "WebP".to_string(),
            extension: "webp".to_string(),
            category: "image".to_string(),
            description: "Web 向け高圧縮画像（内蔵）".to_string(),
        },
        TargetFormat {
            id: "ico".to_string(),
            label: "ICO".to_string(),
            extension: "ico".to_string(),
            category: "image".to_string(),
            description: "アイコン形式（最大256px・内蔵）".to_string(),
        },
        TargetFormat {
            id: "pdf".to_string(),
            label: "PDF".to_string(),
            extension: "pdf".to_string(),
            category: "document".to_string(),
            description: "1ページの PDF として保存（内蔵）".to_string(),
        },
    ]
}

pub fn convert_svg(
    input_path: &Path,
    output_path: &Path,
    target_format: &str,
    options: Option<&ConvertOptions>,
) -> Result<(), ConvertError> {
    let svg_data = fs::read(input_path)?;
    let usvg_opt = Options::default();
    let tree = Tree::from_data(&svg_data, &usvg_opt)
        .map_err(|e| ConvertError::ConversionFailed(format!("SVG の解析に失敗しました: {e}")))?;

    let orig_size = tree.size();
    let orig_w = orig_size.width().max(1.0);
    let orig_h = orig_size.height().max(1.0);

    // オプションで最大幅が指定されていればそれに合わせ、未指定なら最低 512px 以上で見やすくレンダリング
    let scale = if let Some(max_w) = options.and_then(|o| o.image_max_width) {
        (max_w as f32 / orig_w).max(0.05)
    } else if orig_w < 512.0 && target_format != "ico" {
        (512.0 / orig_w).min(4.0)
    } else {
        1.0
    };

    let target_w = ((orig_w * scale).round() as u32).clamp(1, 8192);
    let target_h = ((orig_h * scale).round() as u32).clamp(1, 8192);

    let mut pixmap = Pixmap::new(target_w, target_h).ok_or_else(|| {
        ConvertError::ConversionFailed("SVG 描画バッファの確保に失敗しました".to_string())
    })?;

    let transform = Transform::from_scale(target_w as f32 / orig_w, target_h as f32 / orig_h);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // tiny-skia の premultiplied RGBA を straight RGBA に変換
    let mut raw = pixmap.take();
    for px in raw.chunks_exact_mut(4) {
        let a = px[3];
        if a > 0 && a < 255 {
            let af = a as f32 / 255.0;
            px[0] = ((px[0] as f32 / af).min(255.0)) as u8;
            px[1] = ((px[1] as f32 / af).min(255.0)) as u8;
            px[2] = ((px[2] as f32 / af).min(255.0)) as u8;
        }
    }

    let rgba = RgbaImage::from_raw(target_w, target_h, raw).ok_or_else(|| {
        ConvertError::ConversionFailed("SVG 画像変換に失敗しました".to_string())
    })?;
    let dyn_img = DynamicImage::ImageRgba8(rgba);

    image_conv::save_dynamic_image(&dyn_img, output_path, target_format, options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_svg_to_png_and_ico() {
        let dir = std::env::temp_dir().join("henkan_test_svg");
        let _ = fs::create_dir_all(&dir);
        let src = dir.join("icon.svg");
        let dst_png = dir.join("icon.png");
        let dst_ico = dir.join("icon.ico");

        fs::write(
            &src,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><circle cx="50" cy="50" r="40" fill="#6366f1"/></svg>"##,
        )
        .unwrap();

        convert_svg(&src, &dst_png, "png", None).unwrap();
        assert!(dst_png.exists() && fs::metadata(&dst_png).unwrap().len() > 0);

        convert_svg(&src, &dst_ico, "ico", None).unwrap();
        assert!(dst_ico.exists() && fs::metadata(&dst_ico).unwrap().len() > 0);

        let _ = fs::remove_dir_all(&dir);
    }
}
