//! Psuedo-Random number generator for packet identification

/// Psuedo-Random packet id generation
pub struct PRN {
    /// Current PRN value
    pub current: u32,
    /// Callsign that is used to XOR for a unique sequence
    pub callsign: u32
}

pub fn new(callsign: [char; 7]) -> Option<PRN> {
    use nbp::address;

    address::encode(callsign).map(|addr| {
        PRN {
            current: 0xFFFFFFFF,
            callsign: addr
        }
    })
}

impl PRN {
    /// Generates a new PRN value from the previous PRN value.
    pub fn next(&mut self) -> u32 {
        //NBP uses a 4-tap poly in the form of 1 + x^25 + x^26 + x^30 + x^32
        let bit = ((self.current >> (32-25)) ^ (self.current >> (32-26)) ^ (self.current >> (32-30)) ^ (self.current >> (32-32))) & 0x1;

        //Shift every bit down, insert newly generated bit at the top
        self.current = (self.current >> 1) | (bit << 31);

        //Make sure to return a unique id by XORing with callsign
        self.current()
    }

    /// Gets the current value of the PRN.
    pub fn current(&self) -> u32 {
        self.current ^ self.callsign
    }

    /// Seeds the PRN with a new start value
    pub fn seed(&mut self, seed: u32) {
        self.current = seed;
    }
}

#[test]
fn test_unique() {
    use nbp::prn_id;

    const SAMPLE_SIZE: usize = 2048;

    let mut table: [u32; SAMPLE_SIZE] = [0; SAMPLE_SIZE];
    let mut prn = match prn_id::new(['K', 'I' ,'7', 'E', 'S', 'T', '0']) {
        Some(s) => s,
        None => {
            assert!(false);
            return
        }
    };

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
    use nbp::prn_id;

    let mut prn_first = match prn_id::new(['K', 'I' ,'7', 'E', 'S', 'T', '0']) {
        Some(s) => s,
        None => {
            assert!(false);
            return
        }
    };

    let mut prn_second = match prn_id::new(['K', 'F' ,'7', 'S', 'J', 'K', '0']) {
        Some(s) => s,
        None => {
            assert!(false);
            return
        }
    };

    for _ in 0..1024 {
        assert!(prn_first.next() != prn_second.next());
    }
}

#[test]
fn test_seed() {
    use nbp::prn_id;

    let mut prn = match prn_id::new(['K', 'I' ,'7', 'E', 'S', 'T', '0']) {
        Some(s) => s,
        None => {
            assert!(false);
            return
        }
    };

    const SEED: u32 = 0xFF123456; 
    prn.seed(SEED);

    let initial: Vec<u32> = (0..1024).map(|_| prn.next()).collect();
    let different: Vec<u32> = (0..1024).map(|_| prn.next()).collect();

    prn.seed(SEED);

    let repeat: Vec<u32> = (0..1024).map(|_| prn.next()).collect();

    assert!(initial == repeat);
    assert!(initial != different);
    assert!(repeat != different);
}