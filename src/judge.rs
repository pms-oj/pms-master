use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct RequestJudge {
    pub uuid: Uuid,
    pub judge_priority: PrioirityWeight,
    pub test_size: usize,
    pub stdin: Vec<Vec<u8>>,
    pub stdout: Vec<Vec<u8>>,
    pub main: Vec<u8>,
    pub checker: Vec<u8>,
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

pub enum TestCaseState {
    Next(Uuid),
    End,
}

// not an iterator!
pub struct TestCaseManager {
    tests: HashMap<Uuid, (Vec<u8>, Vec<u8>)>,
    uuid_map: HashMap<Uuid, TestCaseState>,
    pub cur: Uuid,
}

impl TestCaseManager {
    pub fn from(stdin: &[Vec<u8>], stdout: &[Vec<u8>]) -> Self {
        let mut testman = TestCaseManager {
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
                .insert(uuids[i], TestCaseState::Next(uuids[i + 1]));
        }
        testman
            .uuid_map
            .insert(testman.cur, TestCaseState::Next(uuids[0]));
        testman.uuid_map.insert(uuids[n - 1], TestCaseState::End);
    }
        testman
    }

    pub fn next(&mut self) -> Uuid {
        if let Some(TestCaseState::Next(uuid_next)) = self.uuid_map.get(&self.cur) {
            self.cur = uuid_next.clone();
            uuid_next.clone()
        } else {
            Uuid::nil()
        }
    }

    pub fn get(&self, test_uuid: Uuid) -> &(Vec<u8>, Vec<u8>) {
        self.tests.get(&test_uuid).unwrap()
    }
}
