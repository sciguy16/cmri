// Copyright 2020 David Young
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use cmri::TX_BUFFER_LEN;
use cmri::{CmriMessage, CmriStateMachine, RxState};
use std::io::Read;
use std::thread;

use rppal::uart::{Parity, Uart};

const UART: &str = "/dev/ttyUSB0";
const BAUD_RATE: u32 = 19200;
//const RTS_PIN: u8 = 11;
const ADDR_START: u8 = 1;
const ADDR_END: u8 = 26;

/// Scans the connection for listening C/MRI nodes

fn main() {
    println!("Scanning for nodes via {}", UART);

    let mut state = CmriStateMachine::new();

    let mut uart =
        Uart::with_path(UART, BAUD_RATE, Parity::None, 8, 2).unwrap();
    let mut message = CmriMessage::new();
    let mut tx_buffer = [0_u8; TX_BUFFER_LEN];
    // Send a Poll request to each node address in turn, allowing some
    // time for it to respond
    for addr in ADDR_START..ADDR_END {
        println!("Trying address {}...", addr);

        // send Poll
        message.address = Some(65 + addr);
        message.encode(&mut tx_buffer);
    }
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
