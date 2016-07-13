///! Transmitting queue for outgoing frames
use std::io;
use nbp::frame;
use nbp::prn_id;
use nbp::routing;

/// Maximum number of packets in flight
pub const MAX_PACKET: usize = 256;
/// Data buffer size
pub const BLOCK_SIZE: usize = 50 * 1024;
/// Data buffer size in flight before congestion control takes effect
pub const CONGEST_CONTROL: usize = 35 * 1024;
/// Number of times a packet will attempt to retry
pub const RETRY_COUNT: usize = 4;
/// Number of milliseconds until we will resend an un-ack'd packet. Grows proportional to the number of retries.
pub const RETRY_DELAY_MS: usize = 100;

/// Queue of packets waiting to be recieved
pub struct Queue {
    /// Packets waiting to go our on the wire
    pending: Vec<PendingPacket>,
    /// Payloads for pending packets
    data: Vec<u8>
}

pub enum QueueError {
    /// Congestion control is underway and this frame was immediately discarded
    Discarded,
    /// There was not enough space to store payload, packet was truncated
    Truncated,
    /// IO Error occurred.
    IO(io::Error)
}

/// Pending packet to be recieved
#[derive(Copy, Clone)]
pub struct PendingPacket {
    /// Packet we're trying to send
    packet: frame::DataHeader,
    /// Last time in ms from when we sent it
    next_send: usize,
    /// Number of retry attempts
    retry_count: u8,
    /// Byte offset for our payload packet
    data_offset: usize
}

/// Constructs a new queue
pub fn new() -> Queue {
    Queue {
        pending: vec!(),
        data: vec!()
    }
}

impl Queue {
    /// Enqueue a new frame, called just after we send out a frame over the wire
    pub fn enqueue<T>(&mut self, header: frame::DataHeader, payload: &mut T) -> Result<(),QueueError> where T: io::Read {
        trace!("Enqueuing frame {}, waiting for ACK", header.prn);

        if self.pending.len( )== MAX_PACKET {
            error!("Tried to queue packet but all available slots were full");
            return Err(QueueError::Discarded)
        }
        
        //Store where we started reading data so we can move our copy back if it fails
        let data_start = self.data.len();

        //Read from our input
        const SCRATCH_SIZE: usize = 256;
        let mut scratch: [u8; SCRATCH_SIZE] = unsafe { ::std::mem::uninitialized() };
        let mut err = Ok(());

        loop {
            let read = payload.read(&mut scratch);

            match read {
                Ok(n) => {
                    if n + self.data.len() < BLOCK_SIZE {
                        continue;
                    } else {
                        error!("Tried to enqueue {} bytes but exceeded BLOCK_SIZE, {} bytes queued", n, self.data.len() - data_start);
                        err = Err(QueueError::Truncated);
                        break;
                    }
                },
                Err(e) => {
                    error!("Tried to read bytes but IO error occurred: {:?}", e);
                    err = Err(QueueError::IO(e));
                    break;
                }
            }
        }

        if err.is_err() {
            self.data.truncate(data_start);
            return err
        }

        self.pending.push(PendingPacket {
            packet: header,
            next_send: RETRY_DELAY_MS,
            retry_count: 0,
            data_offset: data_start
        });

        Ok(())
    }

    //Check any packets that have expired, resend is called on packets we want to retry, discard on packets that have exceeded the retry count
    pub fn tick<E,R,D>(&mut self, elapsed_ms: u32, retry: R, discard: D) -> Result<(),E>
        where
            R: Fn(&frame::DataHeader, &[u8]) -> Result<(),E>,
            D: Fn(&frame::DataHeader)
    {
        Ok(())
    }
}

#[cfg(test)]
fn create_sample_packet(size: usize) -> (frame::DataHeader, Vec<u8>) {
    let mut prn = prn_id::new(['K', 'I', '7', 'E', 'S', 'T', '0']).unwrap();
    let mut data = (0..256).collect::<Vec<u8>>();
    let callsign = prn.callsign;

    let header = frame::new_data(&mut prn, &[callsign, routing::ADDRESS_SEPARATOR, callsign], data.len()).unwrap();

    (header, data)
}

#[test]
fn test_equeue() {
    let (header, data) = create_sample_packet(256);

    let mut queue = new();

    use std::io;
    match queue.enqueue(header, &mut io::Cursor::new(&data)) {
        Ok(()) => (),
        Err(e) => assert!(false)
    };
}