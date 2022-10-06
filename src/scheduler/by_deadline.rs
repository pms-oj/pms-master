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
    tx: Arc<Mutex<Sender<SchedulerMessage>>>,
    node_time: Arc<Mutex<Vec<u64>>>,
}

#[async_trait]
impl SchedulerWeighted for ByDeadlineWeighted {
    fn new(tx: Arc<Mutex<Sender<SchedulerMessage>>>) -> Self {
        Self {
            node_cnt: Mutex::new(0),
            nodes: Arc::new(Mutex::new(vec![])),
            pending: Arc::new(Mutex::new(vec![])),
            nodes_sz: Arc::new(Mutex::new(vec![])),
            nodes_by_sz: Arc::new(Mutex::new(BTreeSet::new())),
            tx,
            node_time: Arc::new(Mutex::new(vec![])),
        }
    }

    async fn register(&mut self) {
        debug!("register new node!");
        let nodes_by_sz = Arc::clone(&self.nodes_by_sz);
        let nodes = Arc::clone(&self.nodes);
        let nodes_sz = Arc::clone(&self.nodes_sz);
        let node_time = Arc::clone(&self.node_time);
        let pending = Arc::clone(&self.pending);
        *self.node_cnt.lock().await += 1;
        nodes.lock().await.push(BinaryHeap::new());
        pending.lock().await.push((Uuid::nil(), 0));
        nodes_sz.lock().await.push(0);
        nodes_by_sz
            .lock()
            .await
            .insert((0, (*self.node_cnt.lock().await - 1) as usize));
        node_time.lock().await.push(0);
    }

    async fn push(&mut self, uuid: Uuid, total_time: u64, weight: u64) -> SchedulerResult<usize> {
        let nodes_by_sz = Arc::clone(&self.nodes_by_sz);
        let nodes = Arc::clone(&self.nodes);
        let nodes_sz = Arc::clone(&self.nodes_sz);
        let node_time = Arc::clone(&self.node_time);
        let mut not_found = false;
        let (mut id, mut _sz, mut _new_sz) = (std::usize::MAX, std::u64::MAX, std::u64::MAX);
        if let Some(&(sz, node_id)) = nodes_by_sz.lock().await.iter().nth(0) {
            // node selector
            let new_sz = sz + total_time * weight;
            let deadline = node_time.lock().await[node_id] + total_time * weight;
            nodes.lock().await[node_id].push((
                Reverse(deadline),
                Reverse(total_time * weight),
                uuid,
            ));
            nodes_sz.lock().await[node_id] = new_sz;
            _sz = sz;
            _new_sz = new_sz;

            id = node_id;
        } else {
            not_found = true;
        }
        if not_found {
            Err(SchedulerError::NoNodeFound)
        } else {
            nodes_by_sz.lock().await.remove(&(_sz, id));
            nodes_by_sz.lock().await.insert((_new_sz, id));
            Ok(id)
        }
    }

    async fn touch(&mut self, node_id: usize) -> SchedulerResult<()> {
        let nodes_by_sz = Arc::clone(&self.nodes_by_sz);
        let nodes = Arc::clone(&self.nodes);
        let nodes_sz = Arc::clone(&self.nodes_sz);
        let node_time = Arc::clone(&self.node_time);
        let pending = Arc::clone(&self.pending);
        {
            let (uuid, sz) = pending.lock().await[node_id];
            if !uuid.is_nil() {
                nodes_by_sz
                    .lock()
                    .await
                    .remove(&(nodes_sz.lock().await[node_id], node_id));
                nodes_sz.lock().await[node_id] -= sz;
                nodes_by_sz
                    .lock()
                    .await
                    .insert((nodes_sz.lock().await[node_id], node_id));
                node_time.lock().await[node_id] += sz;
                pending.lock().await[node_id] = (Uuid::nil(), 0);
            }
        }
        if let Some((Reverse(_), Reverse(sz), uuid)) = nodes.lock().await[node_id].pop() {
            pending.lock().await[node_id] = (uuid, sz);
            self.tx
                .lock()
                .await
                .send(SchedulerMessage::Send(uuid, node_id as u32))
                .await
                .ok();
        }
        Ok(())
    }
}
