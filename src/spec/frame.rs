//! NBP Frame management
use std::io;
use byteorder::{ReadBytesExt, WriteBytesExt, BigEndian};
use spec::crc16;
use spec::prn_id;
use spec::routing;

/// MTU of payload
pub const MTU: usize = 1500;

/// Max size for an ack (Data + PRN + (Addr + delim) + CRC)
pub const MAX_ACK_SIZE: usize = 4 + 4 * (routing::MAX_LENGTH + 1) + 2;

/// Max size for a packet (Data + PRN + Addr + CRC)
pub const MAX_PACKET_SIZE: usize = MAX_ACK_SIZE + MTU;

/// Represents a single NBP Frame. NBP has two types of frames, data and ack frames.
/// And header with zero size is an ACK frame.
#[derive(Copy,Clone,Eq,PartialEq,Debug)]
pub struct Frame {
    /// Pseudo-Random unique identifier for this packet. This is combination of PRN + XOR of callsign.
    pub prn: u32,
    /// Forward and return address routing. Each path can contain up to 16 addresses plus a single separator.
    pub address_route: routing::Route
}

/// Error cases for converting from raw bytes to a frame.
#[derive(Debug)]
pub enum ReadError {
    /// IO error occured while reading.
    IO(io::Error),
    /// Frame was truncated and didn't contain enough bytes to be parsed correctly.
    Truncated,
    /// Address format is malformed and could not be read.
    BadAddress,
    /// Frame failed CRC validation and contains invalid bits.
    CRCFailure
}

/// Error cases for encoding a packet
#[derive(Debug)]
pub enum EncodeError {
    /// Dest address was more than 15 stations
    AddressTooLong,
    /// Address didn't contain a source -> dest separator
    AddressSeparatorNotFound
}

/// Error cases for converting from a frame to raw bytes.
#[derive(Debug)]
pub enum WriteError {
    /// IO error occured while writing.
    IO(io::Error)
}

// Constructs a new ack frame
pub fn new_ack(prn: u32, dest: routing::Route) -> Frame {
    Frame {
        prn: prn,
        address_route: dest
    }
}

/// Constructs a new data frame
pub fn new_header<T>(prn: &mut prn_id::PRN, dest: T) -> Result<Frame, EncodeError> where T: Iterator<Item=u32> {
    let mut addr: routing::Route = [0; routing::MAX_LENGTH];

    //Encode and look for valid addr
    let mut found_sep = false;
    for (i, dest_addr) in dest.enumerate() {
        if i == routing::MAX_LENGTH {
            return Err(EncodeError::AddressTooLong)
        }

        found_sep = found_sep || dest_addr == routing::ADDRESS_SEPARATOR;

        addr[i] = dest_addr;
    }

    if !found_sep {
        return Err(EncodeError::AddressSeparatorNotFound)
    }

    Ok(Frame {
        prn: prn.next(),
        address_route: addr
    })
}

fn read_u32<T>(bytes: &mut T, crc: &mut crc16::CRC) -> Result<u32, ReadError> where T: io::Read {
    let value = try!(bytes.read_u32::<BigEndian>().map_err(|e| ReadError::IO(e)));
    *crc = crc16::update_u32(value, *crc);

    Ok(value)
}

/// Read in a frame from a series of bytes.
pub fn from_bytes<T>(bytes: &mut T, out_payload: &mut [u8], size: usize) -> Result<(Frame, usize), ReadError> where T: io::Read {
    trace!("Reading frame from bytes");

    let mut crc = crc16::new();
    let mut err = None;

    //All frames start with PRN
    let prn = try!(read_u32(bytes, &mut crc));

    debug!("Decoding frame with PRN {} size {}", prn, size);

    //Scan in our address. We're looking for u32+, 0x0, u32+, 0x0.
    let mut addr_marker = 0;
    let mut addr = [0; routing::MAX_LENGTH];
    let mut addr_len = 0;

    debug!("Decoding routing address");

    for _ in 0..routing::MAX_LENGTH {
        let value = try!(read_u32(bytes, &mut crc));

        if value == routing::ADDRESS_SEPARATOR {
            addr_marker += 1;
        }

        addr[addr_len] = value;
        addr_len += 1;

        if addr_marker == 2 {
            trace!("End of addr, len {}", addr_len);
            break;
        }
    }

    //If we saw 17 values that means that the 18th one must be a 0x0 separator, otherwise this is malformed
    if addr_len == routing::MAX_LENGTH && addr_marker != 2 {
        let value = try!(read_u32(bytes, &mut crc));
        addr_len += 1;

        trace!("End of addr, len {}", addr_len);

        if value != 0 {
            error!("Malformed address in packet {}, {:?}", prn, addr);
            err = Some(ReadError::BadAddress);
        }
    }

    let header_size = 4 + addr_len * 4 + 2;

    if size < header_size {
        return Err(ReadError::IO(io::Error::new(io::ErrorKind::InvalidData, "Packet was malformed")))
    } 

    //size - (PRN + ADDR size + CRC)
    let payload_size = size - header_size;

    debug!("Decode payload of {} bytes", payload_size);

    if payload_size > out_payload.len() {
        error!("Payload exceeded output buffer size {} > {} in packet {}", payload_size, out_payload.len(), prn);
        err = Some(ReadError::Truncated);
    }

    use std::io::Read;
    try!(bytes.take(payload_size as u64).read(out_payload).map_err(|e| ReadError::IO(e)));

    trace!("Read payload");

    //Update CRC
    crc = out_payload[..payload_size].iter().fold(crc, |crc, byte| {
        crc16::update_u8(*byte, crc)
    });

    debug!("Read DATA frame with PRN {} Callsign {}", prn, routing::format_route(&addr));

    let frame = (Frame {
        prn: prn,
        address_route: addr
    }, payload_size);

    crc = crc16::finish(crc);

    //Validate our CRC
    let frame_crc = try!(bytes.read_u16::<BigEndian>().map_err(|e| ReadError::IO(e)));

    trace!("Checking CRC {} {}", frame_crc, crc);

    if frame_crc != crc {
        error!("CRC check failed in packet {}", prn);
        err = Some(ReadError::CRCFailure);
    }

    trace!("Successfully decoded packet");

    err.map(|err| Err(err))
        .unwrap_or(Ok(frame))
}

fn write_u32<T>(value: u32, bytes: &mut T, crc: &mut crc16::CRC) -> Result<usize, WriteError> where T: io::Write {
   	try!(bytes.write_u32::<BigEndian>(value).map_err(|e| WriteError::IO(e)));
    *crc = crc16::update_u32(value, *crc);

    Ok(4)
}

/// Convert a frame to a series of bytes.
pub fn to_bytes<T>(bytes: &mut T, frame: &Frame, payload: Option<&[u8]>) -> Result<usize, WriteError> where T: io::Write {
    let mut crc = crc16::new();
    let mut size = 0;

    debug!("Encoding DATA frame {} to bytes", frame.prn);

    //Start with PRN
    size += try!(write_u32(frame.prn, bytes, &mut crc));

    //Address follows, it's in for format of <source>, 0x0, <dest>, 0x0
    let mut delim_count = 0;
    for addr in frame.address_route.iter() {
        if *addr == routing::ADDRESS_SEPARATOR {
            delim_count += 1;
        }

        size += try!(write_u32(*addr, bytes, &mut crc));

        //If we found the last delimiter we are done
        if delim_count == 2 {
            break;
        }
    }

    //If we only saw one delimiter then we need to manually include the trailing one
    if delim_count == 1 {
        size += try!(write_u32(routing::ADDRESS_SEPARATOR, bytes, &mut crc));
    }

    //Handle the actual payload
    match payload {
        Some(data) => {
            try!(bytes.write_all(data).map_err(|e| WriteError::IO(e)));
            size += data.len();

            for byte in data {
                crc = crc16::update_u8(*byte, crc);
            }
        },
        None => ()
    }

    //Last part of the packet is our CRC
    crc = crc16::finish(crc);

    try!(bytes.write_u16::<BigEndian>(crc).map_err(|e| WriteError::IO(e)));
    size += 2;

    trace!("Finished encoding packet {} bytes", size);

    Ok(size)
}

#[cfg(test)]
use spec::address;

#[test]
fn serialize_ack_test() {
    use std::io::Cursor;

    let mut prn = prn_id::new(address::encode(['K', 'I', '7', 'E', 'S', 'T', '0']).unwrap());
    let ack = new_ack(prn.next(), routing::gen_route(&[prn.callsign, routing::ADDRESS_SEPARATOR, prn.callsign]));

    let mut data = vec!();

    let count = to_bytes(&mut data, &ack, None).unwrap();
    assert_eq!(count, 4 + 4 * 4 + 2);

    let mut reader = Cursor::new(data);
    let mut payload = [0; MTU];
    match from_bytes(&mut reader, &mut payload, count) {
        Ok((header, payload_len)) => {
            assert_eq!(header.prn, ack.prn);
            assert_eq!(header.address_route, ack.address_route);
            assert_eq!(payload_len, 0);
        }
        _ => assert!(false)
    }
}

#[cfg(test)]
use std::iter;

#[cfg(test)]
fn serialize_packet(dest: &[u32], payload: &[u8]) -> Vec<u8> {
    let mut prn = prn_id::new(address::encode(['K', 'I', '7', 'E', 'S', 'T', '0']).unwrap());
    let data_packet = new_header(&mut prn, dest.iter().cloned()).unwrap();

    let mut data = vec!();
    let count = to_bytes(&mut data, &data_packet, Some(payload)).unwrap();

    assert_eq!(count, 4 + 4 * (1 + dest.len()) + payload.len() + 2);

    data
}

#[cfg(test)]
fn serialize_deserialize_packet(dest: &[u32], payload: &[u8]) {
    use std::io::Cursor;

    let data = serialize_packet(dest, payload);
    let count = data.len();

    let mut reader = Cursor::new(data);
    let mut read_payload = [0; MTU];
    match from_bytes(&mut reader, &mut read_payload, count) {
        Ok((header, size)) => {
            assert_eq!(size, payload.len());
            for (i, byte) in payload.iter().cloned().enumerate() {
                assert_eq!(read_payload[i], byte);
            }

            for (i, test_addr) in dest.iter().cloned().enumerate() {
                assert_eq!(header.address_route[i], test_addr);
            }
        },
        _ => assert!(false)
    }
}

#[test]
fn serialize_data_test() {
    use spec::address;

    let dest_addr = address::encode(['K', 'F', '7', 'S', 'J', 'K', '0']).unwrap();
    let src_addr = address::encode(['K', 'I', '7', 'E', 'S', 'T', '0']).unwrap();

    let addr: Vec<u32> = iter::once(dest_addr)
        .chain(iter::once(routing::ADDRESS_SEPARATOR))
        .chain(iter::once(src_addr))
        .collect();

    let packet = [1, 2, 3, 4, 5];
    serialize_deserialize_packet(&addr, &packet);
}

#[test]
fn test_addr_permuatations() {
    use spec::address;

    for size in 1..15 {
        //Build address
        let src_addr = address::encode(['K', 'I', '7', 'E', 'S', 'T', '0']).unwrap();

        for i in 0..size {
            fn gen_addr(num: u8) -> [char; 7] {
                if num > 9 {
                    ['T', 'E', 'S', 'T', address::symbol_to_character(num / 10), address::symbol_to_character(num % 10), '0']
                } else {
                    ['T', 'E', 'S', 'T', address::symbol_to_character(num), '0', '0']
                }
            }

            let pre_sep = (0..i).into_iter()
                .map(|i| {
                    gen_addr(i)
                })
                .filter_map(|addr| address::encode(addr));

            let post_sep = (0..size-i).into_iter()
                .rev()
                .map(|i| {
                    gen_addr(i)
                })
                .filter_map(|addr| address::encode(addr));

            let addr: Vec<u32> = iter::once(src_addr)
                .chain(pre_sep)
                .chain(iter::once(routing::ADDRESS_SEPARATOR))
                .chain(post_sep)
                .collect();

            let packet = [1, 2, 3, 4, 5];
            serialize_deserialize_packet(&addr, &packet);
        }
    }
}

#[test]
fn test_payload_permutations() {
    use spec::address;

    for size in 0..MTU+1 {
        let dest_addr = address::encode(['K', 'F', '7', 'S', 'J', 'K', '0']).unwrap();
        let src_addr = address::encode(['K', 'I', '7', 'E', 'S', 'T', '0']).unwrap();

        let addr: Vec<u32> = iter::once(dest_addr)
            .chain(iter::once(routing::ADDRESS_SEPARATOR))
            .chain(iter::once(src_addr))
            .collect();

        let packet: Vec<u8> = (0..size).into_iter()
            .map(|value| value as u8)
            .collect();

        serialize_deserialize_packet(&addr, &packet);
    }
}

#[test]
fn test_corrupt_bit() {
    use spec::address;
    use std::io::Cursor;

    let dest_addr = address::encode(['K', 'F', '7', 'S', 'J', 'K', '0']).unwrap();
    let src_addr = address::encode(['K', 'I', '7', 'E', 'S', 'T', '0']).unwrap();

    let addr: Vec<u32> = iter::once(dest_addr)
        .chain(iter::once(routing::ADDRESS_SEPARATOR))
        .chain(iter::once(src_addr))
        .collect();

    let packet: Vec<u8> = (0..256).into_iter()
        .map(|value| value as u8)
        .collect();

    let mut data = serialize_packet(&addr, &packet);

    for byte in 0..256 {
        for bit in 0..7 {
            //Mutate a bit
            let mask = (1 as u8) << bit;
            data[byte] ^= mask;

            //Validate that we get a CRC error
            {
                let count = data.len();
                let mut reader = Cursor::new(&data);
                let mut payload = [0; MTU];
                match from_bytes(&mut reader, &mut payload, count) {
                    Err(ReadError::CRCFailure) => (),
                    _ => assert!(false)
                }
            }

            //Restore the bit for the next run
            data[byte] ^= mask;
        }
    }
}

#[test]
fn test_max_size() {
    let mut prn = prn_id::new(address::encode(['K', 'I', '7', 'E', 'S', 'T', '0']).unwrap());
    let data = (0..1500).map(|x| x as u8).collect::<Vec<_>>();
    use std::iter;
    let route = (0..15).map(|_| routing::BROADCAST_ADDRESS)
        .chain(iter::once(routing::ADDRESS_SEPARATOR))
        .chain(iter::once(prn.callsign))
        .collect::<Vec<u32>>();
    let header = new_header(&mut prn, route.iter().cloned()).unwrap();

    let mut packet = vec!();

    to_bytes(&mut packet, &header, Some(&data)).unwrap();

    assert_eq!(MAX_PACKET_SIZE, packet.len());

    let ack_header = new_ack(prn.next(), routing::gen_route(route.iter()));
    packet.drain(..);
    to_bytes(&mut packet, &ack_header, None).unwrap();

    assert_eq!(MAX_ACK_SIZE, packet.len());
}