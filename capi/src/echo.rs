use std::io;
use std::cmp;

pub struct EchoInterface {
    data: Vec<u8>
}

pub fn new() -> EchoInterface {
    EchoInterface {
        data: vec!()
    }
}

impl io::Write for EchoInterface {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.data.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl io::Read for EchoInterface {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.data.len() > 0 {
            let read = cmp::min(buf.len(), self.data.len());
            buf[0..read].copy_from_slice(&self.data[0..read]);

            self.data.drain(0..read);

            Ok(read)
        } else {
            Ok(0)
        }
    }
}