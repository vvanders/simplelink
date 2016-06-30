///! NBP node module
mod prn_table;
mod tx_queue;

use std::io;
use nbp::prn_id;
use nbp::frame;

pub struct Node {
    prn: prn_id::PRN,
    
    recv_prn_table: prn_table::Table
}

pub enum NodeError {
    /// The passed in callsign is not valid
    BadCallsign
}

/// Constructs a new NBP node that can be used to communicate with other NBP nodes
pub fn new(callsign: [char; 7]) -> Result<Node, NodeError> {
    let prn = match prn_id::new(callsign) {
        Some(prn) => prn,
        _ => return Err(NodeError::BadCallsign)
    };

    Ok(Node {
        prn: prn,
        recv_prn_table: prn_table::new()
    })
}

impl Node {
    /// Sends a packet out on the wire. Returns the PRN of the packet that was sent
    pub fn send<B,T>(&mut self, in_data: B, addr_route: &[u32], tx_drain: T) -> io::Result<prn_id::PrnValue> 
        where
            B: Iterator<Item=u8>,
            T: io::Write
    {
        Ok(self.prn.current())
    }

    /// Receives any packets, sends immediate acks, packets are delivered via packet_drain callback
    pub fn recv<R,T,P>(&mut self, rx_source: R, tx_drain: T, packet_drain: P) -> io::Result<()>
        where
            R: io::Read,
            T: io::Write,
            P: Fn(&frame::Frame)
    {
        Ok(())
    }

    /// Ticks any packet retries that need to be sent
    pub fn tick<T>(&mut self, tx_drain: T, elapsed_ms: usize) -> io::Result<()> 
        where
            T: io::Write
    {
        Ok(())
    }
}