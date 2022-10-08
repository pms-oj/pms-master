use uuid::Uuid;

use super::*;
use crate::constants::*;

// 1 | sp-graph;r_i;d_i | L_max - scheduler (approximated for P | sp-graph;r_i;d_i | L_max) implement for pms-master

use async_std::channel::Sender;
use async_std::sync::{Arc, Mutex};
use async_trait::async_trait;

use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap};

use log::*;

// TODO: reduce overhead
pub struct ByDeadlineWeighted {
    node_cnt: Mutex<usize>,
    nodes: Arc<Mutex<Vec<BinaryHeap<(Reverse<u64>, Reverse<u64>, Uuid)>>>>,
    pending: Arc<Mutex<Vec<(Uuid, u64)>>>,
    nodes_sz: Arc<Mutex<Vec<u64>>>,
    nodes_by_sz: Arc<Mutex<BTreeSet<(u64, usize)>>>,
    tx: Arc<Sender<SchedulerMessage>>,
    node_time: Arc<Mutex<Vec<u64>>>,
}

#[async_trait]
impl SchedulerWeighted for ByDeadlineWeighted {
    fn new(tx: Sender<SchedulerMessage>) -> Self {
        Self {
            node_cnt: Mutex::new(1),
            nodes: Arc::new(Mutex::new(vec![BinaryHeap::new()])),
            pending: Arc::new(Mutex::new(vec![(Uuid::nil(), 0)])),
            nodes_sz: Arc::new(Mutex::new(vec![0])),
            nodes_by_sz: Arc::new(Mutex::new(BTreeSet::new())),
            tx: Arc::new(tx),
            node_time: Arc::new(Mutex::new(vec![0])),
        }
    }

    async fn register(&mut self) {
        debug!("register new node!");
        let mut nodes_by_sz = self.nodes_by_sz.lock().await;
        let mut nodes = self.nodes.lock().await;
        let mut nodes_sz = self.nodes_sz.lock().await;
        let mut node_time = self.node_time.lock().await;
        let mut pending = self.pending.lock().await;
        *self.node_cnt.lock().await += 1;
        nodes.push(BinaryHeap::new());
        pending.push((Uuid::nil(), 0));
        nodes_sz.push(0);
        nodes_by_sz.insert((0, (*self.node_cnt.lock().await - 1) as usize));
        node_time.push(0);
        drop(nodes_by_sz);
        drop(nodes);
        drop(nodes_sz);
        drop(node_time);
        drop(pending);
    }

    async fn push(&mut self, uuid: Uuid, total_time: u64, weight: u64) -> SchedulerResult<usize> {
        let mut nodes_by_sz = self.nodes_by_sz.lock().await;
        let mut nodes = self.nodes.lock().await;
        let mut nodes_sz = self.nodes_sz.lock().await;
        let mut node_time = self.node_time.lock().await;
        let pending = self.pending.lock().await;
        let mut not_found = false;
        let (mut id, mut _sz, mut _new_sz) = (std::usize::MAX, std::u64::MAX, std::u64::MAX);
        if let Some(&(sz, node_id)) = nodes_by_sz.iter().nth(0) {
            // node selector
            let new_sz = sz + total_time * weight;
            let deadline = node_time[node_id] + total_time * weight;
            nodes[node_id].push((Reverse(deadline), Reverse(total_time * weight), uuid));
            nodes_sz[node_id] = new_sz;
            _sz = sz;
            _new_sz = new_sz;

            id = node_id;
        } else {
            not_found = true;
        }
        if not_found {
            Err(SchedulerError::NoNodeFound)
        } else {
            nodes_by_sz.remove(&(_sz, id));
            nodes_by_sz.insert((_new_sz, id));
            drop(nodes_by_sz);
            drop(nodes);
            drop(nodes_sz);
            drop(node_time);
            drop(pending);
            if self.pending.lock().await[id] == (Uuid::nil(), 0) {
                self.touch(id).await?;
            }
            Ok(id)
        }
    }

    async fn touch(&mut self, node_id: usize) -> SchedulerResult<()> {
        let mut nodes_by_sz = self.nodes_by_sz.lock().await;
        let mut nodes = self.nodes.lock().await;
        let mut nodes_sz = self.nodes_sz.lock().await;
        let mut node_time = self.node_time.lock().await;
        let mut pending = self.pending.lock().await;
        {
            let (uuid, sz) = pending[node_id];
            if !uuid.is_nil() {
                nodes_by_sz.remove(&(nodes_sz[node_id], node_id));
                nodes_sz[node_id] -= sz;
                nodes_by_sz.insert((nodes_sz[node_id], node_id));
                node_time[node_id] += sz;
                pending[node_id] = (Uuid::nil(), 0);
            }
        }
        if let Some((Reverse(_), Reverse(sz), uuid)) = nodes[node_id].pop() {
            pending[node_id] = (uuid, sz);
            self.tx
                .send(SchedulerMessage::Send(uuid, node_id as u32))
                .await
                .ok();
        }
        Ok(())
    }

    async fn unregister(&mut self, node_id: usize) -> SchedulerResult<()> {
        let mut nodes_by_sz = self.nodes_by_sz.lock().await;
        let mut nodes_sz = self.nodes_sz.lock().await;
        let mut nodes = self.nodes.lock().await;
        nodes_by_sz.remove(&(nodes_sz[node_id], node_id));
        while let Some((Reverse(_), Reverse(time), uuid)) = nodes[node_id].pop() {
            let sz = nodes_sz[NODE_ZERO];
            let new_sz = sz + time;
            nodes[NODE_ZERO].push((Reverse(0), Reverse(time), uuid));
            nodes_sz[NODE_ZERO] = new_sz;
        }
        drop(nodes_by_sz);
        drop(nodes_sz);
        drop(nodes);
        self.rebalance().await?;
        Ok(())
    }

    async fn rebalance(&mut self) -> SchedulerResult<()> {
        if *self.node_cnt.lock().await > 1 {
            let mut nodes_by_sz = self.nodes_by_sz.lock().await;
            let mut nodes_sz = self.nodes_sz.lock().await;
            let mut nodes = self.nodes.lock().await;
            let node_time = self.node_time.lock().await;
            let mut returns = vec![];
            let mut used_nodes = BTreeSet::new();
            while let Some((Reverse(_), Reverse(time), uuid)) = nodes[NODE_ZERO].pop() {
                if let Some(&(sz, node_id)) = nodes_by_sz.iter().nth(0) {
                    // node selector
                    let new_sz = sz + time;
                    let deadline = node_time[node_id] + time;
                    nodes[node_id].push((Reverse(deadline), Reverse(time), uuid));
                    nodes_sz[node_id] = new_sz;
                    nodes_by_sz.remove(&(sz, node_id));
                    nodes_by_sz.insert((new_sz, node_id));
                    used_nodes.insert(node_id);
                } else {
                    returns.push((time, uuid));
                }
            }
            drop(nodes_by_sz);
            drop(nodes_sz);
            drop(nodes);
            drop(node_time);
            for node_id in used_nodes {
                if self.pending.lock().await[node_id] == (Uuid::nil(), 0) {
                    self.touch(node_id).await?;
                }
            }
        }
        Ok(())
    }
}
