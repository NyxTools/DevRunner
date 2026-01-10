use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use anyhow::Result;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub ignore_paths: Vec<String>,
    #[serde(default)]
    pub custom_scripts: Vec<CustomScript>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CustomScript {
    pub name: String,
    pub command: String,
}

pub fn load_config(path: Option<PathBuf>, root_dir: &Path) -> Result<AppConfig> {
    if let Some(p) = path {
        if p.exists() {
            let content = fs::read_to_string(p)?;
            // Try parsing as JSON first, then TOML
            let config: AppConfig = serde_json::from_str(&content).or_else(|_| {
                toml::from_str(&content)
            })?;
            return Ok(config);
        }
    }

    let possible_names = [".devrunner.json", "devrunner.json", ".devrunner.toml", "devrunner.toml"];
    for name in possible_names {
        let p = root_dir.join(name);
        if p.exists() {
            let content = fs::read_to_string(p)?;
            if name.ends_with(".toml") {
                let config: AppConfig = toml::from_str(&content)?;
                return Ok(config);
            } else {
                 let config: AppConfig = serde_json::from_str(&content)?;
                 return Ok(config);
            }
        }
    }

    Ok(AppConfig::default())
}
