#[macro_use]
extern crate log;

pub mod broker;
pub mod config;
pub mod constants;
pub mod event;
pub mod handler;
pub mod judge;
pub mod scheduler;
pub mod stream;
pub mod timer;

#[cfg(test)]
mod tests {

    use super::*;

    use log::*;

    fn init() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();
    }

    #[test]
    fn test_ac1() {
        use crate::config::Config;
        use crate::handler::{HandlerMessage, HandlerService};
        use actix::prelude::*;
        use async_std::task::sleep;
        use std::net::SocketAddr;
        use std::str::FromStr;
        let sys = System::new();
        sys.block_on(async {
            init();
            use std::time::Duration;
            sleep(Duration::from_secs(1)).await;
            let cfg = Config {
                host: SocketAddr::from_str("127.0.0.1:3030").unwrap(),
                host_pass: String::from("asdf"),
            };
            use async_std::channel::unbounded;
            use judge::*;
            let (event_tx, event_rx) = unbounded();
            let handler_service = HandlerService {
                state: None,
                cfg,
                event_tx,
            };
            let addr = handler_service.start();
            let judge_uuid = uuid::Uuid::new_v4();
            addr.send(HandlerMessage::Judge(RequestJudge {
                uuid: judge_uuid,
                judge_priority: PrioirityWeight::First,
                test_size: 1,
                stdin: vec![include_bytes!("../assets/stdin/1.in").to_vec()],
                stdout: vec![include_bytes!("../assets/stdout/1.out").to_vec()],
                main: include_bytes!("../assets/cpp/ac_1.cpp").to_vec(),
                checker: include_bytes!("../assets/checker/lcmp.cpp").to_vec(),
                main_lang_uuid: uuid::Uuid::from_str("aea02f71-ab0d-470e-9d0d-3577ec870e29")
                    .unwrap(),
                checker_lang_uuid: uuid::Uuid::from_str("ad9d152c-abbd-4dd2-b484-5825b6a7e4bb")
                    .unwrap(),
                time_limit: 1000,
                mem_limit: 1048576,
            }))
            .await
            .ok();
            use crate::event::EventMessage;
            use judge_protocol::judge::*;
            while let Ok(EventMessage::JudgeResult(uuid, status)) = event_rx.recv().await {
                assert_eq!(uuid, judge_uuid);
                if let JudgeState::Accepted(test_uuid, time, mem) = status {
                    debug!("(AC) uuid: {}, time: {}ms, mem: {}kB", test_uuid, time, mem);
                    break;
                }
            }
        });
    }

    #[test]
    fn test_tle1() {
        use crate::config::Config;
        use crate::handler::{HandlerMessage, HandlerService};
        use actix::prelude::*;
        use async_std::task::sleep;
        use std::net::SocketAddr;
        use std::str::FromStr;
        let sys = System::new();
        sys.block_on(async {
            init();
            use std::time::Duration;
            sleep(Duration::from_secs(1)).await;
            let cfg = Config {
                host: SocketAddr::from_str("127.0.0.1:3030").unwrap(),
                host_pass: String::from("asdf"),
            };
            use async_std::channel::unbounded;
            use judge::*;
            let (event_tx, event_rx) = unbounded();
            let handler_service = HandlerService {
                state: None,
                cfg,
                event_tx,
            };
            let addr = handler_service.start();
            let judge_uuid = uuid::Uuid::new_v4();
            addr.send(HandlerMessage::Judge(RequestJudge {
                uuid: judge_uuid,
                judge_priority: PrioirityWeight::First,
                test_size: 1,
                stdin: vec![include_bytes!("../assets/stdin/1.in").to_vec()],
                stdout: vec![include_bytes!("../assets/stdout/1.out").to_vec()],
                main: include_bytes!("../assets/cpp/tle_1.cpp").to_vec(),
                checker: include_bytes!("../assets/checker/lcmp.cpp").to_vec(),
                main_lang_uuid: uuid::Uuid::from_str("aea02f71-ab0d-470e-9d0d-3577ec870e29")
                    .unwrap(),
                checker_lang_uuid: uuid::Uuid::from_str("ad9d152c-abbd-4dd2-b484-5825b6a7e4bb")
                    .unwrap(),
                time_limit: 1000,
                mem_limit: 1048576,
            }))
            .await
            .ok();
            use crate::event::EventMessage;
            use judge_protocol::judge::*;
            while let Ok(EventMessage::JudgeResult(uuid, status)) = event_rx.recv().await {
                assert_eq!(uuid, judge_uuid);
                if let JudgeState::TimeLimitExceed(test_uuid) = status {
                    debug!("(TLE) uuid: {}", test_uuid);
                    break;
                }
            }
        });
    }

    #[test]
    fn test_rte1() {
        use crate::config::Config;
        use crate::handler::{HandlerMessage, HandlerService};
        use actix::prelude::*;
        use async_std::task::sleep;
        use std::net::SocketAddr;
        use std::str::FromStr;
        let sys = System::new();
        sys.block_on(async {
            init();
            use std::time::Duration;
            sleep(Duration::from_secs(1)).await;
            let cfg = Config {
                host: SocketAddr::from_str("127.0.0.1:3030").unwrap(),
                host_pass: String::from("asdf"),
            };
            use async_std::channel::unbounded;
            use judge::*;
            let (event_tx, event_rx) = unbounded();
            let handler_service = HandlerService {
                state: None,
                cfg,
                event_tx,
            };
            let addr = handler_service.start();
            let judge_uuid = uuid::Uuid::new_v4();
            addr.send(HandlerMessage::Judge(RequestJudge {
                uuid: judge_uuid,
                judge_priority: PrioirityWeight::First,
                test_size: 1,
                stdin: vec![include_bytes!("../assets/stdin/1.in").to_vec()],
                stdout: vec![include_bytes!("../assets/stdout/1.out").to_vec()],
                main: include_bytes!("../assets/cpp/rte_1.cpp").to_vec(),
                checker: include_bytes!("../assets/checker/lcmp.cpp").to_vec(),
                main_lang_uuid: uuid::Uuid::from_str("aea02f71-ab0d-470e-9d0d-3577ec870e29")
                    .unwrap(),
                checker_lang_uuid: uuid::Uuid::from_str("ad9d152c-abbd-4dd2-b484-5825b6a7e4bb")
                    .unwrap(),
                time_limit: 1000,
                mem_limit: 1048576,
            }))
            .await
            .ok();
            use crate::event::EventMessage;
            use judge_protocol::judge::*;
            while let Ok(EventMessage::JudgeResult(uuid, status)) = event_rx.recv().await {
                assert_eq!(uuid, judge_uuid);
                if let JudgeState::RuntimeError(test_uuid, code) = status {
                    debug!("(RTE) uuid: {}, exit code: {}", test_uuid, code);
                    break;
                }
            }
        });
    }
}
