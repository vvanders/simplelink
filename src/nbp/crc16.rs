//! CRC-CCITT16 implemenation for packet integrity verification from http://srecord.sourceforge.net/crc16-ccitt.html

const CRC_POLY: u16 = 0x1021;

pub type CRC = u16;

/// Calculate a CRC on an iterator of data.
///
/// # Examples
/// ```
/// use nbplink::nbp::crc16;
/// //Generate some data
/// let mut data: Vec<u8> = (0..32)
///     .flat_map(|i| {
///         [
///             i as u8,
///             (i >> 8) as u8,
///             (i >> 16) as u8,
///             (i >> 24) as u8
///         ].into_iter().cloned().collect::<Vec<u8>>()
///     })
///     .collect();
///
/// //Caclulate base CRC
/// let crc = crc16::calc(data.iter().cloned());
///
/// //Flip a bit
/// data[0] ^= 1 << 4;
///
/// //Different CRC
/// assert!(crc != crc16::calc(data.iter().cloned()));
/// ```
pub fn calc<T>(data: T) -> CRC where T: Iterator<Item=u8> {
    let crc = data.fold(new(), |calc, byte| {
        update_u8(byte, calc)
    });

    finish(crc)
}

/// Create a new CRC value
pub fn new() -> CRC {
    0xFFFF
}

/// Process 32 bits of data for CRC
pub fn update_u32(int: u32, mut crc: CRC) -> CRC {
    let bytes = [
        (int >> 24) as u8,
        (int >> 16) as u8,
        (int >> 8) as u8,
        int as u8
    ];

    for byte in &bytes {
        crc = update_u8(*byte, crc);
    }

    crc
}

/// Process 8 bits of data for CRC
pub fn update_u8(byte: u8, mut crc: CRC) -> CRC {
    let mut bit = 0x80; //Highest bit of 8-bit value;

    for _ in 0..8 {
        let xor_flag = (crc & 0x8000) == 0x8000;

        crc = crc << 1;

        if byte & bit == bit {
            crc += 1;
        }

        if xor_flag {
            crc ^= CRC_POLY;
        }

        bit >>= 1;
    }

    crc
}


/// Finish calculating a CRC
pub fn finish(mut crc: CRC) -> CRC {
    for _ in 0..16 {
        let xor_flag = crc & 0x8000 == 0x8000;

        crc <<= 1;

        if xor_flag {
            crc ^= CRC_POLY;
        }
    }

    crc
}

#[test]
fn crc_test() {
    use nbp::prn_id;

    let mut prn = match prn_id::new(['K', 'I', '7', 'E', 'S', 'T', '0']) {
        Some(s) => s,
        None => {
            assert!(false);
            return
        }
    };

    const SAMPLES: usize = 128;

    //Generate SAMPLES bytes of random data
    let mut data: Vec<u8> = (0..SAMPLES).map(|_| prn.next())
        .flat_map(|id| {
            [
                id as u8,
                (id >> 8) as u8,
                (id >> 16) as u8,
                (id >> 24) as u8
            ].into_iter().cloned().collect::<Vec<u8>>()
        })
        .collect();

    //Caclulate base CRC
    let crc = calc(data.iter().cloned());

    for i in 0..SAMPLES*4 {
        for n in 0..8 {
            let bit = 1 << n;

            data[i] ^= bit;
            assert!(calc(data.iter().cloned()) != crc);
            data[i] ^= bit;

        } 
    }

    assert!(calc(data.iter().cloned()) == crc);
}

#[test]
fn crc_test_u32() {
    let bytes = [0x2, 0x5, 0x7, 0x9];

    let mut first_crc = new();
    for byte in &bytes {
        first_crc = update_u8(*byte, first_crc);
    }
    first_crc = finish(first_crc);

    let mut second_crc = new();
    second_crc = update_u32(((bytes[0] as u32) << 24) | ((bytes[1] as u32) << 16) | ((bytes[2] as u32) << 8) | (bytes[3] as u32), second_crc);
    second_crc = finish(second_crc);

    assert!(first_crc == second_crc);
}