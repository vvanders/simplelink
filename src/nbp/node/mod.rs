///! NBP node module
pub mod prn_table;
pub mod tx_queue;

use std::io;
use std::mem;
use nbp::prn_id;
use nbp::frame;
use nbp::routing;
use kiss;

pub struct Node {
    prn: prn_id::PRN,
    
    recv_prn_table: prn_table::Table,
    tx_queue: tx_queue::Queue,

    recv_buffer: Vec<u8>,
    kiss_frame_scratch: Vec<u8>
}

#[derive(Debug)]
pub enum NodeError {
    /// The passed in callsign is not valid
    BadCallsign
}

#[derive(Debug)]
pub enum SendError {
    /// Frame formatting error occured
    Frame(frame::EncodeError),
    /// Error occured while queuing packet
    Enqueue(tx_queue::QueueError),
    /// Write Error occured
    Write(frame::WriteError),
    /// IO Error occured
    Io(io::Error),
    /// Packet was larger than MTU
    Truncated
}

impl From<frame::EncodeError> for SendError {
    fn from(err: frame::EncodeError) -> SendError {
        SendError::Frame(err)
    }
}

impl From<frame::WriteError> for SendError {
    fn from(err: frame::WriteError) -> SendError {
        SendError::Write(err)
    }
}

impl From<io::Error> for SendError {
    fn from(err: io::Error) -> SendError {
        SendError::Io(err)
    }
}

#[derive(Debug)]
pub enum RecvError {
    /// Error decoding frame
    Frame(frame::ReadError),
    /// IO Error occured
    Ack(frame::WriteError),
    /// Error reading or writing to IO
    Io(io::Error),
    /// Parse error reading address
    Routing(routing::ParseError),
    /// Error sending ack/routing packet during recv
    Send(SendError)
}

impl From<frame::ReadError> for RecvError {
    fn from(err: frame::ReadError) -> RecvError {
        RecvError::Frame(err)
    }
}

impl From<io::Error> for RecvError {
    fn from(err: io::Error) -> RecvError {
        RecvError::Io(err)
    }
}

impl From<frame::WriteError> for RecvError {
    fn from(err: frame::WriteError) -> RecvError {
        RecvError::Ack(err)
    }
}

impl From<routing::ParseError> for RecvError {
    fn from(err: routing::ParseError) -> RecvError {
        RecvError::Routing(err)
    }
}

impl From<SendError> for RecvError {
    fn from(err: SendError) -> RecvError {
        RecvError::Send(err)
    }
}

/// Constructs a new NBP node that can be used to communicate with other NBP nodes
pub fn new(callsign: u32) -> Node {
    Node {
        prn: prn_id::new(callsign),
        recv_prn_table: prn_table::new(),
        tx_queue: tx_queue::new(),
        recv_buffer: vec!(),
        kiss_frame_scratch: vec!()
    }
}

impl Node {
    /// Sends a packet out on the wire. Returns the PRN of the packet that was sent
    pub fn send<B,T,A>(&mut self, in_data: B, addr_route: A, tx_drain: &mut T) -> Result<prn_id::PrnValue, SendError> 
        where
            B: Iterator<Item=u8>,
            T: io::Write,
            A: Iterator<Item=u32>
    {
        //Copy data into scratch array
        let mut scratch: [u8; frame::MTU] = unsafe { mem::uninitialized() };
        
        let data_size = in_data
            .fold(0, |idx, byte| {
                if idx < frame::MTU {
                    scratch[idx] = byte;
                }

                idx+1
            });

        if data_size > frame::MTU {
            trace!("Tried sending packet but larger than MTU");
            return Err(SendError::Truncated)
        }

        self.send_slice(&scratch[..data_size], addr_route, tx_drain)
    }

    /// Sends a packet out on the wire. Returns the PRN of the packet that was sent
    pub fn send_slice<T,A>(&mut self, in_data: &[u8], addr_route: A, tx_drain: &mut T) -> Result<prn_id::PrnValue, SendError>
        where
            T: io::Write,
            A: Iterator<Item=u32>
    {
        use std::iter;

        if in_data.len() > frame::MTU {
            trace!("Tried sending packet but larger than MTU");
            return Err(SendError::Truncated)
        }

        let final_route = addr_route
            .chain(iter::once(routing::ADDRESS_SEPARATOR))
            .chain(iter::once(self.prn.callsign));

        let header = try!(frame::new_data(&mut self.prn, final_route));
        try!(self.send_frame(header, in_data, tx_drain));

        Ok(self.prn.current())
    }

    fn send_frame<T>(&mut self, header: frame::DataHeader, in_data: &[u8], tx_drain: &mut T) -> Result<(), SendError>
        where T: io::Write
    {
        //Save packet for resend
        match self.tx_queue.enqueue(header, in_data) {
            Ok(()) => {
                let mut packet_data: [u8; frame::MAX_PACKET_SIZE] = unsafe { mem::uninitialized() };
                let packet_len = try!(frame::to_bytes(&mut io::Cursor::new(&mut packet_data[..frame::MAX_PACKET_SIZE]), &frame::Frame::Data(header), Some(in_data)));
                try!(kiss::encode(&mut io::Cursor::new(&packet_data[..packet_len]), tx_drain, 0));
                trace!("Sent frame {}", header.prn);
            },
            Err(e) => {
                trace!("Error sending frame {:?}", e);
                return Err(SendError::Enqueue(e))
            }
        }

        Ok(())
    }

    /// Receives any packets, sends immediate acks, packets are delivered via packet_drain callback
    pub fn recv<R,T,P,O>(&mut self, rx_source: &mut R, tx_drain: &mut T, mut recv_drain: P, mut observe_drain: O) -> Result<(), RecvError>
        where
            R: io::Read,
            T: io::Write,
            P: FnMut(&frame::Frame, &[u8]),
            O: FnMut(&frame::Frame, &[u8])
    {
        const SCRACH_SIZE: usize = 256;
        let mut scratch: [u8; SCRACH_SIZE] = unsafe { mem::uninitialized() };

        loop {
            let bytes = try!(rx_source.read(&mut scratch));

            if bytes == 0 {
                break;
            }

            //Copy data to our read buffer
            self.recv_buffer.extend_from_slice(&scratch[..bytes]);
            
            //Parse any KISS frames
            self.kiss_frame_scratch.drain(..);
            match kiss::decode(self.recv_buffer.iter().cloned(), &mut self.kiss_frame_scratch) {
                Some(decoded) => {
                    let mut payload: [u8; frame::MTU] = unsafe { mem::uninitialized() };
                    let (packet, payload_size) = try!(frame::from_bytes(&mut io::Cursor::new(&self.kiss_frame_scratch[..decoded.payload_size]), &mut payload, decoded.payload_size));
                    
                    try!(self.dispatch_recv(tx_drain, &packet, &payload[..payload_size], &mut observe_drain, &mut recv_drain));
                },
                None => ()
            }
        }

        Ok(())
    }

    /// Dispaches packet based on data/ack and if this was a routing destination
    fn dispatch_recv<T,P,O>(&mut self, tx_drain: &mut T, packet: &frame::Frame, payload: &[u8], observe_drain: &mut P, recv_drain: &mut O) -> Result<(), RecvError>
        where 
            T: io::Write,
            P: FnMut(&frame::Frame, &[u8]),
            O: FnMut(&frame::Frame, &[u8])
    {
        let target = match packet {
            &frame::Frame::Ack(ack) => {
                trace!("Recieved ack {}", ack.prn);
                self.tx_queue.ack_recv(ack.prn);

                false
            },
            &frame::Frame::Data(header) => {
                if routing::is_destination(&header.address_route, self.prn.callsign) && routing::should_route(&header.address_route) {
                    trace!("Recieved packet with our address in the route {}", header.prn);

                    //Respond that we've received this packet
                    let ack = frame::new_ack(header.prn, self.prn.callsign);
                    let mut ack_packet: [u8; frame::MAX_ACK_SIZE] = unsafe { mem::uninitialized() };
                    let ack_packet_len = try!(frame::to_bytes(&mut io::Cursor::new(&mut ack_packet[..frame::MAX_ACK_SIZE]), &frame::Frame::Ack(ack), None));
                    try!(kiss::encode(&mut io::Cursor::new(&ack_packet[..ack_packet_len]), tx_drain, 0));
                    trace!("Sending ack for {}", header.prn);

                    //Don't process duplicates
                    if !self.recv_prn_table.contains(header.prn) {
                        trace!("New packet that we haven't seen yet");
                        self.recv_prn_table.add(header.prn);

                        //If we're the final destination then we should process this packet
                        if routing::final_addr(&header.address_route) {
                            trace!("Final dest, surfacing packet as data");
                            true
                        } else {    //Route this packet along
                            trace!("Packet has routes yet to complete, sending");
                            let mut routed_header = header;
                            routed_header.address_route = try!(routing::advance(&header.address_route, self.prn.callsign));

                            try!(self.send_frame(routed_header, payload, tx_drain));

                            false
                        }
                    } else {
                        trace!("Duplicate packet already recieved before");
                        false
                    }
                } else {
                    trace!("Data frame but addr is not our dest");
                    false
                }
            }
        };

        //Only share this with our client if we haven't seen if before
        observe_drain(packet, payload);

        if target {
            recv_drain(packet, payload);
        }

        Ok(())
    }

    /// Ticks any packet retries that need to be sent
    pub fn tick<T,R,D>(&mut self, tx_drain: &mut T, elapsed_ms: usize, mut retry_drain: R, discard_drain: D) -> Result<(), SendError>
        where
            T: io::Write,
            R: FnMut(&frame::DataHeader, &[u8]),
            D: FnMut(&frame::DataHeader, &[u8]),
    {
        try!(self.tx_queue.tick::<_,_,SendError>(elapsed_ms,
            |header, data| {
                trace!("Packet {} retrying", header.prn);

                //Retry our frame
                try!(frame::to_bytes(tx_drain, &frame::Frame::Data(*header), Some(data)));

                //Notify client that we resent
                retry_drain(header, data);

                Ok(())
            },
            discard_drain));

        Ok(())
    }
}


#[cfg(test)]
use nbp::address;

#[test]
fn test_send() {
    let addr = [
        address::encode(['K', 'F', '7', 'S', 'J', 'K', '0']).unwrap(),
        address::encode(['K', 'I', '7', 'E', 'S', 'T', '0']).unwrap()
    ];

    let mut node = new(addr[1]);

    let mut tx: Vec<u8> = vec!();

    node.send((0..5).map(|x| x as u8), addr.iter().cloned(), &mut tx).unwrap();

    assert!(tx.len() > 0);
}

#[test]
fn test_send_recv() {
    let data = (0..5).map(|x| x as u8).collect::<Vec<_>>();

    let local_addr = address::encode(['K', 'I', '7', 'E', 'S', 'T', '0']).unwrap();
    let remote_addr = address::encode(['K', 'F', '7', 'S', 'J', 'K', '0']).unwrap();

    let mut tx_local = Vec::new();
    let mut tx_remote = Vec::new();

    let mut local = new(local_addr);
    let mut remote = new(remote_addr);

    let prn = local.send(data.iter().cloned(), [remote_addr].iter().cloned(), &mut tx_local).unwrap();

    let mut match_recv = false;
    remote.recv(&mut io::Cursor::new(&tx_local), &mut tx_remote,
        |_,recv_data| {
            match_recv = true;
            assert!(recv_data.iter().eq(data.iter()));
        },
        |_,_| {

        }).unwrap();

    assert!(match_recv);

    tx_local.drain(..);

    let mut match_ack = false;
    local.recv(&mut io::Cursor::new(&tx_remote), &mut tx_local,
        |_,_| {},
        |header,_| {
            match header {
                &frame::Frame::Ack(ack) => {
                    match_ack = true;
                    assert_eq!(prn, ack.prn);
                    assert_eq!(ack.src_addr, remote_addr);
                },
                _ => assert!(false)
            }
        }).unwrap();

    assert!(match_ack);
    assert_eq!(local.tx_queue.pending_packets(), 0);
}