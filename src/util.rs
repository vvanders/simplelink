///! Utility functions
use log;
use fern;
use time;
use std::io;

pub fn init_log(trace: log::LogLevelFilter) {
    //Print is gated by trace level
    let print_logger = fern::DispatchConfig {
        format: Box::new(|msg: &str, level: &log::LogLevel, _location: &log::LogLocation| {
            format!("[{}] {}", level, msg)
        }),
        output: vec![fern::OutputConfig::stdout()],
        level: trace,
    };

    //Always log trace to the file with a bit more info
    let file_logger = fern::DispatchConfig {
        format: Box::new(|msg: &str, level: &log::LogLevel, _location: &log::LogLocation| {
            //Log unique MS time and date
            format!("[{}][{}][{}] {}", time::precise_time_ns() / 1_000_000, time::now().strftime("%Y-%m-%d][%H:%M:%S").unwrap(), level, msg)
        }),
        output: vec![fern::OutputConfig::file("output.log"), fern::OutputConfig::child(print_logger)],
        level: log::LogLevelFilter::Trace,
    };


    if let Err(e) = fern::init_global_logger(file_logger, log::LogLevelFilter::Trace) {
        panic!("Failed to initialize global logger: {}", e);
    }
}

pub struct WriteDispatch<'a> {
    pub write: &'a mut io::Write
}

impl<'a> io::Write for WriteDispatch<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.write.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.write.flush()
    }
}

pub fn new_write_dispatch<'a>(write: &'a mut io::Write) -> WriteDispatch<'a> {
    WriteDispatch {
        write: write
    }
}

pub struct ReadWriteDispatch<'a> {
    read: &'a mut io::Read,
    write: &'a mut io::Write
}

impl <'a> io::Write for ReadWriteDispatch<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.write.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.write.flush()
    }
}

impl <'a> io::Read for ReadWriteDispatch<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.read.read(buf)
    }
}

pub fn new_read_write_dispatch<'a>(read: &'a mut io::Read, write: &'a mut io::Write) -> ReadWriteDispatch<'a> {
    ReadWriteDispatch {
        read: read,
        write: write
    }
}