//! NBP Frame management
use std::io;
use byteorder::{ReadBytesExt, WriteBytesExt, BigEndian};
use nbp::crc16;

pub const MTU: usize = 1500;

/// Represents a single NBP Ack Frame
pub struct AckFrame {
    /// Psuedo-Random unique identifier that this packet is an ack for.
    pub prn: u32,
    /// Source station that acknowledged the packet.
    pub src_addr: u32,
    /// CRC to verify integrity.
    pub crc: u16
}

/// Represents a single NBP Data Frame
pub struct DataFrame {
    /// Psuedo-Random unique identifier for this packet. This is combination of PRN + XOR of callsign.
    pub prn: u32,
    /// Forward and return address routing. Each path can contain up to 16 addresses plus a single separator.
    pub address_route: [u32; 33],
    /// Payload data. MTU is 1500 so stop there
    pub payload: [u8; MTU],
    /// Payload size
    pub payload_size: usize,
    /// CRC to verify integrity.
    pub crc: u16
}

/// All possible NBP frames
pub enum Frame {
    Data(DataFrame),
    Ack(AckFrame)
}

/// Error cases for converting from raw bytes to a frame.
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

/// Error cases for converting from a frame to raw bytes.
pub enum WriteError {
    /// IO error occured while writing.
    IO(io::Error)
}

fn read_u32<T>(bytes: &mut T, crc: &mut crc16::CRC) -> Result<u32, ReadError> where T: io::Read {
    let value = try!(bytes.read_u32::<BigEndian>().map_err(|e| ReadError::IO(e)));
    *crc = crc16::update_u32(value, *crc);

    Ok(value)
}

/// Read in a frame from a series of bytes.
pub fn from_bytes<T>(bytes: &mut T, size: usize) -> Result<Frame, ReadError> where T: io::Read {
    let mut crc = crc16::new();

    //All frames start with PRN
    let prn = try!(read_u32(bytes, &mut crc));

    //If we have just a PRN, addr and CRC this is an ack frame
    let frame = if size == 4 + 4 + 2 {
        let addr = try!(read_u32(bytes, &mut crc));

        Frame::Ack(AckFrame {
            prn: prn,
            src_addr: addr,
            crc: crc
        })
    } else {
        //Scan in our address. We're looking for u32+, 0x0, u32+, 0x0.
        let mut addr_marker = 0;
        let mut addr = [0; 33];
        let mut addr_len = 0;

        for _ in 0..34 {
            let value = try!(read_u32(bytes, &mut crc));

            if value == 0 {
                addr_marker += 1;
            }

            addr[addr_len] = value;
            addr_len += 1;

            if addr_marker == 2 {
                break;
            }
        }

        //If we saw 33 values that means that the 34th one must be a 0x0 separator, otherwise this is malformed
        if addr_len == 33 && addr_marker != 2 {
            let value = try!(read_u32(bytes, &mut crc));
            addr_len += 1;

            if value != 0 {
                return Err(ReadError::BadAddress)
            }
        }

        //size - (PRN + ADDR size + CRC)
        let payload_size = size - (4 + addr_len * 4 - 2);

        let mut payload = [0; 1500];
        try!(bytes.read(&mut payload[..payload_size]).map_err(|e| ReadError::IO(e)));

        //Update CRC
        crc = payload.iter().fold(crc, |crc, byte| {
            crc16::update_u8(*byte, crc)
        });

        Frame::Data(DataFrame{
            prn: prn,
            address_route: addr,
            payload: payload,
            payload_size: payload_size,
            crc: crc
        })
    };

    crc = crc16::finish(crc);

    //Validate our CRC
    let frame_crc = try!(bytes.read_u16::<BigEndian>().map_err(|e| ReadError::IO(e)));

    if frame_crc != crc {
        return Err(ReadError::CRCFailure)
    }

    Ok(frame)
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
                if *addr == 0x0 {
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
                size += try!(write_u32(0x0, bytes, &mut crc));
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

            //Only include this stations callsign since we need that to comply with FCC Part 97. If our last trasmission is an ACK it must include our callsign
            size += try!(write_u32(ack_frame.src_addr, bytes, &mut crc));
        }
    }

    //Last part of the packet is our CRC
    crc = crc16::finish(crc);

    try!(bytes.write_u16::<BigEndian>(crc).map_err(|e| WriteError::IO(e)));
    size += 2;

    Ok(size)
}