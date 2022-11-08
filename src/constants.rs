use std::time::Duration;
use uuid::Uuid;

pub const CHECK_ALIVE_TIME: Duration = Duration::from_secs(5);
pub const NODE_ZERO: Uuid = Uuid::nil();
pub const FIRST_PRIORITY: u64 = 1;
pub const DEFAULT_MAIN_PATH: &'static str = "graders/main.pms";
pub const DEFAULT_OBJECT_PATH: &'static str = "graders/object.pms";
pub const DEFAULT_PROCESSES: usize = 2;
