///! Utility functions
use log;
use fern;
use time;

pub trait CollectSlice<T>: Iterator<Item=T> {
    fn collect_slice(&mut self, out_slice: &mut [T]) {
        self.collect_slice_offset(out_slice, 0);
    }

    fn collect_slice_offset(&mut self, out_slice: &mut [T], offset: usize) {
        let mut idx = 0;
        for item in self.skip(offset) {
            out_slice[idx] = item;
            idx += 1;
        }
    }
}

impl<I: Iterator<Item=T>, T> CollectSlice<T> for I {}

pub fn init_log(level: log::LogLevelFilter) {
    let logger_config = fern::DispatchConfig {
        format: Box::new(|msg: &str, level: &log::LogLevel, _location: &log::LogLocation| {
                //Log unique MS time and date
                format!("[{}][{}][{}] {}", time::precise_time_ns() / 1_000_000, time::now().strftime("%Y-%m-%d][%H:%M:%S").unwrap(), level, msg)
            }),
        output: vec![fern::OutputConfig::stdout(), fern::OutputConfig::file("output.log")],
        level: log::LogLevelFilter::Trace,
    };

    if let Err(e) = fern::init_global_logger(logger_config, level) {
        panic!("Failed to initialize global logger: {}", e);
    }
}