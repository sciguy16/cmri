use std::error::Error;
use std::fmt;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::mpsc::{self, TryRecvError};
use std::thread;
use std::time::Duration;

use rppal::gpio::Gpio;
use rppal::uart::{Parity, Uart};

const UART: &str = "/dev/ttyAMA1";
const BAUD_RATE: u32 = 19200;
const RTS_PIN: u8 = 11;
const PORT: u16 = 4000;
const CMRI_START_BYTE: u8 = 0x02;
const CMRI_STOP_BYTE: u8 = 0x03;
// number of byte-lengths extra to wait to account for delays
const EXTRA_TX_TIME: u64 = 2;

#[derive(Copy, Clone)]
enum CmriState {
    Idle,
    Receiving,
}

struct CmriPacket {
    payload: Vec<u8>,
}

impl CmriPacket {
    fn new() -> Self {
        Self {
            // capacity is the max length from
            // https://github.com/madleech/ArduinoCMRI/blob/master/CMRI.h
            payload: Vec::with_capacity(258),
        }
    }

    fn append(&mut self, buf: &mut [u8]) {
        self.payload.extend_from_slice(buf);
    }

    fn len(&self) -> usize {
        self.payload.len()
    }
}

impl fmt::Display for CmriPacket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.payload)
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("Hello, world!");

    let (tcp_to_485_tx, tcp_to_485_rx): (
        mpsc::Sender<CmriPacket>,
        mpsc::Receiver<CmriPacket>,
    ) = mpsc::channel();

    thread::spawn(move || start_listener(tcp_to_485_tx));

    let mut rts_pin = Gpio::new()?.get(RTS_PIN)?.into_output();
    rts_pin.set_high(); // set RTS high to put MAX485 into RX mode
    let mut uart = Uart::with_path(UART, BAUD_RATE, Parity::None, 8, 1)?;
    let mut buffer = [0_u8];
    let mut rx_packet = CmriPacket::new();
    let mut state = CmriState::Idle;

    // 8 bits * microseconds * seconds per bit
    let byte_time = (8_f64 * 1_000_000_f64 * 1_f64 / (BAUD_RATE as f64)) as u64;

    loop {
        // Handle all of the UART stuff
        // Check the mpsc in case there is a packet to transmit
        match tcp_to_485_rx.try_recv() {
            Ok(packet) => {
                // send the packet out of the uart
                println!("Sending down uart");
                rts_pin.set_low();
                uart.write(&packet.payload)?; // default non-blocking
                thread::sleep(Duration::from_micros(
                    (EXTRA_TX_TIME + packet.len() as u64) * byte_time,
                )); // wait until all data transmitted
                rts_pin.set_high();
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                println!("A bad");
                break;
            }
        }

        // Try reading the uart
        if uart.read(&mut buffer)? > 0 {
            match buffer[0] {
                CMRI_START_BYTE => {
                    // Force state to rx
                    state = CmriState::Receiving;
                    rx_packet = CmriPacket::new();
                    rx_packet.append(&mut buffer);
                }
                CMRI_STOP_BYTE => {
                    rx_packet.append(&mut buffer);
                    println!("Received from uart: {}", rx_packet);
                    rx_packet = CmriPacket::new();
                    // force state to idle
                    state = CmriState::Idle;
                }
                _ => {
                    // only append byte if we are currently receiving,
                    // otherwise ignore because it's probably an idle 0xFF
                    match state {
                        CmriState::Receiving => {
                            rx_packet.append(&mut buffer);
                        }
                        CmriState::Idle => {}
                    }
                }
            }
            buffer = [0];
        }
    }

    Ok(())
}

fn start_listener(tcp_to_485_tx: mpsc::Sender<CmriPacket>) {
    let listener = TcpListener::bind(format!("[::1]:{}", PORT)).unwrap();
    println!("Server listening on port {}", PORT);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("Connection from {}", stream.peer_addr().unwrap());
                let sender = tcp_to_485_tx.clone();
                thread::spawn(move || handle_tcp_client(stream, sender));
            }
            Err(e) => {
                println!("Connection failed iwth error \"{}\"", e);
            }
        }
    }
    drop(listener);
}

fn handle_tcp_client(mut stream: TcpStream, channel: mpsc::Sender<CmriPacket>) {
    let mut buf = [0_u8; 1];
    let mut packet: CmriPacket = CmriPacket::new();
    let mut state = CmriState::Idle;
    loop {
        // try reading a byte off the stream
        match stream.read(&mut buf) {
            Ok(1) => {
                // got a single byte
                match buf[0] {
                    CMRI_START_BYTE => {
                        // make a new packet
                        state = CmriState::Receiving;
                        packet = CmriPacket::new();
                        packet.append(&mut buf);
                    }
                    CMRI_STOP_BYTE => {
                        // packet has finished, send it away down the uart
                        packet.append(&mut buf);
                        println!("Got packet {}", packet);
                        channel.send(packet).unwrap();
                        packet = CmriPacket::new();
                        state = CmriState::Idle;

                    }
                    _b => {
                        // any other character, append to buffer
                        match state {
                            CmriState::Receiving => {
                                packet.append(&mut buf);
                            }
                            CmriState:: Idle => {}
                        }
                    }
                }
            }
            Err(e) => {
                println!("Read failed: {}", e);
                stream.shutdown(Shutdown::Both).unwrap();
            }
            _ => {}
        }

        // if there is data to send then send it
    }
}
