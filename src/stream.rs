use async_std::channel::{Receiver, Sender};
use async_std::io::prelude::*;
use async_std::io::{Error, ErrorKind};
use async_std::net::TcpStream;
use async_std::sync::Arc;
use futures::select;
use futures::FutureExt;
use futures::StreamExt;
use judge_protocol::packet::*;

use crate::broker::*;
use crate::scheduler::*;

pub async fn serve_stream(
    scheduler_tx: Sender<SchedulerMessage>,
    broker_tx: Sender<BrokerMessage>,
    packet_rx: &mut Receiver<Vec<u8>>,
    stream: Arc<TcpStream>,
    node_id: u32,
) {
    loop {
        select! {
            packet = Packet::from_stream(Arc::clone(&stream)).fuse() => match packet {
                Ok(packet) => {
                    let stream_cloned = Arc::clone(&stream);
                    broker_tx.send(BrokerMessage::Packet(stream_cloned, packet)).await;
                }
                Err(_) => {}
            },
            msg = packet_rx.next().fuse() => match msg {
                Some(msg) => {
                    let mut stream = &*stream;
                    if let Err(err) = stream.peer_addr() {
                        let kind = err.kind();
                        if let ErrorKind::NotConnected = kind {
                            debug!("try to down node!");
                            scheduler_tx.send(SchedulerMessage::DownNode(node_id)).await.ok();
                            break;
                        }
                    }
                    stream.write_all(&msg).await.ok();
                    stream.flush().await.ok();
                },
                None => {}
            }
        }
    }
}
