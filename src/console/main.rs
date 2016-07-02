extern crate clap;
extern crate serial;
extern crate nbplink;

mod echo;

use nbplink::nbp::{frame, address, prn_id, routing};
use nbplink::kiss;
use std::time::Duration;
use std::io;
use std::iter;

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
        .arg(clap::Arg::with_name("echo")
            .short("e")
            .long("echo")
            .help("Enable echo mode, rs232 port is disabled and all data is echoed back to the client"))
        .get_matches();

    let port = matches.value_of_os("port").expect("No port specified");
    let callsign = matches.value_of("callsign").expect("No callsign specified");

    let cmds = match matches.values_of("cmd") {
        Some(cmds) => cmds.collect::<Vec<&str>>(),
        None => vec!()
    };

    let echo = matches.is_present("echo");

    let mut echo_port = echo::new();
    let mut port = if !echo {
        Some(match configure_port(port) {
            Ok(port) => port,
            Err(e) => {
                match e.kind() {
                    serial::ErrorKind::NoDevice => println!("Unable to open port, no device found for {:?}", port),
                    serial::ErrorKind::InvalidInput => println!("Unable to open port, {:?} is not a valid device name", port),
                    serial::ErrorKind::Io(io_e) => println!("Unable to open port, IO error: {:?}", io_e)
                }
                return
            }
        })
    } else {
        None
    };

    let mut prn = match prn_id::new(string_to_addr(callsign)) {
        Some(prn) => prn,
        None => {
            println!("Unable to parse callsign, a valid callsign is up to seven characters containing A-Z, 0-9");
            return;
        }
    };

    loop {
        let mut input = String::new();
        let mut pending = vec!();
        let mut pending_bytes = 0;

        match io::stdin().read_line(&mut input) {
            Ok(_) => {
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
                                println!("Unable to send frame: {:?}", e);
                            }
                        }
                    }
                }
            },
            Err(e) => {
                println!("Failed to read from stdin: {:?}", e);
                return
            } 
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
                println!("Tried to read bytes from serial port but IO error occurred: {:?}", e);
                return
            }
        };

        pending_bytes += read;
        read_frame(&mut pending, &mut pending_bytes);
    }
}

fn configure_port(name: &std::ffi::OsStr) -> serial::Result<serial::SystemPort> {
    use serial::SerialPort;

    let mut port = try!(serial::open(name));

    //@todo: pull these out into cmd flags
    try!(port.reconfigure(&|settings| {
        try!(settings.set_baud_rate(serial::Baud9600));
        settings.set_char_size(serial::Bits8);
        settings.set_parity(serial::ParityNone);
        settings.set_stop_bits(serial::Stop1);
        settings.set_flow_control(serial::FlowNone);

        Ok(())
    }));

    //Return immediately
    try!(port.set_timeout(Duration::from_millis(0)));

    Ok(port)
}

fn read_frame(pending: &mut Vec<u8>, pending_bytes: &mut usize) {
    let mut kiss_frame = vec!();
    match kiss::decode(pending.iter().cloned().take(*pending_bytes), &mut kiss_frame) {
        Some(frame) => {
            let mut nbp_payload = vec!();
            match frame::from_bytes(&mut io::Cursor::new(&kiss_frame), &mut nbp_payload, frame.bytes_read) {
                Ok(nbp_frame) => {
                    match nbp_frame {
                        frame::Frame::Data(header) => {
                            let source = pretty_print_route(header.address_route);

                            match std::str::from_utf8(&nbp_payload) {
                                Ok(msg) => {
                                    println!("{}: {}", source, msg);
                                },
                                Err(e) => {
                                    println!("{}: Malformed UTF-8 error: {}", source, e);
                                }
                            }
                        },
                        frame::Frame::Ack(header) => {
                            println!("{}: {} ACK", header.prn, address::decode(header.src_addr).into_iter().cloned().collect::<String>());
                        }
                    }
                },
                Err(e) => {
                    println!("Unable to parse NBP frame: {:?}", e);
                }
            }

            //Remove the data we parsed
            assert!(frame.bytes_read >= *pending_bytes);
            pending.drain(..*pending_bytes);
            *pending_bytes -= frame.bytes_read;
        },
        None => ()  //Nothing decoded yet, we need more data
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
                    println!("Unable to create frame: {:?}", e);
                    return Ok(())
                }
                Ok(frame) => frame
            };

            let mut full_frame = vec!();
            try!(frame::to_bytes(&mut full_frame, &frame::Frame::Data(frame), Some(message)).map_err(|_| io::ErrorKind::InvalidData));

            //Encode into kiss frame
            let mut kiss_frame = vec!();
            kiss::encode(full_frame.iter().cloned(), &mut kiss_frame, 0);

            return port.write_all(&kiss_frame).map_err(|err| err.kind())
        },
        Err(msg) => {
            println!("{}", msg);
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

fn pretty_print_route(route: [u32; 17]) -> String {
    route.into_iter().cloned()
        .map(|addr| address::decode(addr).into_iter().cloned().collect::<String>())
        .fold(String::new(), |route, addr| {
            if route.len() > 0 {
                route + " -> " + addr.as_str()
            } else {
                addr
            }
        })
}