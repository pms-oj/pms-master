pub mod by_deadline;

use uuid::Uuid;

use async_std::channel::{Receiver, Sender};
use async_std::sync::{Arc, Mutex};
use async_trait::async_trait;

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
    async fn register(&mut self);
    async fn rebalance(&mut self) -> SchedulerResult<()>;
    async fn unregister(&mut self, node_id: usize) -> SchedulerResult<()>;
    async fn push(&mut self, uuid: Uuid, total_time: u64, weight: u64) -> SchedulerResult<usize>;
    async fn touch(&mut self, node_id: usize) -> SchedulerResult<()>;
}

pub async fn serve_scheduler(
    state: Arc<Mutex<State>>,
    scheduler_rx: &mut Receiver<SchedulerMessage>,
) {
    loop {
        if let Ok(msg) = scheduler_rx.try_recv() {
            match msg {
                SchedulerMessage::Send(uuid, node_id) => {
                    state
                        .lock()
                        .await
                        .handle_judge_send(uuid, node_id)
                        .await
                        .ok();
                }
                SchedulerMessage::DownNode(node_id) => {
                    debug!("down node {}", node_id);
                    state.lock().await.down_node(node_id).await;
                }
                _ => {}
            }
        }
    }
}
