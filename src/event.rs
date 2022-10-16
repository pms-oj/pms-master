use actix::prelude::*;
use judge_protocol::judge::*;
use uuid::Uuid;

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub enum EventMessage {
    JudgeResult(Uuid, JudgeState), // (judge UUID, state)
}