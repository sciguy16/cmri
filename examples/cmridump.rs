// Copyright 2020 David Young
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use cmri::{CmriMessage, CmriStateMachine, RxState};
use std::io::Read;
use std::net::{Shutdown, TcpListener, TcpStream};
use std::thread;

const PORT: u16 = 4000;

/// Listens on [::1]:4000 and prints out incoming packets

fn main() {
    let listener = TcpListener::bind(format!("[::1]:{}", PORT)).unwrap();
    println!("Server listening on port {}", PORT);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("Connection from {}", stream.peer_addr().unwrap());
                thread::spawn(move || tcp_rx(stream));
            }
            Err(e) => {
                println!("Connection failed with error \"{}\"", e);
            }
        }
    }
    drop(listener);
}

fn tcp_rx(mut stream: TcpStream) {
    use RxState::*;
    // Single-byte buffer so that we process one byte at a time
    let mut buf = [0_u8; 1];
    // State machine for receiving
    let mut state = CmriStateMachine::new();
    loop {
        // try reading a byte off the stream
        //TODO timeout
        match stream.read(&mut buf) {
            Ok(1) => {
                // got a single byte
                match state.process(buf[0]) {
                    Ok(Listening) => {
                        // Do nothing, is listening still
                    }
                    Ok(Complete) => {
                        // Message is complete, print it
                        if let Err(e) = print_message(state.message()) {
                            println!("Error: {}", e);
                        }
                    }
                    Err(e) => {
                        println!("Receive error: {:?}", e);
                        break;
                    }
                }
            }
            Err(e) => {
                println!("Read failed: {}", e);
                stream.shutdown(Shutdown::Both).unwrap();
                break;
            }
            _ => {}
        }
    }
    println!("client exited");
}

fn print_message(m: &CmriMessage) -> Result<(), cmri::Error> {
    use cmri::Error;

    let payload = hex::encode(&m.payload[..m.len]);
    println!(
        "{}\t{}\t{:?}",
        m.address.ok_or(Error::MissingAddress)?,
        m.message_type.ok_or(Error::MissingType)?,
        payload,
    );
    Ok(())
}
