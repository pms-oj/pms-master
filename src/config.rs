use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Config {
    pub host: SocketAddr,
    pub host_pass: String,
    pub write_time_out: Duration,
    pub read_time_out: Duration,
}
