///! NBP node module
pub mod prn_table;
pub mod tx_queue;

use std::io;
use std::mem;
use nbp::prn_id;
use nbp::frame;
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
    /// IO Error occured
    Io(frame::WriteError),
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
    Io(io::Error)
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

/// Constructs a new NBP node that can be used to communicate with other NBP nodes
pub fn new(callsign: [char; 7]) -> Result<Node, NodeError> {
    let prn = match prn_id::new(callsign) {
        Some(prn) => prn,
        _ => return Err(NodeError::BadCallsign)
    };

    Ok(Node {
        prn: prn,
        recv_prn_table: prn_table::new(),
        tx_queue: tx_queue::new(),
        recv_buffer: vec!(),
        kiss_frame_scratch: vec!()
    })
}

impl Node {
    /// Sends a packet out on the wire. Returns the PRN of the packet that was sent
    pub fn send<B,T>(&mut self, in_data: B, addr_route: &[u32], tx_drain: &mut T) -> Result<prn_id::PrnValue, SendError> 
        where
            B: Iterator<Item=u8>,
            T: io::Write
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
            return Err(SendError::Truncated)
        }

        self.send_slice(&scratch[..data_size], addr_route, tx_drain)
    }

    pub fn send_slice<T>(&mut self, in_data: &[u8], addr_route: &[u32], tx_drain: &mut T) -> Result<prn_id::PrnValue, SendError>
        where T: io::Write
    {
        let header = try!(frame::new_data(&mut self.prn, addr_route));

        //Save packet for resend
        match self.tx_queue.enqueue(header, in_data) {
            Ok(()) => {
                try!(frame::to_bytes(tx_drain, &frame::Frame::Data(header), Some(in_data)));
            },
            Err(e) => return Err(SendError::Enqueue(e))
        }

        Ok(self.prn.current())
    }

    /// Receives any packets, sends immediate acks, packets are delivered via packet_drain callback
    pub fn recv<R,T,P>(&mut self, rx_source: &mut R, tx_drain: &mut T, packet_drain: P) -> Result<(), RecvError>
        where
            R: io::Read,
            T: io::Write,
            P: Fn(&frame::Frame, &[u8])
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
                    
                    let duplicate = match &packet {
                        &frame::Frame::Ack(ack) => {
                            self.tx_queue.ack_recv(ack.prn);

                            //We never flag ack packets as duplicates even if they could be
                            //so that clients can use an ack response as a way to do network discovery
                            false
                        },
                        &frame::Frame::Data(header) => {
                            //Respond that we've received this packet
                            let ack = frame::new_ack(header.prn, self.prn.callsign);
                            try!(frame::to_bytes(tx_drain, &frame::Frame::Ack(ack), None));

                            if !self.recv_prn_table.contains(header.prn) {
                                self.recv_prn_table.add(header.prn);
                                false
                            } else {    //We've got a duplicate, don't handle it
                                true
                            }
                        }
                    };

                    //Only share this with our client if we haven't seen if before
                    if !duplicate {
                        packet_drain(&packet, &payload[..payload_size]);
                    }
                },
                None => ()
            }
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
            move |header, data| {
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