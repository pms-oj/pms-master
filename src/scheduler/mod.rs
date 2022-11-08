pub mod by_deadline;

use uuid::Uuid;

use async_std::path::Path;
use async_std::sync::Arc;

use actix::dev::ToEnvelope;
use actix::prelude::*;

use crate::event::*;
use crate::handler::*;

use k256::ecdh::SharedSecret;

pub type SchedulerResult<T> = Result<T, SchedulerError>;

#[derive(Clone, Message)]
#[rtype(result = "SchedulerResp")]
pub enum SchedulerIssue {
    NewNode(Uuid),           // (node_id)
    RemoveNode(Uuid),        // (node_id)
    NewTask(Uuid, u64, u64), // (judge_id, ...)
    Touch(Uuid),             // (node_id)
    HandshakeEstablished(Uuid, Arc<SharedSecret>),
    Exists(Uuid),
    // NOT RECOMMEND FOR GENERAL USAGE
    ForceRebalance,
}

#[derive(Clone, Debug, MessageResponse)]
pub enum SchedulerResp {
    None,
    Node(Uuid),
    Error(SchedulerError),
}

#[derive(Clone, Debug)]
pub enum SchedulerError {
    NoNodeFound,
    NodeAlreadyExists,
    Unknown,
}

pub trait SchedulerWeighted<T, P>: Actor + Handler<SchedulerIssue>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    fn register(&mut self, node_id: Uuid) -> SchedulerResult<()>;
    fn promote(&mut self, node_id: Uuid, key: Arc<SharedSecret>) -> SchedulerResult<()>;
    fn rebalance(&mut self) -> SchedulerResult<()>;
    fn unregister(&mut self, node_id: Uuid) -> SchedulerResult<()>;
    fn push(&mut self, uuid: Uuid, total_time: u64, weight: u64) -> SchedulerResult<Uuid>;
    fn touch(&mut self, node_id: Uuid) -> SchedulerResult<()>;
}
