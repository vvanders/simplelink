extern crate clap;
extern crate serial;
extern crate pdcurses;
extern crate nbplink;
#[macro_use]
extern crate log;
extern crate fern;
extern crate time;

mod echo;
mod display;

use std::time::Duration;
use std::io;
use std::iter;
use std::thread;

use nbplink::nbp::{frame, address, prn_id, routing};
use nbplink::kiss;

fn main() {
    //Parse command line arguments
    let matches = clap::App::new("NBPLink Command line interface")
        .version("0.1.0")
        .author("Val Vanderschaegen <valere.vanderschaegen@gmail.com>")
        .about("Command line interface for sending and recieving packets with the NBP protocol")
        .arg(clap::Arg::with_name("port")
            .required(true)
            .help("rs232 port of the TNC, ex: 'COM1', '/dev/ttyUSB0'"))
        .arg(clap::Arg::with_name("callsign")
            .required(true)
            .help("User's callsign, must be less than 7 letters+numbers, ex: 'KI7EST'"))
        .arg(clap::Arg::with_name("cmd")
            .short("c")
            .long("cmd")
            .multiple(true)
            .takes_value(true)
            .number_of_values(1)
            .help("Command to run before starting TNC link, can be specified multiple times, ex: '-c KISS -c RESTART'"))
        .arg(clap::Arg::with_name("baud")
            .short("b")
            .long("baud")
            .takes_value(true)
            .number_of_values(1)
            .help("Sets baud rate for rs232 serial port"))
        .arg(clap::Arg::with_name("echo")
            .short("e")
            .long("echo")
            .help("Enable echo mode, rs232 port is disabled and all data is echoed back to the client"))
        .arg(clap::Arg::with_name("debug")
            .short("d")
            .long("debug")
            .takes_value(true)
            .number_of_values(1)
            .help("Debug mode to enable, supports: Off|Error|Warn|Info|Debug|Trace, Default: Info"))
        .get_matches();

    {
        let debug = match matches.value_of("debug") {
            Some(debug) => debug,
            None => "Infp"
        };

        let filter = match debug.to_lowercase().as_str() {
            "off" => log::LogLevelFilter::Off,
            "error" => log::LogLevelFilter::Error,
            "warn" => log::LogLevelFilter::Warn,
            "info" => log::LogLevelFilter::Info,
            "debug" => log::LogLevelFilter::Debug,
            "trace" => log::LogLevelFilter::Trace,
            _ => log::LogLevelFilter::Error
        };

        nbplink::util::init_log(filter);
    }

    let port = matches.value_of_os("port").expect("No port specified");
    let callsign = matches.value_of("callsign").expect("No callsign specified");
    let baud = matches.value_of("baud").and_then(|baud| baud.parse::<usize>().map(|r| Some(r)).unwrap_or(None));

    let cmds = match matches.values_of("cmd") {
        Some(cmds) => cmds.collect::<Vec<&str>>(),
        None => vec!()
    };

    let echo = matches.is_present("echo");

    let mut echo_port = echo::new();
    let mut port = if !echo {
        Some(match configure_port(port, baud) {
            Ok(port) => port,
            Err(e) => {
                match e.kind() {
                    serial::ErrorKind::NoDevice => error!("Unable to open port, no device found for {:?}", port),
                    serial::ErrorKind::InvalidInput => error!("Unable to open port, {:?} is not a valid device name", port),
                    serial::ErrorKind::Io(io_e) => error!("Unable to open port, IO error: {:?}", io_e)
                }
                return
            }
        })
    } else {
        None
    };

    for cmd in cmds {
        let write_port: &mut io::Write = if echo {
            &mut echo_port
        } else {
            port.as_mut().unwrap()
        };
        
        let write_cmd = cmd.to_string() + "\n";

        use std::io::Write;
        match write_port.write_all(write_cmd.as_bytes()) {
            Ok(_) => info!("Sending '{}' to TNC", cmd),
            Err(e) => {
                error!("Unable to send '{}' to TNC {:?}", cmd, e);
            }
        }
    }

    let mut prn = match prn_id::new(string_to_addr(callsign)) {
        Some(prn) => prn,
        None => {
            println!("Unable to parse callsign, a valid callsign is up to seven characters containing A-Z, 0-9");
            return;
        }
    };

    let mut display = display::new();

    loop {
        let start_ms = time::precise_time_ns() / 1_000_000;

        let mut pending = vec!();
        let mut pending_bytes = 0;

        match display.get_input() {
            Some(input) => {
                match input.len() {
                    0 => (),
                    _ => {
                        let write_port: &mut io::Write = if echo {
                            &mut echo_port
                        } else {
                            port.as_mut().unwrap()
                        };

                        match send_frame(&mut prn, &input, write_port) {
                            Ok(()) => (),
                            Err(e) => {
                                error!("Unable to send frame: {:?}", e);
                            }
                        }
                    }
                }
            },
            None => ()
        }

        //Make sure we can always read at least the MTU
        pending.resize(pending_bytes + frame::MTU, 0);

        let read_port: &mut io::Read = if echo {
            &mut echo_port
        } else {
            port.as_mut().unwrap()
        };

        use std::io::Read;
        let read = match read_port.read(&mut pending[pending_bytes..]) {
            Ok(r) => r,
            Err(e) => {
                match e.kind() {
                    io::ErrorKind::TimedOut => 0,
                    _ => {
                        error!("Tried to read bytes from serial port but IO error occurred: {:?}", e);
                        return
                    }
                }
            }
        };

        if read > 0 {
            pending_bytes += read;
            match read_frame(&mut pending, &mut pending_bytes) {
                Some(msg) => {
                    display.push_message(&msg);
                },
                None => ()
            }
        }

        let exec_ms = time::precise_time_ns() / 1_000_000;

        //Throttle our updates to 30hz
        if start_ms + 33 < exec_ms {
            let sleep_ms = exec_ms - (start_ms + 33);
            thread::sleep(Duration::from_millis(sleep_ms));
        }
    }
}

fn configure_port(name: &std::ffi::OsStr, baud: Option<usize>) -> serial::Result<serial::SystemPort> {
    use serial::SerialPort;

    let mut port = try!(serial::open(name));

    try!(port.reconfigure(&|settings| {
        match baud {
            Some(baud) => {
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
            },
            _ => ()
        }
        Ok(())
    }));

    //Return immediately
    try!(port.set_timeout(Duration::from_millis(1)));

    Ok(port)
}

fn read_frame(pending: &mut Vec<u8>, pending_bytes: &mut usize) -> Option<String> {
    trace!("Reading wire frame {:?}", &pending[..*pending_bytes]);
    let mut kiss_frame = vec!();
    match kiss::decode(pending.iter().cloned().take(*pending_bytes), &mut kiss_frame) {
        Some(frame) => {
            debug!("Decoded KISS frame {:?}", &kiss_frame);

            let mut nbp_payload = vec!();
            nbp_payload.resize(frame::MTU, 0);
            let result = match frame::from_bytes(&mut io::Cursor::new(&kiss_frame), &mut nbp_payload, frame.payload_size) {
                Ok(nbp_frame) => {
                    match nbp_frame {
                        frame::Frame::Data(header) => {
                            let source = routing::format_route(header.address_route);

                            match std::str::from_utf8(&nbp_payload[..header.payload_size]) {
                                Ok(msg) => {
                                    Some(format!("{}: {}", source, msg.trim()))
                                },
                                Err(e) => {
                                    error!("{}: Malformed UTF-8 error: {}", source, e);
                                    None
                                }
                            }
                        },
                        frame::Frame::Ack(header) => {
                            Some(format!("{}: {} ACK", header.prn, address::decode(header.src_addr).into_iter().cloned().collect::<String>()))
                        }
                    }
                },
                Err(e) => {
                    error!("Unable to parse NBP frame: {:?}", e);
                    None
                }
            };

            //Remove the data we parsed
            assert!(frame.bytes_read >= *pending_bytes);
            pending.drain(..*pending_bytes);
            *pending_bytes -= frame.bytes_read;

            result
        },
        None => None  //Nothing decoded yet, we need more data
    }
}

fn send_frame(prn: &mut prn_id::PRN, input: &String, port: &mut io::Write) -> Result<(), io::ErrorKind> {
    let (dest, message) = match input.find(' ') {
        Some(split) => {
            let (addr, msg) = input.split_at(split);

            //Translate into real addresses
            let path = addr.split("->")
                .map(|path| {
                    address::encode(string_to_addr(path))
                        .map(|value| Ok(value))
                        .unwrap_or(Err(format!("Unable to encode {} as callsign", path)))
                })
                .collect::<Result<Vec<_>, _>>()
                .map(|route| {
                    //We need to propertly format our address to contain src SEP route
                    iter::once(prn.callsign)
                        .chain(iter::once(routing::ADDRESS_SEPARATOR))
                        .chain(route)
                        .collect::<Vec<u32>>()
                });
                

            (path, msg.as_bytes())
        },
        None => {
            println!("Invalid syntax, message follow: 'CALLSIG MESSAGE...' or 'CALLSI1->CALLSI2->CALLSI3 MESSAGE...'");
            return Ok(())
        }
    };

    match dest {
        Ok(dest) => {
            let frame = match frame::new_data(prn, &dest, message.len()) {
                Err(e) => {
                    error!("Unable to create frame: {:?}", e);
                    return Ok(())
                }
                Ok(frame) => frame
            };

            let mut full_frame = vec!();
            try!(frame::to_bytes(&mut full_frame, &frame::Frame::Data(frame), Some(message)).map_err(|_| io::ErrorKind::InvalidData));
            debug!("Encoding KISS frame {:?}", &full_frame);
            
            //Encode into kiss frame
            let mut kiss_frame = vec!();
            kiss::encode(full_frame.iter().cloned(), &mut kiss_frame, 0);

            trace!("Sending frame over the wire {:?}", &kiss_frame);
            return port.write_all(&kiss_frame).map_err(|err| err.kind())
        },
        Err(msg) => {
            error!("{}", msg);
            return Ok(())
        }
    }
}

fn string_to_addr(addr: &str) -> [char; 7] {
    //Translate from string into array up to 7 characters
    let mut local_addr = ['0'; 7];
    for (i, chr) in addr.chars().take(7).enumerate() {
        local_addr[i] = chr;
    }

    local_addr
}