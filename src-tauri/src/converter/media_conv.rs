use super::tools::{new_hidden_command, resolve_program};
use super::{ConvertError, TargetFormat};
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

    all.into_iter().filter(|t| t.id != input_ext).collect()
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

    all.into_iter().filter(|t| t.id != input_ext).collect()
}

pub fn convert_media(
    input_path: &Path,
    output_path: &Path,
    target_format: &str,
    ffmpeg_path: Option<&str>,
) -> Result<(), ConvertError> {
    let program = resolve_program(ffmpeg_path, "ffmpeg");
    let mut cmd = new_hidden_command(&program);

    cmd.arg("-y").arg("-i").arg(input_path);

    match target_format {
        "mp4" => {
            cmd.args(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac"]);
        }
        "webm" => {
            cmd.args(["-c:v", "libvpx-vp9", "-b:v", "0", "-crf", "32", "-c:a", "libopus"]);
        }
        "mkv" => {
            cmd.args(["-c:v", "libx264", "-c:a", "aac"]);
        }
        "mov" => {
            cmd.args(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac"]);
        }
        "gif" => {
            // パレットを自動生成して高品質な GIF に変換（幅最大 640px・12fps）
            cmd.args([
                "-vf",
                "fps=12,scale='min(640,iw)':-1:flags=lanczos,split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse",
            ]);
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
                "FFmpeg が見つかりません。PC に FFmpeg をインストールするか、画面上部のツール設定から ffmpeg 実行ファイルを指定してください。".to_string(),
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
        assert!(!mp4_targets.iter().any(|t| t.id == "mp4"));
        assert!(mp4_targets.iter().any(|t| t.id == "mp3"));
        assert!(mp4_targets.iter().any(|t| t.id == "gif"));
    }
}
