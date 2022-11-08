use actix::dev::ToEnvelope;
use actix::prelude::*;

use async_std::path::Path;
use async_std::sync::Arc;

use judge_protocol::handshake::*;
use judge_protocol::packet::*;

use uuid::Uuid;

use k256::ecdh::EphemeralSecret;

use crate::event::*;
use crate::handler::connection::*;
use crate::handler::*;
use crate::scheduler::by_deadline::*;
use crate::scheduler::*;

pub async fn handle_handshake<T, P>(
    node_id: Uuid,
    packet: Packet,
    pass: Arc<Vec<u8>>,
    key: Arc<EphemeralSecret>,
    conn_addr: Addr<ConnectionService<T, P>>,
    scheduler: Addr<ByDeadlineWeighted<T, P>>,
    handler: Addr<HandlerService<T, P>>,
) where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    if let Ok(handshake_req) = bincode::deserialize::<HandshakeRequest>(&packet.body) {
        if handshake_req.pass == *pass {
            let shared_key = Arc::new(key.diffie_hellman(&handshake_req.client_pubkey));
            scheduler
                .send(SchedulerIssue::HandshakeEstablished(node_id, shared_key))
                .await;
            info!("Handshake is established from node UUID: {}", node_id);
            let handshake_resp = HandshakeResponse {
                result: HandshakeResult::Success,
                node_id: Some(node_id),
                server_pubkey: Some(key.public_key().clone()),
            };
            let req_packet = Packet::make_packet(
                Command::Handshake,
                bincode::serialize(&handshake_resp).unwrap(),
            );
            conn_addr
                .send(ConnectionMessage::Send(node_id, req_packet.serialize()))
                .await;
        } else {
            warn!("Wrong password from node UUID: {}", node_id);
            let handshake_resp = HandshakeResponse {
                result: HandshakeResult::PasswordNotMatched,
                node_id: None,
                server_pubkey: None,
            };
            let req_packet = Packet::make_packet(
                Command::Handshake,
                bincode::serialize(&handshake_resp).unwrap(),
            );
            conn_addr
                .send(ConnectionMessage::Send(node_id, req_packet.serialize()))
                .await;
            handler.send(HandlerMessage::DownNode(node_id)).await;
        }
    } else {
        error!("Some error occurred when processing Command::Handshake");
    }
}
