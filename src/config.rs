use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct ConnectionProfile {
    pub name: String,
    pub url: String,
    pub db: Option<u8>,
}

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct Config {
    #[serde(rename = "connections")]
    pub profiles: Vec<ConnectionProfile>,
}

impl Config {
    fn default_config() -> Self {
        Config {
            profiles: vec![
                ConnectionProfile {
                    name: "Default Local".to_string(),
                    url: "redis://127.0.0.1:6379".to_string(),
                    db: None,
                }
            ]
        }
    }

    pub fn load() -> Self {
        if let Some(base_dirs) = directories::BaseDirs::new() {
            let config_dir = base_dirs.config_dir().join("lazyredis");
            let config_file_path = config_dir.join("lazyredis.toml");

            if config_file_path.exists() {
                match fs::read_to_string(&config_file_path) {
                    Ok(contents) => match toml::from_str(&contents) {
                        Ok(config) => return config,
                        Err(e) => {
                            eprintln!(
                                "Failed to parse config file at '{}': {}. Using default in-memory config.", 
                                config_file_path.display(), e
                            );
                            // Fall through to return default_config without writing
                        }
                    },
                    Err(e) => {
                        eprintln!(
                            "Failed to read config file at '{}': {}. Using default in-memory config.",
                            config_file_path.display(), e
                        );
                        // Fall through to return default_config without writing
                    }
                }
            } else {
                eprintln!("Config file not found at '{}'. Attempting to create a default one.", config_file_path.display());
                let default_cfg = Self::default_config();
                match toml::to_string_pretty(&default_cfg) {
                    Ok(toml_string) => {
                        if let Err(e) = fs::create_dir_all(&config_dir) {
                            eprintln!("Failed to create config directory '{}': {}", config_dir.display(), e);
                        } else {
                            if let Err(e) = fs::write(&config_file_path, toml_string) {
                                eprintln!("Failed to write default config file to '{}': {}", config_file_path.display(), e);
                            } else {
                                eprintln!("Default config file created at '{}'", config_file_path.display());
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to serialize default config: {}", e);
                    }
                }
                return default_cfg;
            }
        }
        // If BaseDirs::new() fails or other paths, return default.
        eprintln!("Could not determine config directory. Using default in-memory config.");
        Self::default_config()
    }
} 