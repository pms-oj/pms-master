pub mod config;
pub mod handler;

use config::Config;
use handler::serve;

use async_std::task::block_on;
use std::net::SocketAddr;
use std::str::FromStr;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serve() {
        let cfg = Config {
            host: SocketAddr::from_str("127.0.0.1:3030").unwrap(),
            host_pass: String::from("asdf"),
        };
        block_on(serve(cfg));
    }
}
