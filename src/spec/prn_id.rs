//! Pseudo-Random number generator for packet identification

/// Pseudo-Random packet id generation
pub struct PRN {
    /// Current PRN value
    pub current: u32,
    /// Callsign that is used to XOR for a unique sequence
    pub callsign: u32
}

/// Value type for actual prn values
pub type PrnValue = u32;

/// Creates new PRN id from an existing callsign
pub fn new(callsign: u32) -> PRN {
    PRN {
        current: 0xFFFFFFFF,
        callsign: callsign
    }
}

impl PRN {
    /// Generates a new packet id value from the previous packet id.
    pub fn next(&mut self) -> PrnValue {
        //use a 4-tap poly in the form of 1 + x^25 + x^26 + x^30 + x^32
        let bit = ((self.current >> (32-25)) ^ (self.current >> (32-26)) ^ (self.current >> (32-30)) ^ (self.current >> (32-32))) & 0x1;

        //Shift every bit down, insert newly generated bit at the top
        self.current = (self.current >> 1) | (bit << 31);

        //Make sure to return a unique id by XORing with callsign
        self.current()
    }

    /// Gets the current packet id.
    pub fn current(&self) -> PrnValue {
        self.current ^ self.callsign
    }

    /// Seeds the PRN with a new start value
    pub fn seed(&mut self, seed: PrnValue) {
        self.current = seed;
    }
}

#[cfg(test)]
use spec::address;

#[test]
fn test_unique() {
    use spec::prn_id;

    const SAMPLE_SIZE: usize = 2048;

    let mut table: [u32; SAMPLE_SIZE] = [0; SAMPLE_SIZE];
    let mut prn = prn_id::new(address::encode(['K', 'I' ,'7', 'E', 'S', 'T', '0']).unwrap());

    for i in 0..SAMPLE_SIZE {
        table[i] = prn.current();
        prn.next();
    }

    for id in table.iter() {
        assert!(table.iter().any(|test| test == id));
    }
}

#[test]
fn test_unique_seq() {
    use spec::prn_id;

    let mut prn_first = prn_id::new(address::encode(['K', 'I' ,'7', 'E', 'S', 'T', '0']).unwrap());
    let mut prn_second = prn_id::new(address::encode(['K', 'F' ,'7', 'S', 'J', 'K', '0']).unwrap());

    for _ in 0..1024 {
        assert!(prn_first.next() != prn_second.next());
    }
}

#[test]
fn test_seed() {
    use spec::prn_id;

    let mut prn = prn_id::new(address::encode(['K', 'I' ,'7', 'E', 'S', 'T', '0']).unwrap());

    const SEED: u32 = 0xFF123456; 
    prn.seed(SEED);

    let initial: Vec<u32> = (0..1024).map(|_| prn.next()).collect();
    let different: Vec<u32> = (0..1024).map(|_| prn.next()).collect();

    prn.seed(SEED);

    let repeat: Vec<u32> = (0..1024).map(|_| prn.next()).collect();

    assert_eq!(initial, repeat);
    assert!(initial != different);
    assert!(repeat != different);
}