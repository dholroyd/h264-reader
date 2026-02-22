//! Parser for `access_unit_delimiter_rbsp()` (NAL type 9, spec 7.3.2.4).

use crate::rbsp::BitRead;
use std::fmt;

/// Indicates which slice types may be present in the primary coded picture
/// of the access unit (Table 7-5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimaryPicType {
    /// I slices only
    I = 0,
    /// I, P slices
    IP = 1,
    /// I, P, B slices
    IPB = 2,
    /// SI slices only
    SI = 3,
    /// SI, SP slices
    SISP = 4,
    /// I, SI slices
    ISI = 5,
    /// I, SI, P, SP slices
    ISIPSP = 6,
    /// I, SI, P, SP, B slices
    ISIPSPB = 7,
}

/// Error returned when a `primary_pic_type` value is out of range.
#[derive(Debug)]
pub struct PrimaryPicTypeError(pub u8);

impl fmt::Display for PrimaryPicTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid primary_pic_type value: {}", self.0)
    }
}

impl PrimaryPicType {
    pub fn from_id(id: u8) -> Result<PrimaryPicType, PrimaryPicTypeError> {
        match id {
            0 => Ok(PrimaryPicType::I),
            1 => Ok(PrimaryPicType::IP),
            2 => Ok(PrimaryPicType::IPB),
            3 => Ok(PrimaryPicType::SI),
            4 => Ok(PrimaryPicType::SISP),
            5 => Ok(PrimaryPicType::ISI),
            6 => Ok(PrimaryPicType::ISIPSP),
            7 => Ok(PrimaryPicType::ISIPSPB),
            _ => Err(PrimaryPicTypeError(id)),
        }
    }

    pub fn id(self) -> u8 {
        self as u8
    }
}

/// Parsed `access_unit_delimiter_rbsp()` (NAL unit type 9).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessUnitDelimiter {
    pub primary_pic_type: PrimaryPicType,
}

impl AccessUnitDelimiter {
    pub fn from_bits<R: BitRead>(mut r: R) -> Result<AccessUnitDelimiter, AudError> {
        let val: u8 = r.read(3, "primary_pic_type")?;
        let primary_pic_type =
            PrimaryPicType::from_id(val).map_err(AudError::InvalidPrimaryPicType)?;
        r.finish_rbsp()?;
        Ok(AccessUnitDelimiter { primary_pic_type })
    }
}

/// Error type for AUD parsing.
#[derive(Debug)]
pub enum AudError {
    InvalidPrimaryPicType(PrimaryPicTypeError),
    RbspError(crate::rbsp::BitReaderError),
}

impl From<crate::rbsp::BitReaderError> for AudError {
    fn from(e: crate::rbsp::BitReaderError) -> Self {
        AudError::RbspError(e)
    }
}

impl fmt::Display for AudError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudError::InvalidPrimaryPicType(e) => write!(f, "{}", e),
            AudError::RbspError(e) => write!(f, "{:?}", e),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::rbsp::BitReader;

    #[test]
    fn parse_all_pic_types() {
        for id in 0u8..=7 {
            // primary_pic_type(3 bits) + rbsp_stop_one_bit(1) + padding(4 zeros)
            let byte = (id << 5) | 0x10;
            let data = [byte];
            let aud = AccessUnitDelimiter::from_bits(BitReader::new(&data[..])).unwrap();
            assert_eq!(aud.primary_pic_type.id(), id);
        }
    }

    #[test]
    fn parse_ipb() {
        // primary_pic_type=2 (IPB): 010 1 0000 = 0x50
        let data = [0x50u8];
        let aud = AccessUnitDelimiter::from_bits(BitReader::new(&data[..])).unwrap();
        assert_eq!(aud.primary_pic_type, PrimaryPicType::IPB);
    }
}
