use super::tools::{new_hidden_command, resolve_ffmpeg_program};
use super::{ConvertError, ConvertOptions, TargetFormat};
use std::path::Path;

const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mov", "mkv", "avi", "webm", "wmv", "flv", "m4v", "ts", "mts",
];

const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "wav", "flac", "aac", "m4a", "ogg", "opus", "wma", "aiff", "aif",
];

pub fn is_video_extension(ext: &str) -> bool {
    VIDEO_EXTENSIONS.contains(&ext)
}

pub fn is_audio_extension(ext: &str) -> bool {
    AUDIO_EXTENSIONS.contains(&ext)
}

pub fn video_targets(input_ext: &str) -> Vec<TargetFormat> {
    let all = vec![
        TargetFormat {
            id: "mp4".to_string(),
            label: "MP4 (H.264)".to_string(),
            extension: "mp4".to_string(),
            category: "video".to_string(),
            description: "最も互換性の高い動画形式（FFmpeg）".to_string(),
        },
        TargetFormat {
            id: "webm".to_string(),
            label: "WebM (VP9)".to_string(),
            extension: "webm".to_string(),
            category: "video".to_string(),
            description: "Web 向け高圧縮動画（FFmpeg）".to_string(),
        },
        TargetFormat {
            id: "mkv".to_string(),
            label: "MKV".to_string(),
            extension: "mkv".to_string(),
            category: "video".to_string(),
            description: "汎用コンテナ形式（FFmpeg）".to_string(),
        },
        TargetFormat {
            id: "mov".to_string(),
            label: "MOV".to_string(),
            extension: "mov".to_string(),
            category: "video".to_string(),
            description: "Apple / 編集ソフト向け（FFmpeg）".to_string(),
        },
        TargetFormat {
            id: "gif".to_string(),
            label: "GIF アニメーション".to_string(),
            extension: "gif".to_string(),
            category: "video".to_string(),
            description: "動画から GIF を生成（FFmpeg）".to_string(),
        },
        TargetFormat {
            id: "mp3".to_string(),
            label: "MP3（音声抽出）".to_string(),
            extension: "mp3".to_string(),
            category: "audio".to_string(),
            description: "動画から音声のみ抽出（FFmpeg）".to_string(),
        },
        TargetFormat {
            id: "wav".to_string(),
            label: "WAV（音声抽出）".to_string(),
            extension: "wav".to_string(),
            category: "audio".to_string(),
            description: "無圧縮音声として抽出（FFmpeg）".to_string(),
        },
        TargetFormat {
            id: "m4a".to_string(),
            label: "M4A / AAC（音声抽出）".to_string(),
            extension: "m4a".to_string(),
            category: "audio".to_string(),
            description: "高音質・軽量音声抽出（FFmpeg）".to_string(),
        },
    ];

    include_same_format(all, input_ext)
}

pub fn audio_targets(input_ext: &str) -> Vec<TargetFormat> {
    let all = vec![
        TargetFormat {
            id: "mp3".to_string(),
            label: "MP3".to_string(),
            extension: "mp3".to_string(),
            category: "audio".to_string(),
            description: "定番の圧縮音声（FFmpeg）".to_string(),
        },
        TargetFormat {
            id: "wav".to_string(),
            label: "WAV".to_string(),
            extension: "wav".to_string(),
            category: "audio".to_string(),
            description: "無圧縮・高音質（FFmpeg）".to_string(),
        },
        TargetFormat {
            id: "flac".to_string(),
            label: "FLAC".to_string(),
            extension: "flac".to_string(),
            category: "audio".to_string(),
            description: "可逆圧縮・ロスレス（FFmpeg）".to_string(),
        },
        TargetFormat {
            id: "m4a".to_string(),
            label: "M4A (AAC)".to_string(),
            extension: "m4a".to_string(),
            category: "audio".to_string(),
            description: "高効率・高音質（FFmpeg）".to_string(),
        },
        TargetFormat {
            id: "ogg".to_string(),
            label: "OGG (Vorbis)".to_string(),
            extension: "ogg".to_string(),
            category: "audio".to_string(),
            description: "オープンな圧縮音声（FFmpeg）".to_string(),
        },
        TargetFormat {
            id: "opus".to_string(),
            label: "Opus".to_string(),
            extension: "opus".to_string(),
            category: "audio".to_string(),
            description: "低ビットレート高音質（FFmpeg）".to_string(),
        },
    ];

    include_same_format(all, input_ext)
}

fn include_same_format(mut targets: Vec<TargetFormat>, input_ext: &str) -> Vec<TargetFormat> {
    for target in &mut targets {
        if target.id == input_ext {
            target.description = "同じ形式で再変換（FFmpeg）".to_string();
        }
    }
    targets
}

pub fn convert_media(
    input_path: &Path,
    output_path: &Path,
    target_format: &str,
    ffmpeg_path: Option<&str>,
    app_data_dir: Option<&Path>,
    options: Option<&ConvertOptions>,
) -> Result<(), ConvertError> {
    let program = resolve_ffmpeg_program(ffmpeg_path, app_data_dir);
    let mut cmd = new_hidden_command(&program);

    cmd.arg("-y").arg("-i").arg(input_path);

    let preset = options
        .and_then(|o| o.video_preset.as_deref())
        .unwrap_or("default");

    match target_format {
        "mp4" | "mov" | "mkv" => {
            cmd.args(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac"]);
            match preset {
                "compact" => {
                    // SNS / 共有向け軽量化（最大高さ 720p、CRF 28、音声 128k）
                    cmd.args([
                        "-vf",
                        "scale=-2:'min(720,ih)'",
                        "-crf",
                        "28",
                        "-b:a",
                        "128k",
                    ]);
                }
                "high" => {
                    cmd.args(["-crf", "18", "-b:a", "256k"]);
                }
                _ => {
                    cmd.args(["-crf", "23"]);
                }
            }
        }
        "webm" => {
            let crf = match preset {
                "compact" => "36",
                "high" => "24",
                _ => "32",
            };
            cmd.args(["-c:v", "libvpx-vp9", "-b:v", "0", "-crf", crf, "-c:a", "libopus"]);
            if preset == "compact" {
                cmd.args(["-vf", "scale=-2:'min(720,ih)'"]);
            }
        }
        "gif" => {
            let width = if preset == "compact" { "480" } else { "640" };
            let vf = format!(
                "fps=12,scale='min({width},iw)':-1:flags=lanczos,split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse"
            );
            cmd.args(["-vf", &vf]);
        }
        "mp3" => {
            cmd.args(["-vn", "-c:a", "libmp3lame", "-q:a", "2"]);
        }
        "wav" => {
            cmd.args(["-vn", "-c:a", "pcm_s16le"]);
        }
        "flac" => {
            cmd.args(["-vn", "-c:a", "flac"]);
        }
        "m4a" => {
            cmd.args(["-vn", "-c:a", "aac", "-b:a", "192k"]);
        }
        "ogg" => {
            cmd.args(["-vn", "-c:a", "libvorbis", "-q:a", "5"]);
        }
        "opus" => {
            cmd.args(["-vn", "-c:a", "libopus", "-b:a", "128k"]);
        }
        other => return Err(ConvertError::UnsupportedTarget(other.to_string())),
    }

    cmd.arg(output_path);

    let output = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ConvertError::ConversionFailed(
                "FFmpeg が見つかりません。画面上部の「FFmpeg」ボタンから「自動ダウンロード」を実行してください。".to_string(),
            )
        } else {
            ConvertError::ConversionFailed(format!("FFmpeg の起動に失敗しました: {e}"))
        }
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail: Vec<&str> = stderr.lines().rev().take(4).collect();
        let detail = tail.into_iter().rev().collect::<Vec<_>>().join(" / ");
        return Err(ConvertError::ConversionFailed(format!(
            "FFmpeg 変換エラー: {detail}"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_media_extension_detection() {
        assert!(is_video_extension("mp4"));
        assert!(is_video_extension("mkv"));
        assert!(!is_video_extension("png"));

        assert!(is_audio_extension("mp3"));
        assert!(is_audio_extension("flac"));
        assert!(!is_audio_extension("mp4"));

        let mp4_targets = video_targets("mp4");
        assert!(mp4_targets.iter().any(|t| t.id == "mp4"));
        assert!(mp4_targets.iter().any(|t| t.id == "mp3"));
        assert!(mp4_targets.iter().any(|t| t.id == "gif"));
        for ext in ["mp4", "webm", "mkv", "mov"] {
            assert!(video_targets(ext).iter().any(|t| t.id == ext));
        }
        for ext in ["mp3", "wav", "flac", "m4a", "ogg", "opus"] {
            assert!(audio_targets(ext).iter().any(|t| t.id == ext));
        }
    }

    #[test]
    fn test_ffmpeg_real_conversion_if_available() {
        use super::super::tools::detect_tool;
        use std::fs;

        if !detect_tool(None, "ffmpeg", "-version").available {
            return;
        }

        let dir = std::env::temp_dir().join("henkan_test_ffmpeg");
        let _ = fs::create_dir_all(&dir);
        let src_mp4 = dir.join("sample.mp4");
        let dst_gif = dir.join("sample.gif");
        let dst_mp3 = dir.join("sample.mp3");
        let dst_wav = dir.join("sample.wav");

        let generated = new_hidden_command("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=1:size=160x120:rate=10",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=1",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
            ])
            .arg(&src_mp4)
            .output()
            .unwrap();
        assert!(generated.status.success(), "テスト用 MP4 生成に失敗");

        // 同形式の再圧縮でも元ファイルを保持し、別名で出力する。
        let original = fs::read(&src_mp4).unwrap();
        let request = super::super::ConvertRequest {
            input_path: src_mp4.to_string_lossy().to_string(),
            target_format: "mp4".to_string(),
            output_dir: None,
            ffmpeg_path: None,
            pandoc_path: None,
            options: Some(ConvertOptions {
                video_preset: Some("compact".to_string()),
                ..Default::default()
            }),
        };
        let result = super::super::convert_file(&request, None).unwrap();
        assert_ne!(Path::new(&result.output_path), src_mp4.as_path());
        assert!(result.output_size > 0);
        assert_eq!(fs::read(&src_mp4).unwrap(), original);

        convert_media(&src_mp4, &dst_gif, "gif", None, None, None).unwrap();
        assert!(dst_gif.exists() && fs::metadata(&dst_gif).unwrap().len() > 0);

        convert_media(&src_mp4, &dst_mp3, "mp3", None, None, None).unwrap();
        assert!(dst_mp3.exists() && fs::metadata(&dst_mp3).unwrap().len() > 0);

        convert_media(&dst_mp3, &dst_wav, "wav", None, None, None).unwrap();
        assert!(dst_wav.exists() && fs::metadata(&dst_wav).unwrap().len() > 0);

        let _ = fs::remove_dir_all(&dir);
    }
}
