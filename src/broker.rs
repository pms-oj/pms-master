use actix::dev::ToEnvelope;
use actix::prelude::*;
use async_std::channel::{Receiver, Sender};
use async_std::net::TcpStream;
use async_std::path::Path;
use async_std::sync::{Arc, Mutex};
use async_std::task::spawn;
use judge_protocol::packet::*;

use crate::event::*;
use crate::handler::State;
use crate::scheduler::*;

#[derive(Debug)]
pub enum BrokerMessage {
    Packet(Arc<TcpStream>, Packet),
    Unknown,
}

pub async fn serve_broker<T, P>(
    scheduler_tx: Sender<SchedulerMessage>,
    broker_tx: Sender<BrokerMessage>,
    broker_rx: &mut Receiver<BrokerMessage>,
    state: Arc<State<T, P>>,
) where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone,
{
    loop {
        if let Ok(msg) = broker_rx.try_recv() {
            match msg {
                BrokerMessage::Packet(stream, packet) => {
                    let state_mutex = Arc::clone(&state);
                    let broker_cloned = broker_tx.clone();
                    let scheduler_cloned = scheduler_tx.clone();
                    spawn(async move {
                        state_mutex
                            .handle_command(scheduler_cloned, broker_cloned, stream, packet)
                            .await
                    });
                }
                _ => {}
            }
        }
    }
}
