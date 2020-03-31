//! Support for handling _Advanced Video Coding Configuration_ data, used in the _ISO Base Media
//! File Format_ (AKA MP4), as the specified in _ISO/IEC 14496-15_.
//!

use crate::nal::{sps, UnitType, NalHeader, NalHeaderError, pps, NalHandler};
use std::convert::TryFrom;
use crate::nal::sps::{ProfileIdc, Level, ConstraintFlags, SeqParameterSet, SeqParameterSetNalHandler};
use crate::Context;
use crate::nal::pps::PicParameterSetNalHandler;
use crate::rbsp;

#[derive(Debug)]
pub enum AvccError {
    NotEnoughData { expected: usize, actual: usize },
    /// The AvcDecoderConfigurationRecord used a version number other than `1`.
    UnsupportedConfigurationVersion(u8),
    ParamSet(ParamSetError),
    Sps(sps::SpsError),
    Pps(pps::PpsError),
}

pub struct AvcDecoderConfigurationRecord<'buf> {
    data: &'buf[u8],
}
impl<'buf> TryFrom<&'buf[u8]> for AvcDecoderConfigurationRecord<'buf> {
    type Error = AvccError;

    fn try_from(data: &'buf[u8]) -> Result<Self, Self::Error> {
        let avcc = AvcDecoderConfigurationRecord { data };
        // we must confirm we have enough bytes for all fixed fields before we do anything else,
        avcc.ck(Self::MIN_CONF_SIZE)?;
        if avcc.configuration_version() != 1 {
            // The spec requires that decoders ignore streams where the version number is not 1,
            // indicating there was an incompatible change in the configuration format,
            return Err(AvccError::UnsupportedConfigurationVersion(avcc.configuration_version()));
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
            let pps_len = (u16::from(data[len]) << 8 | u16::from(data[len +1 ])) as usize;
            len += 2;
            avcc.ck(len + pps_len)?;
            len += pps_len;
            num_pps -= 1;

        }

        Ok(avcc)
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
    fn ck(&self, len: usize)  -> Result<(), AvccError> {
        if self.data.len() < len {
            Err(AvccError::NotEnoughData { expected: len, actual: self.data.len() })
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
    pub fn sequence_parameter_sets(&self) -> impl Iterator<Item = Result<&'buf[u8], ParamSetError>> {
        let num = self.num_of_sequence_parameter_sets();
        let data = &self.data[Self::MIN_CONF_SIZE..];
        ParamSetIter::new(data, UnitType::SeqParameterSet)
            .take(num)
    }
    pub fn picture_parameter_sets(&self) -> impl Iterator<Item = Result<&'buf[u8], ParamSetError>> + 'buf {
        let offset = self.seq_param_sets_end().unwrap();
        let num = self.data[offset];
        let data = &self.data[offset+1..];
        ParamSetIter::new(data, UnitType::PicParameterSet)
            .take(num as usize)
    }

    /// Creates an H264 parser context from the given user context, using the settings encoded into
    /// this `AvcDecoderConfigurationRecord`.
    ///
    /// In particular, the _sequence parameter set_ and _picture parameter set_ values of this
    /// configuration record will be inserted into the resulting context.
    pub fn create_context<C>(&self, ctx: C) -> Result<Context<C>, AvccError> {
        let mut ctx = Context::new(ctx);
        let mut sps_decode = rbsp::RbspDecoder::new(SeqParameterSetNalHandler::new());
        for sps in self.sequence_parameter_sets() {
            sps_decode.push(&mut ctx, sps.map_err(AvccError::ParamSet)?);
            sps_decode.end(&mut ctx);
        }
        let mut pps_decode = rbsp::RbspDecoder::new(PicParameterSetNalHandler::new());
        for pps in self.picture_parameter_sets() {
            pps_decode.push(&mut ctx, pps.map_err(AvccError::ParamSet)?);
            pps_decode.end(&mut ctx);
        }
        Ok(ctx)
    }
}

#[derive(Debug)]
pub enum ParamSetError {
    NalHeader(NalHeaderError),
    IncorrectNalType { expected: UnitType, actual: UnitType },
    /// A _sequence parameter set_ found within the AVC decoder config was not consistent with the
    /// settings of the decoder config itself
    IncompatibleSps(SeqParameterSet),
}

struct ParamSetIter<'buf>(&'buf[u8], UnitType);

impl<'buf> ParamSetIter<'buf> {
    pub fn new(buf: &'buf[u8], unit_type: UnitType) -> ParamSetIter<'buf> {
        ParamSetIter(buf, unit_type)
    }
}
impl<'buf> Iterator for ParamSetIter<'buf>
{
    type Item = Result<&'buf[u8], ParamSetError>;

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
                        Ok(&data[1..])  // trim off the nal_header byte
                    } else {
                        Err(ParamSetError::IncorrectNalType { expected: self.1, actual: nal_header.nal_unit_type() })
                    }
                },
                Err(err) => Err(ParamSetError::NalHeader(err)),
            };
            Some(res)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::nal::pps::ParamSetId;
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
        let ctx = avcc.create_context(()).unwrap();
        let sps = ctx.sps_by_id(ParamSetId::from_u32(0).unwrap())
            .expect("missing sps");
        assert_eq!(avcc.avc_level_indication(), sps.level());
        assert_eq!(avcc.avc_profile_indication(), sps.profile_idc);
        assert_eq!(ParamSetId::from_u32(0).unwrap(), sps.seq_parameter_set_id);
        let _pps = ctx.pps_by_id(ParamSetId::from_u32(0).unwrap())
            .expect("missing pps");
    }
    #[test]
    fn sps_with_emulation_protection() {
        // From a Hikvision 2CD2032-I.
        let avcc_data = hex!("014d401e ffe10017 674d401e 9a660a0f
                              ff350101 01400000 fa000003 01f40101
                              000468ee 3c80");
        let avcc = AvcDecoderConfigurationRecord::try_from(&avcc_data[..]).unwrap();
        let _sps_data = avcc.sequence_parameter_sets().next().unwrap().unwrap();
        let ctx = avcc.create_context(()).unwrap();
        let _sps = ctx.sps_by_id(ParamSetId::from_u32(0).unwrap())
            .expect("missing sps");
    }
}