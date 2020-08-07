#![no_std]

pub use error::{Error, Result};

mod error;

/// This is the length calculated from
/// https://github.com/madleech/ArduinoCMRI/blob/master/CMRI.h
/// (64 i/o cards @ 32 bits each + packet type and address bytes)
//const RX_BUFFER_LEN: usize = 258;
const MAX_PAYLOAD_LEN: usize = 256;

const CMRI_PREAMBLE_BYTE: u8 = 0xff;
const CMRI_START_BYTE: u8 = 0x02;
const CMRI_STOP_BYTE: u8 = 0x03;
const CMRI_ESCAPE_BYTE: u8 = 0x10;

/// Possible states of the C/MRI system
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum CmriState {
    Idle,
    Attn,
    Start,
    Addr,
    Type,
    Data,
    Escape,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum RxState {
    Listening,
    Complete,
}

/// Main state machine, including decoding logic
pub struct CmriStateMachine {
    state: CmriState,
    message: CmriMessage,
    /// If set, decoding will only accept messages directed at this
    /// address and discard all others
    address_filter: Option<u8>,
}

pub struct CmriMessage {
    address: u8,
    message_type: u8,
    payload: [u8; MAX_PAYLOAD_LEN],
    position: usize,
}

impl CmriMessage {
    pub fn new() -> Self {
        Self {
            address: 0,
            message_type: 0,
            payload: [0; MAX_PAYLOAD_LEN],
            position: 0,
        }
    }

    /// Push a byte onto the payload
    fn push(&mut self, byte: u8) -> Result<()> {
        if self.position == MAX_PAYLOAD_LEN {
            // Buffer is full, which is problematic
            return Err(Error::OutOfBounds);
        }
        self.payload[self.position] = byte;
        self.position += 1;
        Ok(())
    }

    /// Empty the rx buffer
    fn clear(&mut self) {
        self.position = 0;
        self.payload = [0_u8; MAX_PAYLOAD_LEN];
    }
}

impl CmriStateMachine {
    pub fn new() -> Self {
        Self {
            state: CmriState::Idle,
            message: CmriMessage::new(),
            address_filter: None,
        }
    }

    /// Returns the current state of the system
    pub fn state(&self) -> CmriState {
        self.state
    }

    /// Sets an address filter so that the state machine will only
    /// accept messages targeted at us
    pub fn filter(&mut self, addr: u8) {
        self.address_filter = Some(addr);
    }

    pub fn clear(&mut self) {
        self.message.clear();
        self.state = CmriState::Idle;
    }

    /// Main process function. Takes in bytes off the wire and builds up
    /// a message in the receive buffer
    pub fn process(&mut self, byte: u8) -> Result<RxState> {
        use CmriState::*;
        match self.state {
            Idle => {
                // Idle to Attn if byte is PREAMBLE
                if byte == CMRI_PREAMBLE_BYTE {
                    self.clear();
                    self.state = Attn;
                }
                // Ignore other bytes while Idle
            }
            Attn => {
                // Attn to Start if byte is PREAMBLE
                if byte == CMRI_PREAMBLE_BYTE {
                    self.state = Start;
                } else {
                    // Otherwise discard and reset to Idle
                    self.clear();
                }
            }
            Start => {
                // start byte must be valid
                if byte == CMRI_START_BYTE {
                    self.state = Addr;
                } else {
                    // Otherwise discard and reset to Idle
                    self.clear();
                }
            }
            Addr => {
                // Take the next byte as-is for an address
                if let Some(addr) = self.address_filter {
                    // A filter has been defined
                    if addr != byte {
                        // Not our address, discard the message
                        self.clear();
                        return Ok(RxState::Listening);
                    }
                }

                self.message.address = byte;
                self.state = Type;
            }
            Type => {
                // Take the next byte as-is for message type
                self.message.message_type = byte;
                self.state = Data;
            }
            Data => {
                match byte {
                    CMRI_ESCAPE_BYTE => {
                        // escape the next byte; do not push the escape
                        // byte
                        //self.push(byte)?;
                        self.state = Escape;
                    }
                    CMRI_STOP_BYTE => {
                        // end transmission
                        self.state = Idle;
                        return Ok(RxState::Complete);
                    }
                    _ => {
                        // any other byte we take as data
                        self.message.push(byte)?;
                    }
                }
            }
            Escape => {
                // Escape the next byte, so accept it as data.
                self.message.push(byte)?;
                self.state = Data;
            }
        }
        Ok(RxState::Listening)
    }
}

impl Default for CmriStateMachine {
    fn default() -> Self {
        Self::new()
    }
}
impl Default for CmriMessage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use CmriState::*;
    use RxState::*;

    #[test]
    fn basic_create_state_machine() {
        let s = CmriStateMachine::new();
        assert_eq!(s.state(), CmriState::Idle);
        assert_eq!(s.message.payload.len(), MAX_PAYLOAD_LEN);
        assert_eq!(s.message.position, 0);
    }

    #[test]
    fn decode_first_preamble() {
        // Create a state machine
        let mut s = CmriStateMachine::new();

        // Send junk
        let res = s.process(0x05).unwrap();
        assert_eq!(res, Listening);

        // Send more junk
        let res = s.process(0xfe).unwrap();
        assert_eq!(res, Listening);

        // Make sure the buffer hasn't recorded any of this
        assert_eq!(s.message.position, 0);
        assert_eq!(s.message.payload[0], 0);
        assert_eq!(s.state, Idle);

        // Send a preamble byte and check that the state has changed to Attn
        let res = s.process(CMRI_PREAMBLE_BYTE).unwrap();
        assert_eq!(res, Listening);
        assert_eq!(s.state, Attn);
        assert_eq!(s.message.position, 0); // preamble does not get saved
    }

    #[test]
    fn decode_second_preamble() {
        // Create a state machine and send two preamble bytes
        let mut s = CmriStateMachine::new();
        let res = s.process(CMRI_PREAMBLE_BYTE);
        assert_eq!(res, Ok(Listening));
        assert_eq!(s.state, Attn);
        let res = s.process(CMRI_PREAMBLE_BYTE);
        assert_eq!(res, Ok(Listening));
        assert_eq!(s.state, Start);

        // Create a new state machine and send one preamble followed by junk
        let mut s = CmriStateMachine::new();
        let res = s.process(CMRI_PREAMBLE_BYTE);
        assert_eq!(res, Ok(Listening));
        assert_eq!(s.state, Attn);
        let res = s.process(0x31);
        assert_eq!(res, Ok(Listening));
        assert_eq!(s.state, Idle);
    }

    #[test]
    fn decode_start_byte() {
        // Create a state machine and send two preamble bytes
        let mut s = CmriStateMachine::new();
        let res = s.process(CMRI_PREAMBLE_BYTE);
        assert_eq!(res, Ok(Listening));
        assert_eq!(s.state, Attn);
        let res = s.process(CMRI_PREAMBLE_BYTE);
        assert_eq!(res, Ok(Listening));
        assert_eq!(s.state, Start);

        // Send a start byte and check that we're in address mode
        let res = s.process(CMRI_START_BYTE);
        assert_eq!(res, Ok(Listening));
        assert_eq!(s.state, Addr);

        // Create a state machine and send two preamble bytes
        let mut s = CmriStateMachine::new();
        let res = s.process(CMRI_PREAMBLE_BYTE);
        assert_eq!(res, Ok(Listening));
        assert_eq!(s.state, Attn);
        let res = s.process(CMRI_PREAMBLE_BYTE);
        assert_eq!(res, Ok(Listening));
        assert_eq!(s.state, Start);

        // Send junk instead of a start byte
        let res = s.process(0x32);
        assert_eq!(res, Ok(Listening));
        assert_eq!(s.state, Idle);
        assert_eq!(s.message.position, 0);
    }

    // Skip Addr and Type because they can each be any byte

    #[test]
    fn decode_escape_byte() {
        let mut s = get_to_data_section(0x43).unwrap();
        // Normal message byte
        let res = s.process(5);
        assert_eq!(res, Ok(Listening));
        assert_eq!(s.state, Data);

        // Escape byte, should not advance the position
        let pos = s.message.position;
        let res = s.process(CMRI_ESCAPE_BYTE);
        assert_eq!(res, Ok(Listening));
        assert_eq!(s.state, Escape);
        assert_eq!(s.message.position, pos);

        // Send an escape byte again, should be escaped and state back
        // to accepting data
        let res = s.process(CMRI_ESCAPE_BYTE);
        assert_eq!(res, Ok(Listening));
        assert_eq!(s.state, Data);
        assert_eq!(s.message.position, pos + 1);

        // Escape byte, should not advance the position
        let pos = s.message.position;
        let res = s.process(CMRI_ESCAPE_BYTE);
        assert_eq!(res, Ok(Listening));
        assert_eq!(s.state, Escape);
        assert_eq!(s.message.position, pos);

        // Send a stop byte, should be escaped and state back
        // to accepting data
        let res = s.process(CMRI_STOP_BYTE);
        assert_eq!(res, Ok(Listening));
        assert_eq!(s.state, Data);
        assert_eq!(s.message.position, pos + 1);
    }

    #[test]
    fn decode_stop_byte() {
        let mut s = get_to_data_section(0x05).unwrap();

        // Normal message byte
        let res = s.process(5);
        assert_eq!(res, Ok(Listening));
        assert_eq!(s.state, Data);

        // Stop byte, should trigger the end of message stuff
        let res = s.process(CMRI_STOP_BYTE);
        assert_eq!(res, Ok(Complete));
        assert_eq!(s.state, Idle);
    }

    #[test]
    fn address_filter() {
        // Initial check to see that no-filter works
        let s = get_to_data_section(0x06).unwrap();
        assert_eq!(s.state, Data);

        // Make a state machine with a filter
        let mut s = CmriStateMachine::new();
        s.filter(0x64);

        // Send the same address
        s.process(CMRI_PREAMBLE_BYTE).unwrap();
        s.process(CMRI_PREAMBLE_BYTE).unwrap();
        s.process(CMRI_START_BYTE).unwrap();
        let res = s.process(0x64);
        assert_eq!(res, Ok(Listening));
        assert_eq!(s.state, Type);

        // Make a new state machine
        let mut s = CmriStateMachine::new();
        s.filter(0x64);

        assert!(s.address_filter.is_some());

        // Send a different address
        s.process(CMRI_PREAMBLE_BYTE).unwrap();
        s.process(CMRI_PREAMBLE_BYTE).unwrap();
        s.process(CMRI_START_BYTE).unwrap();
        let res = s.process(0x65);
        assert_eq!(res, Ok(Listening));
        assert_eq!(s.state, Idle);
        assert_eq!(s.message.position, 0);
    }

    #[test]
    fn buffer_overrun() {
        let mut s = CmriStateMachine::new();

        // Cheekily force the buffer to be "full"
        // Note that this is not possible for a library user because the
        // `position` member variable is private
        s.message.position = MAX_PAYLOAD_LEN - 3;
        s.message.push(3).unwrap();
        s.message.push(2).unwrap();
        s.message.push(1).unwrap();
        let res = s.message.push(0);
        assert_eq!(res, Err(Error::OutOfBounds));
    }

    /// Utility function to produce a state machine in the "accepting
    /// data" state to make testing later states easier
    fn get_to_data_section(addr: u8) -> Result<CmriStateMachine> {
        let mut s = CmriStateMachine::new();
        s.process(CMRI_PREAMBLE_BYTE)?;
        s.process(CMRI_PREAMBLE_BYTE)?;
        s.process(CMRI_START_BYTE)?;
        s.process(addr)?; // Address
        s.process(0x65)?; // Message type

        Ok(s)
    }
}
