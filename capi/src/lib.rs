extern crate libc;
extern crate serial;
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
    retry_callback: Option<extern "C" fn(u32)>,
    observe_callback: Option<extern "C" fn(*const u32, u32, *const u8, usize)>,
}

#[no_mangle]
pub unsafe extern "C" fn new(callsign: u32) -> *mut Link {
    simplelink::util::init_log(log::LogLevelFilter::Trace);

    let boxed = Box::new(Link {
        link: simplelink::spec::node::new(callsign),
        rx_tx: None,
        recv_callback: None,
        ack_callback: None,
        expire_callback: None,
        retry_callback: None,
        observe_callback: None
    });

    Box::into_raw(boxed)
}

#[no_mangle]
pub unsafe extern "C" fn open_port(link: *mut Link, port: *const libc::c_char, baud: usize) -> bool {
    let port_str = match ffi::CStr::from_ptr(port).to_str() {
        Ok(p) => p,
        Err(e) => {
            println!("Error converting port name {:?}", e);
            return false
        }
    };

    use serial::SerialPort;
    use std::time::Duration;

    let mut port = match serial::open(port_str) {
        Ok(p) => p,
        Err(e) => {
            println!("Unable to open serial port {:?}", e);
            return false
        }
    };

    let reconfigure = port.reconfigure(&|settings| {
        if baud != 0 {
            let enum_baud = match baud {
                110 => serial::Baud110,
                600 => serial::Baud600,
                1200 => serial::Baud1200,
                2400 => serial::Baud2400,
                4800 => serial::Baud4800,
                9600 => serial::Baud9600,
                19200 => serial::Baud19200,
                38400 => serial::Baud38400,
                57600 => serial::Baud57600,
                115200 => serial::Baud115200,
                n => serial::BaudOther(n)
            };

            try!(settings.set_baud_rate(enum_baud));
       }
       Ok(())
    });

    match reconfigure {
        Ok(()) => (),
        Err(e) => {
            println!("Unable to configure port {:?}", e);
            return false
        }
    }

    //Return immediately
    match port.set_timeout(Duration::from_millis(1)) {
        Ok(()) => (),
        Err(e) => {
            println!("Error setting timeout {:?}", e);
            return false
        }
    }

    (*link).rx_tx = Some(Box::new(port));

    println!("Opened serial port {}", port_str);

    true
}

#[no_mangle]
pub unsafe extern "C" fn open_loopback(link: *mut Link) -> bool {
    (*link).rx_tx = Some(Box::new(echo::new()));

    println!("Opened loopback port");

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
                                None => ()
                            }
                        } else {
                            match (*link).ack_callback {
                                Some(ack) => ack(frame.address_route.as_ptr(), frame.prn),
                                None => ()
                            }
                        }
                    },
                    |frame,data| {
                       match (*link).observe_callback {
                            Some(obs) => obs(frame.address_route.as_ptr(), frame.prn, data.as_ptr(), data.len()),
                            None => ()
                        }
                    }) {
                Ok(()) => (),
                Err(e) => {
                    println!("Error recieving {:?}", e);
                    return false
                }
            }

            match (*link).link.tick(rx_tx, elapsed, 
                    |frame,_| {
                        match (*link).retry_callback {
                            Some(retry) => retry(frame.prn),
                            None => ()
                        }
                    },
                    |frame,_| {
                        match (*link).expire_callback {
                            Some(retry) => retry(frame.prn),
                            None => ()
                        }
                    }) {
                Ok(()) => (),
                Err(e) => {
                    println!("Error updating {:?}", e);
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
            match (*link).link.send_slice(std::slice::from_raw_parts(data, size), std::slice::from_raw_parts(dest, 15).iter().cloned(), rx_tx) {
                Ok(prn) => prn,
                Err(e) => {
                    println!("Error sending {:?}", e);
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
pub unsafe extern "C" fn set_retry_callback(link: *mut Link, callback: extern "C" fn(u32)) {
    (*link).retry_callback = Some(callback);
}

#[no_mangle]
pub unsafe extern "C" fn set_observe_callback(link: *mut Link, callback: extern "C" fn(*const u32, u32, *const u8, usize)) {
    (*link).observe_callback = Some(callback);
}

#[no_mangle]
pub unsafe extern "C" fn str_to_addr(addr: *const libc::c_char) -> u32 {
    let addr_str = match ffi::CStr::from_ptr(addr).to_str() {
        Ok(s) => s,
        Err(e) => {
            println!("Unablet to convert addr {:?}", e);
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
pub unsafe extern "C" fn addr_to_str(addr: u32, out_str: *mut libc::c_char) {
    let decoded = simplelink::spec::address::decode(addr);

    for (i, chr) in decoded.iter().enumerate() {
        *out_str.offset(i as isize) = *chr as i8;
    }
}