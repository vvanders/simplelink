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
use std::thread;

use nbplink::nbp::{address, frame, routing, node};
use nbplink::util;

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

    let mut echo_tx = echo::new();
    let mut echo_rx = echo::new();
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
            &mut echo_tx
        } else {
            port.as_mut().unwrap()
        };
        
        let write_cmd = cmd.to_string() + "\n";

        match write_port.write_all(write_cmd.as_bytes()) {
            Ok(_) => info!("Sending '{}' to TNC", cmd),
            Err(e) => {
                error!("Unable to send '{}' to TNC {:?}", cmd, e);
            }
        }
    }

    let callsign_id = match address::encode(string_to_addr(callsign)) {
        Some(prn) => prn,
        None => {
            println!("Unable to parse callsign, a valid callsign is up to seven characters containing A-Z, 0-9");
            return;
        }
    };

    let mut display = display::new();
    let mut node = node::new(callsign_id);

    loop {
        let start_ms = time::precise_time_ns() / 1_000_000;

        match display.get_input() {
            Some(input) => {
                match input.len() {
                    0 => (),
                    _ => {
                        let write_port: &mut io::Write = if echo {
                            &mut echo_tx
                        } else {
                            port.as_mut().unwrap()
                        };

                        match send_frame(&mut node, &input, write_port) {
                            Ok(_) => (),
                            Err(e) => {
                                error!("Unable to send frame: {:?}", e);
                            }
                        }
                    }
                }
            },
            None => ()
        }

        if echo {
            read_frames(&mut node, &mut util::new_read_write_dispatch(&mut echo_rx, &mut echo_tx), &mut display);

            //Swap so we read output on next tick
            echo_rx = echo_tx;
            echo_tx = echo::new();
        } else {
            read_frames(&mut node, port.as_mut().unwrap(), &mut display);
        }
        
        let exec_ms = time::precise_time_ns() / 1_000_000;

        //Throttle our updates to 30hz
        const UPDATE_RATE_MS: u64 = 33;
        if exec_ms - start_ms < UPDATE_RATE_MS {
            let sleep_ms = UPDATE_RATE_MS - (exec_ms - start_ms);
            thread::sleep(Duration::from_millis(sleep_ms));
        }
    }
}

fn format_data(header: &frame::DataHeader, payload: &[u8]) -> String {
    use std::str;
    match str::from_utf8(payload) {
        Ok(msg) => {
            let line = msg.to_string();
            let route = routing::format_route(&header.address_route);

            route + ": " + line.as_str()
        },
        Err(e) => format!("Unable to decode UTF-8 {:?}", e)
    }
}

fn read_frames<T>(node: &mut node::Node, io: &mut T, display: &mut display::Display) where T: io::Read + io::Write {
    let mut obs_msg = vec!();
    let read = node.recv(io,
        |header,payload| {
            display.push_message(&format_data(header, payload));
        },
        |header,payload| {
            match header {
                &frame::Frame::Data(header) => {
                    let msg = format_data(&header, payload);
                    obs_msg.push(format!("OBS - {}", msg));
                },
                &frame::Frame::Ack(header) => {
                    obs_msg.push(format!("OBS - ACK {} {}", header.prn, address::format_addr(header.src_addr)));
                }
            }
        });

    for msg in obs_msg {
        display.push_message(&msg);
    }

    match read {
        Ok(()) => (),
        Err(e) => error!("Tried to read bytes from serial port but IO error occurred: {:?}", e)
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

fn send_frame(node: &mut node::Node, input: &String, port: &mut io::Write) -> Result<(), node::SendError> {
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
                .collect::<Result<Vec<_>, _>>();

            (path, msg.as_bytes())
        },
        None => {
            println!("Invalid syntax, message follow: 'CALLSIG MESSAGE...' or 'CALLSI1->CALLSI2->CALLSI3 MESSAGE...'");
            return Ok(())
        }
    };

    match dest {
        Ok(dest) => node.send(message.iter().cloned(), dest.iter().cloned(), &mut util::new_write_dispatch(port)).map(|_| ()),
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