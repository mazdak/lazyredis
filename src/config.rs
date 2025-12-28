use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}};

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct ConnectionProfile {
    pub name: String,
    pub url: String,
    pub db: Option<u8>,
    pub dev: Option<bool>,
    pub color: Option<String>,
}

impl ConnectionProfile {
    pub fn resolved_color(&self) -> Color {
        self.color
            .as_deref()
            .map(parse_color)
            .unwrap_or(Color::White)
    }
}

fn parse_color(spec: &str) -> Color {
    match spec.trim().to_lowercase().as_str() {
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "gray" | "grey" => Color::Gray,
        "darkgray" | "darkgrey" => Color::DarkGray,
        "lightred" => Color::LightRed,
        "lightgreen" => Color::LightGreen,
        "lightyellow" => Color::LightYellow,
        "lightblue" => Color::LightBlue,
        "lightmagenta" => Color::LightMagenta,
        "lightcyan" => Color::LightCyan,
        "white" => Color::White,
        other => {
            if let Some(hex) = other.strip_prefix('#') {
                if hex.len() == 6 {
                    if let (Ok(r), Ok(g), Ok(b)) = (
                        u8::from_str_radix(&hex[0..2], 16),
                        u8::from_str_radix(&hex[2..4], 16),
                        u8::from_str_radix(&hex[4..6], 16),
                    ) {
                        return Color::Rgb(r, g, b);
                    }
                }
            }
            Color::White
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Default, PartialEq)]
pub struct Config {
    #[serde(rename = "connections")]
    pub profiles: Vec<ConnectionProfile>,
}

impl Config {
    fn default_config() -> Self {
        Config {
            profiles: vec![ConnectionProfile {
                name: "Default".to_string(),
                url: "redis://127.0.0.1:6379".to_string(),
                db: Some(0),
                dev: Some(true),
                color: Some("green".to_string()),
            }],
        }
    }

    fn determine_config_file_path(base_path_override: Option<&Path>) -> Option<PathBuf> {
        if let Some(base_path) = base_path_override {
            Some(base_path.join("lazyredis").join("lazyredis.toml"))
        } else {
            directories::BaseDirs::new().map(|base_dirs| {
                base_dirs
                    .config_dir()
                    .join("lazyredis")
                    .join("lazyredis.toml")
            })
        }
    }

    fn load_internal(base_path_override: Option<&Path>, emit_logs: bool) -> Self {
        let log = |message: String| {
            if emit_logs {
                eprintln!("{}", message);
            }
        };

        if let Some(config_file_path) = Self::determine_config_file_path(base_path_override) {
            let config_dir = config_file_path.parent().unwrap_or_else(|| Path::new("."));

            if config_file_path.exists() {
                match fs::read_to_string(&config_file_path) {
                    Ok(contents) => match toml::from_str(&contents) {
                        Ok(config) => return config,
                        Err(e) => {
                            log(format!(
                                "Failed to parse config file at '{}': {}. Using default in-memory config.",
                                config_file_path.display(),
                                e
                            ));
                            return Self::default_config();
                        }
                    },
                    Err(e) => {
                        log(format!(
                            "Failed to read config file at '{}': {}. Using default in-memory config.",
                            config_file_path.display(),
                            e
                        ));
                        return Self::default_config();
                    }
                }
            } else {
                log(format!(
                    "Config file not found at '{}'. Attempting to create a default one.",
                    config_file_path.display()
                ));
                let default_cfg = Self::default_config();
                match toml::to_string_pretty(&default_cfg) {
                    Ok(toml_string) => {
                        if let Err(e) = fs::create_dir_all(config_dir) {
                            log(format!(
                                "Failed to create config directory '{}': {}",
                                config_dir.display(),
                                e
                            ));
                        } else if let Err(e) = fs::write(&config_file_path, toml_string) {
                            log(format!(
                                "Failed to write default config file to '{}': {}",
                                config_file_path.display(),
                                e
                            ));
                        } else {
                            log(format!(
                                "Default config file created at '{}'",
                                config_file_path.display()
                            ));
                        }
                    }
                    Err(e) => {
                        log(format!("Failed to serialize default config: {}", e));
                    }
                }
                return default_cfg;
            }
        }

        log("Could not determine config directory. Using default in-memory config.".to_string());
        Self::default_config()
    }

    pub fn load(base_path_override: Option<&Path>) -> Self {
        Self::load_internal(base_path_override, true)
    }

    pub fn load_quiet(base_path_override: Option<&Path>) -> Self {
        Self::load_internal(base_path_override, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::tempdir;

    #[test]
    #[serial]
    fn load_creates_default_when_missing() {
        let dir = tempdir().unwrap();
        let cfg = Config::load(Some(dir.path()));
        let cfg_file = dir.path().join("lazyredis").join("lazyredis.toml");
        assert!(
            cfg_file.exists(),
            "Config file should have been created at {}",
            cfg_file.display()
        );
        let on_disk = fs::read_to_string(cfg_file).unwrap();
        let loaded: Config = toml::from_str(&on_disk).unwrap();
        assert_eq!(cfg, loaded);
        assert_eq!(cfg.profiles.len(), 1);
        assert_eq!(cfg.profiles[0].name, "Default");
        assert_eq!(cfg.profiles[0].color.as_deref(), Some("green"));
    }

    #[test]
    #[serial]
    fn load_reads_existing_file() {
        let dir = tempdir().unwrap();
        let config_base_path = dir.path();
        let config_dir = config_base_path.join("lazyredis");
        fs::create_dir_all(&config_dir).unwrap();
        let cfg_file = config_dir.join("lazyredis.toml");
        let custom_cfg = Config {
            profiles: vec![ConnectionProfile {
                name: "Test".to_string(),
                url: "redis://localhost:6379".to_string(),
                db: Some(1),
                dev: Some(false),
                color: Some("red".to_string()),
            }],
        };
        fs::write(&cfg_file, toml::to_string(&custom_cfg).unwrap()).unwrap();
        let loaded = Config::load(Some(config_base_path));
        assert_eq!(loaded, custom_cfg);
    }
}
