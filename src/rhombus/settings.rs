use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};

use super::rpc::validate_endpoint;

/// Only the non-sensitive node control address is persisted. Its token remains
/// in the in-memory application state.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Settings {
    pub node_endpoint: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            node_endpoint: "127.0.0.1:18440".into(),
        }
    }
}

impl Settings {
    fn path() -> Result<PathBuf, String> {
        dirs::config_dir()
            .map(|path| path.join("rhombus").join("settings.toml"))
            .ok_or_else(|| "The system configuration directory is unavailable.".into())
    }

    fn validate(&self) -> Result<(), String> {
        validate_endpoint(&self.node_endpoint)
    }

    pub fn load() -> (Self, Option<String>) {
        let result = (|| {
            let path = Self::path()?;
            let contents = match fs::read_to_string(path) {
                Ok(contents) => contents,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(Self::default());
                }
                Err(_) => {
                    return Err("Could not read connection settings; defaults loaded.".to_string());
                }
            };
            let settings: Self = toml::from_str(&contents)
                .map_err(|_| "Invalid connection settings; defaults loaded.".to_string())?;
            settings.validate()?;
            Ok(settings)
        })();
        match result {
            Ok(settings) => (settings, None),
            Err(error) => (Self::default(), Some(error)),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        self.validate()?;
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|_| "Cannot create the configuration directory.".to_string())?;
        }
        let contents = toml::to_string_pretty(self)
            .map_err(|_| "Cannot encode connection settings.".to_string())?;
        fs::write(path, contents)
            .map_err(|_| "Cannot write the connection settings file.".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persisted_settings_only_contain_loopback_endpoints() {
        let serialized = toml::to_string(&Settings::default()).unwrap();
        let table: toml::Table = toml::from_str(&serialized).unwrap();
        assert_eq!(table.len(), 1);
        assert!(table.contains_key("node_endpoint"));
        assert!(Settings::default().validate().is_ok());
        let external = Settings {
            node_endpoint: "192.0.2.1:18440".into(),
            ..Settings::default()
        };
        assert!(external.validate().is_err());
    }
}
