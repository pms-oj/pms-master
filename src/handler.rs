use super::config::Config;
use async_std::io::{Error, ErrorKind};
use async_std::net::{TcpListener, TcpStream};
use async_std::prelude::*;
use async_std::sync::*;
use async_std::task::spawn;
use futures::stream::StreamExt;

use judge_protocol::packet::*;
use k256::ecdh::{EphemeralSecret, SharedSecret};
use k256::PublicKey;
use rand::thread_rng;
use std::pin::Pin;

pub struct State {
    pub cfg: Arc<Config>,
    key: Arc<EphemeralSecret>,
    shared: Arc<Mutex<Option<SharedSecret>>>,
}

pub async fn serve(cfg: Config) {
    let key = EphemeralSecret::random(thread_rng());
    let state = Arc::new(Mutex::new(State {
        cfg: Arc::new(cfg.clone()),
        key: Arc::new(key),
        shared: Arc::new(Mutex::new(None)),
    }));
    let listener = TcpListener::bind(cfg.host)
        .await
        .expect(&format!("Cannot bind {:?}", cfg.host));
    listener
        .incoming()
        .for_each_concurrent(None, |stream| async {
            let stream = stream.unwrap();
            let state_mutex = Arc::clone(&state);
            spawn(async move { (state_mutex.lock().await).handle_connection(stream).await });
            //drop(state_mutex);
        })
        .await;
}

impl State {
    pub async fn handle_connection(&mut self, mut stream: TcpStream) -> async_std::io::Result<()> {
        let packet = Packet::from_stream(Pin::new(&mut stream)).await?;
        self.handle_command(stream, packet).await
    }

    async fn send_packet(
        &self,
        mut stream: TcpStream,
        packet: Packet,
    ) -> async_std::io::Result<()> {
        stream
            .write_all(&bincode::serialize(&packet).unwrap())
            .await?;
        Ok(())
    }

    async fn handle_command(
        &mut self,
        stream: TcpStream,
        packet: Packet,
    ) -> async_std::io::Result<()> {
        match packet.heady.header.command {
            Command::HANDSHAKE => {
                if let Ok(client_pubkey) = bincode::deserialize::<PublicKey>(&packet.heady.body) {
                    self.shared =
                        Arc::new(Mutex::new(Some(self.key.diffie_hellman(&client_pubkey))));
                    let req_packet = Packet::make_packet(
                        Command::HANDSHAKE,
                        bincode::serialize(&self.key.public_key()).unwrap(),
                    );
                    self.send_packet(stream, req_packet).await
                } else {
                    Err(Error::new(ErrorKind::InvalidData, "Invalid packet"))
                }
            }
            _ => Ok(()),
        }
    }
}
