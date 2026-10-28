use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme: String,
    pub skin: String,
    pub nickname: String,
    pub use_default_names: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsFile {
    pub settings: AppSettings,
    pub path: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            skin: "blue".into(),
            nickname: String::new(),
            use_default_names: false,
        }
    }
}

pub fn settings_path() -> Result<PathBuf, String> {
    let dir = dirs::data_local_dir()
        .ok_or_else(|| "无法解析本地数据目录".to_string())?
        .join("com.tal.notebook");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("settings.json"))
}

pub fn load() -> Result<SettingsFile, String> {
    let path = settings_path()?;
    let settings = if path.is_file() {
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&text).unwrap_or_else(|_| AppSettings::default())
    } else {
        let settings = AppSettings::default();
        let text = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
        fs::write(&path, text).map_err(|e| e.to_string())?;
        settings
    };
    Ok(SettingsFile {
        path: path.to_string_lossy().to_string(),
        settings,
    })
}

pub fn save(settings: AppSettings) -> Result<SettingsFile, String> {
    let path = settings_path()?;
    let theme = match settings.theme.as_str() {
        "light" | "dark" | "system" => settings.theme,
        _ => "system".into(),
    };
    let skin = match settings.skin.as_str() {
        "blue" | "green" | "violet" | "amber" | "slate" => settings.skin,
        _ => "blue".into(),
    };
    let stored = AppSettings {
        theme,
        skin,
        nickname: settings.nickname.trim().chars().take(24).collect(),
        use_default_names: settings.use_default_names,
    };
    let text = serde_json::to_string_pretty(&stored).map_err(|e| e.to_string())?;
    fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(SettingsFile {
        path: path.to_string_lossy().to_string(),
        settings: stored,
    })
}
