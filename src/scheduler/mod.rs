pub mod by_deadline;

use uuid::Uuid;

use actix::dev::ToEnvelope;
use actix::prelude::*;

use async_std::channel::{Receiver, Sender};
use async_std::path::Path;
use async_std::sync::{Arc, Mutex};
use async_trait::async_trait;

use crate::event::*;
use crate::handler::State;

pub type SchedulerResult<T> = Result<T, SchedulerError>;

#[derive(Clone, Debug)]
pub enum SchedulerMessage {
    Send(Uuid, u32),
    DownNode(u32),
}

#[derive(Clone, Debug)]
pub enum SchedulerError {
    NoNodeFound,
    Unknown,
}

#[async_trait]
pub trait SchedulerWeighted {
    fn new(tx: Sender<SchedulerMessage>) -> Self;
    async fn register(&self);
    async fn rebalance(&self) -> SchedulerResult<()>;
    async fn unregister(&self, node_id: usize) -> SchedulerResult<()>;
    async fn push(&self, uuid: Uuid, total_time: u64, weight: u64) -> SchedulerResult<usize>;
    async fn touch(&self, node_id: usize) -> SchedulerResult<()>;
}

pub async fn serve_scheduler<T, P>(
    state: Arc<State<T, P>>,
    scheduler_rx: &mut Receiver<SchedulerMessage>,
) where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone,
{
    loop {
        if let Ok(msg) = scheduler_rx.try_recv() {
            match msg {
                SchedulerMessage::Send(uuid, node_id) => {
                    state
                        .handle_judge_send(uuid, node_id)
                        .await
                        .ok();
                }
                SchedulerMessage::DownNode(node_id) => {
                    debug!("down node {}", node_id);
                    state.down_node(node_id).await;
                }
                _ => {}
            }
        }
    }
}
