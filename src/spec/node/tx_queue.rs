///! Transmitting queue for outgoing frames
use std::fmt;
use rand;
use spec::frame;

/// Maximum number of packets in flight
pub const MAX_PACKET: usize = 256;
/// Data buffer size
pub const BLOCK_SIZE: usize = 50 * 1024;
/// Data buffer size in flight before congestion control takes effect
pub const CONGEST_CONTROL: usize = 35 * 1024;
/// Number of times a packet will attempt to retry
pub const RETRY_COUNT: usize = 4;
/// Number of milliseconds until we will resend an un-ack'd packet. Grows proportional to the number of retries.
pub const RETRY_DELAY_MS: usize = 500;

/// Queue of packets waiting to be recieved
pub struct Queue {
    /// Packets waiting to go our on the wire
    pending: Vec<PendingPacket>,
    /// Payloads for pending packets
    data: Vec<u8>
}

#[derive(Debug)]
pub enum QueueError {
    /// Congestion control is underway and this frame was immediately discarded
    Discarded
}

/// Pending packet to be recieved
#[derive(Copy, Clone)]
pub struct PendingPacket {
    /// Packet we're trying to send
    packet: frame::Frame,
    /// Last time in ms from when we sent it
    next_send: usize,
    /// Number of retry attempts
    retry_count: usize,
    /// Byte offset for our payload packet
    data_offset: usize,
    /// Size of our data packet
    data_size : usize
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
    pub fn enqueue(&mut self, header: frame::Frame, payload: &[u8]) -> Result<(),QueueError> {
        trace!("Enqueuing frame {} with {} bytes, waiting for ACK", header.prn, payload.len());

        if self.data.len() + payload.len() > BLOCK_SIZE {
            error!("Tried to queue packet but congestion control is under way and was discarded");
            return Err(QueueError::Discarded);
        }

        //Store where we started reading data so we can move our copy back if it fails
        let data_start = self.data.len();

        self.data.extend_from_slice(payload);

        self.pending.push(PendingPacket {
            packet: header,
            next_send: RETRY_DELAY_MS,
            retry_count: 0,
            data_offset: data_start,
            data_size: payload.len()
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
            R: FnMut(&frame::Frame, &[u8], usize) -> Result<(),E>,
            D: FnMut(&frame::Frame, &[u8]),
            E: fmt::Debug
    {
        //trace!("Ticking send queue for {}ms", elapsed_ms);
        let mut idx = 0;
        while idx < self.pending.len() {
            if self.pending[idx].next_send <= elapsed_ms {
                let will_discard = self.pending[idx].retry_count >= RETRY_COUNT || self.data.len() > CONGEST_CONTROL;
                let will_retry = self.pending[idx].retry_count < RETRY_COUNT;

                //If we're going to retry do it first in case we're in a congestion scenario
                if will_retry {
                    trace!("Retrying {} packet with retry count {}", self.pending[idx].packet.prn, self.pending[idx].retry_count);

                    //Note that we increment our retry count here in case something about this packet prevents it
                    //from being sent so we won't hang the whole link
                    self.pending[idx].retry_count += 1;

                    //Determine when we want to retry again. Note that we randomize so two transmitters won't collide
                    use rand::distributions::IndependentSample;
                    let rnd = rand::distributions::Range::new(0.0, 1.0).ind_sample(&mut rand::thread_rng());
                    let next_send = ((1.0 + self.pending[idx].retry_count as f32 * rnd) * RETRY_DELAY_MS as f32) as usize;
                    self.pending[idx].next_send = next_send;

                    match retry(&self.pending[idx].packet, self.get_packet_data(&self.pending[idx]), next_send) {
                        Ok(()) => (),
                        Err(e) => {
                            trace!("Error retrying packet {:?}, incrementing retry counter and aborting", &e);
                            return Err(e)
                        }
                    }
                }

                //Discard our packet if we've flagged it for discarding
                if will_discard {
                    if self.data.len() > CONGEST_CONTROL {
                        trace!("Congestion control underway, discarding packet after last retry");
                    } else {
                        trace!("Packet {} has exceeded retry count, discarding", self.pending[idx].packet.prn);
                    }

                    discard(&self.pending[idx].packet, self.get_packet_data(&self.pending[idx]));

                    //Discard our packet
                    self.discard(idx);
                }

                //If we didn't discard advance to the next packet, otherwise we can keep idx the same and continue with the next item
                if !will_discard {
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
        let data_end = data_start + self.pending[idx].data_size;
        self.data.drain(data_start..data_end);
        
        //Remove packet
        self.pending.remove(idx);

        //Update offsets
        for packet in &mut self.pending {
            if packet.data_offset >= data_end {
                packet.data_offset -= data_end - data_start;
            }
        }
    }

    fn get_packet_data<'a>(&'a self, pending: &'a PendingPacket) -> &'a [u8] {
        &self.data[pending.data_offset..pending.data_offset+pending.data_size]
    }

    pub fn pending_packets(&self) -> usize {
        self.pending.len()
    }
}

#[cfg(test)]
use std::io;
#[cfg(test)]
use spec::prn_id;
#[cfg(test)]
use spec::routing;
#[cfg(test)]
use spec::address;

#[cfg(test)]
fn create_sample_packet(prn: &mut prn_id::PRN, size: u32) -> (frame::Frame, Vec<u8>) {
    let data = (0..size).map(|value| value as u8).collect::<Vec<u8>>();
    let callsign = prn.callsign;

    let header = frame::new_header(prn, [callsign, routing::ADDRESS_SEPARATOR, callsign].iter().cloned()).unwrap();

    (header, data)
}

#[cfg(test)]
fn create_packet_with<T>(prn: &mut prn_id::PRN, data: T) -> (frame::Frame, Vec<u8>) where T: Iterator<Item=u8> {
    let data = data.collect::<Vec<u8>>();
    let callsign = prn.callsign;

    let header = frame::new_header(prn, [callsign, routing::ADDRESS_SEPARATOR, callsign].iter().cloned()).unwrap();

    (header, data)
}

#[test]
fn test_enqueue() {
    let mut prn = prn_id::new(address::encode(['K', 'I', '7', 'E', 'S', 'T', '0']).unwrap());
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
    let mut prn = prn_id::new(address::encode(['K', 'I', '7', 'E', 'S', 'T', '0']).unwrap());
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
                    QueueError::Discarded => ()
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
        |_,_| {
            discard_count += 1;
        });

    assert!(result.is_ok());
    assert_eq!(retry_count, 0);
    assert_eq!(discard_count, 0);
}

#[test]
fn test_tick_lifetime() {
    let mut prn = prn_id::new(address::encode(['K', 'I', '7', 'E', 'S', 'T', '0']).unwrap());
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

    //Force a retry and discard
    for _ in 0..(calc_retry(RETRY_COUNT) / 50) + 1 {
        let result = queue.tick::<_,_,io::ErrorKind>(50,
            |header,_| {
                assert_eq!(header.prn, header_prn);
                retry_count += 1;
                Ok(())
            },
            |header,_| {
                assert_eq!(header.prn, header_prn);
                discard_count += 1;
            });

        assert!(result.is_ok());
    }

    assert_eq!(retry_count, RETRY_COUNT);
    assert_eq!(discard_count, 1);
}

#[test]
fn test_tick_bad_io() {
    let mut prn = prn_id::new(address::encode(['K', 'I', '7', 'E', 'S', 'T', '0']).unwrap());
    let mut queue = new();
    let (header, data) = create_sample_packet(&mut prn, 1);

    let mut retry_count = 0;
    let mut discard_count = 0;

    assert!(queue.enqueue(header, &data).is_ok());

    //Force all packets to try and eventually discard
    for _ in 0..RETRY_COUNT+1 {
        let is_discard = retry_count == RETRY_COUNT;

        let result = queue.tick(RETRY_DELAY_MS * (1 + RETRY_COUNT),
            |_,_| {
                retry_count += 1;
                Err(io::ErrorKind::NotConnected)
            },
            |_,_| {
                discard_count += 1;
            });

        if !is_discard {
            assert!(result.is_err());
        } else {
            assert!(result.is_ok());
        }
    }

    assert_eq!(retry_count, RETRY_COUNT);
    assert_eq!(discard_count, 1);
}

#[test]
fn test_discard_mixed() {
    let mut prn = prn_id::new(address::encode(['K', 'I', '7', 'E', 'S', 'T', '0']).unwrap());
    let packets = (0..5).map(|_| create_sample_packet(&mut prn, 8)).collect::<Vec<_>>();

    let mut queue = new();

    for &(ref header, ref data) in &packets {
        queue.enqueue(*header, data).unwrap();
    }

    assert_eq!(queue.data.len(), queue.pending.len() * 8);

    for i in 0..queue.pending.len() {
        assert_eq!(queue.pending[i].data_offset, i*8);
    }

    let ack_prn = queue.pending[1].packet.prn;
    queue.ack_recv(ack_prn);

    assert_eq!(queue.data.len(), queue.pending.len() * 8);

    for i in 0..queue.pending.len() {
        assert_eq!(queue.pending[i].data_offset, i*8);
    }
}

#[test]
fn test_multi_ack() {
    let mut prn = prn_id::new(address::encode(['K', 'I', '7', 'E', 'S', 'T', '0']).unwrap());
    let discard = (0..5).map(|_| create_sample_packet(&mut prn, 8)).collect::<Vec<_>>();
    let ack = (0..10).map(|_| create_sample_packet(&mut prn, 16)).collect::<Vec<_>>();

    let mut queue = new();

    //Add all the ack and discard packets
    for &(ref header, ref data) in &discard {
        queue.enqueue(*header, data).unwrap();
    }

    for &(ref header, ref data) in &ack {
        queue.enqueue(*header, data).unwrap();
    }

    let mut discard_count = 0;

    //Ack every ack packet
    for &(ref header, _) in &ack {
        queue.ack_recv(header.prn);

        let result = queue.tick::<_,_,io::ErrorKind>(1,
            |_,_| {
                Ok(())
            },
            |_,_| {
                discard_count += 1;
            });

        assert!(result.is_ok());
    }

    //Time out the discard packets
    for _ in 0..RETRY_COUNT+1 {
        queue.tick::<_,_,io::ErrorKind>(RETRY_DELAY_MS * (1 + RETRY_COUNT),
            |_,_| {
                Ok(())
            },
            |header, data| {
                assert!(discard.iter().any(|&(ref discard,_)| discard.prn == header.prn));
                assert_eq!(data.len(), 8);
                discard_count += 1;
            }).unwrap();
    }

    assert_eq!(discard_count, discard.len());
}

#[test]
fn test_congestion() {
    let mut prn = prn_id::new(address::encode(['K', 'I', '7', 'E', 'S', 'T', '0']).unwrap());
    let mut queue = new();

    //Create 40 packets 1024 in length to force congestion control
    let packets = (0..40).map(|i| create_packet_with(&mut prn, (0..1024).map(|_| i as u8))).collect::<Vec<_>>();

    for (header, data) in packets {
        queue.enqueue(header, &data).unwrap();
    }

    let mut retry_count = 0;
    let mut discard_count = 0;

    queue.tick::<_,_,io::ErrorKind>(RETRY_DELAY_MS,
        |_,_| {
            retry_count += 1;
            Ok(())
        },
        |_,_| {
            discard_count += 1;
        }).unwrap();

    assert_eq!(retry_count, 40);
    
    //Only 5 should discard before we drop out of congestion control
    assert_eq!(discard_count, 5);
}