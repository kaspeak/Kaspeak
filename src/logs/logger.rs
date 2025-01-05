use crate::constants::DEFAULT_LOGS_PATH;
use crate::logs::multi;
use chrono::Local;
use env_logger::Builder;
use log::{LevelFilter, Record};
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

pub fn init() {
    let file_path = Path::new(DEFAULT_LOGS_PATH);
    if !file_path.exists() {
        if let Some(dir) = file_path.parent() {
            fs::create_dir_all(dir).unwrap();
        }
    }
    let log_file = OpenOptions::new().create(true).append(true).open(file_path).expect("Failed to open (or create) kaspeak.log");
    let multi_writer = multi::MultiWriter::new(std::io::stdout(), log_file);
    let mut builder = Builder::new();
    builder
        .target(env_logger::Target::Pipe(Box::new(multi_writer)))
        .format(|buf, record: &Record| {
            let now = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            let file = record.file().unwrap_or("unknown_file");
            let level = record.level();

            writeln!(buf, "{} [{}][{:5}] {}", now, file, level, record.args())
        })
        .filter_level(LevelFilter::Off);
    builder.filter_module("kaspeak", LevelFilter::Info);
    builder.init();
}
