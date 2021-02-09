use crate::parser::bytes::streaming::take;
use crate::Error;
use core::convert::TryFrom;
use nom::error::ErrorKind;
use nom::*;

/// This is the length calculated from
/// https://github.com/madleech/ArduinoCMRI/blob/master/CMRI.h
/// (64 i/o cards @ 32 bits each + packet type and address bytes)
//const RX_BUFFER_LEN: usize = 258;
pub const MAX_PAYLOAD_LEN: usize = 256;

const CMRI_PREAMBLE_BYTE: u8 = 0xff;
const CMRI_START_BYTE: u8 = 0x02;
const CMRI_STOP_BYTE: u8 = 0x03;
const CMRI_ESCAPE_BYTE: u8 = 0x10;

#[derive(Copy, Clone, Debug)]
pub struct CmriMessage {
    pub address: u8,
    pub message_type: MessageType,
    pub payload: Payload,
}

/// To avoid allocations, we store payloads in a fixed-length buffer and
/// keep the length alongside
#[derive(Copy, Clone, Debug)]
pub struct Payload {
    buf: [u8; Self::MAX_LEN],
    len: usize,
}

impl Payload {
    pub const MAX_LEN: usize = MAX_PAYLOAD_LEN;

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn new() -> Self {
        Self {
            buf: [0; Self::MAX_LEN],
            len: 0,
        }
    }

    pub fn try_push(&mut self, inp: u8) -> Result<(), Error> {
        if self.len == Self::MAX_LEN {
            return Err(Error::DataTooLong);
        }
        self.buf[self.len] = inp;
        self.len += 1;
        Ok(())
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

impl Default for Payload {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u8)]
pub enum MessageType {
    /// Initialisation
    Init = 'I' as u8,
    /// Controller -> Node
    Set = 'T' as u8,
    /// Node -> Controller
    Get = 'R' as u8,
    /// Controller requests status from node
    Poll = 'P' as u8,
}

impl TryFrom<&[u8]> for MessageType {
    type Error = Error;
    fn try_from(t: &[u8]) -> Result<Self, Self::Error> {
        use MessageType::*;
        match t[0] as char {
            'I' => Ok(Init),
            'T' => Ok(Set),
            'R' => Ok(Get),
            'P' => Ok(Poll),
            _ => Err(Error::InvalidMessageType),
        }
    }
}

impl TryFrom<char> for MessageType {
    type Error = Error;
    fn try_from(t: char) -> Result<Self, Self::Error> {
        use MessageType::*;
        match t {
            'I' => Ok(Init),
            'T' => Ok(Set),
            'R' => Ok(Get),
            'P' => Ok(Poll),
            _ => Err(Error::InvalidMessageType),
        }
    }
}

impl core::fmt::Display for MessageType {
    fn fmt(
        &self,
        fmt: &mut core::fmt::Formatter<'_>,
    ) -> core::result::Result<(), core::fmt::Error> {
        write!(fmt, "{:?}", self)
    }
}

named!(
    preamble,
    tag!([CMRI_PREAMBLE_BYTE, CMRI_PREAMBLE_BYTE, CMRI_START_BYTE])
);

named!(
    message_type<MessageType>,
    map_res!(one_of!("ITRP"), MessageType::try_from)
);

named!(stop, tag!(&[CMRI_STOP_BYTE]));
named!(esc, tag!(&[CMRI_ESCAPE_BYTE]));

fn payload(inp: &[u8]) -> IResult<&[u8], Payload> {
    let mut payload = Payload::new();
    let mut is_escape = false;

    // Need to track the index into the input so that the correct slice
    // can be returned at the end
    for (idx, byte) in inp.iter().enumerate() {
        if is_escape {
            //TODO error if byte is not valid to have been escaped
            payload.try_push(*byte).map_err(|_| {
                nom::Err::Failure(nom::error::Error {
                    input: inp,
                    code: ErrorKind::TooLarge,
                })
            })?;

            is_escape = false;
        } else if *byte == CMRI_STOP_BYTE {
            // Message complete; return it
            // The remaining input is from the next index (current is the
            // STOP byte)
            return Ok((&inp[(idx + 1)..], payload));
        } else if *byte == CMRI_ESCAPE_BYTE {
            // Escape byte -> ignore this and just accept the next
            // byte
            is_escape = true;
        } else {
            // Any other bytes should be accepted
            payload.try_push(*byte).map_err(|_| {
                nom::Err::Failure(nom::error::Error {
                    input: inp,
                    code: ErrorKind::TooLarge,
                })
            })?;
        }
    }

    // Reached end of input without a STOP message
    Result::Err(Err::Incomplete(Needed::Unknown))
}

pub fn parse_cmri(inp: &[u8]) -> IResult<&[u8], CmriMessage> {
    let (inp, _) = preamble(inp)?;
    let (inp, addr) = take(1_usize)(inp)?;
    let (inp, message_type) = message_type(inp)?;
    let (inp, payload) = payload(inp)?;

    Ok((
        inp,
        CmriMessage {
            address: addr[0],
            message_type,
            payload,
        },
    ))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_preamble() {
        let pre = [CMRI_PREAMBLE_BYTE, CMRI_PREAMBLE_BYTE, CMRI_START_BYTE];

        let inp = b"\xff\xff\x02";
        let (_, parsed) = preamble(inp).unwrap();
        assert_eq!(parsed, pre);

        let inp = b"\xff\xfe\x02";
        let res = preamble(inp);
        assert!(res.is_err());
    }

    #[test]
    fn test_message_type() {
        let inp = b"I";
        let (_, parsed) = message_type(inp).unwrap();
        assert_eq!(parsed, MessageType::Init);

        let inp = b"T";
        let (_, parsed) = message_type(inp).unwrap();
        assert_eq!(parsed, MessageType::Set);

        let inp = b"R";
        let (_, parsed) = message_type(inp).unwrap();
        assert_eq!(parsed, MessageType::Get);

        let inp = b"P";
        let (_, parsed) = message_type(inp).unwrap();
        assert_eq!(parsed, MessageType::Poll);

        let inp = b"L";
        let res = message_type(inp);
        assert!(res.is_err());
    }

    #[test]
    fn test_stop() {
        let inp = b"\x03other stuff";
        let (_, parsed) = stop(inp).unwrap();
        assert_eq!(parsed, &[CMRI_STOP_BYTE]);

        let inp = b"\x04other";
        let res = stop(inp);
        assert!(res.is_err());
    }

    #[test]
    fn test_esc() {
        let inp = b"\x10other stuff";
        let (_, parsed) = esc(inp).unwrap();
        assert_eq!(parsed, &[CMRI_ESCAPE_BYTE]);

        let inp = b"\x11other";
        let res = esc(inp);
        assert!(res.is_err());
    }

    #[test]
    fn test_payload() {
        // Regular, valid message
        // STOP = 0x03
        let inp: [u8; 9] = *b"ABCDEFG\x03H";
        let (rem, p) = payload(&inp).unwrap();
        // Should match the input without the stop byte
        assert_eq!(p.as_slice(), b"ABCDEFG", "regular - values");
        std::println!("rem: {:?}", rem);
        assert_eq!(rem.len(), 1, "regular - remainder len");
        assert_eq!(rem[0], b'H', "regular - remainder val");

        // Test escape byte = 0x10
        let inp: &[u8] = &[
            0x13,
            0x12,
            0x11,
            CMRI_ESCAPE_BYTE,
            CMRI_ESCAPE_BYTE,
            0x09,
            0x08,
            CMRI_STOP_BYTE,
        ];
        let (rem, p) = payload(inp).unwrap();
        assert_eq!(
            p.as_slice(),
            &[0x13, 0x12, 0x11, 0x10, 0x09, 0x08],
            "esc - values"
        );
        std::println!("rem: {:?}", rem);
        assert_eq!(rem.len(), 0, "esc - remainder len");

        // Escape a few bytes to make sure the counters line up
        let inp: &[u8] = &[
            b'A',
            b'B',
            b'C',
            b'D',
            CMRI_ESCAPE_BYTE,
            CMRI_ESCAPE_BYTE,
            b'E',
            b'F',
            CMRI_ESCAPE_BYTE,
            CMRI_STOP_BYTE,
            CMRI_ESCAPE_BYTE,
            CMRI_STOP_BYTE,
            b'G',
            b'H',
            b'I',
            CMRI_ESCAPE_BYTE,
            CMRI_ESCAPE_BYTE,
            CMRI_ESCAPE_BYTE,
            CMRI_STOP_BYTE,
            CMRI_STOP_BYTE,
            b'S',
            b'P',
            b'A',
            b'R',
            b'E',
        ];
        let (rem, p) = payload(inp).unwrap();
        assert_eq!(p.as_slice(), b"ABCD\x10EF\x03\x03GHI\x10\x03");
        assert_eq!(rem, b"SPARE");

        // Test a message that's too short and missing its STOP byte
        let inp: &[u8] = b"HELLO THIS IS A MESSAGE";
        let res = payload(&inp);
        assert!(res.is_err());
        if let Err(nom::Err::Incomplete(_)) = res {
            // all OK!
        } else {
            panic!("Expected INCOMPLETE error");
        }

        // Test a message that never terminates (and is too long)
        let inp: &[u8] = &[b'A'; MAX_PAYLOAD_LEN + 10];
        let res = payload(inp);

        assert!(res.is_err());
        if let Err(nom::Err::Failure(e)) = res {
            assert_eq!(e.code, ErrorKind::TooLarge);
        } else {
            panic!("Expected FAILURE error!");
        }
    }

    #[test]
    fn decode_full_message() {
        use MessageType::*;
        #[rustfmt::skip]
        let message1 = [
            CMRI_PREAMBLE_BYTE,
            CMRI_PREAMBLE_BYTE,
            CMRI_START_BYTE,
            0x86,                               // Address
            Init as u8,                         // Type
            0x41, 0x41, 0x41, 0x41,             // Message
            CMRI_STOP_BYTE,
        ];

        let (_, msg1) = parse_cmri(&message1).unwrap();
        assert_eq!(msg1.address, 0x86);
        assert_eq!(msg1.message_type, Init);
        assert_eq!(msg1.payload.as_slice(), b"AAAA");

        #[rustfmt::skip]
        let message2 = [
            CMRI_PREAMBLE_BYTE,
            CMRI_PREAMBLE_BYTE,
            CMRI_START_BYTE,
            0xa2,                               // Address
            Init as u8,                         // Type
            0x41, 0x41, 0x41, 0x41,             // Message
            CMRI_ESCAPE_BYTE, CMRI_STOP_BYTE,   // Message
            0x42, 0x42, 0x42, 0x42,             // Message
            CMRI_ESCAPE_BYTE, CMRI_ESCAPE_BYTE, // Message
            0x43, 0x43, 0x43, 0x43,             // Message
            CMRI_STOP_BYTE,
        ];

        let (_, msg2) = parse_cmri(&message2).unwrap();
        assert_eq!(msg2.address, 0xa2);
        assert_eq!(msg2.message_type, Init);
        assert_eq!(msg2.payload.as_slice(), b"AAAA\x03BBBB\x10CCCC");
    }
}
