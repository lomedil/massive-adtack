use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub url: String,
    pub base_dn: String,
    pub user: String,
    pub password: String,
    pub starttls: bool,
    pub tls_ca_cert: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = if let Ok(val) = env::var("MAD_CONFIG") {
            PathBuf::from(val)
        } else {
            let agents_path = PathBuf::from(".agents/config.toml");
            if agents_path.exists() {
                agents_path
            } else {
                PathBuf::from("config.toml")
            }
        };

        if !config_path.exists() {
            anyhow::bail!("Configuration file not found: {:?}", config_path);
        }

        let content = fs::read_to_string(&config_path)
            .with_context(|| format!("Could not read config file: {:?}", config_path))?;

        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Could not parse config file: {:?}", config_path))?;

        Ok(config)
    }
}
