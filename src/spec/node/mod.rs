pub mod prn_table;
pub mod tx_queue;

use std::io;
use std::mem;
use spec::prn_id;
use spec::frame;
use spec::routing;
use spec::address;
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

/// Constructs a new SimpleLink node that can be used to communicate with other SimpleLink nodes
pub fn new(callsign: u32) -> Node {
    info!("New link created with callsign {:?}", address::decode(callsign));

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

        let header = try!(frame::new_header(&mut self.prn, final_route));
        try!(self.enqueue_frame(header, in_data, tx_drain));

        Ok(self.prn.current())
    }

    fn enqueue_frame<T>(&mut self, header: frame::Frame, in_data: &[u8], tx_drain: &mut T) -> Result<(), SendError>
        where T: io::Write
    {
        //Save packet for resend
        match self.tx_queue.enqueue(header, in_data) {
            Ok(()) => {
                try!(self.send_frame(header, in_data, tx_drain));
            },
            Err(e) => {
                trace!("Error sending frame {:?}", e);
                return Err(SendError::Enqueue(e))
            }
        }

        Ok(())
    }

    fn send_frame<T>(&self, header: frame::Frame, in_data: &[u8], tx_drain: &mut T) -> Result<(), SendError>
        where T: io::Write
    {
        let mut packet_data: [u8; frame::MAX_PACKET_SIZE] = unsafe { mem::uninitialized() };
        let packet_len = try!(frame::to_bytes(&mut io::Cursor::new(&mut packet_data[..frame::MAX_PACKET_SIZE]), &header, Some(in_data)));
        try!(kiss::encode(&mut io::Cursor::new(&packet_data[..packet_len]), tx_drain, 0));
        trace!("Sent frame {}", header.prn);

        Ok(())
    }

    /// Receives any packets, sends immediate acks, packets are delivered via packet_drain callback
    pub fn recv<RW,P,O>(&mut self, rx_tx: &mut RW, mut recv_drain: P, mut observe_drain: O) -> Result<(), RecvError>
        where
            RW: io::Read + io::Write,
            P: FnMut(&frame::Frame, &[u8]),
            O: FnMut(&frame::Frame, &[u8])
    {
        const SCRACH_SIZE: usize = 256;
        let mut scratch: [u8; SCRACH_SIZE] = unsafe { mem::uninitialized() };

        loop {
            let bytes = try!(rx_tx.read(&mut scratch));

            if bytes == 0 {
                break;
            }

            //Copy data to our read buffer
            self.recv_buffer.extend_from_slice(&scratch[..bytes]);
            
            //Parse any KISS frames
            loop {
                self.kiss_frame_scratch.drain(..);
                match kiss::decode(self.recv_buffer.iter().cloned(), &mut self.kiss_frame_scratch) {
                    Some(decoded) => {
                        let mut payload: [u8; frame::MTU] = unsafe { mem::uninitialized() };
                        let result = match frame::from_bytes(&mut io::Cursor::new(&self.kiss_frame_scratch[..decoded.payload_size]), &mut payload, decoded.payload_size) {
                            Ok((packet, payload_size)) => {
                                self.dispatch_recv(rx_tx, &packet, &payload[..payload_size], &mut recv_drain, &mut observe_drain)
                            },
                            Err(e) => Err(e).map_err(|e| RecvError::Frame(e))
                        };
                        
                        //Clear recieved, make sure we do this even on error
                        self.recv_buffer.drain(..decoded.bytes_read);

                        try!(result);
                    },
                    None => break
                }
            }
        }

        Ok(())
    }

    /// Dispaches packet based on data/ack and if this was a routing destination
    fn dispatch_recv<T,P,O>(&mut self, tx_drain: &mut T, packet: &frame::Frame, payload: &[u8], recv_drain: &mut P, observe_drain: &mut O) -> Result<(), RecvError>
        where 
            T: io::Write,
            P: FnMut(&frame::Frame, &[u8]),
            O: FnMut(&frame::Frame, &[u8])
    {
        if routing::is_destination(&packet.address_route, self.prn.callsign) {
            trace!("Recieved packet with our address in the route {}", packet.prn);

            //Respond that we've received this packet if we're the final destination, note that
            //we might ack something we've already receieved since the sender may have not
            //heard the ack.
            if routing::final_addr(&packet.address_route) {
                //If we got an ack packet then pass that along to our tx queue
                if payload.len() == 0 {
                    trace!("Recieved ack {}", packet.prn);
                    self.tx_queue.ack_recv(packet.prn);
                    recv_drain(&packet, payload);
                } else {
                    let ack = frame::new_ack(packet.prn, routing::reverse(&packet.address_route));
                    let mut ack_packet: [u8; frame::MAX_ACK_SIZE] = unsafe { mem::uninitialized() };
                    let ack_packet_len = try!(frame::to_bytes(&mut io::Cursor::new(&mut ack_packet[..frame::MAX_ACK_SIZE]), &ack, None));
                    try!(kiss::encode(&mut io::Cursor::new(&ack_packet[..ack_packet_len]), tx_drain, 0));
                    trace!("Sending ack for {}", packet.prn);

                    let new_packet = !self.recv_prn_table.contains(packet.prn);

                    //Don't process duplicates
                    if new_packet {
                        trace!("New packet that we haven't seen yet");
                        self.recv_prn_table.add(packet.prn);

                        //If we're the final destination then we should process this packet
                        trace!("Final dest, surfacing packet as data");
                        recv_drain(&packet, payload);
                    } else {
                        trace!("Duplicate packet already recieved before");
                    }
                }
            } else {    //Route this packet along
                trace!("Packet has routes yet to complete, sending");
                let mut routed_header = *packet;
                routed_header.address_route = try!(routing::advance(&packet.address_route, self.prn.callsign));

                //@todo: Reject packets that already have this ID in the source path since that means we've seen it before

                //Just pass along, we don't ack unless we are the end host
                try!(self.send_frame(routed_header, payload, tx_drain));
            }
        } else {
            trace!("Data frame but addr {:?} is not our dest {:?}", address::decode(packet.address_route[0]), address::decode(self.prn.callsign));
        }

        trace!("obs");
        observe_drain(packet, payload);

        Ok(())
    }

    /// Ticks any packet retries that need to be sent
    pub fn tick<T,R,D>(&mut self, tx_drain: &mut T, elapsed_ms: usize, mut retry_drain: R, discard_drain: D) -> Result<(), SendError>
        where
            T: io::Write,
            R: FnMut(&frame::Frame, &[u8], usize),
            D: FnMut(&frame::Frame, &[u8]),
    {
        try!(self.tx_queue.tick::<_,_,SendError>(elapsed_ms,
            |header, data, next_retry| {
                trace!("Packet {} retrying", header.prn);

                //Retry our frame
                try!(frame::to_bytes(tx_drain, header, Some(data)));

                //Notify client that we resent
                retry_drain(header, data, next_retry);

                Ok(())
            },
            discard_drain));

        Ok(())
    }
}


#[cfg(test)]
use util;

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

    let mut tx_local = vec!();
    let mut tx_remote = vec!();

    let mut local = new(local_addr);
    let mut remote = new(remote_addr);

    let prn = local.send(data.iter().cloned(), [remote_addr].iter().cloned(), &mut tx_local).unwrap();

    let mut match_recv = false;
    remote.recv(&mut util::new_read_write_dispatch(&mut io::Cursor::new(&tx_local), &mut tx_remote),
        |_,recv_data| {
            match_recv = true;
            assert!(recv_data.iter().eq(data.iter()));
        },
        |_,_| {

        }).unwrap();

    assert!(match_recv);

    tx_local.drain(..);

    let mut match_ack = false;
    local.recv(&mut util::new_read_write_dispatch(&mut io::Cursor::new(&tx_remote), &mut tx_local),
        |_,_| {},
        |header,payload| {
            if payload.len() == 0 {
                match_ack = true;
                assert_eq!(prn, header.prn);
                assert_eq!(header.address_route, routing::gen_route(&[local_addr, routing::ADDRESS_SEPARATOR, remote_addr]));
            } else {
                assert!(false);
            }
        }).unwrap();

    assert!(match_ack);
    assert_eq!(local.tx_queue.pending_packets(), 0);
}

#[cfg(test)]
fn gen_callsign(idx: usize) -> [char; 7] {
    ['T', 'E', 'S', 'T', address::symbol_to_character((idx / 10) as u8), address::symbol_to_character((idx % 10) as u8), '0']
}

#[test]
fn test_route() {
    const CALL_COUNT: usize = 16;

    let route = (0..CALL_COUNT-1)
        .map(|i| gen_callsign(i))
        .map(|cs| address::encode(cs).unwrap())
        .collect::<Vec<_>>();

    let local = address::encode(['K', 'I', '7', 'E', 'S', 'T', '0']).unwrap();

    use std::iter;
    let mut nodes = iter::once(local).chain(route.iter().cloned())
        .map(|addr| new(addr))
        .collect::<Vec<_>>();

    let mut tx_frame = vec!();
    let mut rx_frame;

    let mut obs = [0; CALL_COUNT];
    let mut recv = [0; CALL_COUNT];

    //Send initial packet
    let data = (0..128).map(|x| x as u8);
    nodes[0].send(data, route.iter().cloned(), &mut tx_frame).unwrap();

    rx_frame = tx_frame.clone();
    tx_frame.drain(..);

    for _ in 0..CALL_COUNT {
        for (i,node) in nodes.iter_mut().enumerate() {
            node.recv(&mut util::new_read_write_dispatch(&mut io::Cursor::new(&rx_frame), &mut tx_frame),
                |_,data| {
                    recv[i] += 1;
                    assert!((0..128).eq(data.iter().cloned()));
                },
                |_,data| {
                    if data.len() > 0 {
                        obs[i] += 1;
                        assert!((0..128).eq(data.iter().cloned()));
                    }
                }).unwrap();
        }

        //Swap TX and RX
        rx_frame = tx_frame.clone();
        tx_frame.drain(..);
    }

    for i in 1..CALL_COUNT {
        assert_eq!(obs[i], 15);
        assert_eq!(nodes[i].tx_queue.pending_packets(), 0);
        assert_eq!(nodes[i].recv_buffer.len(), 0);
    }

    for i in 1..CALL_COUNT-1 {
        assert_eq!(recv[i], 0);
    }

    assert_eq!(recv[CALL_COUNT-1], 1);
}

#[test]
fn test_broadcast_route() {
    const CALL_COUNT: usize = 16;

    let route = (0..CALL_COUNT-1)
        .map(|i| (i, gen_callsign(i)))
        .map(|(i, cs)| {
            if i == 1 {
                routing::BROADCAST_ADDRESS
            } else {
                address::encode(cs).unwrap()
            }
        })
        .collect::<Vec<_>>();

    let node_addr = (0..CALL_COUNT-1)
        .map(|i| gen_callsign(i))
        .map(|cs| address::encode(cs).unwrap());

    let local = address::encode(['K', 'I', '7', 'E', 'S', 'T', '0']).unwrap();

    use std::iter;
    let mut nodes = iter::once(local).chain(node_addr)
        .map(|addr| new(addr))
        .collect::<Vec<_>>();

    let mut tx_frame = vec!();
    let mut rx_frame;

    let mut obs = [0; CALL_COUNT];
    let mut recv = [0; CALL_COUNT];

    //Send initial packet
    let data = (0..128).map(|x| x as u8);
    nodes[0].send(data, route.iter().cloned(), &mut tx_frame).unwrap();

    rx_frame = tx_frame.clone();
    tx_frame.drain(..);

    for _ in 0..CALL_COUNT {
        for (i,node) in nodes.iter_mut().enumerate() {
            node.recv(&mut util::new_read_write_dispatch(&mut io::Cursor::new(&rx_frame), &mut tx_frame),
                |_,data| {
                    if data.len() > 0 {
                        recv[i] += 1;
                        assert!((0..128).eq(data.iter().cloned()));
                    }
                },
                |_,data| {
                    if data.len() > 0 {
                        obs[i] += 1;
                        assert!((0..128).eq(data.iter().cloned()));
                    }
                }).unwrap();
        }

        //Swap TX and RX
        rx_frame = tx_frame.clone();
        tx_frame.drain(..);
    }

    for i in 1..CALL_COUNT {
        assert_eq!(obs[i], 210);
        assert_eq!(nodes[i].tx_queue.pending_packets(), 0);
        assert_eq!(nodes[i].recv_buffer.len(), 0);
    }

    for i in 1..CALL_COUNT-1 {
        assert_eq!(recv[i], 0);
    }

    assert_eq!(recv[CALL_COUNT-1], 1);
}

#[test]
fn test_split_path() {
    fn get_split(prn: &mut prn_id::PRN) -> Vec<u8> {
        let mut packet = vec!();
        let callsign = prn.current;
        let header = frame::new_header(prn, [callsign, routing::ADDRESS_SEPARATOR, callsign, callsign].into_iter().cloned()).unwrap();
        let data = (0..32).map(|x| x as u8).collect::<Vec<_>>();
        frame::to_bytes(&mut packet, &header, Some(&data)).unwrap();
        let mut kiss = vec!();
        kiss::encode(&mut io::Cursor::new(packet), &mut kiss, 0).unwrap();

        kiss
    }

    let mut prn = prn_id::new(address::encode(['K', 'I', '7', 'E', 'S', 'T', '0']).unwrap());

    let mut left_packet = get_split(&mut prn);
    let right_packet = left_packet.clone();

    left_packet.extend_from_slice(&right_packet);

    let mut node = new(prn.callsign);

    let mut rx_count = 0;
    let mut obs_count = 0;

    let mut tx = vec!();
    node.recv(&mut util::new_read_write_dispatch(&mut io::Cursor::new(&left_packet), &mut tx),
        |_, payload| {
            if payload.len() > 0 {
                rx_count += 1;
            }
        },
        |_, payload| {
            if payload.len() > 0 {
                obs_count += 1;
            }
        }).unwrap();
    
    assert_eq!(rx_count, 1);
    assert_eq!(obs_count, 2);
}

#[test]
fn test_recv_bad_data() {
    let prn = prn_id::new(address::encode(['K', 'I', '7', 'E', 'S', 'T', '0']).unwrap());

    for i in 0..frame::MTU+2 {
        let mut node = new(prn.callsign);

        let bad_data = (0..i).map(|x| x as u8).collect::<Vec<_>>();
        let mut bad_kiss = vec!();
        kiss::encode(&mut io::Cursor::new(bad_data), &mut bad_kiss, 0).unwrap();

        let result = node.recv(&mut util::new_read_write_dispatch(&mut io::Cursor::new(bad_kiss), &mut vec!()),
            |_,_| {},
            |_,_| {});

        match result {
            Ok(()) => assert!(false),
            _ => ()
        }

        let mut packet = vec!();
        use std::iter;
        node.send((0..5).map(|x| x as u8), iter::once(prn.callsign), &mut util::new_read_write_dispatch(&mut io::Cursor::new(vec!()), &mut packet)).unwrap();

        node.recv(&mut util::new_read_write_dispatch(&mut io::Cursor::new(packet), &mut vec!()),
            |_,data| {
                for i in 0..5 {
                    assert_eq!(data[i], i as u8);
                }
            },
            |_,_| {

            }).unwrap();
    }
}