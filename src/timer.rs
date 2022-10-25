use async_std::channel::Sender;
use async_std::net::TcpStream;
use async_std::path::Path;
use async_std::sync::Arc;
use async_std::task::sleep;

use std::time::Duration;

use actix::dev::ToEnvelope;
use actix::prelude::*;

use crate::constants::CHECK_ALIVE_TIME;
use crate::event::*;
use crate::handler::{HandlerMessage, HandlerService};

pub async fn check_alive<T, P>(
    node_id: u32,
    handler_addr: Addr<HandlerService<T, P>>,
    stream: Arc<TcpStream>,
) where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone,
{
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
