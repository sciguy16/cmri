use crate::Error;
use core::convert::TryFrom;
use nom::*;

const CMRI_PREAMBLE_BYTE: u8 = 0xff;
const CMRI_START_BYTE: u8 = 0x02;
const CMRI_STOP_BYTE: u8 = 0x03;
const CMRI_ESCAPE_BYTE: u8 = 0x10;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MessageType {
    /// Initialisation
    Init = 'I' as isize,
    /// Controller -> Node
    Set = 'T' as isize,
    /// Node -> Controller
    Get = 'R' as isize,
    /// Controller requests status from node
    Poll = 'P' as isize,
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
}
