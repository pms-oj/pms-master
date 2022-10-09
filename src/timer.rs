use async_std::channel::Sender;
use async_std::io::prelude::*;
use async_std::net::TcpStream;
use async_std::sync::Arc;
use async_std::task::sleep;

use std::time::Duration;

use crate::constants::CHECK_ALIVE_TIME;
use crate::handler::HandlerMessage;

pub async fn check_alive(node_id: u32, action_tx: Sender<HandlerMessage>, stream: Arc<TcpStream>) {
    loop {
        sleep(Duration::from_secs(CHECK_ALIVE_TIME)).await;
        if let Ok(val) = stream.peek(&mut vec![0]).await {
            if val == 0 {
                action_tx.send(HandlerMessage::DownNode(node_id)).await.ok();
                break;
            }
        }
    }
}
