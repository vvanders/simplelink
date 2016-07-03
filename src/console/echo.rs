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
       println!("R {:?}", &buf[..read]);
       self.buffer.drain(..read);

       Ok(read)
    }
}

impl io::Write for Port {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        println!("W {:?}", &buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}