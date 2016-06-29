//! NBP Frame management
use std::io;
use byteorder::{ReadBytesExt, WriteBytesExt, BigEndian};
use nbp::crc16;
use nbp::prn_id;
use nbp::routing;

pub const MTU: usize = 1500;

/// Represents a single NBP Ack Frame
#[derive(Copy,Clone)]
pub struct AckFrame {
    /// Pseudo-Random unique identifier that this packet is an ack for.
    pub prn: u32,
    /// Source station that acknowledged the packet.
    pub src_addr: u32
}

/// Represents a single NBP Data Frame
pub struct DataFrame {
    /// Pseudo-Random unique identifier for this packet. This is combination of PRN + XOR of callsign.
    pub prn: u32,
    /// Forward and return address routing. Each path can contain up to 16 addresses plus a single separator.
    pub address_route: [u32; 17],
    /// Payload data. MTU is 1500 so stop there
    pub payload: [u8; MTU],
    /// Payload size
    pub payload_size: usize
}

//Work around the fact that arrays with > 32 elements can't be cloned yet
impl Clone for DataFrame {
    fn clone(&self) -> DataFrame {
        DataFrame {
            prn: self.prn,
            address_route: self.address_route,
            payload: self.payload,
            payload_size: self.payload_size
        }
    }
}

/// All possible NBP frames
pub enum Frame {
    Data(DataFrame),
    Ack(AckFrame)
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
    /// Data packet was more than 1500 bytes
    Truncated,
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

/// Constructs a new ACK frame
pub fn new_ack(prn: u32, src_addr: u32) -> AckFrame {
    AckFrame {
        prn: prn,
        src_addr: src_addr
    }
}

/// Constructs a new data frame
pub fn new_data<T>(prn: &mut prn_id::PRN, dest: &[u32], data: T) -> Result<DataFrame, EncodeError> where T: Iterator<Item=u8> {
    let mut addr: [u32; 17] = [0; 17];
    let mut payload: [u8; MTU] = [0; MTU];

    if dest.len() > 17 {
        return Err(EncodeError::AddressTooLong)
    }

    //Encode and look for valid addr
    let mut found_sep = false;
    for (i, dest_addr) in dest.iter().cloned().enumerate() {
        found_sep = found_sep || dest_addr == routing::ADDRESS_SEPARATOR;

        addr[i] = dest_addr;
    }

    if !found_sep {
        return Err(EncodeError::AddressSeparatorNotFound)
    }

    let payload_size = data.fold(0, |count, byte| {
        if count < payload.len() {
            payload[count] = byte;
        }

        count + 1
    });

    //We truncate at 1500 bytes
    if payload_size > MTU {
        return Err(EncodeError::Truncated)
    }

    Ok(DataFrame {
        prn: prn.next(),
        address_route: addr,
        payload: payload,
        payload_size: payload_size
    })
}

fn read_u32<T>(bytes: &mut T, crc: &mut crc16::CRC) -> Result<u32, ReadError> where T: io::Read {
    let value = try!(bytes.read_u32::<BigEndian>().map_err(|e| ReadError::IO(e)));
    *crc = crc16::update_u32(value, *crc);

    Ok(value)
}

/// Read in a frame from a series of bytes.
pub fn from_bytes<T>(bytes: &mut T, size: usize) -> Result<Frame, ReadError> where T: io::Read {
    let mut crc = crc16::new();
    let mut err = None;

    //All frames start with PRN
    let prn = try!(read_u32(bytes, &mut crc));

    //If we have just a PRN, addr and CRC this is an ack frame
    let frame = if size == 4 + 4 + 2 {
        let addr = try!(read_u32(bytes, &mut crc));

        Frame::Ack(AckFrame {
            prn: prn,
            src_addr: addr
        })
    } else {
        //Scan in our address. We're looking for u32+, 0x0, u32+, 0x0.
        let mut addr_marker = 0;
        let mut addr = [0; 17];
        let mut addr_len = 0;

        for _ in 0..17 {
            let value = try!(read_u32(bytes, &mut crc));

            if value == routing::ADDRESS_SEPARATOR {
                addr_marker += 1;
            }

            addr[addr_len] = value;
            addr_len += 1;

            if addr_marker == 2 {
                break;
            }
        }

        //If we saw 17 values that means that the 18th one must be a 0x0 separator, otherwise this is malformed
        if addr_len == 17 && addr_marker != 2 {
            let value = try!(read_u32(bytes, &mut crc));
            addr_len += 1;

            if value != 0 {
                err = Some(ReadError::BadAddress);
            }
        }

        //size - (PRN + ADDR size + CRC)
        let payload_size = size - (4 + addr_len * 4 + 2);

        let mut payload = [0; 1500];
        try!(bytes.read(&mut payload[..payload_size]).map_err(|e| ReadError::IO(e)));

        //Update CRC
        crc = payload[..payload_size].iter().fold(crc, |crc, byte| {
            crc16::update_u8(*byte, crc)
        });

        Frame::Data(DataFrame{
            prn: prn,
            address_route: addr,
            payload: payload,
            payload_size: payload_size
        })
    };

    crc = crc16::finish(crc);

    //Validate our CRC
    let frame_crc = try!(bytes.read_u16::<BigEndian>().map_err(|e| ReadError::IO(e)));

    if frame_crc != crc {
        err = Some(ReadError::CRCFailure);
    }

    err.map(|err| Err(err))
        .unwrap_or(Ok(frame))
}

fn write_u32<T>(value: u32, bytes: &mut T, crc: &mut crc16::CRC) -> Result<usize, WriteError> where T: io::Write {
   	try!(bytes.write_u32::<BigEndian>(value).map_err(|e| WriteError::IO(e)));
    *crc = crc16::update_u32(value, *crc);

    Ok(4)
}

/// Convert a frame to a series of bytes.
pub fn to_bytes<T>(bytes: &mut T, frame: &Frame) -> Result<usize, WriteError> where T: io::Write {
    let mut crc = crc16::new();
    let mut size = 0;

    match frame {
        &Frame::Data(ref data_frame) => {
            //Start with PRN
            size += try!(write_u32(data_frame.prn, bytes, &mut crc));

            //Address follows, it's in for format of <source>, 0x0, <dest>, 0x0
            let mut delim_count = 0;
            for addr in data_frame.address_route.iter() {
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
            let final_payload = &data_frame.payload[..data_frame.payload_size];

            try!(bytes.write(final_payload).map_err(|e| WriteError::IO(e)));
            size += data_frame.payload_size;

            for byte in final_payload {
                crc = crc16::update_u8(*byte, crc);
            }
        },
        &Frame::Ack(ref ack_frame) => {
            //Start with PRN
            size += try!(write_u32(ack_frame.prn, bytes, &mut crc));

            //Only include this station's callsign since we need that to comply with FCC Part 97. If our last trasmission is an ACK it must include our callsign
            size += try!(write_u32(ack_frame.src_addr, bytes, &mut crc));
        }
    }

    //Last part of the packet is our CRC
    crc = crc16::finish(crc);

    try!(bytes.write_u16::<BigEndian>(crc).map_err(|e| WriteError::IO(e)));
    size += 2;

    Ok(size)
}

#[test]
fn serialize_ack_test() {
    use std::io::Cursor;

    let mut prn = prn_id::new(['K', 'I', '7', 'E', 'S', 'T', '0']).unwrap();
    let ack = new_ack(prn.next(), prn.callsign);

    let mut data = vec!();

    let count = to_bytes(&mut data, &Frame::Ack(ack.clone())).unwrap();
    assert!(count == 4 + 4 + 2);

    let mut reader = Cursor::new(data);
    match from_bytes(&mut reader, count).unwrap() {
        Frame::Ack(read_ack) => {
            assert!(read_ack.prn == ack.prn);
            assert!(read_ack.src_addr == ack.src_addr);
        }
        _ => assert!(false)
    }
}

#[cfg(test)]
use std::iter;

#[cfg(test)]
fn serialize_packet(dest: &[u32], payload: &[u8]) -> Vec<u8> {
    let mut prn = prn_id::new(['K', 'I', '7', 'E', 'S', 'T', '0']).unwrap();
    let data_packet = new_data(&mut prn, dest, payload.iter().cloned()).unwrap();

    let mut data = vec!();

    let count = to_bytes(&mut data, &Frame::Data(data_packet.clone())).unwrap();
    assert!(count == 4 + 4 * (1 + dest.len()) + payload.len() + 2);

    data
}

#[cfg(test)]
fn serialize_deserialize_packet(dest: &[u32], payload: &[u8]) {
    use std::io::Cursor;

    let data = serialize_packet(dest, payload);
    let count = data.len();

    let mut reader = Cursor::new(data);
    match from_bytes(&mut reader, count).unwrap() {
        Frame::Data(read_data) => {
            assert!(read_data.payload_size == payload.len());
            for (i, byte) in payload.iter().cloned().enumerate() {
                assert!(read_data.payload[i] == byte);
            }

            for (i, test_addr) in dest.iter().cloned().enumerate() {
                assert!(read_data.address_route[i] == test_addr);
            }
        },
        _ => assert!(false)
    }
}

#[test]
fn serialize_data_test() {
    use nbp::address;

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
    use nbp::address;

    for size in 1..15 {
        //Build address
        let src_addr = address::encode(['K', 'I', '7', 'E', 'S', 'T', '0']).unwrap();

        let mut addr: Vec<u32> = (0..size).into_iter()
            .map(|i| {
                if i > 9 {
                    ['T', 'E', 'S', 'T', address::symbol_to_character(i / 10), address::symbol_to_character(i % 10), '0']
                } else {
                    ['T', 'E', 'S', 'T', address::symbol_to_character(i), '0', '0']
                }
            })
            .filter_map(|addr| address::encode(addr))
            .chain(iter::once(routing::ADDRESS_SEPARATOR))
            .chain(iter::once(src_addr))
            .collect();

        for i in 0..size {
            let packet = [1, 2, 3, 4, 5];
            serialize_deserialize_packet(&addr, &packet);

            //Advance the route
            routing::advance(&mut addr);
            assert!(addr[(size - i) as usize - 1] == routing::ADDRESS_SEPARATOR);
        }
    }
}

#[test]
fn test_payload_permutations() {
    use nbp::address;

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
    use nbp::address;
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
            let count = data.len();

            {
                let mut reader = Cursor::new(&data);
                match from_bytes(&mut reader, count) {
                    Err(ReadError::CRCFailure) => (),
                    _ => assert!(false)
                }
            }

            //Restore the bit for the next run
            data[byte] ^= mask;
        }
    }
}