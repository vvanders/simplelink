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

    pub struct DecodedFrame {
        pub port: u8,
        pub bytes_read: usize
    }

    //@todo: Use a drain iterator here instead so we can remove + process in one pass
    pub fn decode<T>(data: T, decoded: &mut Vec<u8>) -> Option<DecodedFrame> where T: Iterator<Item=u8> {
        let (reserved, _) = data.size_hint();
        decoded.reserve(reserved);

        let (_, port, last_idx) = data.enumerate()    //Keep track of idx so we can return the last idx we processed to the caller
            //Find our first valid start + end frame
            .scan((None, None), |&mut (ref mut start_frame, ref mut end_frame), (idx, byte)| {
                //If we've already found a valid range then stop iterating
                if start_frame.is_some() && end_frame.is_some() {
                    None
                } else {
                    let value =
                        //Looking for start of the frame
                        if start_frame.is_none() {
                            if byte == FEND {
                                *start_frame = Some(idx);
                                Some((idx, byte))
                            } else {
                                None
                            }
                        } else {   //Looking for the end
                            if byte == FEND {
                                //Empty frame, just restart the scan
                                if start_frame.unwrap()+1 == idx {
                                    *start_frame = Some(idx);
                                } else {
                                    *end_frame = Some(idx);
                                }
                            }

                            Some((idx, byte))
                        };

                    Some(value)
                }
            })
            //Filter out any empty frames or data we don't want to process
            .filter_map(|x| {
                x.and_then(|(idx, value)| {
                    match value {
                        FEND => None,   //Don't include frame delimiters
                        _ => Some((idx, value))
                    }
                })
            })
            //Decode escaped values
            .scan(false, |was_esc, (idx, byte)| {
                let value = if byte == FESC {
                    *was_esc = true;
                    None    //Don't include escaped characters
                } else if *was_esc {
                    *was_esc = false;
                    
                    match byte {
                        TFEND => Some((idx, FEND)),
                        TFESC => Some((idx, FESC)),
                        _ => None //This is a bad value, just discard the byte for now since we don't know how to handle it
                    }
                } else {
                    Some((idx, byte))
                };

                Some(value)
            })
            .filter_map(|x| x)  //Skip things we don't want
            //Decode frame into output buffer
            .fold((decoded, None, None), |(out_decode, mut port, _), (idx, byte)| {
                //If we've already defined the port that means we're on the data part of the frame
                if port.is_some() {
                    out_decode.push(byte);
                } else {    //First byte is cmd + port, cmd should always be data(0x00)
                    port = Some(byte >> 4);
                }

                (out_decode, port, Some(idx))
            });

        //Check if we found anything
        port.and_then(|port| {
            last_idx.and_then(|idx| {
                Some(DecodedFrame {
                    port: port,
                    bytes_read: idx+2   //Note that since we truncate the FEND we need to add an extra offset here
                })
            })
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
        match decode(data.iter().cloned(), &mut decoded) {
            Some(result) => {
                assert!(result.port == 5);
                assert!(result.bytes_read == data.len());
                assert!(expected == decoded);
            },
            None => assert!(false)
        }
    }

    #[cfg(test)]
    fn test_decode_single<T>(data: &mut Vec<u8>, expected: T, port: u8) where T: Iterator<Item=u8> {
        let mut decoded = vec!();

        match decode(data.iter().cloned(), &mut decoded) {
            Some(result) => {
                assert!(result.port == port);
                assert!(expected.eq(decoded.into_iter()));

                //Remove the data so subsequent reads work
                data.drain(0..result.bytes_read);
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

    #[test]
    fn test_empty_frame() {
        let mut data = vec!();
        let expected: Vec<u8> = ['T', 'E', 'S', 'T'].iter().map(|chr| *chr as u8).collect();

        data.push(FEND);
        data.push(FEND);
        data.push(FEND);

        encode(expected.iter().cloned(), &mut data, 0);
        
        let mut decoded = vec!();
        match decode(data.iter().cloned(), &mut decoded) {
            Some(result) => {
                assert!(result.bytes_read == data.len());
                assert!(result.port == 0);

                assert!(expected.iter().cloned().eq(decoded.into_iter()));
            },
            None => assert!(false)
        }
    }

    #[test]
    fn test_multi_frame() {
        let expected_one: Vec<u8> = ['T', 'E', 'S', 'T'].iter().map(|chr| *chr as u8).collect();
        let expected_two: Vec<u8> = ['H', 'E', 'L', 'L', 'O'].iter().map(|chr| *chr as u8).collect();
        let expected_three = [FEND, FESC];

        let mut data = vec!();

        encode(expected_one.iter().cloned(), &mut data, 0);
        encode(expected_two.iter().cloned(), &mut data, 0);
        encode(expected_three.iter().cloned(), &mut data, 0);

        test_decode_single(&mut data, expected_one.iter().cloned(), 0);
        test_decode_single(&mut data, expected_two.iter().cloned(), 0);
        test_decode_single(&mut data, expected_three.iter().cloned(), 0);
    }
}

