use super::config::Config;
use async_std::io::{Error, ErrorKind};
use async_std::net::{TcpListener, TcpStream};
use async_std::prelude::*;
use async_std::sync::*;
use async_std::task::spawn;
use bincode::Options;
use futures::stream::StreamExt;
use judge_protocol::constants::*;
use judge_protocol::handshake::*;
use judge_protocol::packet::*;
use k256::ecdh::{EphemeralSecret, SharedSecret};
use k256::sha2::digest::typenum::private::IsEqualPrivate;
use k256::PublicKey;
use log::*;
use rand::thread_rng;
use std::pin::Pin;
use sha3::{Sha3_256, Digest};

pub struct State {
    pub cfg: Arc<Mutex<Config>>,
    host_pass: Arc<Mutex<Vec<u8>>>,
    count: Mutex<u32>,
    key: Arc<EphemeralSecret>,
    pubkey: Arc<Mutex<Vec<PublicKey>>>,
    shared: Arc<Mutex<Vec<SharedSecret>>>,
}

pub async fn serve(cfg: Config) {
    let mut hasher = Sha3_256::new();
    hasher.update(cfg.host_pass.as_bytes());
    let key = EphemeralSecret::random(thread_rng());
    let state = Arc::new(Mutex::new(State {
        cfg: Arc::new(Mutex::new(cfg.clone())),
        host_pass: Arc::new(Mutex::new(hasher.finalize().to_vec())),
        count: Mutex::new(0),
        key: Arc::new(key),
        shared: Arc::new(Mutex::new(vec![])),
        pubkey: Arc::new(Mutex::new(vec![])),
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
        info!("Established connection from {:?}", stream.peer_addr());
        let packet = Packet::from_stream(Pin::new(&mut stream)).await?;
        self.handle_command(stream, packet).await
    }

    async fn handle_command(
        &mut self,
        mut stream: TcpStream,
        packet: Packet,
    ) -> async_std::io::Result<()> {
        match packet.heady.header.command {
            Command::Handshake => {
                if let Ok(handshake_req) = bincode::DefaultOptions::new()
                    .with_big_endian()
                    .with_fixint_encoding()
                    .deserialize::<HandshakeRequest>(&packet.heady.body)
                {
                    if handshake_req.pass.as_bytes() == *self.host_pass.lock().await {
                    self.shared
                        .lock()
                        .await
                        .push(self.key.diffie_hellman(&handshake_req.client_pubkey));
                    self.pubkey.lock().await.push(handshake_req.client_pubkey);
                    let handshake_res = HandshakeResponse {
                        result: HandshakeResult::Success,
                        node_id: Some((*self.count.lock().await) + 1),
                        server_pubkey: Some(self.key.public_key().clone()),
                    };
                    let req_packet = Packet::make_packet(
                        Command::Handshake,
                        bincode::DefaultOptions::new()
                            .with_big_endian()
                            .with_fixint_encoding()
                            .serialize(&handshake_res)
                            .unwrap(),
                    );
                    (*self.count.lock().await) += 1;
                    req_packet.send(Pin::new(&mut stream)).await
                    } else {
                        let handshake_res = HandshakeResponse {
                            result: HandshakeResult::PasswordNotMatched,
                            node_id: None,
                            server_pubkey: None,
                        };
                        let req_packet = Packet::make_packet(
                            Command::Handshake,
                            bincode::DefaultOptions::new()
                                .with_big_endian()
                                .with_fixint_encoding()
                                .serialize(&false)
                                .unwrap(),
                        );
                        req_packet.send(Pin::new(&mut stream)).await
                    }
                } else {
                    Err(Error::new(ErrorKind::InvalidData, "Invalid packet"))
                }
            }
            Command::VerifyToken => {
                if let Ok(body) = bincode::DefaultOptions::new()
                    .with_big_endian()
                    .with_fixint_encoding()
                    .deserialize::<BodyAfterHandshake<()>>(&packet.heady.body)
                {
                    let client_pubkey = body.client_pubkey;
                    let ret = (*self.count.lock().await <= body.node_id)
                        || (self.pubkey.lock().await[body.node_id as usize] == client_pubkey);
                    let req_packet = Packet::make_packet(
                        Command::ReqVerifyToken,
                        bincode::DefaultOptions::new()
                            .with_big_endian()
                            .with_fixint_encoding()
                            .serialize(&ret)
                            .unwrap(),
                    );
                    req_packet.send(Pin::new(&mut stream)).await
                } else {
                    Err(Error::new(ErrorKind::InvalidData, "Invalid packet"))
                }
            }
            _ => Ok(()),
        }
    }
}
