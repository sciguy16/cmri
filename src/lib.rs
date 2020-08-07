#![no_std]

pub use error::{Error, Result};

mod error;

/// This is the length calculated from
/// https://github.com/madleech/ArduinoCMRI/blob/master/CMRI.h
const RX_BUFFER_LEN: usize = 258;

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
    payload: [u8; RX_BUFFER_LEN],
    position: usize,
}

impl CmriStateMachine {
    pub fn new() -> Self {
        Self {
            state: CmriState::Idle,
            payload: [0_u8; RX_BUFFER_LEN],
            position: 0,
        }
    }

    /// Returns the current state of the system
    pub fn state(&self) -> CmriState {
        self.state
    }

    /// Push a byte onto the rx buffer
    fn push(&mut self, byte: u8) -> Result<()> {
        if self.position == RX_BUFFER_LEN {
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
        self.payload = [0_u8; RX_BUFFER_LEN];
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
                    self.push(byte)?;
                    self.state = Attn;
                }
                // Ignore other bytes while Idle
            }
            Attn => {
                // Attn to Start if byte is PREAMBLE
                if byte == CMRI_PREAMBLE_BYTE {
                    self.push(byte)?;
                    self.state = Start;
                } else {
                    // Otherwise discard and reset to Idle
                    self.clear();
                    self.state = Idle;
                }
            }
            Start => {
                // start byte must be valid
                if byte == CMRI_START_BYTE {
                    self.push(byte)?;
                    self.state = Addr;
                } else {
                    // Otherwise discard and reset to Idle
                    self.clear();
                    self.state = Idle;
                }
            }
            Addr => {
                // Take the next byte as-is for an address
                //TODO implement address filter
                self.push(byte)?;
                self.state = Type;
            }
            Type => {
                // Take the next byte as-is for message type
                self.push(byte)?;
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
                        self.push(byte)?;
                        self.state = Idle;
                        return Ok(RxState::Complete);
                    }
                    _ => {
                        // any other byte we take as data
                        self.push(byte)?;
                    }
                }
            }
            Escape => {
                // Escape the next byte, so accept it as data.
                self.push(byte)?;
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

#[cfg(test)]
mod test {
    use super::*;

    use CmriState::*;
    use RxState::*;

    #[test]
    fn basic_create_state_machine() {
        let s = CmriStateMachine::new();
        assert_eq!(s.state(), CmriState::Idle);
        assert_eq!(s.payload.len(), RX_BUFFER_LEN);
        assert_eq!(s.position, 0);
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
        assert_eq!(s.position, 0);
        assert_eq!(s.payload[0], 0);
        assert_eq!(s.state, Idle);

        // Send a preamble byte and check that the state has changed to Attn
        let res = s.process(CMRI_PREAMBLE_BYTE).unwrap();
        assert_eq!(res, Listening);
        assert_eq!(s.state, Attn);
        assert_eq!(s.payload[0], 0xff);
        assert_eq!(s.position, 1);
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
        assert_eq!(s.position, 2);
        let res = s.process(0x32);
        assert_eq!(res, Ok(Listening));
        assert_eq!(s.state, Idle);
        assert_eq!(s.position, 0);
    }

    // Skip Addr and Type because they can each be any byte

    #[test]
    fn decode_escape_byte() {
        let mut s = get_to_data_section().unwrap();
        // Normal message byte
        let res = s.process(5);
        assert_eq!(res, Ok(Listening));
        assert_eq!(s.state, Data);

        // Escape byte, should not advance the position
        let pos = s.position;
        let res = s.process(CMRI_ESCAPE_BYTE);
        assert_eq!(res, Ok(Listening));
        assert_eq!(s.state, Escape);
        assert_eq!(s.position, pos);

        // Send an escape byte again, should be escaped and state back
        // to accepting data
        let res = s.process(CMRI_ESCAPE_BYTE);
        assert_eq!(res, Ok(Listening));
        assert_eq!(s.state, Data);
        assert_eq!(s.position, pos + 1);

        // Escape byte, should not advance the position
        let pos = s.position;
        let res = s.process(CMRI_ESCAPE_BYTE);
        assert_eq!(res, Ok(Listening));
        assert_eq!(s.state, Escape);
        assert_eq!(s.position, pos);

        // Send a stop byte, should be escaped and state back
        // to accepting data
        let res = s.process(CMRI_STOP_BYTE);
        assert_eq!(res, Ok(Listening));
        assert_eq!(s.state, Data);
        assert_eq!(s.position, pos + 1);
    }

    #[test]
    fn decode_stop_byte() {
        let mut s = get_to_data_section().unwrap();

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
    fn buffer_overrun() {
        let mut s = CmriStateMachine::new();

        // Cheekily force the buffer to be "full"
        // Note that this is not possible for a library user because the
        // `position` member variable is private
        s.position = RX_BUFFER_LEN - 3;
        s.push(3).unwrap();
        s.push(2).unwrap();
        s.push(1).unwrap();
        let res = s.push(0);
        assert_eq!(res, Err(Error::OutOfBounds));
    }

    /// Utility function to produce a state machine in the "accepting
    /// data" state to make testing later states easier
    fn get_to_data_section() -> Result<CmriStateMachine> {
        let mut s = CmriStateMachine::new();
        s.process(CMRI_PREAMBLE_BYTE)?;
        s.process(CMRI_PREAMBLE_BYTE)?;
        s.process(CMRI_START_BYTE)?;
        s.process(0x43)?; // Address
        s.process(0x65)?; // Message type

        Ok(s)
    }
}
