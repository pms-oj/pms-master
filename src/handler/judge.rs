use actix::dev::ToEnvelope;
use actix::prelude::*;

use async_std::path::Path;

use uuid::Uuid;

use chacha20poly1305::consts::U32;

use generic_array::GenericArray;

use judge_protocol::judge::*;
use judge_protocol::security::*;

use async_compression::futures::write::BrotliEncoder;

use async_tar::Builder;

use crate::constants::*;
use crate::event::*;
use crate::handler::connection::*;
use crate::handler::testcase::*;
use crate::handler::*;
use crate::judge::*;
use crate::scheduler::*;

pub type JudgeContext<T, P> = Context<JudgeService<T, P>>;

pub fn estimated_time<P>(req: &RequestJudge<P>) -> u64 {
    (req.test_size as u64) * req.time_limit
}

#[derive(Clone, Message)]
#[rtype(result = "()")]
pub enum JudgeMessage {
    Dispatch(Uuid, GenericArray<u8, U32>),
    JudgeStateUpdate(Uuid, JudgeState),
    EndJudge,
}

pub struct JudgeService<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    judge_uuid: Uuid,
    event_addr: Addr<T>,
    scheduler_addr: Addr<ByDeadlineWeighted<T, P>>,
    handler_addr: Addr<HandlerService<T, P>>,
    connection_addr: Addr<ConnectionService<T, P>>, // Hmm this is not mistake. This stands for feature to be added later.
    judgement_info: RequestJudge<P>,
    testcase_addr: Option<Addr<TestcaseService<T, P>>>,
}

impl<T, P> JudgeService<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    pub fn start(
        judge_uuid: Uuid,
        event_addr: Addr<T>,
        scheduler_addr: Addr<ByDeadlineWeighted<T, P>>,
        handler_addr: Addr<HandlerService<T, P>>,
        connection_addr: Addr<ConnectionService<T, P>>,
        judgement_info: RequestJudge<P>,
    ) -> Addr<Self> {
        Self {
            judge_uuid,
            event_addr,
            scheduler_addr,
            handler_addr,
            connection_addr,
            judgement_info,
            testcase_addr: None,
        }
        .start()
    }

    pub fn halt(&mut self, ctx: &mut JudgeContext<T, P>) {
        ctx.terminate();
    }
}

impl<T, P> Actor for JudgeService<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    type Context = JudgeContext<T, P>;

    fn started(&mut self, ctx: &mut Self::Context) {
        debug!("Starting judgement service for UUID: {}", self.judge_uuid);
        self.scheduler_addr.do_send(SchedulerIssue::NewTask(
            self.judge_uuid,
            estimated_time(&self.judgement_info),
            self.judgement_info.judge_priority as u64,
        ));
    }

    fn stopping(&mut self, ctx: &mut Self::Context) -> Running {
        debug!("Stopping judgement service for UUID: {}", self.judge_uuid);
        Running::Stop
    }
}

impl<T, P> Handler<JudgeMessage> for JudgeService<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    type Result = ();

    fn handle(&mut self, msg: JudgeMessage, ctx: &mut Self::Context) -> Self::Result {
        match msg {
            JudgeMessage::Dispatch(node_id, key) => match self.judgement_info.judgement_type {
                JudgementType::Simple => {
                    let body = JudgeRequestBody {
                        uuid: self.judgement_info.uuid,
                        main_lang: self.judgement_info.main_lang_uuid,
                        checker_lang: self.judgement_info.checker_lang_uuid,
                        checker_code: EncMessage::generate(&key, &self.judgement_info.checker),
                        main_code: EncMessage::generate(&key, &self.judgement_info.main),
                        mem_limit: self.judgement_info.mem_limit,
                        time_limit: self.judgement_info.time_limit,
                    };
                    let testman = TestcaseManager::from(
                        &self.judgement_info.stdin,
                        &self.judgement_info.stdout,
                    );
                    let packet =
                        Packet::make_packet(Command::GetJudge, bincode::serialize(&body).unwrap());
                    let test_service = TestcaseService::start(
                        node_id,
                        self.judge_uuid,
                        testman,
                        ctx.address(),
                        self.handler_addr.clone(),
                        self.connection_addr.clone(),
                        key,
                    );
                    self.testcase_addr = Some(test_service);
                    self.connection_addr
                        .do_send(ConnectionMessage::Send(node_id, packet.serialize()));
                }
                JudgementType::Novel => {
                    let sys = System::new();
                    sys.block_on(async {
                        let mut graders = Builder::new(Vec::new());
                        graders
                            .append_dir_all(
                                "graders",
                                self.judgement_info
                                    .graders
                                    .as_ref()
                                    .expect("No grader found"),
                            )
                            .await
                            .ok();
                        let graders_data = graders
                            .into_inner()
                            .await
                            .expect("Failed to make tar archive");
                        let graders_encoder = BrotliEncoder::new(graders_data);
                        let body = JudgeRequestBodyv2 {
                            uuid: self.judgement_info.uuid,
                            main_lang: self.judgement_info.main_lang_uuid,
                            checker_lang: self.judgement_info.checker_lang_uuid,
                            checker_code: EncMessage::generate(&key, &self.judgement_info.checker),
                            main_code: EncMessage::generate(&key, &self.judgement_info.main),
                            manager_code: EncMessage::generate(
                                &key,
                                &self
                                    .judgement_info
                                    .manager
                                    .clone()
                                    .expect("No manager found"),
                            ),
                            manager_lang: self
                                .judgement_info
                                .manager_lang_uuid
                                .expect("No manager language uuid found"),
                            main_path: self.judgement_info.main_path.clone().unwrap_or_else(|| {
                                let path = String::from(DEFAULT_MAIN_PATH);
                                warn!(
                                    "No relation main file path found. use default value: {}",
                                    &path
                                );
                                path
                            }),
                            object_path: self.judgement_info.object_path.clone().unwrap_or_else(
                                || {
                                    let path = String::from(DEFAULT_OBJECT_PATH);
                                    warn!(
                                        "No relation main file path found. use default value: {}",
                                        &path
                                    );
                                    path
                                },
                            ),
                            graders: EncMessage::generate(&key, &graders_encoder.into_inner()),
                            mem_limit: self.judgement_info.mem_limit,
                            time_limit: self.judgement_info.time_limit,
                            procs: self
                                .judgement_info
                                .procs
                                .unwrap_or_else(|| DEFAULT_PROCESSES),
                        };
                        let packet = Packet::make_packet(
                            Command::GetJudgev2,
                            bincode::serialize(&body).unwrap(),
                        );
                        let testman = TestcaseManager::from(
                            &self.judgement_info.stdin,
                            &self.judgement_info.stdout,
                        );
                        let test_service = TestcaseService::start(
                            node_id,
                            self.judge_uuid,
                            testman,
                            ctx.address(),
                            self.handler_addr.clone(),
                            self.connection_addr.clone(),
                            key,
                        );
                        self.testcase_addr = Some(test_service);
                        self.connection_addr
                            .do_send(ConnectionMessage::Send(node_id, packet.serialize()));
                    });
                    sys.run();
                }
                _ => {}
            },
            JudgeMessage::JudgeStateUpdate(node_id, state) => match state {
                JudgeState::DoCompile => {
                    debug!("Started to compile main code for judge {}", self.judge_uuid);
                }
                JudgeState::CompleteCompile(stdout) => {
                    debug!(
                        "Success to compile main code for judge {}, stdout: {stdout}",
                        self.judge_uuid
                    );
                    self.testcase_addr
                        .as_ref()
                        .unwrap()
                        .do_send(TestcaseMessage::Pop);
                }
                JudgeState::CompileError(stderr) => {
                    debug!(
                        "Failed to compile main code for judge {}, stdout: {stderr}",
                        self.judge_uuid
                    );
                    self.halt(ctx);
                }
                JudgeState::Complete(test_uuid, score, time, mem) => {
                    debug!("(PC) node_id: {node_id}, judge_uuid: {}, test_uuid: {test_uuid} (score: {score}, time: {time}ms, mem: {mem}kB)", self.judge_uuid);
                    self.testcase_addr
                        .as_ref()
                        .unwrap()
                        .do_send(TestcaseMessage::Pop);
                }
                JudgeState::Accepted(test_uuid, time, mem) => {
                    debug!("(AC) node_id: {node_id}, judge_uuid: {}, test_uuid: {test_uuid} (time: {time}ms, mem: {mem}kB)", self.judge_uuid);
                    self.testcase_addr
                        .as_ref()
                        .unwrap()
                        .do_send(TestcaseMessage::Pop);
                }
                JudgeState::WrongAnswer(test_uuid, time, mem) => {
                    debug!("(WA) node_id: {node_id}, judge_uuid: {}, test_uuid: {test_uuid} (time: {time}ms, mem: {mem}kB)", self.judge_uuid);
                    self.testcase_addr
                        .as_ref()
                        .unwrap()
                        .do_send(TestcaseMessage::Pop);
                }
                JudgeState::RuntimeError(test_uuid, exit_code) => {
                    debug!("(RTE - NZEC) node_id: {node_id}, judge_uuid: {}, test_uuid: {test_uuid} (exit_code: {exit_code})", self.judge_uuid);
                    self.testcase_addr
                        .as_ref()
                        .unwrap()
                        .do_send(TestcaseMessage::Pop);
                }
                JudgeState::DiedOnSignal(test_uuid, exit_sig) => {
                    debug!("(RTE - DiedOnSignal) node_id: {node_id}, judge_uuid: {}, test_uuid: {test_uuid} (exit_sig: {exit_sig})", self.judge_uuid);
                    self.testcase_addr
                        .as_ref()
                        .unwrap()
                        .do_send(TestcaseMessage::Pop);
                }
                JudgeState::TimeLimitExceed(test_uuid) => {
                    debug!(
                        "(TLE) node_id: {node_id}, judge_uuid: {}, test_uuid: {test_uuid}",
                        self.judge_uuid
                    );
                    self.testcase_addr
                        .as_ref()
                        .unwrap()
                        .do_send(TestcaseMessage::Pop);
                }
                JudgeState::MemLimitExceed(test_uuid) => {
                    debug!(
                        "(MLE) node_id: {node_id}, judge_uuid: {}, test_uuid: {test_uuid}",
                        self.judge_uuid
                    );
                    self.testcase_addr
                        .as_ref()
                        .unwrap()
                        .do_send(TestcaseMessage::Pop);
                }
                JudgeState::GeneralError(stderr) => {
                    debug!("(GE) node_id: {node_id}, judge_uuid: {}", self.judge_uuid);
                    self.halt(ctx);
                }
                _ => {
                    error!("Unknown judgement report");
                    self.halt(ctx);
                }
            },
            JudgeMessage::EndJudge => {
                self.halt(ctx);
            }
            _ => {}
        }
    }
}
