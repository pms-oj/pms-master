use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct General {
    pub host: String,
    pub host_pass: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TlsConfig {
    pub key_pem: String,
    pub cert_pem: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Config {
    pub general: General,
    pub tls: TlsConfig,
}
