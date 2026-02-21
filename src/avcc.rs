//! Support for handling _Advanced Video Coding Configuration_ data, used in the _ISO Base Media
//! File Format_ (AKA MP4), as the specified in _ISO/IEC 14496-15_.
//!

use crate::nal::sps::{ConstraintFlags, Level, ProfileIdc, SeqParameterSet};
use crate::nal::{pps, sps, Nal, NalHeader, NalHeaderError, RefNal, UnitType};
use crate::Context;
use std::convert::TryFrom;

#[derive(Debug)]
pub enum AvccError {
    NotEnoughData {
        expected: usize,
        actual: usize,
    },
    /// The AvcDecoderConfigurationRecord used a version number other than `1`.
    UnsupportedConfigurationVersion(u8),
    ParamSet(ParamSetError),
    Sps(sps::SpsError),
    Pps(pps::PpsError),
}

pub struct AvcDecoderConfigurationRecord<'buf> {
    data: &'buf [u8],
    /// Byte offset of extension fields (chroma_format, bit_depth, SPS ext) if present.
    extension_offset: Option<usize>,
}
impl<'buf> TryFrom<&'buf [u8]> for AvcDecoderConfigurationRecord<'buf> {
    type Error = AvccError;

    fn try_from(data: &'buf [u8]) -> Result<Self, Self::Error> {
        let avcc = AvcDecoderConfigurationRecord {
            data,
            extension_offset: None,
        };
        // we must confirm we have enough bytes for all fixed fields before we do anything else,
        avcc.ck(Self::MIN_CONF_SIZE)?;
        if avcc.configuration_version() != 1 {
            // The spec requires that decoders ignore streams where the version number is not 1,
            // indicating there was an incompatible change in the configuration format,
            return Err(AvccError::UnsupportedConfigurationVersion(
                avcc.configuration_version(),
            ));
        }
        // Do a whole load of work to ensure that the buffer is large enough for all the optional
        // fields actually indicated to be present, so that we don't have to put these checks into
        // the accessor functions of individual fields,
        let mut len = avcc.seq_param_sets_end()?;

        avcc.ck(len + 1)?;
        let mut num_pps = data[len];
        len += 1;
        while num_pps > 0 {
            avcc.ck(len + 2)?;
            let pps_len = (u16::from(data[len]) << 8 | u16::from(data[len + 1])) as usize;
            len += 2;
            avcc.ck(len + pps_len)?;
            len += pps_len;
            num_pps -= 1;
        }

        // Per ISO/IEC 14496-15, profiles with chroma info have extension fields after the PPS
        // array: chroma_format, bit_depth_luma_minus8, bit_depth_chroma_minus8, and an optional
        // array of SPS extension NAL units.
        let extension_offset = if avcc.avc_profile_indication().has_chroma_info()
            && data.len() > len
        {
            let ext_start = len;
            avcc.ck(len + 4)?;
            len += 3; // chroma_format, bit_depth_luma_minus8, bit_depth_chroma_minus8
            let num_sps_ext = data[len] as usize;
            len += 1;
            for _ in 0..num_sps_ext {
                avcc.ck(len + 2)?;
                let sps_ext_len = (u16::from(data[len]) << 8 | u16::from(data[len + 1])) as usize;
                len += 2;
                avcc.ck(len + sps_ext_len)?;
                len += sps_ext_len;
            }
            Some(ext_start)
        } else {
            None
        };

        Ok(AvcDecoderConfigurationRecord {
            data,
            extension_offset,
        })
    }
}
impl<'buf> AvcDecoderConfigurationRecord<'buf> {
    const MIN_CONF_SIZE: usize = 6;

    fn seq_param_sets_end(&self) -> Result<usize, AvccError> {
        let mut num_sps = self.num_of_sequence_parameter_sets();
        let mut len = Self::MIN_CONF_SIZE;
        while num_sps > 0 {
            self.ck(len + 2)?;
            let sps_len = (u16::from(self.data[len]) << 8 | u16::from(self.data[len + 1])) as usize;
            len += 2;
            self.ck(len + sps_len)?;
            len += sps_len;
            num_sps -= 1;
        }
        Ok(len)
    }
    fn ck(&self, len: usize) -> Result<(), AvccError> {
        if self.data.len() < len {
            Err(AvccError::NotEnoughData {
                expected: len,
                actual: self.data.len(),
            })
        } else {
            Ok(())
        }
    }
    pub fn configuration_version(&self) -> u8 {
        self.data[0]
    }
    pub fn num_of_sequence_parameter_sets(&self) -> usize {
        (self.data[5] & 0b0001_1111) as usize
    }
    pub fn avc_profile_indication(&self) -> ProfileIdc {
        self.data[1].into()
    }
    pub fn profile_compatibility(&self) -> ConstraintFlags {
        self.data[2].into()
    }
    pub fn avc_level_indication(&self) -> Level {
        Level::from_constraint_flags_and_level_idc(self.profile_compatibility(), self.data[3])
    }
    /// Number of bytes used to specify the length of each NAL unit
    /// 0 => 1 byte, 1 => 2 bytes, 2 => 3 bytes, 3 => 4 bytes
    pub fn length_size_minus_one(&self) -> u8 {
        self.data[4] & 0b0000_0011
    }
    pub fn sequence_parameter_sets(
        &self,
    ) -> impl Iterator<Item = Result<&'buf [u8], ParamSetError>> {
        let num = self.num_of_sequence_parameter_sets();
        let data = &self.data[Self::MIN_CONF_SIZE..];
        ParamSetIter::new(data, UnitType::SeqParameterSet).take(num)
    }
    /// Returns the chroma format (0-3) from the extension fields, if present.
    pub fn chroma_format(&self) -> Option<u8> {
        self.extension_offset
            .map(|off| self.data[off] & 0b0000_0011)
    }
    /// Returns bit_depth_luma_minus8 (0-7) from the extension fields, if present.
    pub fn bit_depth_luma_minus8(&self) -> Option<u8> {
        self.extension_offset
            .map(|off| self.data[off + 1] & 0b0000_0111)
    }
    /// Returns bit_depth_chroma_minus8 (0-7) from the extension fields, if present.
    pub fn bit_depth_chroma_minus8(&self) -> Option<u8> {
        self.extension_offset
            .map(|off| self.data[off + 2] & 0b0000_0111)
    }
    pub fn sequence_parameter_set_extensions(
        &self,
    ) -> impl Iterator<Item = Result<&'buf [u8], ParamSetError>> + 'buf {
        let (data, num) = if let Some(off) = self.extension_offset {
            let num = self.data[off + 3] as usize;
            (&self.data[off + 4..], num)
        } else {
            (&self.data[..0], 0)
        };
        ParamSetIter::new(data, UnitType::SeqParameterSetExtension).take(num)
    }
    pub fn picture_parameter_sets(
        &self,
    ) -> impl Iterator<Item = Result<&'buf [u8], ParamSetError>> + 'buf {
        let offset = self.seq_param_sets_end().unwrap();
        let num = self.data[offset];
        let data = &self.data[offset + 1..];
        ParamSetIter::new(data, UnitType::PicParameterSet).take(num as usize)
    }

    /// Creates an H264 parser context, using the settings encoded into
    /// this `AvcDecoderConfigurationRecord`.
    ///
    /// In particular, the _sequence parameter set_ and _picture parameter set_ values of this
    /// configuration record will be inserted into the resulting context.
    pub fn create_context(&self) -> Result<Context, AvccError> {
        let mut ctx = Context::new();
        for sps in self.sequence_parameter_sets() {
            let sps = sps.map_err(AvccError::ParamSet)?;
            let sps = RefNal::new(&sps[..], &[], true);
            let sps = crate::nal::sps::SeqParameterSet::from_bits(sps.rbsp_bits())
                .map_err(AvccError::Sps)?;
            ctx.put_seq_param_set(sps);
        }
        for pps in self.picture_parameter_sets() {
            let pps = pps.map_err(AvccError::ParamSet)?;
            let pps = RefNal::new(&pps[..], &[], true);
            let pps = crate::nal::pps::PicParameterSet::from_bits(&ctx, pps.rbsp_bits())
                .map_err(AvccError::Pps)?;
            ctx.put_pic_param_set(pps);
        }
        Ok(ctx)
    }
}

#[derive(Debug)]
pub enum ParamSetError {
    NalHeader(NalHeaderError),
    IncorrectNalType {
        expected: UnitType,
        actual: UnitType,
    },
    /// A _sequence parameter set_ found within the AVC decoder config was not consistent with the
    /// settings of the decoder config itself
    IncompatibleSps(SeqParameterSet),
}

struct ParamSetIter<'buf>(&'buf [u8], UnitType);

impl<'buf> ParamSetIter<'buf> {
    pub fn new(buf: &'buf [u8], unit_type: UnitType) -> ParamSetIter<'buf> {
        ParamSetIter(buf, unit_type)
    }
}
impl<'buf> Iterator for ParamSetIter<'buf> {
    type Item = Result<&'buf [u8], ParamSetError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_empty() {
            None
        } else {
            let len = u16::from(self.0[0]) << 8 | u16::from(self.0[1]);
            let data = &self.0[2..];
            let res = match NalHeader::new(data[0]) {
                Ok(nal_header) => {
                    if nal_header.nal_unit_type() == self.1 {
                        let (data, remainder) = data.split_at(len as usize);
                        self.0 = remainder;
                        Ok(data)
                    } else {
                        Err(ParamSetError::IncorrectNalType {
                            expected: self.1,
                            actual: nal_header.nal_unit_type(),
                        })
                    }
                }
                Err(err) => Err(ParamSetError::NalHeader(err)),
            };
            Some(res)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::nal::pps::PicParamSetId;
    use crate::nal::sps::SeqParamSetId;
    use hex_literal::*;

    #[test]
    fn it_works() {
        let avcc_data = hex!("0142c01e ffe10020 6742c01e b91061ff 78088000 00030080 00001971 3006d600 daf7bdc0 7c2211a8 01000468 de3c80");
        let avcc = AvcDecoderConfigurationRecord::try_from(&avcc_data[..]).unwrap();
        assert_eq!(1, avcc.configuration_version());
        assert_eq!(1, avcc.num_of_sequence_parameter_sets());
        assert_eq!(ProfileIdc::from(66), avcc.avc_profile_indication());
        let flags = avcc.profile_compatibility();
        assert!(flags.flag0());
        assert!(flags.flag1());
        assert!(!flags.flag2());
        assert!(!flags.flag3());
        assert!(!flags.flag4());
        assert!(!flags.flag5());
        // Baseline profile has no extension fields
        assert_eq!(avcc.chroma_format(), None);
        assert_eq!(avcc.bit_depth_luma_minus8(), None);
        assert_eq!(avcc.bit_depth_chroma_minus8(), None);
        assert_eq!(avcc.sequence_parameter_set_extensions().count(), 0);
        let ctx = avcc.create_context().unwrap();
        let sps = ctx
            .sps_by_id(SeqParamSetId::from_u32(0).unwrap())
            .expect("missing sps");
        assert_eq!(avcc.avc_level_indication(), sps.level());
        assert_eq!(avcc.avc_profile_indication(), sps.profile_idc);
        assert_eq!(
            SeqParamSetId::from_u32(0).unwrap(),
            sps.seq_parameter_set_id
        );
        let _pps = ctx
            .pps_by_id(PicParamSetId::from_u32(0).unwrap())
            .expect("missing pps");
    }
    #[test]
    fn high_profile_extension_fields() {
        // Hand-crafted avcC with High profile (100) and extension fields:
        //   chroma_format=1 (4:2:0), bit_depth_luma_minus8=0, bit_depth_chroma_minus8=0,
        //   0 SPS extension NAL units.
        // Base: version=1, profile=100(High), compat=0x00, level=31
        //   lengthSizeMinusOne=3 (0xff = reserved|3)
        //   1 SPS (0xe1 = reserved|1)
        let sps_nalu = hex!("6764001e acd940a0 2ff96100 00030001 00000300 3c9c5802 d0000bb8 00004e20 6e200000 10000003 00010000 03000321");
        let pps_nalu = hex!("68eb e3cb 22c0");
        let mut avcc_data: Vec<u8> = Vec::new();
        // Fixed header
        avcc_data.extend_from_slice(&[0x01, 0x64, 0x00, 0x1e, 0xff]);
        // 1 SPS
        avcc_data.push(0xe1);
        avcc_data.extend_from_slice(&(sps_nalu.len() as u16).to_be_bytes());
        avcc_data.extend_from_slice(&sps_nalu);
        // 1 PPS
        avcc_data.push(0x01);
        avcc_data.extend_from_slice(&(pps_nalu.len() as u16).to_be_bytes());
        avcc_data.extend_from_slice(&pps_nalu);
        // Extension fields: chroma_format=1, bit_depth_luma=0, bit_depth_chroma=0, 0 SPS ext
        avcc_data.extend_from_slice(&[0xfd, 0xf8, 0xf8, 0x00]);

        let avcc = AvcDecoderConfigurationRecord::try_from(&avcc_data[..]).unwrap();
        assert_eq!(avcc.avc_profile_indication(), ProfileIdc::from(100));
        assert_eq!(avcc.chroma_format(), Some(1));
        assert_eq!(avcc.bit_depth_luma_minus8(), Some(0));
        assert_eq!(avcc.bit_depth_chroma_minus8(), Some(0));
        assert_eq!(avcc.sequence_parameter_set_extensions().count(), 0);
    }
    #[test]
    fn high_profile_without_extension() {
        // High profile avcC that omits the optional extension fields.
        let sps_nalu = hex!("6764001e acd940a0 2ff96100 00030001 00000300 3c9c5802 d0000bb8 00004e20 6e200000 10000003 00010000 03000321");
        let pps_nalu = hex!("68eb e3cb 22c0");
        let mut avcc_data: Vec<u8> = Vec::new();
        avcc_data.extend_from_slice(&[0x01, 0x64, 0x00, 0x1e, 0xff]);
        avcc_data.push(0xe1);
        avcc_data.extend_from_slice(&(sps_nalu.len() as u16).to_be_bytes());
        avcc_data.extend_from_slice(&sps_nalu);
        avcc_data.push(0x01);
        avcc_data.extend_from_slice(&(pps_nalu.len() as u16).to_be_bytes());
        avcc_data.extend_from_slice(&pps_nalu);
        // No extension fields appended

        let avcc = AvcDecoderConfigurationRecord::try_from(&avcc_data[..]).unwrap();
        assert_eq!(avcc.avc_profile_indication(), ProfileIdc::from(100));
        assert_eq!(avcc.chroma_format(), None);
        assert_eq!(avcc.bit_depth_luma_minus8(), None);
        assert_eq!(avcc.bit_depth_chroma_minus8(), None);
    }
    #[test]
    fn sps_with_emulation_protection() {
        // From a Hikvision 2CD2032-I.
        let avcc_data = hex!(
            "014d401e ffe10017 674d401e 9a660a0f
                              ff350101 01400000 fa000003 01f40101
                              000468ee 3c80"
        );
        let avcc = AvcDecoderConfigurationRecord::try_from(&avcc_data[..]).unwrap();
        let _sps_data = avcc.sequence_parameter_sets().next().unwrap().unwrap();
        let ctx = avcc.create_context().unwrap();
        let _sps = ctx
            .sps_by_id(SeqParamSetId::from_u32(0).unwrap())
            .expect("missing sps");
    }
}
