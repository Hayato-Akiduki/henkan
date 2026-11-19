use serde::Serialize;
use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    pub available: bool,
    pub command: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsOverview {
    pub ffmpeg: ToolStatus,
    pub pandoc: ToolStatus,
}

/// Windows でコンソールウィンドウが一瞬表示されるのを防ぐ Command 生成ヘルパー
pub fn new_hidden_command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

pub fn resolve_program(custom_path: Option<&str>, default_name: &str) -> String {
    match custom_path {
        Some(p) if !p.trim().is_empty() => p.trim().to_string(),
        _ => default_name.to_string(),
    }
}

pub fn detect_tool(custom_path: Option<&str>, default_name: &str, version_arg: &str) -> ToolStatus {
    let program = resolve_program(custom_path, default_name);
    let output = new_hidden_command(&program).arg(version_arg).output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let first_line = stdout
                .lines()
                .next()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            ToolStatus {
                available: true,
                command: program,
                version: first_line,
            }
        }
        _ => ToolStatus {
            available: false,
            command: program,
            version: None,
        },
    }
}

pub fn check_all_tools(ffmpeg_path: Option<&str>, pandoc_path: Option<&str>) -> ToolsOverview {
    ToolsOverview {
        ffmpeg: detect_tool(ffmpeg_path, "ffmpeg", "-version"),
        pandoc: detect_tool(pandoc_path, "pandoc", "--version"),
    }
}
