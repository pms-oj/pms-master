use async_std::fs::read;
use async_std::path::Path;
use async_std::task::block_on;

use std::collections::HashMap;
use std::fmt::Debug;
use std::io;
use uuid::Uuid;

#[derive(Clone, Debug, Copy)]
pub enum JudgementType {
    Simple,
    Novel,
}

#[derive(Clone, Debug)]
pub struct RequestJudge<P> {
    pub uuid: Uuid,
    pub judge_priority: PrioirityWeight,
    pub test_size: usize,
    pub stdin: Vec<P>,
    pub stdout: Vec<P>,
    pub main: Vec<u8>,
    pub checker: Vec<u8>,
    pub judgement_type: JudgementType,
    pub main_path: Option<String>,
    pub object_path: Option<String>,
    pub manager: Option<Vec<u8>>,
    pub graders: Option<P>,
    pub manager_lang_uuid: Option<Uuid>,
    pub procs: Option<usize>,
    pub main_lang_uuid: Uuid,
    pub checker_lang_uuid: Uuid,
    pub time_limit: u64,
    pub mem_limit: u64,
}

#[derive(Clone, Copy, Debug)]
#[repr(u64)]
pub enum PrioirityWeight {
    First = 1,
    Second = 2,
}

pub enum TestcaseState {
    Next(Uuid),
    End,
}

// not an iterator!
pub struct TestcaseManager<P> {
    tests: HashMap<Uuid, (P, P)>,
    uuid_map: HashMap<Uuid, TestcaseState>,
    pub cur: Uuid,
}

impl<P> TestcaseManager<P>
where
    P: AsRef<Path> + Clone,
{
    pub fn from(stdin: &[P], stdout: &[P]) -> Self {
        let mut testman = TestcaseManager {
            tests: HashMap::new(),
            uuid_map: HashMap::new(),
            cur: Uuid::nil(),
        };
        assert_eq!(stdin.len(), stdout.len());
        let n = stdin.len();
        let mut uuids = vec![];
        for i in 0..n {
            uuids.push(Uuid::new_v4());
            testman
                .tests
                .insert(uuids[i], (stdin[i].clone(), stdout[i].clone()));
        }
        if n > 0 {
            for i in 0..(n - 1) {
                testman
                    .uuid_map
                    .insert(uuids[i], TestcaseState::Next(uuids[i + 1]));
            }
            testman
                .uuid_map
                .insert(testman.cur, TestcaseState::Next(uuids[0]));
            testman.uuid_map.insert(uuids[n - 1], TestcaseState::End);
        }
        testman
    }

    pub fn next(&mut self) -> Uuid {
        if let Some(TestcaseState::Next(uuid_next)) = self.uuid_map.get(&self.cur) {
            self.cur = uuid_next.clone();
            uuid_next.clone()
        } else {
            Uuid::nil()
        }
    }

    pub fn get(&self, test_uuid: Uuid) -> io::Result<(Vec<u8>, Vec<u8>)> {
        block_on(async {
            let (stdin, stdout) = self.tests.get(&test_uuid).unwrap();
            let (stdin_f, stdout_f) = (read(&stdin).await?, read(&stdout).await?);
            Ok((stdin_f, stdout_f))
        })
    }
}
