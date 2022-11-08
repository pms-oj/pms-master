use actix::prelude::*;
use actix::dev::ToEnvelope;

use async_std::path::Path;

use s2n_quic::stream::ReceiveStream;

use uuid::Uuid;

use crate::event::*;
use crate::handler::*;

#[derive(Clone, Debug, Message)]
#[rtype(result = "()")]
pub struct PacketMessage {
    raw: Packet,
}

#[derive(Clone, Debug)]
pub struct RecvStreamActor<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{   
    handler_addr: Addr<HandlerService<T, P>>,
    node_id: Uuid,
}

impl<T, P> RecvStreamActor<T, P> 
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin, {
    pub fn start(node_id: Uuid, mut recv: ReceiveStream, handler: Addr<HandlerService<T, P>>) -> Addr<Self> {
        let ret = Self {
            handler_addr: handler,
            node_id,
        }.start();
        let addr_cloned = ret.clone();
        actix::spawn(async move {
            while let Ok(packet) = Packet::from_stream(&mut recv).await {
                addr_cloned.do_send(PacketMessage { raw: packet });
            }
        });
        ret
    }
}

impl<T, P> Handler<PacketMessage> for RecvStreamActor<T, P>
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

impl<T, P> Actor for RecvStreamActor<T, P> 
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin, {
        type Context = Context<Self>;

        fn started(&mut self, ctx: &mut Self::Context) {
            debug!("Starting receive stream actor");
        }
}