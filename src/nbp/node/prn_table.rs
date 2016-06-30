///! Table for tracking recieved PRNs
use nbp::prn_id;

const TABLE_SIZE: usize = 1000;

///Table of last 1000 recieved PRNs
pub struct Table {
    prns: [prn_id::PrnValue; TABLE_SIZE],
    last_idx: usize
}

pub fn new() -> Table {
    Table {
        prns: [0; TABLE_SIZE],
        last_idx: 0
    }
}

impl Table {
    /// Adds a prn to the table
    pub fn add(&mut self, prn: prn_id::PrnValue) {
        self.prns[self.last_idx] = prn;

        self.last_idx += 1;

        if self.last_idx >= 1000 {
            self.last_idx = 0;
        }
    }

    /// Checks if a prn is contained within the table
    pub fn contains(&self, prn: prn_id::PrnValue) -> bool {
        self.prns.iter().any(|search| *search == prn)
    }
}

#[test]
fn test_contains() {
    let mut prn = prn_id::new(['K', 'I' ,'7', 'E', 'S', 'T', '0']).unwrap();
    let mut table = new();

    for _ in 0..TABLE_SIZE*2 {
        let prn_value = prn.next();
        table.add(prn_value);
        assert!(table.contains(prn_value));
    }
}

#[test]
fn test_last_1000() {
    let mut prn = prn_id::new(['K', 'I' ,'7', 'E', 'S', 'T', '0']).unwrap();
    let mut table = new();

    let first_prn = prn.next();
    table.add(first_prn);
    assert!(table.contains(first_prn));

    for _ in 0..TABLE_SIZE {
        table.add(prn.next());
    }

    assert!(!table.contains(first_prn));
}