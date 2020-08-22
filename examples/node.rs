// Copyright 2020 David Young
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.


use cmri::{
    payload_from_slice, CmriMessage, CmriStateMachine, MessageType, NodeType,
    RxState,
};
use std::convert::TryFrom;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::thread;
use std::time::SystemTime;

const PORT: u16 = 4000;
const NODE_ADDRESS: u8 = 1;

/// Listens on [::1]:4000 and prints out incoming packets

fn main() {
    let listener = TcpListener::bind(format!("[::1]:{}", PORT)).unwrap();
    println!("Server listening on port {}", PORT);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("Connection from {}", stream.peer_addr().unwrap());
                thread::spawn(move || tcp_handle(stream));
            }
            Err(e) => {
                println!("Connection failed with error \"{}\"", e);
            }
        }
    }
    drop(listener);
}

fn tcp_handle(mut stream: TcpStream) {
    use MessageType::*;
    use RxState::*;
    // Single-byte buffer so that we process one byte at a time
    let mut buf = [0_u8; 1];
    // State machine for receiving
    let mut state = CmriStateMachine::new();
    // Transmit buffer
    let mut tx_buffer = [0_u8; cmri::TX_BUFFER_LEN];
    // Payload buffer
    let mut state_payload: Vec<u8> = Vec::new();
    // tx payload buffer
    let mut tx_payload_buffer = [0_u8; cmri::MAX_PAYLOAD_LEN];
    // Set the address: 65 + the node addr
    state.filter(65 + NODE_ADDRESS);
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
                        let msg = state.message();

                        if msg.address.is_none() || msg.message_type.is_none() {
                            // address or type is missing; shouldn't
                            // ever happen but best be sure
                            continue;
                        }
                        let msgtype = msg.message_type.unwrap();
                        println!("Received {} message", msgtype);

                        match msgtype {
                            Init => {
                                // Init message tells us what type of
                                // node the controller is expecting
                                if let Ok(node_type) =
                                    NodeType::try_from(msg.payload[0])
                                {
                                    println!("Node type: {}", node_type);
                                }
                            }
                            Set => {
                                // Set output bits
                            }
                            Get => {
                                // Shouldn't receive one of these - these
                                // are for nodes to send to the controller
                            }
                            Poll => {
                                // Send the controller our status
                                state_payload.clear();
                                // Set every sensor to ACTIVE if current
                                // unix time in seconds is even, else
                                // INACTIVE
                                let byte = match SystemTime::now()
                                    .duration_since(SystemTime::UNIX_EPOCH)
                                {
                                    Ok(n) => {
                                        if n.as_secs() % 2 == 0 {
                                            0xff
                                        } else {
                                            0x00
                                        }
                                    }
                                    Err(_) => 0x7f,
                                };
                                state_payload.extend_from_slice(&[byte; 64]);
                                let message = CmriMessage {
                                    address: Some(65 + NODE_ADDRESS),
                                    message_type: Some(MessageType::Get),
                                    payload: tx_payload_buffer,
                                    len: state_payload.len(),
                                };
                                if let Err(e) = payload_from_slice(
                                    &mut tx_payload_buffer,
                                    &state_payload,
                                ) {
                                    println!("Error: {}", e);
                                }
                                if let Err(e) = message.encode(&mut tx_buffer) {
                                    println!("Error: {}", e);
                                }
                                if let Err(e) = stream.write_all(&tx_buffer) {
                                    println!("Error: {}", e);
                                }
                            }
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
                if let Err(e) = stream.shutdown(Shutdown::Both) {
                    println!("Error: {}", e);
                }
                break;
            }
            _ => {}
        }
    }
    println!("client exited");
}
