use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Config {
    pub host: SocketAddr,
    pub host_pass: String,
}
