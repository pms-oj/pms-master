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
    use crate::event::*;
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
            host: "127.0.0.1:3030".to_string(),
            host_pass: String::from("asdf"),
        };
        let judge_uuid = uuid::Uuid::new_v4();
        use judge::*;
        struct JudgeService {}
        impl Actor for JudgeService {
            type Context = Context<Self>;
        }
        impl Handler<EventMessage> for JudgeService {
            type Result = ();
            fn handle(&mut self, msg: EventMessage, ctx: &mut Context<Self>) -> () {
                debug!("{:?}", msg);
                match msg {
                    EventMessage::JudgeResult(uuid, status) => {
                        if let JudgeState::Accepted(test_uuid, time, mem) = status {
                            debug!("(AC) uuid: {}, time: {}ms, mem: {}kB", test_uuid, time, mem);
                            ctx.terminate();
                        }
                    }
                    _ => {}
                }
            }
        }
        let event_addr = JudgeService {}.start();
        let handler_service = HandlerService {
            state: None,
            cfg,
            event_addr: event_addr.clone(),
        };
        let addr = handler_service.start();
        addr.send(HandlerMessage::Judge(RequestJudge {
            uuid: judge_uuid,
            judge_priority: PrioirityWeight::First,
            test_size: 1,
            stdin: vec![include_bytes!("../../assets/stdin/1.in").to_vec()],
            stdout: vec![include_bytes!("../../assets/stdout/1.out").to_vec()],
            main: include_bytes!("../../assets/cpp/ac_1.cpp").to_vec(),
            checker: include_bytes!("../../assets/checker/lcmp.cpp").to_vec(),
            main_lang_uuid: uuid::Uuid::from_str("aea02f71-ab0d-470e-9d0d-3577ec870e29").unwrap(),
            checker_lang_uuid: uuid::Uuid::from_str("ad9d152c-abbd-4dd2-b484-5825b6a7e4bb")
                .unwrap(),
            time_limit: 1000,
            mem_limit: 1048576,
        }))
        .await
        .ok();
        use crate::event::EventMessage;
        use judge_protocol::judge::*;
        while event_addr.connected() {
            sleep(Duration::from_secs(1)).await;
        }
    });
}

#[test]
fn test_tle1() {
    use crate::config::Config;
    use crate::event::*;
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
            host: "127.0.0.1:3030".to_string(),
            host_pass: String::from("asdf"),
        };
        let judge_uuid = uuid::Uuid::new_v4();
        use judge::*;
        struct JudgeService {}
        impl Actor for JudgeService {
            type Context = Context<Self>;
        }
        impl Handler<EventMessage> for JudgeService {
            type Result = ();
            fn handle(&mut self, msg: EventMessage, ctx: &mut Context<Self>) -> () {
                debug!("{:?}", msg);
                match msg {
                    EventMessage::JudgeResult(uuid, status) => {
                        if let JudgeState::TimeLimitExceed(test_uuid) = status {
                            debug!("(TLE) uuid: {}", test_uuid);
                            ctx.terminate();
                        }
                    }
                    _ => {}
                }
            }
        }
        let event_addr = JudgeService {}.start();
        let handler_service = HandlerService {
            state: None,
            cfg,
            event_addr: event_addr.clone(),
        };
        let addr = handler_service.start();
        addr.send(HandlerMessage::Judge(RequestJudge {
            uuid: judge_uuid,
            judge_priority: PrioirityWeight::First,
            test_size: 1,
            stdin: vec![include_bytes!("../../assets/stdin/1.in").to_vec()],
            stdout: vec![include_bytes!("../../assets/stdout/1.out").to_vec()],
            main: include_bytes!("../../assets/cpp/tle_1.cpp").to_vec(),
            checker: include_bytes!("../../assets/checker/lcmp.cpp").to_vec(),
            main_lang_uuid: uuid::Uuid::from_str("aea02f71-ab0d-470e-9d0d-3577ec870e29").unwrap(),
            checker_lang_uuid: uuid::Uuid::from_str("ad9d152c-abbd-4dd2-b484-5825b6a7e4bb")
                .unwrap(),
            time_limit: 1000,
            mem_limit: 1048576,
        }))
        .await
        .ok();
        use crate::event::EventMessage;
        use judge_protocol::judge::*;
        while event_addr.connected() {
            sleep(Duration::from_secs(1)).await;
        }
    });
}

#[test]
fn test_rte1() {
    use crate::config::Config;
    use crate::event::*;
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
            host: "127.0.0.1:3030".to_string(),
            host_pass: String::from("asdf"),
        };
        let judge_uuid = uuid::Uuid::new_v4();
        use judge::*;
        struct JudgeService {}
        impl Actor for JudgeService {
            type Context = Context<Self>;
        }
        impl Handler<EventMessage> for JudgeService {
            type Result = ();
            fn handle(&mut self, msg: EventMessage, ctx: &mut Context<Self>) -> () {
                debug!("{:?}", msg);
                match msg {
                    EventMessage::JudgeResult(uuid, status) => {
                        if let JudgeState::RuntimeError(test_uuid, exit_code) = status {
                            debug!("(RTE) uuid: {}, exit code: {}", test_uuid, exit_code);
                            ctx.terminate();
                        }
                    }
                    _ => {}
                }
            }
        }
        let event_addr = JudgeService {}.start();
        let handler_service = HandlerService {
            state: None,
            cfg,
            event_addr: event_addr.clone(),
        };
        let addr = handler_service.start();
        addr.send(HandlerMessage::Judge(RequestJudge {
            uuid: judge_uuid,
            judge_priority: PrioirityWeight::First,
            test_size: 1,
            stdin: vec![include_bytes!("../../assets/stdin/1.in").to_vec()],
            stdout: vec![include_bytes!("../../assets/stdout/1.out").to_vec()],
            main: include_bytes!("../../assets/cpp/rte_1.cpp").to_vec(),
            checker: include_bytes!("../../assets/checker/lcmp.cpp").to_vec(),
            main_lang_uuid: uuid::Uuid::from_str("aea02f71-ab0d-470e-9d0d-3577ec870e29").unwrap(),
            checker_lang_uuid: uuid::Uuid::from_str("ad9d152c-abbd-4dd2-b484-5825b6a7e4bb")
                .unwrap(),
            time_limit: 1000,
            mem_limit: 1048576,
        }))
        .await
        .ok();
        use crate::event::EventMessage;
        use judge_protocol::judge::*;
        while event_addr.connected() {
            sleep(Duration::from_secs(1)).await;
        }
    });
}
