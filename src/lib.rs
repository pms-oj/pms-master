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

    #[test]
    fn test_serve() {
        use log::*;
        log::set_logger(&logger::StdoutLogger).map(|()| log::set_max_level(LevelFilter::Info));
        let cfg = Config {
            host: SocketAddr::from_str("127.0.0.1:3030").unwrap(),
            host_pass: String::from("asdf"),
        };
        block_on(serve(cfg));
    }
}
