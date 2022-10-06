pub mod by_deadline;

use uuid::Uuid;

use async_std::channel::Sender;
use async_std::sync::{Arc, Mutex};
use async_trait::async_trait;

pub type SchedulerResult<T> = Result<T, SchedulerError>;

#[derive(Clone, Debug)]
pub enum SchedulerMessage {
    Send(Uuid, u32),
}

#[derive(Clone, Debug)]
pub enum SchedulerError {
    NoNodeFound,
    Unknown,
}

#[async_trait]
pub trait SchedulerWeighted {
    fn new(tx: Arc<Mutex<Sender<SchedulerMessage>>>) -> Self;
    async fn register(&mut self);
    async fn push(&mut self, uuid: Uuid, total_time: u64, weight: u64) -> SchedulerResult<usize>;
    async fn touch(&mut self, node_id: usize) -> SchedulerResult<()>;
}
