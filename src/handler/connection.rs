use actix::dev::ToEnvelope;
use actix::prelude::*;

use s2n_quic::Server;

use async_std::fs::read_to_string;
use async_std::path::Path;
use async_std::sync::Arc;

use s2n_quic::stream::PeerStream;

use std::collections::HashMap;

use futures::FutureExt;
use futures::{join, select};

use uuid::Uuid;

use crate::config::*;
use crate::event::*;
use crate::handler::stream::*;
use crate::handler::recv_stream::*;
use crate::handler::*;
use crate::scheduler::by_deadline::ByDeadlineWeighted;
use crate::scheduler::*;

#[derive(Message)]
#[rtype(result = "()")]
pub enum ConnectionMessage<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    NewStream(Uuid, Addr<StreamActor<T, P>>),
    DeleteStream(Uuid),
    Send(Uuid, Vec<u8>),
}

pub struct ConnectionService<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    cfg: Arc<Config>,
    scheduler_addr: Addr<ByDeadlineWeighted<T, P>>,
    handler_addr: Addr<HandlerService<T, P>>,
    connections: HashMap<Uuid, Addr<StreamActor<T, P>>>,
}

impl<T, P> ConnectionService<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    pub fn start(
        cfg: Arc<Config>,
        scheduler_addr: Addr<ByDeadlineWeighted<T, P>>,
        handler_addr: Addr<HandlerService<T, P>>,
    ) -> Addr<Self> {
        Self {
            cfg,
            scheduler_addr,
            handler_addr,
            connections: HashMap::new(),
        }
        .start()
    }
}

impl<T, P> Actor for ConnectionService<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("Starting connection management service");
        let addr = ctx.address();
        let cfg = Arc::clone(&self.cfg);
        let handler_addr = self.handler_addr.clone();
        let scheduler_addr = self.scheduler_addr.clone();
        actix::spawn(async move {
            let (cert, key) = join!(
                read_to_string(cfg.tls.cert_pem.clone()),
                read_to_string(cfg.tls.key_pem.clone())
            );
            let mut server: Server = Server::builder()
                .with_tls((
                    cert.expect("Certficiation is not found").as_str(),
                    key.expect("Key is not found").as_str(),
                ))
                .expect("Failed to configure TLS")
                .with_io(cfg.general.host.as_str())
                .expect(&format!(
                    "Failed to open address: {}",
                    cfg.general.host.clone()
                ))
                .start()
                .expect(&format!(
                    "Failed to start server: {}",
                    cfg.general.host.clone()
                ));
            while let Some(mut connection) = server.accept().await {
                let node_id = Uuid::new_v4();
                info!("New connection from {}", connection.remote_addr().unwrap());
                let scheduler_addr = scheduler_addr.clone();
                let addr = addr.clone();
                let handler_addr = handler_addr.clone();
                actix::spawn(async move {
                    while let Ok(Some(peer_stream)) = connection.accept().await {
                        match peer_stream {
                            PeerStream::Bidirectional(stream) => {
                                scheduler_addr.send(SchedulerIssue::NewNode(node_id)).await;
                                let stream_addr =
                                    StreamActor::start(node_id, handler_addr.clone(), stream);
                                addr.send(ConnectionMessage::NewStream(node_id, stream_addr))
                                    .await;
                            }
                            PeerStream::Receive(stream) => {
                                if let Ok(SchedulerResp::None) = scheduler_addr.send(SchedulerIssue::Exists(node_id)).await {
                                    let recv_addr = RecvStreamActor::start(node_id, stream, handler_addr.clone());
                                    // TODO - Dead stream collector
                                } else {
                                    error!("Node {node_id} issued to open receive stream before open bidirectional stream");
                                }
                            }
                        }
                    }
                });
            }
        });
    }
}

impl<T, P> Handler<ConnectionMessage<T, P>> for ConnectionService<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    type Result = ();

    fn handle(&mut self, msg: ConnectionMessage<T, P>, ctx: &mut Self::Context) -> Self::Result {
        match msg {
            ConnectionMessage::NewStream(node_id, stream_addr) => {
                self.connections.insert(node_id, stream_addr);
            }
            ConnectionMessage::Send(node_id, raw) => {
                self.connections[&node_id].do_send(StreamMessage::Send(raw));
            }
            ConnectionMessage::DeleteStream(node_id) => {
                self.connections[&node_id].do_send(StreamMessage::Delete);
                self.connections.remove(&node_id);
            }
            _ => {}
        }
    }
}
