use std::io;
use std::cmp;

pub struct Port {
    buffer: [u8; 2000],
    offset: usize
}

pub fn new() -> Port {
    Port {
        buffer: [0; 2000],
        offset: 0
    }
}

impl io::Read for Port {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
       let read = cmp::min(buf.len(), self.offset);
       self.offset -= read;

       buf[..read].clone_from_slice(&self.buffer[..read]);

       Ok(read)
    }
}

impl io::Write for Port {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let write = cmp::min(self.buffer.len() - self.offset, buf.len());
        self.buffer[self.offset..self.offset+write].clone_from_slice(&buf[..write]);

        self.offset += write;

        Ok(write)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}