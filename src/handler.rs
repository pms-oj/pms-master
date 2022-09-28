use super::config::Config;
use async_std::io::{Error, ErrorKind};
use async_std::net::{TcpListener, TcpStream};
use async_std::prelude::*;
use async_std::sync::*;
use async_std::task::spawn;
use futures::stream::StreamExt;
use judge_protocol::constants::*;
use judge_protocol::packet::*;
use judge_protocol::handshake::*;
use k256::ecdh::{EphemeralSecret, SharedSecret};
use k256::PublicKey;
use rand::thread_rng;
use std::pin::Pin;
use bincode::Options;

pub struct State {
    pub cfg: Arc<Config>,
    count: Mutex<u32>,
    key: Arc<EphemeralSecret>,
    shared: Arc<Mutex<Vec<SharedSecret>>>,
}

pub async fn serve(cfg: Config) {
    let key = EphemeralSecret::random(thread_rng());
    let state = Arc::new(Mutex::new(State {
        cfg: Arc::new(cfg.clone()),
        count: Mutex::new(0),
        key: Arc::new(key),
        shared: Arc::new(Mutex::new(vec![])),
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
        dbg!(stream.clone());
        let packet = Packet::from_stream(Pin::new(&mut stream)).await?;
        #[cfg(debug_assertions)]
        {
            let header = packet.heady.header;
            let body = packet.heady.body.clone();
            let checksum = packet.checksum;
            let mut s = vec![];
            s.append(&mut bincode::DefaultOptions::new().with_big_endian().with_fixint_encoding().serialize(&header).unwrap());
            s.append(&mut body.clone());
            s.append(&mut checksum.to_vec());
            use encoding::{Encoding, DecoderTrap};
            use encoding::all::ISO_8859_1;
            dbg!(ISO_8859_1.decode(&s, DecoderTrap::Strict).unwrap());
        }
        self.handle_command(stream, packet).await
    }

    async fn handle_command(
        &mut self,
        mut stream: TcpStream,
        packet: Packet,
    ) -> async_std::io::Result<()> {
        match packet.heady.header.command {
            Command::Handshake => {
                if let Ok(client_pubkey) = bincode::DefaultOptions::new().with_big_endian().with_fixint_encoding().deserialize::<PublicKey>(&packet.heady.body) {
                    self.shared.lock().await.push(self.key.diffie_hellman(&client_pubkey));
                    let handshake_res = HandshakeResult {
                        node_id: (*self.count.lock().await)+1,
                        server_pubkey: self.key.public_key().clone(),
                    };
                    let req_packet = Packet::make_packet(
                        Command::Handshake,
                        bincode::DefaultOptions::new().with_big_endian().with_fixint_encoding().serialize(&handshake_res).unwrap(),
                    );
                    (*self.count.lock().await) += 1;
                    req_packet.send(Pin::new(&mut stream)).await
                } else {
                    Err(Error::new(ErrorKind::InvalidData, "Invalid packet"))
                }
            }
            _ => Ok(()),
        }
    }
}
