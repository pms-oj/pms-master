use actix::prelude::*;

use chacha20poly1305::consts::U32;

use judge_protocol::judge::*;
use judge_protocol::packet::*;
use judge_protocol::security::*;

use uuid::Uuid;

use generic_array::GenericArray;

use crate::event::*;
use crate::handler::connection::*;
use crate::handler::*;
use crate::judge::*;

pub struct TestcaseService<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    node_id: Uuid,
    judge_uuid: Uuid,
    testman: TestcaseManager<P>,
    judge_addr: Addr<JudgeService<T, P>>,
    handler_addr: Addr<HandlerService<T, P>>,
    connection_addr: Addr<ConnectionService<T, P>>,
    key: GenericArray<u8, U32>,
}

#[derive(Clone, Debug, Message)]
#[rtype(result = "()")]
pub enum TestcaseMessage {
    Pop,
}

impl<T, P> Actor for TestcaseService<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        debug!(
            "Starting testcase management service for UUID: {}",
            self.judge_uuid
        );
    }
}

impl<T, P> TestcaseService<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    pub fn start(
        node_id: Uuid,
        judge_uuid: Uuid,
        testman: TestcaseManager<P>,
        judge_addr: Addr<JudgeService<T, P>>,
        handler_addr: Addr<HandlerService<T, P>>,
        connection_addr: Addr<ConnectionService<T, P>>,
        key: GenericArray<u8, U32>,
    ) -> Addr<Self> {
        Self {
            node_id,
            judge_uuid,
            testman,
            judge_addr,
            handler_addr,
            connection_addr,
            key,
        }
        .start()
    }
}

impl<T, P> Handler<TestcaseMessage> for TestcaseService<T, P>
where
    T: Actor + Handler<EventMessage>,
    <T as actix::Actor>::Context: ToEnvelope<T, EventMessage>,
    P: AsRef<Path> + 'static + Send + Sync + Clone + Unpin,
{
    type Result = ();

    fn handle(&mut self, msg: TestcaseMessage, ctx: &mut Self::Context) -> Self::Result {
        match msg {
            TestcaseMessage::Pop => {
                let test_uuid = self.testman.next();
                if test_uuid.is_nil() {
                    let packet = Packet::make_packet(Command::TestCaseEnd, vec![]);
                    self.connection_addr
                        .do_send(ConnectionMessage::Send(self.node_id, packet.serialize()));
                    self.handler_addr
                        .do_send(HandlerMessage::UnlockSlave(self.judge_uuid));
                    self.judge_addr.do_send(JudgeMessage::EndJudge);
                    ctx.terminate();
                } else {
                    let (stdin, stdout) = self
                        .testman
                        .get(test_uuid)
                        .expect("Failed to read stdin, stdout");
                    let body = TestCaseUpdateBody {
                        uuid: self.judge_uuid,
                        test_uuid,
                        stdin: EncMessage::generate(&self.key, &stdin),
                        stdout: EncMessage::generate(&self.key, &stdout),
                    };
                    let packet = Packet::make_packet(
                        Command::TestCaseUpdate,
                        bincode::serialize::<TestCaseUpdateBody>(&body).unwrap(),
                    );
                    self.connection_addr
                        .do_send(ConnectionMessage::Send(self.node_id, packet.serialize()));
                }
            }
        }
    }
}
