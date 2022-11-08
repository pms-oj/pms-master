use async_std::channel::{Receiver, Sender};
use async_std::io::prelude::*;
use async_std::io::ErrorKind;
use async_std::net::TcpStream;
use async_std::path::Path;
use async_std::sync::Arc;
use async_std::task::block_on;
use futures::select;
use futures::stream;
use futures::FutureExt;
use futures::StreamExt;
use judge_protocol::packet::*;

use actix::dev::ToEnvelope;
use actix::prelude::*;

use s2n_quic::stream::*;

use uuid::Uuid;

use crate::constants::*;
use crate::event::*;
use crate::handler::*;
use crate::scheduler::*;

pub struct StreamActor<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    node_id: Uuid,
    handler_addr: Addr<HandlerService<T, P>>,
    stream: SendStream,
}

#[derive(Clone, Debug, Message)]
#[rtype(result = "()")]
pub enum StreamMessage {
    Send(Vec<u8>),
    Delete,
}

#[derive(Clone, Debug, Message)]
#[rtype(result = "()")]
pub struct PacketMessage {
    raw: Packet,
}

impl<T, P> Actor for StreamActor<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("Starting stream actor");
        ctx.run_interval(
            CHECK_ALIVE_TIME,
            |stream: &mut StreamActor<T, P>, ctx: &mut Context<StreamActor<T, P>>| {
                block_on(async move {
                    if let Err(e) = stream.stream.connection().ping() {
                        error!(
                            "Failed to write stream; getting down node (node_id: {})",
                            stream.node_id
                        );
                        stream
                            .handler_addr
                            .send(HandlerMessage::DownNode(stream.node_id))
                            .await;
                    }
                });
            },
        );
    }
}

impl<T, P> StreamActor<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    pub fn start(
        node_id: Uuid,
        handler_addr: Addr<HandlerService<T, P>>,
        stream: BidirectionalStream,
    ) -> Addr<Self> {
        let (mut recv, send) = stream.split();
        let addr = Self {
            node_id,
            handler_addr,
            stream: send,
        }
        .start();
        let addr_cloned = addr.clone();
        actix::spawn(async move {
            while let Ok(packet) = Packet::from_stream(&mut recv).await {
                addr_cloned.do_send(PacketMessage { raw: packet });
            }
        });
        addr
    }
}

impl<T, P> Handler<PacketMessage> for StreamActor<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    type Result = ();

    fn handle(&mut self, msg: PacketMessage, ctx: &mut Self::Context) -> Self::Result {
        self.handler_addr
            .do_send(HandlerMessage::ReceivePacket(self.node_id, msg.raw))
    }
}

impl<T, P> Handler<StreamMessage> for StreamActor<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    type Result = ();

    fn handle(&mut self, msg: StreamMessage, ctx: &mut Self::Context) -> Self::Result {
        match msg {
            StreamMessage::Send(raw) => {
                let mut sys = System::new();
                sys.block_on(async {
                    if let Err(e) = self.stream.send(raw.into()).await {
                        error!(
                            "Failed to write stream; getting down node (node_id: {})",
                            self.node_id
                        );
                        self.handler_addr
                            .send(HandlerMessage::DownNode(self.node_id))
                            .await;
                    }
                });
                sys.run();
            }
            StreamMessage::Delete => {
                ctx.terminate();
            }
        }
    }
}
