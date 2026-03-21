use anyhow::{Context, Result};
use chaos_core::livestream::model::LiveManifest;
use std::process::{Command, Stdio};
use tracing::{info, warn};

/// 支持的外部播放器
#[derive(Debug, Clone, Copy)]
pub enum Player {
    IINAPlus,
    PotPlayer,
    VLC,
}

impl Player {
    /// 自动检测可用播放器
    pub fn detect() -> Self {
        #[cfg(target_os = "macos")]
        {
            if Self::is_available(Self::IINAPlus) {
                return Self::IINAPlus;
            }
        }

        #[cfg(target_os = "windows")]
        {
            if Self::is_available(Self::PotPlayer) {
                return Self::PotPlayer;
            }
        }

        if Self::is_available(Self::VLC) {
            return Self::VLC;
        }

        #[cfg(target_os = "macos")]
        {
            if Self::is_available(Self::VLC) {
                return Self::VLC;
            }
        }

        // 默认返回 VLC，即使可能不可用（会在 play 时报错）
        Self::VLC
    }

    /// 检查播放器是否可用
    fn is_available(player: Player) -> bool {
        match player {
            Player::IINAPlus => Self::check_command("iina"),
            Player::PotPlayer => Self::check_windows_app("PotPlayer"),
            Player::VLC => Self::check_command("vlc"),
        }
    }

    fn check_command(cmd: &str) -> bool {
        Command::new("which")
            .arg(cmd)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    #[cfg(target_os = "windows")]
    fn check_windows_app(_app: &str) -> bool {
        // 检查常见的安装路径
        let program_files = std::env::var("ProgramFiles").unwrap_or_default();
        let program_files_x86 = std::env::var("ProgramFiles(x86)").unwrap_or_default();

        let paths = [
            format!("{}\\DAUM\\PotPlayer\\PotPlayerMini.exe", program_files),
            format!("{}\\DAUM\\PotPlayer\\PotPlayerMini64.exe", program_files),
            format!("{}\\DAUM\\PotPlayer\\PotPlayerMini.exe", program_files_x86),
            format!(
                "{}\\DAUM\\PotPlayer\\PotPlayerMini64.exe",
                program_files_x86
            ),
        ];

        paths.iter().any(|p| std::path::Path::new(p).exists())
    }

    #[cfg(not(target_os = "windows"))]
    fn check_windows_app(_app: &str) -> bool {
        false
    }

    /// 启动播放器播放流
    pub fn play(&self, url: &str, manifest: &LiveManifest) -> Result<()> {
        info!("Launching player: {:?}", self);

        match self {
            Player::IINAPlus => self.play_with_iina(url, manifest),
            Player::PotPlayer => self.play_with_potplayer(url, manifest),
            Player::VLC => self.play_with_vlc(url, manifest),
        }
    }

    fn play_with_iina(&self, url: &str, _manifest: &LiveManifest) -> Result<()> {
        let mut cmd = Command::new("iina");
        cmd.arg(url);

        info!("Running: iina {}", url);

        cmd.spawn()
            .context("Failed to launch IINA+. Make sure IINA+ is installed and in PATH")?;

        Ok(())
    }

    fn play_with_potplayer(&self, url: &str, _manifest: &LiveManifest) -> Result<()> {
        // 查找 PotPlayer 可执行文件
        let potplayer_exe = self.find_potplayer_exe()?;

        let mut cmd = Command::new(potplayer_exe);
        cmd.arg(url);

        info!("Running: PotPlayer {}", url);

        cmd.spawn().context("Failed to launch PotPlayer")?;

        Ok(())
    }

    fn find_potplayer_exe(&self) -> Result<std::path::PathBuf> {
        #[cfg(target_os = "windows")]
        {
            let program_files = std::env::var("ProgramFiles").unwrap_or_default();
            let program_files_x86 = std::env::var("ProgramFiles(x86)").unwrap_or_default();

            let candidates = [
                format!("{}\\DAUM\\PotPlayer\\PotPlayerMini64.exe", program_files),
                format!("{}\\DAUM\\PotPlayer\\PotPlayerMini.exe", program_files),
                format!(
                    "{}\\DAUM\\PotPlayer\\PotPlayerMini64.exe",
                    program_files_x86
                ),
                format!("{}\\DAUM\\PotPlayer\\PotPlayerMini.exe", program_files_x86),
            ];

            for path in &candidates {
                let p = std::path::Path::new(path);
                if p.exists() {
                    return Ok(p.to_path_buf());
                }
            }
        }

        anyhow::bail!("PotPlayer not found. Please install PotPlayer or add it to PATH")
    }

    fn play_with_vlc(&self, url: &str, _manifest: &LiveManifest) -> Result<()> {
        let vlc_cmd = if cfg!(target_os = "windows") {
            "vlc.exe"
        } else {
            "vlc"
        };

        let mut cmd = Command::new(vlc_cmd);
        cmd.arg(url);

        // VLC 在 Windows 上可能需要额外参数
        if cfg!(target_os = "windows") {
            cmd.arg("--intf=qt");
        }

        info!("Running: {} {}", vlc_cmd, url);

        match cmd.spawn() {
            Ok(_) => Ok(()),
            Err(e) => {
                warn!("Failed to launch VLC: {}", e);
                anyhow::bail!(
                    "Failed to launch VLC. Please install VLC from https://www.videolan.org/vlc/"
                )
            }
        }
    }
}
