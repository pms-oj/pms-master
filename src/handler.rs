use async_compression::futures::write::BrotliEncoder;
use async_std::channel::{unbounded, Receiver, Sender};
use async_std::io::{Error, ErrorKind};
use async_std::net::{TcpListener, TcpStream};
use async_std::path::Path;
use async_std::sync::*;
use async_std::task::spawn;
use async_tar::Builder;

use bincode::Options;

use futures::stream::StreamExt;

use judge_protocol::constants::*;
use judge_protocol::handshake::*;
use judge_protocol::judge::*;
use judge_protocol::packet::*;
use judge_protocol::security::*;

use k256::ecdh::{EphemeralSecret, SharedSecret};
use k256::PublicKey;

use log::*;

use rand::thread_rng;

use std::collections::HashMap;

use uuid::Uuid;

use actix::dev::ToEnvelope;
use actix::prelude::*;

use crate::broker::*;
use crate::config::Config;
use crate::constants::*;
use crate::event::*;
use crate::judge::{JudgementType, RequestJudge, TestCaseManager};
use crate::scheduler::{by_deadline::ByDeadlineWeighted, *};
use crate::stream::*;
use crate::timer::*;

pub struct State<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone,
{
    pub cfg: Arc<Config>,
    host_pass: Arc<Vec<u8>>,
    count: RwLock<u32>,
    key: Arc<EphemeralSecret>,
    pubkey: Arc<RwLock<Vec<PublicKey>>>,
    shared: Arc<RwLock<Vec<SharedSecret>>>,
    judges: Arc<RwLock<HashMap<Uuid, RequestJudge<P>>>>,
    testman: Arc<RwLock<Vec<Option<Box<TestCaseManager<P>>>>>>,
    peers: Arc<RwLock<Vec<Sender<Vec<u8>>>>>,
    scheduler: Arc<ByDeadlineWeighted>,
    handler_addr: Addr<HandlerService<T, P>>,
    event_addr: Addr<T>,
}

#[derive(Clone, Debug, MessageResponse)]
pub enum HandlerResponse {
    None,
}

#[derive(Clone, Debug, Message)]
#[rtype(result = "()")]

pub enum HandlerMessage<P> {
    Judge(RequestJudge<P>),
    DownNode(u32),
    Shutdown,
    Unknown,
}

#[derive(Clone)]
pub struct HandlerService<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone,
{
    pub cfg: Config,
    pub event_addr: Addr<T>,
    pub state: Option<Arc<State<T, P>>>,
}

impl<T, P> Actor for HandlerService<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone,
{
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        info!("pms-master {}", env!("CARGO_PKG_VERSION"));
        let key = EphemeralSecret::random(thread_rng());
        let pubkey = key.public_key();
        let (scheduler_tx, mut scheduler_rx) = unbounded();
        let (reversed_tx, _) = unbounded();
        self.state = Some(Arc::new(State {
            cfg: Arc::new(self.cfg.clone()),
            host_pass: Arc::new(blake3::hash(self.cfg.host_pass.as_bytes()).as_bytes().to_vec()),
            count: RwLock::new(1),
            key: Arc::new(key),
            shared: Arc::new(RwLock::new(vec![SharedSecret::from(
                [0; KEY_SIZE]
                    .to_vec()
                    .into_iter()
                    .collect::<k256::FieldBytes>(),
            )])),
            judges: Arc::new(RwLock::new(HashMap::new())),
            pubkey: Arc::new(RwLock::new(vec![pubkey])),
            peers: Arc::new(RwLock::new(vec![reversed_tx])),
            testman: Arc::new(RwLock::new(vec![None])),
            scheduler: Arc::new(ByDeadlineWeighted::new(scheduler_tx.clone())),
            handler_addr: ctx.address(),
            event_addr: self.event_addr.clone(),
        }));
        let (broker_tx, mut broker_rx) = unbounded();
        let state = Arc::clone(self.state.as_ref().unwrap());
        let host = self.cfg.host.clone();
        actix::spawn(async move {
            {
                let state_arc = Arc::clone(&state);
                spawn(
                    async move { serve_scheduler(Arc::clone(&state_arc), &mut scheduler_rx).await },
                );
            }
            {
                let state_arc = Arc::clone(&state);
                let broker_cloned = broker_tx.clone();
                let scheduler_cloned = scheduler_tx.clone();
                spawn(async move {
                    serve_broker(scheduler_cloned, broker_cloned, &mut broker_rx, state_arc).await
                });
            }
            let state_arc = Arc::clone(&state);
            spawn(async move {
                let listener = TcpListener::bind(host.clone())
                    .await
                    .expect(&format!("Cannot bind {:?}", host));
                listener
                    .incoming()
                    .for_each_concurrent(None, |stream| async {
                        let stream = stream.unwrap();
                        let state_arc = Arc::clone(&state_arc);
                        let broker_cloned = broker_tx.clone();
                        let scheduler_cloned = scheduler_tx.clone();
                        spawn(async move {
                            state_arc
                                .handle_connection(scheduler_cloned, broker_cloned, stream)
                                .await
                                .ok();
                        });
                    })
                    .await
            });
        });
    }
}

impl<T, P> Handler<HandlerMessage<P>> for HandlerService<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone,
{
    type Result = ();

    fn handle(&mut self, msg: HandlerMessage<P>, ctx: &mut Context<Self>) -> Self::Result {
        match msg {
            HandlerMessage::Judge(judge) => {
                let state_arc = Arc::clone(&self.state.as_ref().unwrap());
                spawn(async move { state_arc.req_judge(judge).await });
            }
            HandlerMessage::DownNode(node_id) => {
                let state_arc = Arc::clone(&self.state.as_ref().unwrap());
                spawn(async move { state_arc.down_node(node_id).await });
            }
            HandlerMessage::Shutdown => {
                unimplemented!()
            }
            _ => {}
        }
    }
}

impl<T, P> State<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone,
{
    pub async fn handle_connection(
        &self,
        scheduler_tx: Sender<SchedulerMessage>,
        broker_tx: Sender<BrokerMessage>,
        stream: TcpStream,
    ) -> async_std::io::Result<()> {
        info!(
            "Established slave connection from {}",
            stream.peer_addr().unwrap()
        );
        let stream = Arc::new(stream);
        let packet = Packet::from_stream(Arc::clone(&stream)).await?;
        self.handle_command(scheduler_tx, broker_tx, Arc::clone(&stream), packet)
            .await
    }

    pub async fn req_judge(&self, judge: RequestJudge<P>) -> SchedulerResult<()> {
        let estimated_time = (judge.test_size as u64) * judge.time_limit;
        let _ = self
            .scheduler
            .push(judge.uuid, estimated_time, judge.judge_priority as u64)
            .await?;
        self.judges.write().await.insert(judge.uuid, judge);
        Ok(())
    }

    pub async fn handle_judge_send(
        &self,
        uuid: Uuid,
        node_id: u32,
    ) -> async_std::io::Result<()> {
        let sender = self.peers.read().await[node_id as usize].clone();
        let judges = self.judges.read().await;
        let judge = judges.get(&uuid).unwrap();
        let shared = &(*self.shared.read().await)[node_id as usize];
        let key = expand_key(&shared);
        match judge.judgement_type {
            JudgementType::Simple => {
                let body = JudgeRequestBody {
                    uuid: judge.uuid,
                    main_lang: judge.main_lang_uuid,
                    checker_lang: judge.checker_lang_uuid,
                    checker_code: EncMessage::generate(&key, &judge.checker),
                    main_code: EncMessage::generate(&key, &judge.main),
                    mem_limit: judge.mem_limit,
                    time_limit: judge.time_limit,
                };
                self.testman.write().await[node_id as usize] =
                    Some(Box::new(TestCaseManager::from(&judge.stdin, &judge.stdout)));
                let packet = Packet::make_packet(
                    Command::GetJudge,
                    bincode::DefaultOptions::new()
                        .with_big_endian()
                        .with_fixint_encoding()
                        .serialize(&body)
                        .unwrap(),
                );
                packet.send_with_sender(sender).await;
                Ok(())
            }
            JudgementType::Novel => {
                let mut graders = Builder::new(Vec::new());
                graders
                    .append_dir_all("graders", judge.graders.as_ref().expect("No grader found"))
                    .await
                    .ok();
                let graders_data = graders
                    .into_inner()
                    .await
                    .expect("Failed to make tar archive");
                let graders_encoder = BrotliEncoder::new(graders_data);
                let body = JudgeRequestBodyv2 {
                    uuid: judge.uuid,
                    main_lang: judge.main_lang_uuid,
                    checker_lang: judge.checker_lang_uuid,
                    checker_code: EncMessage::generate(&key, &judge.checker),
                    main_code: EncMessage::generate(&key, &judge.main),
                    manager_code: EncMessage::generate(
                        &key,
                        &judge.manager.clone().expect("No manager found"),
                    ),
                    manager_lang: judge
                        .manager_lang_uuid
                        .expect("No manager language uuid found"),
                    main_path: judge.main_path.clone().unwrap_or_else(|| {
                        let path = String::from(DEFAULT_MAIN_PATH);
                        warn!(
                            "No relation main file path found. use default value: {}",
                            &path
                        );
                        path
                    }),
                    object_path: judge.object_path.clone().unwrap_or_else(|| {
                        let path = String::from(DEFAULT_OBJECT_PATH);
                        warn!(
                            "No relation main file path found. use default value: {}",
                            &path
                        );
                        path
                    }),
                    graders: EncMessage::generate(&key, &graders_encoder.into_inner()),
                    mem_limit: judge.mem_limit,
                    time_limit: judge.time_limit,
                    procs: judge.procs.unwrap_or_else(|| DEFAULT_PROCESSES),
                };
                self.testman.write().await[node_id as usize] =
                    Some(Box::new(TestCaseManager::from(&judge.stdin, &judge.stdout)));
                let packet = Packet::make_packet(
                    Command::GetJudgev2,
                    bincode::DefaultOptions::new()
                        .with_big_endian()
                        .with_fixint_encoding()
                        .serialize(&body)
                        .unwrap(),
                );
                packet.send_with_sender(sender).await;
                Ok(())
            }
        }
    }

    async fn unlock_slave(&self, node_id: u32) {
        // TODO
        trace!("[node#{}] unlocked!", node_id);
        self.scheduler
            .touch(node_id as usize)
            .await
            .ok();
        drop(self.testman.read().await.iter().nth(node_id as usize));
    }

    async fn send_testcase(
        &self,
        stream: Sender<Vec<u8>>,
        judge_uuid: Uuid,
        node_id: u32,
    ) -> bool {
        let test_uuid = self.testman.write().await[node_id as usize]
            .as_mut()
            .unwrap()
            .next();
        if test_uuid.is_nil() {
            let packet = Packet::make_packet(Command::TestCaseEnd, vec![]);
            packet.send_with_sender(stream).await;
            self.unlock_slave(node_id).await;
            self.event_addr
                .send(EventMessage::EndJudge(judge_uuid))
                .await
                .ok();
            false
        } else {
            let testman = self.testman.read().await;
            let key = expand_key(&self.shared.read().await[node_id as usize]);
            let (stdin, stdout) = testman[node_id as usize]
                .as_ref()
                .unwrap()
                .get(test_uuid)
                .expect("Failed to read stdin, stdout");
            let body = TestCaseUpdateBody {
                uuid: judge_uuid,
                test_uuid,
                stdin: EncMessage::generate(&key, &stdin),
                stdout: EncMessage::generate(&key, &stdout),
            };
            let packet = Packet::make_packet(
                Command::TestCaseUpdate,
                bincode::DefaultOptions::new()
                    .with_big_endian()
                    .with_fixint_encoding()
                    .serialize::<TestCaseUpdateBody>(&body)
                    .unwrap(),
            );
            packet.send_with_sender(stream).await;
            true
        }
    }

    pub async fn down_node(&self, node_id: u32) {
        let testman = self.testman.write().await;
        trace!("ok ok down {}", node_id);
        self.scheduler
            .unregister(node_id as usize)
            .await
            .expect("failed to down node");
        trace!("ok.. down {}", node_id);
        drop(testman.iter().nth(node_id as usize));
    }

    pub async fn handle_command(
        &self,
        scheduler_tx: Sender<SchedulerMessage>,
        broker_tx: Sender<BrokerMessage>,
        stream: Arc<TcpStream>,
        packet: Packet,
    ) -> async_std::io::Result<()> {
        match packet.heady.header.command {
            Command::GetJudgeStateUpdate => {
                if let Ok(body) = bincode::DefaultOptions::new()
                    .with_big_endian()
                    .with_fixint_encoding()
                    .deserialize::<BodyAfterHandshake<JudgeResponseBody>>(&packet.heady.body)
                {
                    self.event_addr
                        .send(EventMessage::JudgeResult(
                            body.req.uuid,
                            body.req.result.clone(),
                        ))
                        .await
                        .ok();
                    match body.req.result {
                        JudgeState::DoCompile => {
                            trace!(
                                "[node#{}] (Judge: {}) started compile codes",
                                body.node_id,
                                body.req.uuid
                            );
                            Ok(())
                        }
                        JudgeState::CompleteCompile(stdout) => {
                            trace!(
                                "[node#{}] (Judge: {}) main code compile stdout: {}",
                                body.node_id,
                                body.req.uuid,
                                stdout
                            );
                            let tx = self.peers.read().await[body.node_id as usize].clone();
                            let _ = self
                                .send_testcase(tx, body.req.uuid, body.node_id)
                                .await;
                            Ok(())
                        }
                        JudgeState::CompileError(stderr) => {
                            trace!("[node#{}] (Judge: {}) master has received report CE of main code. stderr: {}", body.node_id, body.req.uuid, stderr);
                            self.unlock_slave(body.node_id).await;
                            Ok(())
                        }
                        JudgeState::Complete(test_uuid, score, time, mem) => {
                            debug!("[node#{}] (Judge: {}) (Test: {}) master has recived report PC of main code. score: {}, time: {}ms, mem: {}kB", body.node_id, body.req.uuid, test_uuid, score, time, mem);
                            let tx = self.peers.read().await[body.node_id as usize].clone();
                            let _ = self
                                .send_testcase(tx, body.req.uuid, body.node_id)
                                .await;
                            Ok(())
                        }
                        JudgeState::Accepted(test_uuid, time, mem) => {
                            debug!("[node#{}] (Judge: {}) (Test: {}) master has recived report AC of main code. time: {}ms, mem: {}kB", body.node_id, body.req.uuid, test_uuid, time, mem);
                            let tx = self.peers.read().await[body.node_id as usize].clone();
                            let _ = self
                                .send_testcase(tx, body.req.uuid, body.node_id)
                                .await;
                            Ok(())
                        }
                        JudgeState::WrongAnswer(test_uuid, time, mem) => {
                            trace!("[node#{}] (Judge: {}) (Test: {}) master has recived report WA of main code. time: {}ms, mem: {}kB", body.node_id, body.req.uuid, test_uuid, time, mem);
                            let tx = self.peers.read().await[body.node_id as usize].clone();
                            let _ = self
                                .send_testcase(tx, body.req.uuid, body.node_id)
                                .await;
                            Ok(())
                        }
                        JudgeState::RuntimeError(test_uuid, exit_code) => {
                            trace!("[node#{}] (Judge: {}) (Test: {}) master has recived report RTE(NZEC) of main code. exit code: {}", body.node_id, body.req.uuid, test_uuid, exit_code);
                            let tx = self.peers.read().await[body.node_id as usize].clone();
                            let _ = self
                                .send_testcase(tx, body.req.uuid, body.node_id)
                                .await;
                            Ok(())
                        }
                        JudgeState::DiedOnSignal(test_uuid, exit_sig) => {
                            trace!("[node#{}] (Judge: {}) (Test: {}) master has recived report RTE(DiedOnSignal) of main code. exit code: {}", body.node_id, body.req.uuid, test_uuid, exit_sig);
                            let tx = self.peers.read().await[body.node_id as usize].clone();
                            let _ = self
                                .send_testcase(tx, body.req.uuid, body.node_id)
                                .await;
                            Ok(())
                        }
                        JudgeState::TimeLimitExceed(test_uuid) => {
                            trace!("[node#{}] (Judge: {}) (Test: {}) master has recived report TLE of main code.", body.node_id, body.req.uuid, test_uuid);
                            let tx = self.peers.read().await[body.node_id as usize].clone();
                            let _ = self
                                .send_testcase(tx, body.req.uuid, body.node_id)
                                .await;
                            Ok(())
                        }
                        JudgeState::MemLimitExceed(test_uuid) => {
                            trace!("[node#{}] (Judge: {}) (Test: {}) master has recived report MLE of main code.", body.node_id, body.req.uuid, test_uuid);
                            let tx = self.peers.read().await[body.node_id as usize].clone();
                            let _ = self
                                .send_testcase(tx, body.req.uuid, body.node_id)
                                .await;
                            Ok(())
                        }
                        JudgeState::GeneralError(stderr) => {
                            trace!("[node#{}] (Judge: {}) master has received report wrong checker. stderr: {}", body.node_id, body.req.uuid, stderr);
                            self.unlock_slave(body.node_id).await;
                            Ok(())
                        }
                        _ => {
                            trace!(
                                "[node#{}] (Judge: {}) judge has failed: {:?}",
                                body.node_id,
                                body.req.uuid,
                                body.req.result,
                            );
                            self.unlock_slave(body.node_id).await;
                            Ok(())
                        }
                    }
                } else {
                    Err(Error::new(ErrorKind::InvalidData, "Invalid packet"))
                }
            }
            Command::Handshake => {
                if let Ok(handshake_req) = bincode::DefaultOptions::new()
                    .with_big_endian()
                    .with_fixint_encoding()
                    .deserialize::<HandshakeRequest>(&packet.heady.body)
                {
                    if handshake_req.pass == *self.host_pass {
                        trace!("Handshake");
                        self.shared
                            .write()
                            .await
                            .push(self.key.diffie_hellman(&handshake_req.client_pubkey));
                        self.pubkey.write().await.push(handshake_req.client_pubkey);
                        self.testman.write().await.push(None);
                        let node_id = *self.count.read().await;
                        let handshake_res = HandshakeResponse {
                            result: HandshakeResult::Success,
                            node_id: Some(node_id),
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
                        (*self.count.write().await) += 1;
                        let (packet_tx, mut packet_rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) =
                            unbounded();
                        let scheduler_cloned = scheduler_tx.clone();
                        let broker_cloned = broker_tx.clone();
                        let stream_cloned = Arc::clone(&stream);
                        spawn(async move {
                            serve_stream(
                                scheduler_cloned,
                                broker_cloned,
                                &mut packet_rx,
                                stream_cloned,
                                node_id,
                            )
                            .await
                        });
                        let serve_tx = self.handler_addr.clone();
                        spawn(async move { check_alive(node_id, serve_tx, stream).await });
                        req_packet.send_with_sender(packet_tx.clone()).await;
                        self.peers.write().await.push(packet_tx);
                        self.scheduler.register().await;
                        Ok(())
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
                                .serialize(&handshake_res)
                                .unwrap(),
                        );
                        req_packet.send(Arc::clone(&stream)).await.ok();
                        Ok(())
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
                    let ret = (*self.count.read().await <= body.node_id)
                        || (self.pubkey.read().await[body.node_id as usize] == client_pubkey);
                    let req_packet = Packet::make_packet(
                        Command::ReqVerifyToken,
                        bincode::DefaultOptions::new()
                            .with_big_endian()
                            .with_fixint_encoding()
                            .serialize(&ret)
                            .unwrap(),
                    );
                    req_packet.send(Arc::clone(&stream)).await
                } else {
                    Err(Error::new(ErrorKind::InvalidData, "Invalid packet"))
                }
            }
            _ => Ok(()),
        }
    }
}
