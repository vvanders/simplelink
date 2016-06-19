//! NBP Frame management
pub struct Frame {
    /// Psuedo-Random unique identifier for this packet. This is combination of PRN + XOR of callsign.
    pub prn: u32,
    /// Forward and return address routing. Each path can contain up to 16 addresses plus a single separator.
    pub address_route: [u32; 33],
    /// Payload data.
    pub payload: Vec<u8>,
    /// CRC to verify integrity.
    pub crc: u16
}