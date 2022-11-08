use actix::dev::ToEnvelope;
use actix::prelude::*;

use async_std::path::Path;

use std::collections::HashMap;

use uuid::Uuid;

use judge_protocol::handshake::*;
use judge_protocol::judge::*;
use judge_protocol::packet::*;

use crate::event::*;
use crate::handler::judge::*;

pub fn handle_judge_state<T, P>(
    node_id: Uuid,
    packet: Packet,
    event_addr: Addr<T>,
    judge_addrs: &HashMap<Uuid, Addr<JudgeService<T, P>>>,
) where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    // Not asynchronous here. Because size of expected packet is small enough to keep overhead.
    if let Ok(body) = bincode::deserialize::<BodyAfterHandshake<JudgeResponseBody>>(&packet.body) {
        event_addr.do_send(EventMessage::JudgeResult(
            body.req.uuid,
            body.req.result.clone(),
        ));
        let judge_addr = judge_addrs[&body.req.uuid].clone();
        judge_addr.do_send(JudgeMessage::JudgeStateUpdate(node_id, body.req.result));
    } else {
        error!("Some error occurred when processing Command::GetJudgeStateUpdate");
    }
}
