use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};

use super::rpc::validate_endpoint;

/// Node preferences only; external-node tokens remain in memory.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Settings {
    pub embedded_node: bool,
    pub network: String,
    /// Comma-separated onion peers, editable without a terminal.
    pub peers: String,
    pub node_endpoint: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            embedded_node: true,
            network: "main".into(),
            peers: String::new(),
            node_endpoint: "127.0.0.1:18440".into(),
        }
    }
}

impl Settings {
    fn path() -> Result<PathBuf, String> {
        dirs::config_dir()
            .map(|path| path.join("kryptos").join("settings.toml"))
            .ok_or_else(|| "The system configuration directory is unavailable.".into())
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_endpoint(&self.node_endpoint)?;
        self.network.parse::<runtime::config::Network>()?;
        for peer in self.peer_list() {
            runtime::tor::validate_endpoint(&peer)
                .map_err(|_| format!("Invalid onion peer: {peer}"))?;
        }
        Ok(())
    }

    pub fn peer_list(&self) -> Vec<String> {
        self.peers
            .split(|ch: char| ch == ',' || ch.is_whitespace())
            .filter(|peer| !peer.is_empty())
            .map(str::to_owned)
            .collect()
    }

    pub fn chain_directory(&self) -> Result<PathBuf, String> {
        let network = self.network.parse::<runtime::config::Network>()?;
        dirs::data_local_dir()
            .map(|path| path.join("Kryptos").join(network.to_string()).join("chain"))
            .ok_or_else(|| "The system data directory is unavailable.".into())
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
        assert_eq!(table.len(), 4);
        assert!(table.contains_key("node_endpoint"));
        assert!(!serialized.contains("token"));
        assert!(Settings::default().validate().is_ok());
        let external = Settings {
            node_endpoint: "192.0.2.1:18440".into(),
            ..Settings::default()
        };
        assert!(external.validate().is_err());
    }

    #[test]
    fn legacy_settings_gain_embedded_defaults_and_reject_invalid_peers() {
        let settings: Settings = toml::from_str("node_endpoint = '127.0.0.1:18440'").unwrap();
        assert!(settings.embedded_node);
        assert_eq!(settings.network, "main");
        let invalid = Settings {
            peers: "127.0.0.1:19000".into(),
            ..settings
        };
        assert!(invalid.validate().is_err());
    }
}
