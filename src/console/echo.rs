///! Utility struct for loopback reading/writing
use std::io;
use std::cmp;

pub struct Port {
    buffer: Vec<u8>
}

pub fn new() -> Port {
    Port {
        buffer: vec!()
    }
}

impl io::Read for Port {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
       let read = cmp::min(buf.len(), self.buffer.len());

        buf[..read].clone_from_slice(&self.buffer[..read]);

        if read > 0 {
            trace!("Loopback Read {:?}", &buf[..read]);
        }

        self.buffer.drain(..read);

        Ok(read)
    }
}

impl io::Write for Port {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        
        if buf.len() > 0 {
            trace!("Loopback Wrote {:?}", &buf);
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}