extern crate serial;
extern crate libc;
extern crate slink;

use std::ffi;

#[no_mangle]
pub unsafe extern "C" fn open_port(link: *mut slink::Link, port: *const libc::c_char, baud: usize) -> bool {
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