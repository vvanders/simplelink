//! Encodes address to/from NBP wire format

const SYMBOL_TABLE: [char; 36] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z'
];

/// Short form broadcast address(usually seen when decoded from the wire)
pub const BROADCAST_ADDRESS_SHORT: [char; 7] = ['3', 'Z', '1', '4', '1', 'Z', '1'];
/// Long form broadcast address
pub const BROADCAST_ADDRESS: [char; 7] = ['*'; 7];

fn symbol_to_character(symbol: u8) -> char {
    SYMBOL_TABLE[symbol as usize]
}

fn character_to_symbol(character: char) -> Option<u8> {
    match character {
        '0' => Some(0),
        '1' => Some(1),
        '2' => Some(2),
        '3' => Some(3),
        '4' => Some(4),
        '5' => Some(5),
        '6' => Some(6),
        '7' => Some(7),
        '8' => Some(8),
        '9' => Some(9),
        'A' => Some(10),
        'B' => Some(11),
        'C' => Some(12),
        'D' => Some(13),
        'E' => Some(14),
        'F' => Some(15),
        'G' => Some(16),
        'H' => Some(17),
        'I' => Some(18),
        'J' => Some(19),
        'K' => Some(20),
        'L' => Some(21),
        'M' => Some(22),
        'N' => Some(23),
        'O' => Some(24),
        'P' => Some(25),
        'Q' => Some(26),
        'R' => Some(27),
        'S' => Some(28),
        'T' => Some(29),
        'U' => Some(30),
        'V' => Some(31),
        'W' => Some(32),
        'X' => Some(33),
        'Y' => Some(34),
        'Z' => Some(35),
        _ => None
    }
}

/// Encodes a human readable address into a 32 bit wire format.
/// NBP uses a modulo-36 encoding supporting up to 7 characters.
/// Only the values [0-9], [A-Z] are supported.
///
/// # Examples
///
/// ```
/// use nbplink::nbp::address;
///
/// match address::encode(['S', '5', '3', 'M', 'V', '0', '0']) {
///     Some(value) => assert!(value == 53098624),
///     None => assert!(false)
/// }
/// 
/// ```
pub fn encode(address: [char; 7]) -> Option<u32> {
    //Special broadcast address
    if address == ['*'; 7] || address == BROADCAST_ADDRESS {
        Some(0xFFFFFFFF)
    } else {
        encode_rec(address, 0)
    }
}

fn encode_rec(address: [char; 7], offset: usize) -> Option<u32> {
    if offset == 6 {
        character_to_symbol(address[6]).map(|x| x as u32)
    } else {
        return encode_rec(address, offset + 1).and_then(|sub| {
            character_to_symbol(address[offset]).map(|sym| {
                sub * 36 + sym as u32
            })
        })
    }
}

/// Decodes an address from wire format into a human readable character array.
///
/// # Examples
///
/// ```
/// use nbplink::nbp::address;
///
/// assert!(address::decode(53098624) == ['S', '5', '3', 'M', 'V', '0', '0']);
/// ```
pub fn decode(address: u32) -> [char; 7] {
    (0..7).fold((['0'; 7], address), |(mut addr, remainder), i| {
        addr[i] = symbol_to_character((remainder % 36) as u8);

        (addr, remainder / 36)
    }).0
}

#[test]
fn encode_test() {
    match encode(['1', '0', '0', '0', '0', '0', '0']) {
        Some(value) => assert!(value == 1),
        None => assert!(false)
    }

    match encode(['1', '1', '0', '0', '0', '0', '0']) {
        Some(value) => assert!(value == 37),
        None => assert!(false)
    }

    match encode(['S', '5', '3', 'M', 'V', '0', '0']) {
        Some(value) => assert!(value == 53098624),
        None => assert!(false)
    }
}

#[test]
fn decode_test() {
    assert!(decode(1) == ['1', '0', '0', '0', '0', '0', '0']);
    assert!(decode(37) == ['1', '1', '0', '0', '0', '0', '0']);
    assert!(decode(53098624) == ['S', '5', '3', 'M', 'V', '0', '0']);
}

#[test]
fn encode_decode_test() {
    let addr1 = ['S', '5', '3', 'M', 'V', '0', '0'];
    let addr2 = ['1', '1', '0', '0', '0', '0', '0'];
    let addr3 = ['1', '0', '0', '0', '0', '0', '0'];

    assert!(decode(encode(addr1).unwrap_or(0)) == addr1);
    assert!(decode(encode(addr2).unwrap_or(0)) == addr2);
    assert!(decode(encode(addr3).unwrap_or(0)) == addr3);
}