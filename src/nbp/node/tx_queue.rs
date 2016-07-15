///! Transmitting queue for outgoing frames
use std::io;
use std::fmt;
use rand;
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
    // Header payload size does not match actual payload
    HeaderMismatch
}

/// Pending packet to be recieved
#[derive(Copy, Clone)]
pub struct PendingPacket {
    /// Packet we're trying to send
    packet: frame::DataHeader,
    /// Last time in ms from when we sent it
    next_send: usize,
    /// Number of retry attempts
    retry_count: usize,
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
    pub fn enqueue(&mut self, header: frame::DataHeader, payload: &[u8]) -> Result<(),QueueError> {
        trace!("Enqueuing frame {} with {} bytes, waiting for ACK", header.prn, payload.len());

        if self.data.len() + payload.len() > BLOCK_SIZE {
            error!("Tried to queue packet but congestion control is under way and was discarded");
            return Err(QueueError::Discarded);
        }

        if header.payload_size != payload.len() {
            error!("Mismatched payload sizes for packet was {} expected {}", payload.len(), header.payload_size);
            return Err(QueueError::HeaderMismatch);
        }
        
        //Store where we started reading data so we can move our copy back if it fails
        let data_start = self.data.len();

        self.data.extend_from_slice(payload);

        self.pending.push(PendingPacket {
            packet: header,
            next_send: RETRY_DELAY_MS,
            retry_count: 0,
            data_offset: data_start
        });

        trace!("Queued packet, buffer at {} of {} bytes", self.data.len(), BLOCK_SIZE);

        Ok(())
    }

    // Called when we recieve an ack packet
    pub fn ack_recv(&mut self, prn: u32) -> bool {
        match self.pending.iter().position(|pending| pending.packet.prn == prn) {
            Some(idx) => {
                self.discard(idx);
                trace!("ACK for {}, buffer at {} bytes", prn, self.data.len());

                true
            },
            None => {
                trace!("Tried to ack packet {} but it wasn't found in our table", prn);
                false
            }
        } 
    }

    // Check any packets that have expired, resend is called on packets we want to retry, discard on packets that have exceeded the retry count
    pub fn tick<R,D,E>(&mut self, elapsed_ms: usize, mut retry: R, mut discard: D) -> Result<(),E>
        where
            R: FnMut(&frame::DataHeader, &[u8]) -> Result<(),E>,
            D: FnMut(&frame::DataHeader),
            E: fmt::Debug
    {
        trace!("Ticking send queue for {} ms", elapsed_ms);
        let mut idx = 0;
        while idx < self.pending.len() {
            if self.pending[idx].next_send <= elapsed_ms {
                if self.pending[idx].retry_count >= RETRY_COUNT {
                    trace!("Packet {} has exceeded retry count, discarding", self.pending[idx].packet.prn);

                    discard(&self.pending[idx].packet);

                    //Discard our packet
                    self.discard(idx);

                    //Since we removed this index we can keep idx the same and continue with the next item
                } else {
                    trace!("Retrying {} packet with retry count {}", self.pending[idx].packet.prn, self.pending[idx].retry_count);

                    //Note that we increment our retry count here in case something about this packet prevents it
                    //from being sent so we won't hang the whole link
                    self.pending[idx].retry_count += 1;

                    match retry(&self.pending[idx].packet, self.get_packet_data(&self.pending[idx])) {
                        Ok(()) => (),
                        Err(e) => {
                            trace!("Error retrying packet {:?}, incrementing retry counter and aborting", &e);
                            return Err(e)
                        }
                    }

                    //Determine when we want to retry again. Note that we randomize so two transmitters won't collide
                    use rand::distributions::IndependentSample;
                    let rnd = rand::distributions::Range::new(0.0, 1.0).ind_sample(&mut rand::thread_rng());
                    self.pending[idx].next_send = ((1.0 + self.pending[idx].retry_count as f32 * rand::random::<f32>()) * RETRY_DELAY_MS as f32) as usize;

                    idx += 1;
                }
            } else {
                self.pending[idx].next_send -= elapsed_ms;
                trace!("Ticking {} {}ms remaining", self.pending[idx].packet.prn, self.pending[idx].next_send);

                idx += 1;
            }
        }

        Ok(())
    }

    fn discard(&mut self, idx: usize) {
        //Erase the data associated
        let data_start = self.pending[idx].data_offset;
        let data_end = data_start + self.pending[idx].packet.payload_size;
        self.data.drain(data_start..data_end);
        
        //Remove packet
        self.pending.remove(idx);
    }

    fn get_packet_data<'a>(&'a self, pending: &'a PendingPacket) -> &'a [u8] {
        &self.data[pending.data_offset..pending.data_offset+pending.packet.payload_size]
    }
}

#[cfg(test)]
fn create_sample_packet(prn: &mut prn_id::PRN, size: u32) -> (frame::DataHeader, Vec<u8>) {
    let mut data = (0..size).map(|value| value as u8).collect::<Vec<u8>>();
    let callsign = prn.callsign;

    let header = frame::new_data(prn, &[callsign, routing::ADDRESS_SEPARATOR, callsign], data.len()).unwrap();

    (header, data)
}

#[cfg(test)]
fn create_packet_with<T>(prn: &mut prn_id::PRN, data: T) -> (frame::DataHeader, Vec<u8>) where T: Iterator<Item=u8> {
    let mut data = data.collect::<Vec<u8>>();
    let callsign = prn.callsign;

    let header = frame::new_data(prn, &[callsign, routing::ADDRESS_SEPARATOR, callsign], data.len()).unwrap();

    (header, data)
}

#[test]
fn test_enqueue() {
    let mut prn = prn_id::new(['K', 'I', '7', 'E', 'S', 'T', '0']).unwrap();
    let (header, data) = create_sample_packet(&mut prn, 256);

    let mut queue = new();
    match queue.enqueue(header, &data) {
        Ok(()) => (),
        Err(_) => assert!(false)
    };

    assert_eq!(data.len(), queue.data.len());
    for (i, byte) in data.iter().enumerate() {
        assert_eq!(*byte, queue.data[i]);
    }

    assert_eq!(queue.pending.len(), 1);
    assert_eq!(queue.pending[0].data_offset, 0);
    assert_eq!(queue.pending[0].retry_count, 0);
    assert_eq!(queue.pending[0].next_send, RETRY_DELAY_MS);
    assert_eq!(queue.pending[0].packet, header);
}

#[test]
fn test_discard() {
    let mut prn = prn_id::new(['K', 'I', '7', 'E', 'S', 'T', '0']).unwrap();
    let mut queue = new();

    for i in 0..50 {
        let iter = (0..1024).map(|_| i as u8);
        let (header, data) = create_packet_with(&mut prn, iter);

        match queue.enqueue(header, &data) {
            Err(_) => assert!(false),
            Ok(()) => ()
        }
    }

    {
        let (header, data) = create_sample_packet(&mut prn, 1);
        match queue.enqueue(header, &data) {
            Ok(()) => assert!(false),
            Err(e) => {
                match e {
                    QueueError::Discarded => (),
                    _ => assert!(false)
                }
            }
        }
    }

    let first_prn = queue.pending[0].packet.prn;
    queue.ack_recv(first_prn);
    
    {
        for _ in 0..4 {
            let (header, data) = create_sample_packet(&mut prn, 256);
            match queue.enqueue(header, &data) {
                Ok(()) => (),
                Err(_) => assert!(false)
            }
        }
    }

    {
        let (header, data) = create_sample_packet(&mut prn, 1);
        match queue.enqueue(header, &data) {
            Ok(()) => assert!(false),
            Err(_) => ()
        }
    }
}

#[test]
fn test_empty_tick() {
    let mut queue = new();

    let mut retry_count = 0;
    let mut discard_count = 0;

    let result = queue.tick::<_,_,io::ErrorKind>(0, 
        |_, _| {
            retry_count += 1;
            Ok(())
        },
        |_| {
            discard_count += 1;
        });

    assert!(result.is_ok());
    assert_eq!(retry_count, 0);
    assert_eq!(discard_count, 0);
}

#[test]
fn test_tick_lifetime() {
    let mut prn = prn_id::new(['K', 'I', '7', 'E', 'S', 'T', '0']).unwrap();
    let mut queue = new();
    let (header, data) = create_sample_packet(&mut prn, 1);

    let header_prn = header.prn;

    let mut retry_count = 0;
    let mut discard_count = 0;

    assert!(queue.enqueue(header, &data).is_ok());

    //Calculate the maximum retry ms we need for a single packet to discard
    fn calc_retry(count: usize) -> usize {
        if count == 0 {
            return RETRY_DELAY_MS
        } else {
            return (1+count) * RETRY_DELAY_MS + calc_retry(count-1)
        }
    }

    for _ in 0..(calc_retry(RETRY_COUNT) / 50) + 1 {
        let result = queue.tick::<_,_,io::ErrorKind>(50,
            |header,_| {
                assert_eq!(header.prn, header_prn);
                retry_count += 1;
                Ok(())
            },
            |header| {
                assert_eq!(header.prn, header_prn);
                discard_count += 1;
            });

        assert!(result.is_ok());
    }

    assert_eq!(retry_count, RETRY_COUNT);
    assert_eq!(discard_count, 1);
}