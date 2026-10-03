use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct FluffyConfig {
    #[serde(default)]
    pub startup_and_wallpaper: StartupAndWallpaperSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct StartupAndWallpaperSettings {
    /// Whether to automatically restore previous wallpaper on daemon startup/login (default: false)
    #[serde(default)]
    pub restore_on_startup: bool,
    /// Whether daemon is configured to start automatically on login
    #[serde(default)]
    pub autostart_daemon: bool,
    /// Wallpaper behavior: pause playback when window is maximized or in fullscreen
    #[serde(default)]
    pub pause_on_fullscreen: bool,
}

impl FluffyConfig {
    pub fn default_config_path() -> PathBuf {
        let config_dir = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        config_dir.join("fluffy").join("config.json")
    }

    pub fn load() -> Self {
        Self::load_from(Self::default_config_path()).unwrap_or_default()
    }

    pub fn load_from<P: AsRef<Path>>(path: P) -> Result<Self> {
        let p = path.as_ref();
        if !p.exists() {
            return Ok(Self::default());
        }
        let data = fs::read_to_string(p)?;
        let config: Self = serde_json::from_str(&data)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        Self::save_to(self, Self::default_config_path())
    }

    pub fn save_to<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let p = path.as_ref();
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        let tmp_path = p.with_extension("tmp");
        fs::write(&tmp_path, json)?;
        fs::rename(&tmp_path, p)?;
        Ok(())
    }
}

/// Stores last known wallpaper state per output for optional restoration.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct DaemonState {
    #[serde(default)]
    pub outputs: HashMap<String, String>, // output_name -> video_path
}

impl DaemonState {
    pub fn default_state_path() -> PathBuf {
        let state_dir = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        state_dir.join("fluffy").join("state.json")
    }

    pub fn load() -> Self {
        Self::load_from(Self::default_state_path()).unwrap_or_default()
    }

    pub fn load_from<P: AsRef<Path>>(path: P) -> Result<Self> {
        let p = path.as_ref();
        if !p.exists() {
            return Ok(Self::default());
        }
        let data = fs::read_to_string(p)?;
        let state: Self = serde_json::from_str(&data)?;
        Ok(state)
    }

    pub fn save(&self) -> Result<()> {
        Self::save_to(self, Self::default_state_path())
    }

    pub fn save_to<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let p = path.as_ref();
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        let tmp_path = p.with_extension("tmp");
        fs::write(&tmp_path, json)?;
        fs::rename(&tmp_path, p)?;
        Ok(())
    }

    pub fn record_output_video(&mut self, output: &str, video_path: &Path) {
        self.outputs
            .insert(output.to_string(), video_path.to_string_lossy().to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDirGuard(PathBuf);
    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn test_config_default_is_not_restore_on_startup() {
        let cfg = FluffyConfig::default();
        assert!(!cfg.startup_and_wallpaper.restore_on_startup);
        assert!(!cfg.startup_and_wallpaper.autostart_daemon);
        assert!(!cfg.startup_and_wallpaper.pause_on_fullscreen);
    }

    #[test]
    fn test_config_save_load_roundtrip() {
        let tmp_dir = std::env::temp_dir().join(format!("fluffy_cfg_test_{}", std::process::id()));
        let _guard = TempDirGuard(tmp_dir.clone());
        let cfg_path = tmp_dir.join("config.json");

        let mut cfg = FluffyConfig::default();
        cfg.startup_and_wallpaper.restore_on_startup = true;
        cfg.startup_and_wallpaper.autostart_daemon = true;

        cfg.save_to(&cfg_path).unwrap();

        let loaded = FluffyConfig::load_from(&cfg_path).unwrap();
        assert_eq!(loaded, cfg);
        assert!(loaded.startup_and_wallpaper.restore_on_startup);
    }

    #[test]
    fn test_daemon_state_save_load_roundtrip() {
        let tmp_dir =
            std::env::temp_dir().join(format!("fluffy_state_test_{}", std::process::id()));
        let _guard = TempDirGuard(tmp_dir.clone());
        let state_path = tmp_dir.join("state.json");

        let mut state = DaemonState::default();
        state.record_output_video("DP-1", Path::new("/path/to/video1.mp4"));
        state.record_output_video("HDMI-A-1", Path::new("/path/to/video2.mp4"));

        state.save_to(&state_path).unwrap();

        let loaded = DaemonState::load_from(&state_path).unwrap();
        assert_eq!(loaded.outputs.get("DP-1").unwrap(), "/path/to/video1.mp4");
        assert_eq!(
            loaded.outputs.get("HDMI-A-1").unwrap(),
            "/path/to/video2.mp4"
        );
    }
}
