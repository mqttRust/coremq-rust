use std::{fs::File, io::BufReader, sync::Arc};

use tokio_rustls::rustls::{
    pki_types::{CertificateDer, PrivateKeyDer},
    ServerConfig,
};

/// Build a rustls `ServerConfig` from PEM cert + key files (for the raw-TLS MQTT worker).
pub fn load_server_config(cert_path: &str, key_path: &str) -> Result<Arc<ServerConfig>, String> {
    let certs = load_certs(cert_path)?;
    let key = load_key(key_path)?;

    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| format!("invalid certificate/key pair: {e}"))?;

    Ok(Arc::new(config))
}

fn load_certs(path: &str) -> Result<Vec<CertificateDer<'static>>, String> {
    let file = File::open(path).map_err(|e| format!("open cert '{path}': {e}"))?;
    let mut reader = BufReader::new(file);
    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut reader)
        .collect::<Result<_, _>>()
        .map_err(|e| format!("parse cert '{path}': {e}"))?;
    if certs.is_empty() {
        return Err(format!("no certificates found in '{path}'"));
    }
    Ok(certs)
}

fn load_key(path: &str) -> Result<PrivateKeyDer<'static>, String> {
    let file = File::open(path).map_err(|e| format!("open key '{path}': {e}"))?;
    let mut reader = BufReader::new(file);
    rustls_pemfile::private_key(&mut reader)
        .map_err(|e| format!("parse key '{path}': {e}"))?
        .ok_or_else(|| format!("no private key found in '{path}'"))
}
