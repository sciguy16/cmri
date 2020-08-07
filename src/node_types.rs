use crate::error::Error;
use core::convert::TryFrom;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum NodeType {
    /// Classic USICand for SUSIC using 24 bit input/output cards.
    Usic = 'N' as isize,
    /// SUSIC using32 bit input/output cards.
    Susic = 'X' as isize,
    /// SMINI with fixed 24 inputs and 48 outputs
    Smini = 'M' as isize,
    /// CPNODEwith 16 to 144input/outputs using8 bit cards.
    Cpnode = 'C' as isize,
}

impl TryFrom<u8> for NodeType {
    type Error = Error;
    fn try_from(nt: u8) -> Result<Self, Error> {
        use NodeType::*;
        match nt as char {
            'N' => Ok(Usic),
            'X' => Ok(Susic),
            'M' => Ok(Smini),
            'C' => Ok(Cpnode),
            _ => Err(Error::InvalidNodeType),
        }
    }
}

impl core::fmt::Display for NodeType {
    fn fmt(
        &self,
        fmt: &mut core::fmt::Formatter<'_>,
    ) -> core::result::Result<(), core::fmt::Error> {
        write!(fmt, "{:?}", self)
    }
}
