///! Address routing functions
use nbp::address;

///Separater value to determine where we are in the routing path
pub const ADDRESS_SEPARATOR: u32 = 0x0;

///Address to broadcast to any link
pub const BROADCAST_ADDRESS: u32 = 0xFFFFFFFF;

/// Maximum amount of addresses in a route
pub const MAX_LENGTH: usize = 17;

///Route of a NBP packet, allows for 16 callsigns + separator to denote where the packet is in its routing
pub type Route = [u32; MAX_LENGTH];

pub enum ParseError {
    /// Route has a bad format
    BadFormat
}

/// Determines if a route has this node as it's current hop
pub fn is_destination(route: &Route, this_addr: u32) -> bool {
    route[0] == this_addr || route[0] == BROADCAST_ADDRESS
}

/// Check if this route should retry the current message
pub fn should_retry(route: &Route) -> bool {
    route[0] != BROADCAST_ADDRESS
}

/// Advances the route with our address(in case we had a broadcast address)
pub fn advance(route: &Route, this_addr: u32) -> Result<Route, ParseError> {
    let sep_idx = match route.iter().position(|addr| *addr == ADDRESS_SEPARATOR) {
        Some(idx) => idx,
        None => return Err(ParseError::BadFormat)
    };

    if sep_idx == 0 || sep_idx+1 == route.len() {
        trace!("No separator found for route");
        return Err(ParseError::BadFormat)
    }

    let mut new_route = *route;

    //Shift all addresses down by one
    for i in 0..sep_idx {
        new_route[i] = new_route[i+1];
    }

    //Add our address to the return route
    new_route[sep_idx] = this_addr;

    Ok(new_route)
}

/// Decodes a route with the format CALLSIGN1 -> CALLSIGN2 -> etc
pub fn format_route(route: &[u32; 17]) -> String {
    route.into_iter().cloned()
        //.filter(|addr| *addr != ADDRESS_SEPARATOR)
        .map(|addr| address::format_addr(addr))
        .fold(String::new(), |route, addr| {
            if route.len() > 0 {
                route + " -> " + addr.as_str()
            } else {
                addr
            }
        })
}

#[cfg(test)]
fn gen_test_addr(mut idx: u8) -> u32 {
    idx += 1;
    let addr = ['T', 'E', 'S', 'T', address::symbol_to_character(idx / 10), address::symbol_to_character(idx % 10), '0'];
    address::encode(addr).unwrap()
}

#[test]
fn test_routing() {
    use log;
    use util;
    util::init_log(log::LogLevelFilter::Trace);

    use std::iter;

    let self_addr = address::encode(['K', 'I', '7', 'E', 'S', 'T', '0']).unwrap();
    let mut route = [0; MAX_LENGTH];
    let route_iter = (0..14).map(|i| gen_test_addr(i))
        .chain(iter::once(self_addr))
        .chain(iter::once(ADDRESS_SEPARATOR))
        .chain(iter::once(self_addr));

    for (i, addr) in route_iter.into_iter().enumerate() {
        route[i] = addr;
    }

    //Try each of our 15 hops
    for i in 0..15 {
        let sep_idx = 15-i;
        assert_eq!(route[sep_idx], ADDRESS_SEPARATOR);

        let dest_size: usize = 15-i;
        let src_size: usize = i+1;

        for dest in 0..dest_size {
            let expect = if dest == dest_size-1 {
                self_addr
            } else {
                gen_test_addr((dest+i) as u8)
            };

            assert_eq!(expect, route[dest]);
        }

        //If we advance we always replace with our address
        for src in 0..src_size {
            assert_eq!(self_addr, route[sep_idx+1 + src]);
        }

        route = advance(&route, self_addr).ok().unwrap();
    }
}