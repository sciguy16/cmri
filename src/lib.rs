#![no_std]

pub use error::{Error, Result};

mod error;

/// This is the length calculated from
/// https://github.com/madleech/ArduinoCMRI/blob/master/CMRI.h
/// (64 i/o cards @ 32 bits each + packet type and address bytes)
//const RX_BUFFER_LEN: usize = 258;
const MAX_PAYLOAD_LEN: usize = 256;
/// * Payload is MAX_PAYLOAD_LEN
/// * Headers are 2x PREAMBLE and a START: 3
/// * Address and type: 2
/// * Trailers are 1x STOP: 1
/// * Then some unknown number of escape bytes, up to MAX_PAYLOAD_LEN
/// Implementations may be be able to get away with a smaller buffer if
/// memory is highly constrained
const TX_BUFFER_LEN: usize = 2 * MAX_PAYLOAD_LEN + 3 + 2 + 1;

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
    len: usize,
}

impl CmriMessage {
    pub fn new() -> Self {
        Self {
            address: 0,
            message_type: 0,
            payload: [0; MAX_PAYLOAD_LEN],
            len: 0,
        }
    }

    /// Push a byte onto the payload
    fn push(&mut self, byte: u8) -> Result<()> {
        if self.len == MAX_PAYLOAD_LEN {
            // Buffer is full, which is problematic
            return Err(Error::OutOfBounds);
        }
        self.payload[self.len] = byte;
        self.len += 1;
        Ok(())
    }

    /// Empty the rx buffer
    fn clear(&mut self) {
        self.len = 0;
        self.payload.iter_mut().for_each(|x| *x = 0);
    }

    /// Encode the message into a transmit buffer
    pub fn encode(self, buf: &mut [u8; TX_BUFFER_LEN]) {
        let mut pos: usize = 0;

        // Two PREAMBLEs
        buf[pos] = CMRI_PREAMBLE_BYTE;
        pos += 1;
        buf[pos] = CMRI_PREAMBLE_BYTE;
        pos += 1;

        // One START
        buf[pos] = CMRI_START_BYTE;
        pos += 1;

        // One ADDRESS
        buf[pos] = self.address;
        pos += 1;

        // One TYPE
        buf[pos] = self.message_type;
        pos += 1;

        // Insert the PAYLOAD
        for payload_byte in self.payload[..self.len].iter() {
            if needs_escape(*payload_byte) {
                buf[pos] = CMRI_ESCAPE_BYTE;
                pos += 1;
            }

            buf[pos] = *payload_byte;
            pos += 1;
        }

        // One STOP
        buf[pos] = CMRI_STOP_BYTE;
        //pos += 1;
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

    /// Gets a reference to the decoded message
    pub fn message(&self) -> &CmriMessage {
        &self.message
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

/// Returns TRUE if the byte is one which needs escaping; currently only
/// STOP and ESCAPE
fn needs_escape(byte: u8) -> bool {
    byte == CMRI_STOP_BYTE || byte == CMRI_ESCAPE_BYTE
}

/// Takes a slice and embeds it in a payload array
pub fn payload_from_slice(
    payload_buffer: &mut [u8; MAX_PAYLOAD_LEN],
    input_payload: &[u8],
) -> Result<()> {
    if input_payload.len() > MAX_PAYLOAD_LEN {
        return Err(Error::DataTooLong);
    }
    for (src, dst) in input_payload.iter().zip(payload_buffer.iter_mut()) {
        *dst = *src;
    }
    Ok(())
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
        assert_eq!(s.message.len, 0);
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
        assert_eq!(s.message.len, 0);
        assert_eq!(s.message.payload[0], 0);
        assert_eq!(s.state, Idle);

        // Send a preamble byte and check that the state has changed to Attn
        let res = s.process(CMRI_PREAMBLE_BYTE).unwrap();
        assert_eq!(res, Listening);
        assert_eq!(s.state, Attn);
        assert_eq!(s.message.len, 0); // preamble does not get saved
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
        assert_eq!(s.message.len, 0);
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
        let pos = s.message.len;
        let res = s.process(CMRI_ESCAPE_BYTE);
        assert_eq!(res, Ok(Listening));
        assert_eq!(s.state, Escape);
        assert_eq!(s.message.len, pos);

        // Send an escape byte again, should be escaped and state back
        // to accepting data
        let res = s.process(CMRI_ESCAPE_BYTE);
        assert_eq!(res, Ok(Listening));
        assert_eq!(s.state, Data);
        assert_eq!(s.message.len, pos + 1);

        // Escape byte, should not advance the position
        let pos = s.message.len;
        let res = s.process(CMRI_ESCAPE_BYTE);
        assert_eq!(res, Ok(Listening));
        assert_eq!(s.state, Escape);
        assert_eq!(s.message.len, pos);

        // Send a stop byte, should be escaped and state back
        // to accepting data
        let res = s.process(CMRI_STOP_BYTE);
        assert_eq!(res, Ok(Listening));
        assert_eq!(s.state, Data);
        assert_eq!(s.message.len, pos + 1);
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
        assert_eq!(s.message.len, 0);
    }

    #[test]
    fn decode_full_message() {
        #[rustfmt::skip]
        let message = [
            CMRI_PREAMBLE_BYTE,
            CMRI_PREAMBLE_BYTE,
            CMRI_START_BYTE,
            0x86, // Address
            0x12, // Type
            0x41, 0x41, 0x41, 0x41, // Message
            CMRI_STOP_BYTE,
        ];

        #[rustfmt::skip]
        let message2 = [
            CMRI_PREAMBLE_BYTE,
            CMRI_PREAMBLE_BYTE,
            CMRI_START_BYTE,
            0xa2, // Address
            0x12, // Type
            0x41, 0x41, 0x41, 0x41, // Message
            CMRI_STOP_BYTE,
        ];

        let mut s = CmriStateMachine::new();

        // Decode the message
        for byte in message.iter() {
            s.process(*byte).unwrap();
        }

        let m = s.message();
        assert_eq!(m.address, 0x86);
        assert_eq!(m.message_type, 0x12);
        assert_eq!(m.payload[..(m.len)], [0x41, 0x41, 0x41, 0x41]);

        // Enable a filter
        s.filter(0x86);
        // Decode the message, excluding final stop byte
        for byte in message[..message.len() - 1].iter() {
            s.process(*byte).unwrap();
        }
        // Decode the final stop byte, capturing the response code
        let res = s.process(message[message.len() - 1]);
        assert_eq!(res, Ok(Complete));

        let m = s.message();
        assert_eq!(m.address, 0x86);
        assert_eq!(m.message_type, 0x12);
        assert_eq!(m.payload[..(m.len)], [0x41, 0x41, 0x41, 0x41]);

        // Decode the message
        for byte in message2.iter() {
            s.process(*byte).unwrap();
        }

        let m = s.message();
        assert_eq!(m.len, 0);
    }

    #[test]
    fn buffer_overrun() {
        let mut s = CmriStateMachine::new();

        // Cheekily force the buffer to be "full"
        // Note that this is not possible for a library user because the
        // `position` member variable is private
        s.message.len = MAX_PAYLOAD_LEN - 3;
        s.message.push(3).unwrap();
        s.message.push(2).unwrap();
        s.message.push(1).unwrap();
        let res = s.message.push(0);
        assert_eq!(res, Err(Error::OutOfBounds));
    }

    #[test]
    fn encode_a_message() {
        let mut payload_buffer = [0_u8; MAX_PAYLOAD_LEN];
        payload_from_slice(&mut payload_buffer, &[0x41, 0x41, 0x43]).unwrap();
        let m = CmriMessage {
            address: 0x58,
            message_type: 0x31,
            payload: payload_buffer,
            len: 3,
        };

        let mut tx_buffer = [0_u8; TX_BUFFER_LEN];
        m.encode(&mut tx_buffer);

        assert_eq!(
            tx_buffer[..9],
            [
                CMRI_PREAMBLE_BYTE,
                CMRI_PREAMBLE_BYTE,
                CMRI_START_BYTE,
                0x58, // Address
                0x31, // Type
                0x41,
                0x41,
                0x43,
                CMRI_STOP_BYTE,
            ]
        );
    }

    #[test]
    fn encode_a_worst_case_message() {}

    #[test]
    fn test_payload_from_slice() {
        let mut payload_buffer = [0_u8; MAX_PAYLOAD_LEN];
        let input = [1_u8, 2, 3];

        // Test a valid input
        payload_from_slice(&mut payload_buffer, &input).unwrap();
        assert_eq!(payload_buffer[..input.len()], input);
        assert_eq!(payload_buffer[input.len()], 0);

        // Test a too-long input
        let input = [5_u8; MAX_PAYLOAD_LEN + 1];
        let res = payload_from_slice(&mut payload_buffer, &input);
        assert_eq!(res, Err(Error::DataTooLong));
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
