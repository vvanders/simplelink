pub mod kiss {
    ///Frame delimiter code, used to represent start and end of frames.
    const FEND: u8 = 0xC0;

    ///Frame escape code, used to escape FESC and FEND codes if they are found in byte stream
    const FESC: u8 = 0xDB;

    ///Escaped FEND value
    const TFEND: u8 = 0xDC;

    ///Escaped FESC value
    const TFESC: u8 = 0xDD;

    const CMD_DATA: u8 = 0x00;
    pub const CMD_TX_DELAY: u8 = 0x01;
    pub const CMD_PERSISTENCE: u8 = 0x02;
    pub const CMD_SLOT_TIME: u8 = 0x03;
    pub const CMD_TX_TAIL: u8 = 0x04;
    pub const CMD_DUPLEX: u8 = 0x05;
    pub const CMD_RETURN: u8 = 0xFF;

    pub fn encode<T>(data: T, encoded: &mut Vec<u8>, port: u8) where T: Iterator<Item=u8> {
        let (reserved, _) = data.size_hint();
        encoded.reserve(reserved + 3);

        let encode = data.map(|byte| {
            match byte {
                FEND => (FESC, Some(TFEND)),
                FESC => (FESC, Some(TFESC)),
                _ => (byte, None)
            }
        });

        encoded.push(FEND);

        //Data frame command, port is high part of the nibble
        encoded.push(CMD_DATA | ((port & 0x0F) << 4));

        for (b1, b2) in encode {
            encoded.push(b1);

            match b2 {
                Some(data) => encoded.push(data),
                _ => ()
            }
        }

        encoded.push(FEND);
    }

    pub fn encode_cmd(encoded: &mut Vec<u8>, cmd: u8, data: u8, port: u8) {
        encoded.push(FEND);

        match cmd {
            //Return uses 0xF0 since it impacts all ports
            CMD_RETURN => encoded.push(CMD_RETURN),
            //Port is high part of the nibble
            _ => {
                encoded.push(cmd | ((port & 0x0F) << 4));
                encoded.push(data);
            }
        }

        encoded.push(FEND);
    }

    fn find_frame(data: &[u8]) -> Option<(usize, usize)> {
        let (start, end) = data.iter()
            .enumerate()
            .fold((None,None), |(mut start, mut end), (idx, byte)| {
            //Looking for start of the frame
            if start.is_none() {
                if *byte == FEND {
                    start = Some(idx);
                }
            } else if end.is_none() {   //Looking for the end
                if *byte == FEND {
                    end = Some(idx);
                }
            }

            (start, end)
        });

        start.and_then(|begin| {
            end.and_then(|end| {
                //Found a valid frame, return it
                Some((begin, end))
            })
        })
    }

    pub struct DecodedFrame {
        pub port: u8,
        pub bytes_read: usize
    }

    pub fn decode(data: &[u8], decoded: &mut Vec<u8>) -> Option<DecodedFrame> {
        decoded.reserve(data.len());

        let (start, end) = match find_frame(data) {
            Some((s, e)) => (s, e),
            None => return None
        };

        let mut decode = data.iter().skip(start+1).take(end - start-1)
            .scan(false, |was_esc, byte| {
                let value = match *byte {
                    FESC => {
                        *was_esc = true;
                        None
                    },
                    _ => {
                        //If we were escaped then we need to look for our value
                        if *was_esc {
                            *was_esc = false;

                            match *byte {
                                TFEND => Some(FEND),
                                TFESC => Some(FESC),
                                _ => None   //This is a bad value, just discard the byte for now since we don't know how to handle it
                            }
                        } else {
                            Some(*byte)
                        }
                    }
                };

                Some(value)
            })
            .filter_map(|value| value);

        let cmd = match decode.next() {
            Some(byte) => byte,
            _ => return None
        };

        for byte in decode {
            decoded.push(byte);
        }

        Some(DecodedFrame {
            port: cmd >> 4,
            bytes_read: end - start + 1
        })
    }


    #[test]
    fn test_encode() {
        {
            let mut data = vec!();
            encode(['T', 'E', 'S', 'T'].iter().map(|chr| *chr as u8), &mut data, 0);
            assert!(data == vec!(FEND, CMD_DATA, 'T' as u8, 'E' as u8, 'S' as u8, 'T' as u8, FEND));
        }

        {
            let mut data = vec!();
            encode(['H', 'E', 'L', 'L', 'O'].iter().map(|chr| *chr as u8), &mut data, 5);
            assert!(data == vec!(FEND, CMD_DATA | 0x50, 'H' as u8, 'E' as u8, 'L' as u8, 'L' as u8, 'O' as u8, FEND));
        }

        {
            let mut data = vec!();
            encode([FEND, FESC].iter().map(|data| *data), &mut data, 0);
            assert!(data == vec!(FEND, CMD_DATA, FESC, TFEND, FESC, TFESC, FEND));
        }

        {
            let mut data = vec!();
            encode_cmd(&mut data, CMD_TX_DELAY, 4, 0);
            assert!(data == vec!(FEND, CMD_TX_DELAY, 0x04, FEND));
        }

        {
            let mut data = vec!();
            encode_cmd(&mut data, CMD_TX_DELAY, 4, 6);
            assert!(data == vec!(FEND, CMD_TX_DELAY | 0x60, 0x04, FEND));
        }

        {
            let mut data = vec!();
            encode_cmd(&mut data, CMD_RETURN, 4, 2);
            assert!(data == vec!(FEND, CMD_RETURN, FEND));
        }
    }

    #[cfg(test)]
    fn test_encode_decode_single<T>(source: T) where T: Iterator<Item=u8> {
        let mut data = vec!();
        let mut decoded = vec!();
        let expected: Vec<u8> = source.collect();

        encode(expected.iter().map(|x| *x), &mut data, 5);
        match decode(&data, &mut decoded) {
            Some(result) => {
                assert!(result.port == 5);
                assert!(result.bytes_read == data.len());
                assert!(expected == decoded);
            },
            None => assert!(false)
        }
    }

    #[test]
    fn test_encode_decode() {
        test_encode_decode_single(['T', 'E', 'S', 'T'].iter().map(|chr| *chr as u8));
        test_encode_decode_single(['H', 'E', 'L', 'L', 'O'].iter().map(|chr| *chr as u8));
        test_encode_decode_single([FEND, FESC].iter().map(|data| *data));
    }
}

