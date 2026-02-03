use std::path::PathBuf;
use std::fs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub target: String,
    pub green_threshold: u64,
    pub yellow_threshold: u64,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            target: "8.8.8.8".to_string(),
            green_threshold: 100,
            yellow_threshold: 200,
        }
    }
}

impl AppConfig {
    pub fn get_config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let config_dir = dirs::config_dir()
            .ok_or("Could not find config directory")?
            .join("PingMonitor");
        
        fs::create_dir_all(&config_dir)?;
        Ok(config_dir.join("config.json"))
    }

    pub fn load() -> Self {
        Self::get_config_path()
            .ok()
            .and_then(|path| {
                if path.exists() {
                    fs::read_to_string(&path)
                        .ok()
                        .and_then(|content| serde_json::from_str::<AppConfig>(&content).ok())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| {
                AppConfig::default()
            })
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::get_config_path()?;
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }
}
