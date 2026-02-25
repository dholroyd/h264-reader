use super::SeiMessage;
use crate::nal::sei::HeaderType;
use crate::rbsp::BitRead;
use crate::rbsp::BitReaderError;

#[derive(Debug)]
pub enum RecoveryPointError {
    ReaderError(BitReaderError),
}
impl From<BitReaderError> for RecoveryPointError {
    fn from(e: BitReaderError) -> Self {
        RecoveryPointError::ReaderError(e)
    }
}

/// Parsed `recovery_point()` SEI message (payloadType == 6, spec D.1.7).
///
/// Signals a recovery point in the bitstream, allowing decoders to identify
/// open-GOP random access points where correct decoding can begin.
#[derive(Debug, PartialEq, Eq)]
pub struct RecoveryPoint {
    pub recovery_frame_cnt: u32,
    pub exact_match_flag: bool,
    pub broken_link_flag: bool,
    pub changing_slice_group_idc: u8,
}

impl RecoveryPoint {
    pub fn read(msg: &SeiMessage<'_>) -> Result<Self, RecoveryPointError> {
        assert_eq!(msg.payload_type, HeaderType::RecoveryPoint);
        let mut r = crate::rbsp::BitReader::new(msg.payload);
        let recovery_frame_cnt = r.read_ue("recovery_frame_cnt")?;
        let exact_match_flag = r.read_bit("exact_match_flag")?;
        let broken_link_flag = r.read_bit("broken_link_flag")?;
        let changing_slice_group_idc = r.read::<2, u8>("changing_slice_group_idc")?;
        r.finish_sei_payload()?;
        Ok(RecoveryPoint {
            recovery_frame_cnt,
            exact_match_flag,
            broken_link_flag,
            changing_slice_group_idc,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse() {
        // recovery_frame_cnt=0 (ue: 1)
        // exact_match_flag=1
        // broken_link_flag=0
        // changing_slice_group_idc=0 (u(2): 00)
        // trailing: 1_000 (stop bit + padding)
        // Bits: 1_1_0_00_1_000 = 0b1100_0100 = 0xC4
        let msg = SeiMessage {
            payload_type: HeaderType::RecoveryPoint,
            payload: &[0xC4],
        };
        let rp = RecoveryPoint::read(&msg).unwrap();
        assert_eq!(rp.recovery_frame_cnt, 0);
        assert!(rp.exact_match_flag);
        assert!(!rp.broken_link_flag);
        assert_eq!(rp.changing_slice_group_idc, 0);
    }

    #[test]
    fn parse_nonzero_frame_cnt() {
        // recovery_frame_cnt=5 (ue: 00110)
        // exact_match_flag=0
        // broken_link_flag=1
        // changing_slice_group_idc=1 (u(2): 01)
        // trailing: 1_0 (stop bit + padding)
        // Bits: 00110_0_1_01_1_0 = 0b0011_0010_1100_0000 = 0x32, 0xC0
        let msg = SeiMessage {
            payload_type: HeaderType::RecoveryPoint,
            payload: &[0x32, 0xC0],
        };
        let rp = RecoveryPoint::read(&msg).unwrap();
        assert_eq!(rp.recovery_frame_cnt, 5);
        assert!(!rp.exact_match_flag);
        assert!(rp.broken_link_flag);
        assert_eq!(rp.changing_slice_group_idc, 1);
    }
}
