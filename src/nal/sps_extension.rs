//! Parser for `seq_parameter_set_extension_rbsp()` (NAL type 13, spec 7.3.2.1.2).

use crate::nal::sps::{SeqParamSetId, SpsError};
use crate::rbsp::BitRead;

/// Auxiliary format information, present when `aux_format_idc != 0`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuxFormatInfo {
    pub bit_depth_aux_minus8: u8,
    pub alpha_incr_flag: bool,
    pub alpha_opaque_value: u32,
    pub alpha_transparent_value: u32,
}

/// Parsed `seq_parameter_set_extension_rbsp()` (NAL unit type 13).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeqParameterSetExtension {
    pub seq_parameter_set_id: SeqParamSetId,
    pub aux_format_idc: u32,
    pub aux_format_info: Option<AuxFormatInfo>,
    pub additional_extension_flag: bool,
}

impl SeqParameterSetExtension {
    pub fn from_bits<R: BitRead>(mut r: R) -> Result<SeqParameterSetExtension, SpsError> {
        let seq_parameter_set_id = SeqParamSetId::from_u32(r.read_ue("seq_parameter_set_id")?)
            .map_err(SpsError::BadSeqParamSetId)?;
        let aux_format_idc = r.read_ue("aux_format_idc")?;
        let aux_format_info = if aux_format_idc != 0 {
            let bit_depth_aux_minus8 = r.read_ue("bit_depth_aux_minus8")?;
            if bit_depth_aux_minus8 > 4 {
                return Err(SpsError::BitDepthOutOfRange(bit_depth_aux_minus8));
            }
            let bit_depth_aux_minus8 = bit_depth_aux_minus8 as u8;
            let alpha_incr_flag = r.read_bool("alpha_incr_flag")?;
            let v = bit_depth_aux_minus8 as u32 + 9;
            let alpha_opaque_value = r.read(v, "alpha_opaque_value")?;
            let alpha_transparent_value = r.read(v, "alpha_transparent_value")?;
            Some(AuxFormatInfo {
                bit_depth_aux_minus8,
                alpha_incr_flag,
                alpha_opaque_value,
                alpha_transparent_value,
            })
        } else {
            None
        };
        let additional_extension_flag = r.read_bool("additional_extension_flag")?;
        r.finish_rbsp()?;
        Ok(SeqParameterSetExtension {
            seq_parameter_set_id,
            aux_format_idc,
            aux_format_info,
            additional_extension_flag,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::rbsp::BitReader;

    #[test]
    fn parse_minimal() {
        // seq_parameter_set_id=0 (ue: 1 bit '1')
        // aux_format_idc=0 (ue: 1 bit '1')
        // additional_extension_flag=0 (1 bit)
        // rbsp_stop_one_bit=1 + alignment padding
        // bits: 1 1 0 1 0000 = 0xD0
        let data = [0xD0u8];
        let ext = SeqParameterSetExtension::from_bits(BitReader::new(&data[..])).unwrap();
        assert_eq!(
            ext.seq_parameter_set_id,
            SeqParamSetId::from_u32(0).unwrap()
        );
        assert_eq!(ext.aux_format_idc, 0);
        assert!(ext.aux_format_info.is_none());
        assert!(!ext.additional_extension_flag);
    }

    #[test]
    fn parse_with_aux_format() {
        // seq_parameter_set_id=0 (ue: '1')               1 bit
        // aux_format_idc=1 (ue: '010')                   3 bits
        // bit_depth_aux_minus8=0 (ue: '1')                1 bit
        // alpha_incr_flag=0                               1 bit
        // alpha_opaque_value: u(9) = 0x1FF                9 bits
        // alpha_transparent_value: u(9) = 0x000           9 bits
        // additional_extension_flag=0                     1 bit
        // rbsp_stop_one_bit=1 + padding                   1 bit + 6 pad
        //
        // bits: 1 010 1 0 111111111 000000000 0 1 000000
        //       byte 0:  1010_1011 = 0xAB
        //       byte 1:  1111_1110 = 0xFE
        //       byte 2:  0000_0000 = 0x00
        //       byte 3:  0100_0000 = 0x40
        let data = [0xABu8, 0xFE, 0x00, 0x40];
        let ext = SeqParameterSetExtension::from_bits(BitReader::new(&data[..])).unwrap();
        assert_eq!(
            ext.seq_parameter_set_id,
            SeqParamSetId::from_u32(0).unwrap()
        );
        assert_eq!(ext.aux_format_idc, 1);
        let info = ext.aux_format_info.as_ref().unwrap();
        assert_eq!(info.bit_depth_aux_minus8, 0);
        assert!(!info.alpha_incr_flag);
        assert_eq!(info.alpha_opaque_value, 0x1FF);
        assert_eq!(info.alpha_transparent_value, 0x000);
        assert!(!ext.additional_extension_flag);
    }
}
