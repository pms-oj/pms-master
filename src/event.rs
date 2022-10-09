use judge_protocol::judge::*;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum EventMessage {
    JudgeResult(Uuid, JudgeState) // (judge UUID, state)
}