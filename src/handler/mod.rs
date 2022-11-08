pub mod connection;
pub mod judge;
pub mod stream;
pub mod testcase;
pub mod recv_stream;

use async_std::path::Path;
use async_std::sync::Arc;

use actix::dev::ToEnvelope;
use actix::prelude::*;

use judge_protocol::packet::*;
use judge_protocol::security::*;

use uuid::Uuid;

use k256::ecdh::{EphemeralSecret, SharedSecret};

use std::collections::HashMap;

use crate::commands::*;
use crate::config::Config;
use crate::event::*;
use crate::judge::*;
use crate::scheduler::by_deadline::ByDeadlineWeighted;
use crate::scheduler::*;

use connection::*;
use judge::*;
use testcase::*;

#[derive(Clone, Debug, MessageResponse)]
pub enum HandlerResponse {
    None,
}

#[derive(Clone, Message)]
#[rtype(result = "()")]

pub enum HandlerMessage<P> {
    ReceivePacket(Uuid, Packet),
    Judge(RequestJudge<P>),
    DispatchJudge(Uuid, Uuid, Arc<SharedSecret>),
    DownNode(Uuid),
    UnlockSlave(Uuid),
    Shutdown,
    Unknown,
}

#[derive(Clone)]
pub struct HandlerService<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    pub cfg: Arc<Config>,
    pub host_pass: Arc<Vec<u8>>,
    pub priv_key: Arc<EphemeralSecret>,
    pub event_addr: Addr<T>,
    pub judge_addrs: HashMap<Uuid, Addr<JudgeService<T, P>>>,
    pub connection_addr: Option<Addr<ConnectionService<T, P>>>,
    pub scheduler_addr: Option<Addr<ByDeadlineWeighted<T, P>>>,
}

impl<T, P> HandlerService<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    pub fn start(cfg: Config, event_addr: Addr<T>) -> Addr<Self> {
        let pass = cfg.general.host_pass.clone();
        Self {
            cfg: Arc::new(cfg),
            host_pass: Arc::new(blake3::hash(pass.as_bytes()).as_bytes().to_vec()),
            priv_key: Arc::new(EphemeralSecret::random(rand::thread_rng())),
            event_addr,
            judge_addrs: HashMap::new(),
            connection_addr: None,
            scheduler_addr: None,
        }
        .start()
    }
}

impl<T, P> Actor for HandlerService<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        info!("pms-master {}", env!("CARGO_PKG_VERSION"));
        self.scheduler_addr = Some(ByDeadlineWeighted::start(ctx.address()));
        self.connection_addr = Some(ConnectionService::start(
            Arc::clone(&self.cfg),
            self.scheduler_addr.as_ref().unwrap().clone(),
            ctx.address(),
        ));
    }
}

impl<T, P> Handler<HandlerMessage<P>> for HandlerService<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    type Result = ();

    fn handle(&mut self, msg: HandlerMessage<P>, ctx: &mut Self::Context) -> Self::Result {
        match msg {
            HandlerMessage::ReceivePacket(node_id, packet) => match packet.header.command {
                Command::Handshake => {
                    let pass = Arc::clone(&self.host_pass);
                    let key = Arc::clone(&self.priv_key);
                    let conn = self.connection_addr.as_ref().unwrap().clone();
                    let scheduler = self.scheduler_addr.as_ref().unwrap().clone();
                    let handler = ctx.address();
                    actix::spawn(async move {
                        handle_handshake(node_id, packet, pass, key, conn, scheduler, handler).await
                    });
                }
                Command::GetJudgeStateUpdate => {
                    handle_judge_state(node_id, packet, self.event_addr.clone(), &self.judge_addrs);
                }
                _ => {}
            },
            HandlerMessage::Judge(judge) => {
                let judge_uuid = judge.uuid;
                let judge_addr = JudgeService::start(
                    judge_uuid,
                    self.event_addr.clone(),
                    self.scheduler_addr.as_ref().unwrap().clone(),
                    ctx.address(),
                    self.connection_addr.as_ref().unwrap().clone(),
                    judge,
                );
                self.judge_addrs.insert(judge_uuid, judge_addr);
            }
            HandlerMessage::DispatchJudge(node_id, judge_uuid, key) => {
                self.judge_addrs[&judge_uuid]
                    .do_send(JudgeMessage::Dispatch(node_id, expand_key(key)));
            }
            HandlerMessage::DownNode(node_id) => {
                self.connection_addr
                    .as_ref()
                    .unwrap()
                    .do_send(ConnectionMessage::DeleteStream(node_id));
                self.scheduler_addr
                    .as_ref()
                    .unwrap()
                    .do_send(SchedulerIssue::RemoveNode(node_id));
            }
            _ => {}
        }
    }
}
