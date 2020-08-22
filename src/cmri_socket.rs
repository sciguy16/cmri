// Copyright 2020 David Young
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.


use crate::Result;
use crate::{CmriMessage, TX_BUFFER_LEN};
use std::boxed::Box;
use std::io::{Read, Write};

// Presents a socket abstraction that provides a convenient abstraction
// for sending and receiving C/MRI messages

pub trait ReadWrite: Read + Write {}

impl<T> ReadWrite for T where T: Read + Write {}

pub struct CmriSocket {
    duplex: Duplex,
    transport: Box<dyn ReadWrite>,
    rx_buffer: CmriMessage,
    tx_buffer: [u8; TX_BUFFER_LEN],
    tx_switch: fn(bool) -> (),
    rx_callback: fn(&CmriMessage) -> (),
}

#[derive(Copy, Clone, Debug)]
pub enum Duplex {
    Half,
    Full,
}

impl CmriSocket {
    pub fn new(
        duplex: Duplex,
        transport: Box<dyn ReadWrite>,
        rx_callback: fn(&CmriMessage),
    ) -> Self {
        CmriSocket {
            duplex,
            transport,
            rx_buffer: CmriMessage::new(),
            tx_buffer: [0; TX_BUFFER_LEN],
            tx_switch: |_| {},
            rx_callback,
        }
    }

    pub fn duplex(&self) -> Duplex {
        self.duplex
    }

    pub fn tx_switch(&mut self, tx_switch: fn(bool) -> ()) {
        self.tx_switch = tx_switch;
    }

    pub fn send(&mut self, msg: &CmriMessage) -> Result<()> {
        // encode message to tx buffer
        msg.encode(&mut self.tx_buffer)?;

        // Toggle TX enable line
        (self.tx_switch)(true);

        // Write the data
        self.transport.write_all(&self.tx_buffer)?;
        self.transport.flush()?;

        // Toggle TX enable again
        (self.tx_switch)(false);

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::MessageType;
    use std::println;

    struct TestTransport;
    impl Write for TestTransport {
        fn write(
            &mut self,
            buf: &[u8],
        ) -> core::result::Result<usize, std::io::Error> {
            println!("Writing {} bytes: {:?}", buf.len(), buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> core::result::Result<(), std::io::Error> {
            println!("flush");
            Ok(())
        }
    }

    impl Read for TestTransport {
        fn read(
            &mut self,
            buf: &mut [u8],
        ) -> core::result::Result<usize, std::io::Error> {
            for ele in buf.iter_mut() {
                *ele = 5;
            }
            Ok(buf.len())
        }
    }

    #[test]
    fn send_message() {
        let transport = TestTransport;
        let mut socket =
            CmriSocket::new(Duplex::Half, Box::new(transport), |msg| {
                println!("addr: {:?}", msg.address);
            });

        let p = [1, 2, 3];
        let mut msg = CmriMessage::new();
        let msg = msg
            .address(1)
            .message_type(MessageType::Poll)
            .payload(&p)
            .unwrap();

        socket.send(&msg).unwrap();
    }

    #[test]
    fn send_message_with_tx_toggle() {
        let transport = TestTransport;
        let mut socket =
            CmriSocket::new(Duplex::Half, Box::new(transport), |msg| {
                println!("addr: {:?}", msg.address);
            });
        socket.tx_switch(|tx| {
            println!("Setting TX mode to `{}`...", tx);
        });

        let p = [1, 2, 3];
        let mut msg = CmriMessage::new();
        let msg = msg
            .address(1)
            .message_type(MessageType::Poll)
            .payload(&p)
            .unwrap();

        socket.send(&msg).unwrap();
    }
}
