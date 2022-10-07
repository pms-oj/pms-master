pub mod config;
pub mod constants;
pub mod handler;
pub mod judge;
pub mod logger;
pub mod scheduler;

use config::Config;
use handler::*;

use async_std::task::{block_on, sleep, spawn};
use std::net::SocketAddr;
use std::str::FromStr;

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use env_logger::{Builder, Target};
    use log::*;

    fn init() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();
    }

    #[test]
    fn test_serve() {
        block_on(async {
            init();
            use std::time::Duration;
            let cfg = Config {
                host: SocketAddr::from_str("127.0.0.1:3030").unwrap(),
                host_pass: String::from("asdf"),
                write_time_out: Duration::from_secs(1),
                read_time_out: Duration::from_secs(1),
            };
            use async_std::channel::unbounded;
            use async_std::sync::Arc;
            use judge::*;
            let (tx, mut rx) = unbounded();
            let message_rx = Arc::new(rx);
            spawn(async move {
                loop {
                    sleep(Duration::from_secs(5)).await;
                    tx.send(HandlerMessage::Judge(RequestJudge {
                        uuid: uuid::Uuid::new_v4(),
                        judge_priority: PrioirityWeight::First,
                        test_size: 0,
                        stdin: vec![],
                        stdout: vec![],
                        main: vec![],
                        checker: vec![],
                        main_lang_uuid: uuid::Uuid::from_str(
                            "aea02f71-ab0d-470e-9d0d-3577ec870e29",
                        )
                        .unwrap(),
                        checker_lang_uuid: uuid::Uuid::from_str(
                            "aea02f71-ab0d-470e-9d0d-3577ec870e29",
                        )
                        .unwrap(),
                        time_limit: 1000,
                        mem_limit: 1024,
                    }))
                    .await;
                }
            });
            serve(cfg, Arc::clone(&message_rx)).await
        });
    }
}
