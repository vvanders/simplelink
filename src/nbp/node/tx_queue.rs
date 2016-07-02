///! Transmitting queue for outgoing frames
use nbp::frame;

/// Maximum number of packets in flight
pub const MAX_PACKET: usize = 2048;
/// Data buffer size
pub const BLOCK_SIZE: usize = 50 * 1024;
/// Data buffer size in flight before congestion control takes effect
pub const CONGEST_CONTROL: usize = 35 * 1024;
/// Number of times a packet will attempt to retry
pub const RETRY_COUNT: usize = 4;

/// Queue of packets waiting to be recieved
pub struct Queue {
    /// Packets waiting to go our on the wire
    pending: [Option<PendingPacket>; MAX_PACKET],
    /// Payloads for pending packets
    data: [u8; BLOCK_SIZE],
    /// Total data size of our packets
    packet_total_size: usize
}

pub enum QueueError {
    /// Congestion control is underway and this frame was immediately discarded
    Discarded
}

/// Pending packet to be recieved
#[derive(Copy, Clone)]
pub struct PendingPacket {
    /// Packet we're trying to send
    packet: frame::DataHeader,
    /// Last time in ms from when we sent it
    next_send: u32,
    /// Number of retry attempts
    retry_count: u8,
    /// Byte offset for our payload packet
    data_offset: usize
}

/// Constructs a new queue
pub fn new() -> Queue {
    Queue {
        pending: [None; MAX_PACKET],
        data: [0; BLOCK_SIZE],
        packet_total_size: 0
    }
}

impl Queue {
    /// Enqueue a new frame, called just after we send out a frame over the wire
    pub fn enqueue<T>(&mut self, header: frame::DataHeader, payload: T) -> Result<(),QueueError> where T: Iterator<Item=u8> {
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

#[test]
fn test_tick() {
}