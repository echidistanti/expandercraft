//! Location of the data files. The Portable Edition (a `Portable.bin` file next to the executable) stores
//! everything in a `Data` sub-folder, like Beeftext.

use std::path::{Path, PathBuf};

pub const APP_NAME: &str = "ExpanderCraft";
pub const PORTABLE_BEACON: &str = "Portable.bin";

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub portable: bool,
}

impl AppPaths {
    pub fn detect() -> Self {
        let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf));
        if let Some(dir) = &exe_dir
            && dir.join(PORTABLE_BEACON).exists()
        {
            return Self { data_dir: dir.join("Data"), portable: true };
        }
        let base = directories::BaseDirs::new()
            .map(|b| b.data_local_dir().to_path_buf())
            .unwrap_or_else(|| exe_dir.clone().unwrap_or_else(|| PathBuf::from(".")));
        let folder = if cfg!(target_os = "linux") { "expandercraft" } else { APP_NAME };
        Self { data_dir: base.join(folder), portable: false }
    }

    pub fn ensure_exists(&self) {
        if let Err(e) = std::fs::create_dir_all(&self.data_dir) {
            eprintln!("Could not create the data folder '{}': {e}", self.data_dir.display());
        }
    }

    pub fn settings_file(&self) -> PathBuf {
        self.data_dir.join("settings.json")
    }
    pub fn log_file(&self) -> PathBuf {
        self.data_dir.join("log.txt")
    }
    pub fn last_use_file(&self) -> PathBuf {
        self.data_dir.join(crate::library::LAST_USE_FILE_NAME)
    }
    pub fn emoji_last_use_file(&self) -> PathBuf {
        self.data_dir.join("emojiLastUse.json")
    }
    pub fn excluded_apps_file(&self) -> PathBuf {
        self.data_dir.join("excludedApps.json")
    }
    pub fn sensitive_apps_file(&self) -> PathBuf {
        self.data_dir.join("sensitiveApps.json")
    }
    pub fn emoji_excluded_apps_file(&self) -> PathBuf {
        self.data_dir.join("emojiExcludedApps.json")
    }
    pub fn default_backup_dir(&self) -> PathBuf {
        self.data_dir.join("Backup")
    }
    pub fn instance_socket(&self) -> PathBuf {
        self.data_dir.join("expandercraft.sock")
    }
}

/// Possible locations of an existing Beeftext combo list, used to offer a migration on first launch.
pub fn beeftext_library_candidates() -> Vec<PathBuf> {
    let mut result = Vec::new();
    if let Some(b) = directories::BaseDirs::new() {
        for base in [b.data_local_dir(), b.data_dir()] {
            result.push(base.join("beeftext.org").join("Beeftext").join(crate::library::LIBRARY_FILE_NAME));
            result.push(base.join("Beeftext").join(crate::library::LIBRARY_FILE_NAME));
        }
    }
    result.into_iter().filter(|p| p.is_file()).collect()
}
