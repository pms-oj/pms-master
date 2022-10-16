use async_std::channel::Sender;
use async_std::net::TcpStream;
use async_std::sync::Arc;
use async_std::task::sleep;

use std::time::Duration;

use actix::prelude::*;

use crate::constants::CHECK_ALIVE_TIME;
use crate::handler::{HandlerMessage, HandlerService};

pub async fn check_alive(node_id: u32, handler_addr: Addr<HandlerService>, stream: Arc<TcpStream>) {
    loop {
        sleep(Duration::from_secs(CHECK_ALIVE_TIME)).await;
        if let Ok(val) = stream.peek(&mut vec![0]).await {
            if val == 0 {
                handler_addr
                    .send(HandlerMessage::DownNode(node_id))
                    .await
                    .ok();
                break;
            }
        }
    }
}
