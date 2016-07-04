///! Address routing functions
use nbp::address;

///Separater value to determine where we are in the routing path
pub const ADDRESS_SEPARATOR: u32 = 0x0;

///Advances the source -> dest separator in a route by one, returns false if no separator was found
pub fn advance(route: &mut [u32]) -> bool {
    let sep_idx = route.iter().enumerate().fold(None, |found, (i, value)| {
        if found.is_some() {
            return found
        }

        match value {
            &ADDRESS_SEPARATOR => Some(i),
            _ => None
        }
    });

    match sep_idx {
        Some(idx) => {
            if idx > 0 {
                route.swap(idx, idx-1);
                true
            } else {
                false
            }
        },
        _ => false
    }
}

/// Decodes a route with the format CALLSIGN1 -> CALLSIGN2 -> etc
pub fn format_route(route: [u32; 17]) -> String {
    route.into_iter().cloned()
        .filter(|addr| *addr != ADDRESS_SEPARATOR)
        .map(|addr| address::format_addr(addr))
        .fold(String::new(), |route, addr| {
            if route.len() > 0 {
                route + " -> " + addr.as_str()
            } else {
                addr
            }
        })
}
