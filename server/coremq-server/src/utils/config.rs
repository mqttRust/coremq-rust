use std::fs;
use crate::models::config::Config;

#[cfg(windows)]
const DEFAULT_CONFIG: &str = r"C:\ProgramData\CoreMQ\config.yaml";
#[cfg(not(windows))]
const DEFAULT_CONFIG: &str = "/etc/coremq/config.yaml";

const FALLBACK_CONFIG: &str = "server/coremq-server/config/config.yaml";

pub fn from_file() -> Result<Config, Box<dyn std::error::Error>> {
    let config_path = std::env::var("COREMQ_CONFIG").unwrap_or_else(|_| {
        if std::path::Path::new(DEFAULT_CONFIG).exists() {
            DEFAULT_CONFIG.to_string()
        } else {
            FALLBACK_CONFIG.to_string()
        }
    });
    println!("Loading config from: {}", config_path);
    let content = fs::read_to_string(&config_path)?;
    let config = serde_yaml::from_str(&content)?;
    Ok(config)
}
