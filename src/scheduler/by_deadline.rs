use uuid::Uuid;

use async_std::path::Path;
use async_std::sync::Arc;

use actix::dev::ToEnvelope;

use super::*;
use crate::constants::*;
use crate::event::*;
use crate::handler::*;

// 1 | sp-graph;r_i;d_i | L_max - scheduler (approximated for P | sp-graph;r_i;d_i | L_max) implement for pms-master

use actix::prelude::*;

use k256::ecdh::SharedSecret;

use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap, HashMap};

use log::*;

// TODO: reduce overhead
pub struct ByDeadlineWeighted<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    nodes: HashMap<Uuid, BinaryHeap<(Reverse<u64>, Reverse<u64>, Uuid)>>,
    shared_key: HashMap<Uuid, Arc<SharedSecret>>,
    pending: HashMap<Uuid, (Uuid, u64)>,
    nodes_sz: HashMap<Uuid, u64>,
    nodes_by_sz: BTreeSet<(u64, Uuid)>,
    handler: Addr<HandlerService<T, P>>,
    node_time: HashMap<Uuid, u64>,
}

impl<T, P> Actor for ByDeadlineWeighted<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("Starting scheduler (scheduler::ByDeadlineWeighted)");
    }
}

impl<T, P> Handler<SchedulerIssue> for ByDeadlineWeighted<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    type Result = SchedulerResp;

    fn handle(&mut self, msg: SchedulerIssue, ctx: &mut Self::Context) -> Self::Result {
        match msg {
            SchedulerIssue::NewNode(node_id) => {
                if let Err(e) = self.register(node_id) {
                    SchedulerResp::Error(e)
                } else {
                    SchedulerResp::None
                }
            }
            SchedulerIssue::RemoveNode(node_id) => {
                if let Err(e) = self.unregister(node_id) {
                    SchedulerResp::Error(e)
                } else {
                    SchedulerResp::None
                }
            }
            SchedulerIssue::ForceRebalance => {
                if let Err(e) = self.rebalance() {
                    SchedulerResp::Error(e)
                } else {
                    SchedulerResp::None
                }
            }
            SchedulerIssue::NewTask(task_uuid, total_time, weight) => {
                match self.push(task_uuid, total_time, weight) {
                    Err(e) => SchedulerResp::Error(e),
                    Ok(node_id) => SchedulerResp::Node(node_id),
                }
            }
            SchedulerIssue::Touch(node_id) => {
                if let Err(e) = self.touch(node_id) {
                    SchedulerResp::Error(e)
                } else {
                    SchedulerResp::None
                }
            }
            SchedulerIssue::HandshakeEstablished(node_id, key) => {
                if let Err(e) = self.promote(node_id, key) {
                    SchedulerResp::Error(e)
                } else {
                    SchedulerResp::None
                }
            }
            SchedulerIssue::Exists(node_id) => {
                if self.nodes.contains_key(&node_id) {
                    SchedulerResp::None
                } else {
                    SchedulerResp::Error(SchedulerError::NoNodeFound)
                }
            }
        }
    }
}

impl<T, P> ByDeadlineWeighted<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    pub fn start(handler: Addr<HandlerService<T, P>>) -> Addr<Self> {
        let mut s = Self {
            nodes: HashMap::new(),
            pending: HashMap::new(),
            nodes_sz: HashMap::new(),
            nodes_by_sz: BTreeSet::new(),
            shared_key: HashMap::new(),
            handler,
            node_time: HashMap::new(),
        };
        s.nodes.insert(NODE_ZERO, BinaryHeap::new());
        s.pending.insert(NODE_ZERO, (Uuid::nil(), 0));
        s.nodes_sz.insert(NODE_ZERO, 0);
        s.node_time.insert(NODE_ZERO, 0);
        s.start()
    }
}

impl<T, P> SchedulerWeighted<T, P> for ByDeadlineWeighted<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    fn register(&mut self, node_id: Uuid) -> SchedulerResult<()> {
        if !self.nodes.contains_key(&node_id) {
            self.nodes.insert(node_id, BinaryHeap::new());
            self.pending.insert(node_id, (Uuid::nil(), 0));
            self.nodes_sz.insert(node_id, 0);
            self.node_time.insert(node_id, 0);
            self.rebalance()?;
            Ok(())
        } else {
            Err(SchedulerError::NodeAlreadyExists)
        }
    }

    fn promote(&mut self, node_id: Uuid, key: Arc<SharedSecret>) -> SchedulerResult<()> {
        if self.nodes.contains_key(&node_id) {
            self.shared_key.insert(node_id, key);
            self.nodes_by_sz.insert((self.nodes_sz[&node_id], node_id));
            Ok(())
        } else {
            Err(SchedulerError::NoNodeFound)
        }
    }

    fn push(&mut self, judge_uuid: Uuid, total_time: u64, weight: u64) -> SchedulerResult<Uuid> {
        let mut not_found = false;
        let (mut id, mut _sz, mut _new_sz) = (Uuid::nil(), std::u64::MAX, std::u64::MAX);
        if let Some(&(sz, node_id)) = self.nodes_by_sz.iter().nth(0) {
            // node selector
            let new_sz = sz + total_time * weight;
            let deadline = self.node_time[&node_id] + total_time * weight;
            let mut node = self.nodes.get_mut(&node_id).unwrap();
            let mut node_sz = self.nodes_sz.get_mut(&node_id).unwrap();
            node.push((Reverse(deadline), Reverse(total_time * weight), judge_uuid));
            *node_sz = new_sz;
            _sz = sz;
            _new_sz = new_sz;

            id = node_id;
        } else {
            not_found = true;
        }
        if not_found {
            let time = total_time * weight;
            let sz = self.nodes_sz[&NODE_ZERO];
            let new_sz = sz + time;
            let mut node_zero = self.nodes.get_mut(&NODE_ZERO).unwrap();
            let mut node_zero_sz = self.nodes_sz.get_mut(&NODE_ZERO).unwrap();
            node_zero.push((Reverse(0), Reverse(time), judge_uuid));
            *node_zero_sz = new_sz;
            Ok(NODE_ZERO)
        } else {
            self.nodes_by_sz.remove(&(_sz, id));
            self.nodes_by_sz.insert((_new_sz, id));
            if self.pending[&id] == (Uuid::nil(), 0) {
                self.touch(id);
            }
            Ok(id)
        }
    }

    fn touch(&mut self, node_id: Uuid) -> SchedulerResult<()> {
        {
            let (uuid, sz) = self.pending[&node_id];
            if !uuid.is_nil() {
                self.nodes_by_sz.remove(&(self.nodes_sz[&node_id], node_id));
                let mut node_sz = self.nodes_sz.get_mut(&node_id).unwrap();
                let mut node_time = self.node_time.get_mut(&node_id).unwrap();
                let mut pending = self.pending.get_mut(&node_id).unwrap();
                *node_sz -= sz;
                self.nodes_by_sz.insert((self.nodes_sz[&node_id], node_id));
                *node_time += sz;
                *pending = (Uuid::nil(), 0);
            }
        }
        if let Some((Reverse(_), Reverse(sz), uuid)) = self.nodes.get_mut(&node_id).unwrap().pop() {
            let mut pending = self.pending.get_mut(&node_id).unwrap();
            *pending = (uuid, sz);
            self.handler.do_send(HandlerMessage::DispatchJudge(
                uuid,
                node_id,
                Arc::clone(&self.shared_key[&node_id]),
            ));
        }
        Ok(())
    }

    fn unregister(&mut self, node_id: Uuid) -> SchedulerResult<()> {
        if self.nodes.contains_key(&node_id) {
            self.nodes_by_sz.remove(&(self.nodes_sz[&node_id], node_id));
            while let Some((Reverse(_), Reverse(time), uuid)) =
                self.nodes.get_mut(&node_id).unwrap().pop()
            {
                let sz = self.nodes_sz[&NODE_ZERO];
                let new_sz = sz + time;
                let node_zero = self.nodes.get_mut(&NODE_ZERO).unwrap();
                let node_sz = self.nodes_sz.get_mut(&NODE_ZERO).unwrap();
                node_zero.push((Reverse(0), Reverse(time), uuid));
                *node_sz = new_sz;
            }
            self.nodes.remove(&node_id);
            self.node_time.remove(&node_id);
            self.nodes_sz.remove(&node_id);
            self.pending.remove(&node_id);
            self.shared_key.remove(&node_id);
            self.rebalance()?;
            Ok(())
        } else {
            Err(SchedulerError::NoNodeFound)
        }
    }

    fn rebalance(&mut self) -> SchedulerResult<()> {
        if self.nodes.len() > 1 {
            let mut returns = vec![];
            let mut used_nodes = BTreeSet::new();
            while let Some((Reverse(_), Reverse(time), uuid)) =
                self.nodes.get_mut(&NODE_ZERO).unwrap().pop()
            {
                if let Some(&(sz, node_id)) = self.nodes_by_sz.iter().nth(0) {
                    // node selector
                    let new_sz = sz + time;
                    let deadline = self.node_time[&node_id] + time;
                    let mut node = self.nodes.get_mut(&node_id).unwrap();
                    let mut node_sz = self.nodes_sz.get_mut(&node_id).unwrap();
                    node.push((Reverse(deadline), Reverse(time), uuid));
                    *node_sz = new_sz;
                    self.nodes_by_sz.remove(&(sz, node_id));
                    self.nodes_by_sz.insert((new_sz, node_id));
                    used_nodes.insert(node_id);
                } else {
                    returns.push((time, uuid));
                }
            }
            for node_id in used_nodes {
                if self.pending[&node_id] == (Uuid::nil(), 0) {
                    self.touch(node_id);
                }
            }
        }
        Ok(())
    }
}
