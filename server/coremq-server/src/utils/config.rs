use std::{fs, path::Path};
use crate::models::config::Config;

#[cfg(windows)]
const DEFAULT_CONFIG: &str = r"C:\ProgramData\CoreMQ\config.yaml";
#[cfg(not(windows))]
const DEFAULT_CONFIG: &str = "/etc/coremq/config.yaml";

const FALLBACK_CONFIG: &str = "server/coremq-server/config/config.yaml";

const DEFAULT_CONFIG_YAML: &str = r#"app:
  name: "coremq"
  env: "production"

middleware:
  model_path: /etc/coremq/model.conf
  policy_path: /etc/coremq/policy.csv
  secret: change-this-to-a-long-random-secret

mqtt:
  listeners:
    - name: "tcp"
      protocol: "tcp"
      host: "0.0.0.0"
      port: 1883

    - name: "ws"
      protocol: "ws"
      host: "0.0.0.0"
      port: 8083
"#;

const DEFAULT_MODEL_CONF: &str = r#"[request_definition]
r = sub, obj, act

[policy_definition]
p = sub, obj, act

[role_definition]
g = _, _

[policy_effect]
e = some(where (p.eft == allow))

[matchers]
m = g(r.sub, p.sub) || (r.sub == p.sub && keyMatch(r.obj, p.obj) && r.act == p.act) || p.sub == "public"
"#;

const DEFAULT_POLICY_CSV: &str = r#"# Admin role — full access to the control-plane API (keyMatch wildcard)
p, admin, /api/v1/*, GET
p, admin, /api/v1/*, POST
p, admin, /api/v1/*, PUT
p, admin, /api/v1/*, DELETE

# Operator role — read-only monitoring
p, user, /api/v1/listeners, GET
p, user, /api/v1/sessions, GET

# Public access
p, public, /api/v1/public/login, POST
"#;

pub fn from_file() -> Result<Config, Box<dyn std::error::Error>> {
    let config_path = std::env::var("COREMQ_CONFIG").unwrap_or_else(|_| {
        if Path::new(DEFAULT_CONFIG).exists() {
            DEFAULT_CONFIG.to_string()
        } else if Path::new(FALLBACK_CONFIG).exists() {
            FALLBACK_CONFIG.to_string()
        } else {
            DEFAULT_CONFIG.to_string()
        }
    });

    if !Path::new(&config_path).exists() {
        create_default_config(&config_path)?;
    }

    println!("Loading config from: {}", config_path);
    let content = fs::read_to_string(&config_path)?;
    let config: Config = serde_yaml::from_str(&content)?;

    ensure_casbin_files(&config)?;

    Ok(config)
}

fn create_default_config(config_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let dir = Path::new(config_path).parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    fs::write(config_path, DEFAULT_CONFIG_YAML)?;
    println!("Created default config at: {}", config_path);
    Ok(())
}

fn ensure_casbin_files(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    if !Path::new(&config.middleware.model_path).exists() {
        if let Some(dir) = Path::new(&config.middleware.model_path).parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(&config.middleware.model_path, DEFAULT_MODEL_CONF)?;
        println!("Created default model at: {}", config.middleware.model_path);
    }

    if !Path::new(&config.middleware.policy_path).exists() {
        if let Some(dir) = Path::new(&config.middleware.policy_path).parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(&config.middleware.policy_path, DEFAULT_POLICY_CSV)?;
        println!("Created default policy at: {}", config.middleware.policy_path);
    }

    Ok(())
}
