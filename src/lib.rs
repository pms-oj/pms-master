pub mod config;
pub mod handler;
pub mod logger;

use config::Config;
use handler::serve;

use async_std::task::block_on;
use std::net::SocketAddr;
use std::str::FromStr;

#[cfg(test)]
mod tests {
    use super::*;
    use log::*;
    use env_logger::{Builder, Target};

    fn init() {
        let _ = env_logger::builder().filter_level(log::LevelFilter::max()).is_test(true).try_init();
    }

    #[test]
    fn test_serve() {
        block_on(async {
            init();
            let cfg = Config {
                host: SocketAddr::from_str("127.0.0.1:3030").unwrap(),
                host_pass: String::from("asdf"),
            };
            serve(cfg).await;
        });
    }
}
