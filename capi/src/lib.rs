extern crate libc;
#[macro_use]
extern crate log;
extern crate simplelink;

mod echo;

use std::io;
use std::ffi;

pub trait ReadWrite: io::Write + io::Read {}
impl<T> ReadWrite for T where T: io::Write + io::Read {}

pub struct Link {
    link: simplelink::spec::node::Node,

    rx_tx: Option<Box<ReadWrite>>,

    recv_callback: Option<extern "C" fn(*const u32, u32, *const u8, usize)>,
    ack_callback: Option<extern "C" fn(*const u32, u32)>,
    expire_callback: Option<extern "C" fn(u32)>,
    retry_callback: Option<extern "C" fn(u32, u32)>,
    observe_callback: Option<extern "C" fn(*const u32, u32, *const u8, usize)>,

    recv_box_cb: Option<Box<Fn([u32; simplelink::spec::routing::MAX_LENGTH], u32, &[u8])>>,
    ack_box_cb: Option<Box<Fn([u32; simplelink::spec::routing::MAX_LENGTH], u32)>>,
    expire_box_cb: Option<Box<Fn(u32)>>,
    retry_box_cb: Option<Box<Fn(u32, usize)>>,
    observe_box_cb: Option<Box<Fn([u32; simplelink::spec::routing::MAX_LENGTH], u32, &[u8])>>,
}

#[no_mangle]
pub unsafe extern "C" fn new(callsign: u32) -> *mut Link {
    simplelink::util::init_log(log::LogLevelFilter::Trace);

    new_nolog(callsign)
}

#[no_mangle]
pub unsafe extern "C" fn new_nolog(callsign: u32) -> *mut Link {
    let boxed = Box::new(Link {
        link: simplelink::spec::node::new(callsign),
        rx_tx: None,
        recv_callback: None,
        ack_callback: None,
        expire_callback: None,
        retry_callback: None,
        observe_callback: None,

        recv_box_cb: None,
        ack_box_cb: None,
        expire_box_cb: None,
        retry_box_cb: None,
        observe_box_cb: None
    });

    Box::into_raw(boxed)
}

pub unsafe fn set_rx_tx(link: *mut Link, rx_tx: Box<ReadWrite>) {
    (*link).rx_tx = Some(rx_tx);
}


#[no_mangle]
pub unsafe extern "C" fn open_loopback(link: *mut Link) -> bool {
    (*link).rx_tx = Some(Box::new(echo::new()));

    trace!("Opened loopback port");

    true
}

#[no_mangle]
pub unsafe extern "C" fn close(link: *mut Link) {
    (*link).rx_tx = None
}

#[no_mangle]
pub unsafe extern "C" fn tick(link: *mut Link, elapsed: usize) -> bool {
    match (*link).rx_tx {
        Some(ref mut rx_tx) => {
            match (*link).link.recv(rx_tx, 
                    |frame,data| {
                        if data.len() != 0 {
                            match (*link).recv_callback {
                                Some(recv) => recv(frame.address_route.as_ptr(), frame.prn, data.as_ptr(), data.len()),
                                None => match (*link).recv_box_cb {
                                    Some(ref recv) => recv(frame.address_route, frame.prn, data),
                                    None => ()
                                }
                            }
                        } else {
                            match (*link).ack_callback {
                                Some(ack) => ack(frame.address_route.as_ptr(), frame.prn),
                                None => match (*link).ack_box_cb {
                                    Some(ref ack) => ack(frame.address_route, frame.prn),
                                    None => ()
                                }
                            }
                        }
                    },
                    |frame,data| {
                       match (*link).observe_callback {
                            Some(obs) => obs(frame.address_route.as_ptr(), frame.prn, data.as_ptr(), data.len()),
                            None => match (*link).observe_box_cb {
                                Some(ref obs) => obs(frame.address_route, frame.prn, data),
                                None => ()
                            }
                        }
                    }) {
                Ok(()) => (),
                Err(e) => {
                    trace!("Error recieving {:?}", e);
                    return false
                }
            }

            match (*link).link.tick(rx_tx, elapsed, 
                    |frame, _, next_retry| {
                        match (*link).retry_callback {
                            Some(retry) => retry(frame.prn, next_retry as u32),
                            None => match (*link).retry_box_cb {
                                Some(ref retry) => retry(frame.prn, next_retry),
                                None => ()
                            }
                        }
                    },
                    |frame,_| {
                        match (*link).expire_callback {
                            Some(expire) => expire(frame.prn),
                            None => match (*link).expire_box_cb {
                                Some(ref expire) => expire(frame.prn),
                                None => ()
                            }
                        }
                    }) {
                Ok(()) => (),
                Err(e) => {
                    trace!("Error updating {:?}", e);
                    return false
                }
            }
        },
        None => ()
    }

    true
}

#[no_mangle]
pub unsafe extern "C" fn send(link: *mut Link, dest: *const u32, data: *const u8, size: usize) -> u32 {
    match (*link).rx_tx {
        Some(ref mut rx_tx) => {
            let route = std::slice::from_raw_parts(dest, 15).iter().cloned()
                .filter(|addr| *addr != 0);

            match (*link).link.send_slice(std::slice::from_raw_parts(data, size), route, rx_tx) {
                Ok(prn) => prn,
                Err(e) => {
                    trace!("Error sending {:?}", e);
                    0
                }
            }
        },
        None => 0
    }
}

#[no_mangle]
pub unsafe extern "C" fn release(link: *mut Link) {
    Box::from_raw(link);
}

#[no_mangle]
pub unsafe extern "C" fn set_recv_callback(link: *mut Link, callback: extern "C" fn(*const u32, u32, *const u8, usize)) {
    (*link).recv_callback = Some(callback);
}

#[no_mangle]
pub unsafe extern "C" fn set_ack_callback(link: *mut Link, callback: extern "C" fn(*const u32, u32)) {
    (*link).ack_callback = Some(callback);
}

#[no_mangle]
pub unsafe extern "C" fn set_expire_callback(link: *mut Link, callback: extern "C" fn(u32)) {
    (*link).expire_callback = Some(callback);
}

#[no_mangle]
pub unsafe extern "C" fn set_retry_callback(link: *mut Link, callback: extern "C" fn(u32, u32)) {
    (*link).retry_callback = Some(callback);
}

#[no_mangle]
pub unsafe extern "C" fn set_observe_callback(link: *mut Link, callback: extern "C" fn(*const u32, u32, *const u8, usize)) {
    (*link).observe_callback = Some(callback);
}

pub unsafe fn set_recv_box_cb<T>(link: *mut Link, callback: T) where T: Fn([u32; simplelink::spec::routing::MAX_LENGTH], u32, &[u8]) + 'static {
    (*link).recv_box_cb = Some(Box::new(callback))
}

pub unsafe fn set_ack_box_cb<T>(link: *mut Link, callback: T) where T: Fn([u32; simplelink::spec::routing::MAX_LENGTH], u32) + 'static {
    (*link).ack_box_cb = Some(Box::new(callback))
}

pub unsafe fn set_expire_box_cb<T>(link: *mut Link, callback: T) where T: Fn(u32) + 'static {
    (*link).expire_box_cb = Some(Box::new(callback))
}

pub unsafe fn set_retry_box_cb<T>(link: *mut Link, callback: T) where T: Fn(u32, usize) + 'static {
    (*link).retry_box_cb = Some(Box::new(callback))
}

pub unsafe fn set_observe_box_cb<T>(link: *mut Link, callback: T) where T: Fn([u32; simplelink::spec::routing::MAX_LENGTH], u32, &[u8]) + 'static {
    (*link).observe_box_cb = Some(Box::new(callback))
}

#[no_mangle]
pub unsafe extern "C" fn str_to_addr(addr: *const libc::c_char) -> u32 {
    let addr_str = match ffi::CStr::from_ptr(addr).to_str() {
        Ok(s) => s,
        Err(e) => {
            trace!("Unablet to convert addr {:?}", e);
            return 0
        }
    };

    let mut arr: [char; 7] = ['0'; 7];

    for (i, c) in addr_str.chars().take(7).enumerate() {
        arr[i] = c;
    }

    use simplelink::spec::address;
    match address::encode(arr) {
        Some(a) => a,
        None => 0
    }
}

#[no_mangle]
#[cfg(target_os = "android")]
pub unsafe extern "C" fn addr_to_str(addr: u32, out_str: *mut libc::c_char) {
    let decoded = simplelink::spec::address::decode(addr);

    for (i, chr) in decoded.iter().enumerate() {
        *out_str.offset(i as isize) = *chr as u8;
    }
}

#[no_mangle]
#[cfg(not(target_os = "android"))]
pub unsafe extern "C" fn addr_to_str(addr: u32, out_str: *mut libc::c_char) {
    let decoded = simplelink::spec::address::decode(addr);

    for (i, chr) in decoded.iter().enumerate() {
        *out_str.offset(i as isize) = *chr as i8;
    }
}