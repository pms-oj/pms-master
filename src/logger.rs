use log::{Level, Log, Metadata, Record};

pub struct StdoutLogger;

impl Log for StdoutLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!("PMS-master: {} - {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}
