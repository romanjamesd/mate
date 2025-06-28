use crate::crypto::Identity;
use crate::storage::Database;
use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Data directory for storing application data
    pub data_dir: PathBuf,
    /// Default bind address for the server
    pub default_bind_addr: String,
    /// Maximum number of concurrent games
    pub max_concurrent_games: usize,
}

impl Default for Config {
    fn default() -> Self {
        let data_dir = Self::default_data_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            data_dir,
            default_bind_addr: "127.0.0.1:8080".to_string(),
            max_concurrent_games: 10,
        }
    }
}

impl Config {
    /// Get the default data directory
    pub fn default_data_dir() -> Result<PathBuf> {
        ProjectDirs::from("dev", "mate", "mate")
            .map(|proj_dirs| proj_dirs.data_dir().to_path_buf())
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))
    }

    /// Get the default config directory
    pub fn default_config_dir() -> Result<PathBuf> {
        ProjectDirs::from("dev", "mate", "mate")
            .map(|proj_dirs| proj_dirs.config_dir().to_path_buf())
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))
    }

    /// Get the default config file path
    pub fn default_config_file() -> Result<PathBuf> {
        Ok(Self::default_config_dir()?.join("config.toml"))
    }

    /// Load configuration from file, creating default if it doesn't exist
    pub fn load_or_create_default() -> Result<Self> {
        let config_file = Self::default_config_file()?;

        if config_file.exists() {
            let content = std::fs::read_to_string(&config_file)
                .context("Failed to read configuration file")?;
            let config: Config =
                toml::from_str(&content).context("Failed to parse configuration file")?;
            Ok(config)
        } else {
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<()> {
        let config_file = Self::default_config_file()?;

        // Ensure config directory exists
        if let Some(parent) = config_file.parent() {
            std::fs::create_dir_all(parent).context("Failed to create config directory")?;
        }

        let content = toml::to_string_pretty(self).context("Failed to serialize configuration")?;

        std::fs::write(&config_file, content).context("Failed to write configuration file")?;

        Ok(())
    }

    /// Get the database path
    pub fn database_path(&self) -> PathBuf {
        self.data_dir.join("database.sqlite")
    }
}

/// Main application state
pub struct App {
    /// Cryptographic identity
    pub identity: Arc<Identity>,
    /// Database connection
    pub database: Database,
    /// Application configuration
    pub config: Config,
}

impl App {
    /// Create a new App instance with proper initialization
    pub async fn new() -> Result<Self> {
        // Load or create configuration
        let config =
            Config::load_or_create_default().context("Failed to initialize configuration")?;

        // Ensure data directory exists
        Self::ensure_data_dir(&config.data_dir).context("Failed to create data directory")?;

        // Load or generate identity
        let identity =
            Arc::new(Identity::load_or_generate().context("Failed to initialize identity")?);

        // Initialize database
        let database =
            Database::new(identity.peer_id().as_str()).context("Failed to initialize database")?;

        Ok(App {
            identity,
            database,
            config,
        })
    }

    /// Ensure data directory exists with proper permissions
    pub fn ensure_data_dir(data_dir: &PathBuf) -> Result<()> {
        if !data_dir.exists() {
            std::fs::create_dir_all(data_dir).with_context(|| {
                format!("Failed to create data directory: {}", data_dir.display())
            })?;
        }

        // Verify directory is writable
        let test_file = data_dir.join(".write_test");
        std::fs::write(&test_file, "test")
            .with_context(|| format!("Data directory is not writable: {}", data_dir.display()))?;
        std::fs::remove_file(&test_file).context("Failed to clean up write test file")?;

        Ok(())
    }

    /// Get the peer ID
    pub fn peer_id(&self) -> &str {
        self.identity.peer_id().as_str()
    }

    /// Get the database path
    pub fn database_path(&self) -> PathBuf {
        self.config.database_path()
    }

    /// Get data directory path
    pub fn data_dir(&self) -> &PathBuf {
        &self.config.data_dir
    }

    /// Reload configuration from file
    pub fn reload_config(&mut self) -> Result<()> {
        self.config = Config::load_or_create_default().context("Failed to reload configuration")?;
        Ok(())
    }

    /// Save current configuration to file
    pub fn save_config(&self) -> Result<()> {
        self.config.save().context("Failed to save configuration")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.default_bind_addr, "127.0.0.1:8080");
        assert_eq!(config.max_concurrent_games, 10);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let serialized = toml::to_string(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();

        assert_eq!(config.default_bind_addr, deserialized.default_bind_addr);
        assert_eq!(
            config.max_concurrent_games,
            deserialized.max_concurrent_games
        );
    }

    #[test]
    fn test_ensure_data_dir() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().join("mate_data");

        // Directory doesn't exist yet
        assert!(!data_dir.exists());

        // Should create directory
        App::ensure_data_dir(&data_dir).unwrap();
        assert!(data_dir.exists());

        // Should work if directory already exists
        App::ensure_data_dir(&data_dir).unwrap();
    }
}
