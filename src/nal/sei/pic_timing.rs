use crate::nal::sei::HeaderType;
use crate::nal::sei::SeiMessage;
use crate::nal::sps;
use crate::rbsp::BitRead;
use crate::rbsp::BitReader;
use crate::rbsp::BitReaderError;

#[derive(Debug)]
pub enum PicTimingError {
    RbspError(BitReaderError),
    InvalidPicStructId(u8),
}
impl From<BitReaderError> for PicTimingError {
    fn from(e: BitReaderError) -> Self {
        PicTimingError::RbspError(e)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Delays {
    cpb_removal_delay: u32,
    dpb_output_delay: u32,
}

#[derive(Debug, Eq, PartialEq)]
pub enum PicStructType {
    Frame,
    TopField,
    BottomField,
    TopFieldBottomField,
    BottomFieldTopField,
    TopFieldBottomFieldTopFieldRepeated,
    BottomFieldTopFieldBottomFieldRepeated,
    FrameDoubling,
    FrameTripling,
    Reserved(u8),
}
impl PicStructType {
    fn from_id(id: u8) -> Result<PicStructType, PicTimingError> {
        match id {
            0 => Ok(PicStructType::Frame),
            1 => Ok(PicStructType::TopField),
            2 => Ok(PicStructType::BottomField),
            3 => Ok(PicStructType::TopFieldBottomField),
            4 => Ok(PicStructType::BottomFieldTopField),
            5 => Ok(PicStructType::TopFieldBottomFieldTopFieldRepeated),
            6 => Ok(PicStructType::BottomFieldTopFieldBottomFieldRepeated),
            7 => Ok(PicStructType::FrameDoubling),
            8 => Ok(PicStructType::FrameTripling),
            9..=15 => Ok(PicStructType::Reserved(id)),
            _ => Err(PicTimingError::InvalidPicStructId(id)),
        }
    }

    fn num_clock_timestamps(&self) -> u8 {
        match *self {
            PicStructType::Frame => 1,
            PicStructType::TopField => 1,
            PicStructType::BottomField => 1,
            PicStructType::TopFieldBottomField => 2,
            PicStructType::BottomFieldTopField => 2,
            PicStructType::TopFieldBottomFieldTopFieldRepeated => 3,
            PicStructType::BottomFieldTopFieldBottomFieldRepeated => 3,
            PicStructType::FrameDoubling => 2,
            PicStructType::FrameTripling => 3,
            PicStructType::Reserved(_) => 0,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum CtType {
    Progressive,
    Interlaced,
    Unknown,
    Reserved,
}
impl CtType {
    fn from_id(id: u8) -> CtType {
        match id {
            0 => CtType::Progressive,
            1 => CtType::Interlaced,
            2 => CtType::Unknown,
            3 => CtType::Reserved,
            _ => panic!("unexpected ct_type {}", id),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum CountingType {
    /// no dropping of `n_frames` values, and no use of `time_offset`
    NoDroppingNoOffset,
    /// no dropping of `n_frames` values
    NoDropping,
    /// dropping of individual '0' `n_frames` values
    DroppingIndividualZero,
    /// dropping of individual 'maxFPS - 1' `n_frames` values
    DroppingIndividualMax,
    /// dropping of individual '0' and '1' `n_frames` values
    DroppingTwoLowest,
    /// dropping of individual unspecified `n_frames` values
    DroppingIndividual,
    /// dropping of unspecified numbers of unspecified `n_frames` values
    Dropping,
    Reserved(u8),
}
impl CountingType {
    fn from_id(id: u8) -> CountingType {
        match id {
            0 => CountingType::NoDroppingNoOffset,
            1 => CountingType::NoDropping,
            2 => CountingType::DroppingIndividualZero,
            3 => CountingType::DroppingIndividualMax,
            4 => CountingType::DroppingTwoLowest,
            5 => CountingType::DroppingIndividual,
            6 => CountingType::Dropping,
            7..=31 => CountingType::Reserved(id),
            _ => panic!("unexpected counting_type {}", id),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum SecMinHour {
    None,
    S(u8),
    SM(u8, u8),
    SMH(u8, u8, u8),
}
impl SecMinHour {
    pub fn seconds(&self) -> u8 {
        match self {
            SecMinHour::None => 0,
            SecMinHour::S(s) => *s,
            SecMinHour::SM(s, _) => *s,
            SecMinHour::SMH(s, _, _) => *s,
        }
    }
    pub fn minutes(&self) -> u8 {
        match self {
            SecMinHour::None => 0,
            SecMinHour::S(_) => 0,
            SecMinHour::SM(_, m) => *m,
            SecMinHour::SMH(_, m, _) => *m,
        }
    }
    pub fn hours(&self) -> u8 {
        match self {
            SecMinHour::None => 0,
            SecMinHour::S(_) => 0,
            SecMinHour::SM(_, _) => 0,
            SecMinHour::SMH(_, _, h) => *h,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ClockTimestamp {
    pub ct_type: CtType,
    pub nuit_field_based_flag: bool,
    pub counting_type: CountingType,
    pub discontinuity_flag: bool,
    pub cnt_dropped_flag: bool,
    pub n_frames: u8,
    pub smh: SecMinHour,
    pub time_offset: Option<i32>,
}
impl ClockTimestamp {
    fn read<R: BitRead>(
        r: &mut R,
        sps: &sps::SeqParameterSet,
    ) -> Result<ClockTimestamp, PicTimingError> {
        let ct_type = CtType::from_id(r.read(2, "ct_type")?);
        let nuit_field_based_flag = r.read_bool("nuit_field_based_flag")?;
        let counting_type = CountingType::from_id(r.read(5, "counting_type")?);
        let full_timestamp_flag = r.read_bool("full_timestamp_flag")?;
        let discontinuity_flag = r.read_bool("discontinuity_flag")?;
        let cnt_dropped_flag = r.read_bool("cnt_dropped_flag")?;
        let n_frames = r.read(8, "n_frames")?;
        let smh = if full_timestamp_flag {
            SecMinHour::SMH(
                r.read(6, "seconds_value")?,
                r.read(6, "minutes_value")?,
                r.read(5, "hours_value")?,
            )
        } else if r.read_bool("seconds_flag")? {
            let seconds = r.read(6, "seconds_value")?;
            if r.read_bool("minutes_flag")? {
                let minutes = r.read(6, "minutes_value")?;
                if r.read_bool("hours_flag")? {
                    let hours = r.read(5, "hours_value")?;
                    SecMinHour::SMH(seconds, minutes, hours)
                } else {
                    SecMinHour::SM(seconds, minutes)
                }
            } else {
                SecMinHour::S(seconds)
            }
        } else {
            SecMinHour::None
        };
        let time_offset_length = if let Some(ref vui) = sps.vui_parameters {
            if let Some(ref hrd) = vui.nal_hrd_parameters {
                hrd.time_offset_length
            } else if let Some(ref hrd) = vui.vcl_hrd_parameters {
                hrd.time_offset_length
            } else {
                24
            }
        } else {
            24
        };
        let time_offset = if time_offset_length == 0 {
            None
        } else {
            Some(r.read(u32::from(time_offset_length), "time_offset_length")?)
        };
        Ok(ClockTimestamp {
            ct_type,
            nuit_field_based_flag,
            counting_type,
            discontinuity_flag,
            cnt_dropped_flag,
            n_frames,
            smh,
            time_offset,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct PicStruct {
    pub pic_struct: PicStructType,
    pub clock_timestamps: Vec<Option<ClockTimestamp>>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct PicTiming {
    pub delays: Option<Delays>,
    pub pic_struct: Option<PicStruct>,
}
impl PicTiming {
    /// Parses a `PicTiming` from the given SEI message.
    /// The caller is expected to have found the correct SPS by buffering the `SeiMessage`
    /// until after examining the following slice header.
    pub fn read(
        sps: &sps::SeqParameterSet,
        msg: &SeiMessage<'_>,
    ) -> Result<PicTiming, PicTimingError> {
        assert_eq!(msg.payload_type, HeaderType::PicTiming);
        let mut r = BitReader::new(msg.payload);
        let pic_timing = PicTiming {
            delays: Self::read_delays(&mut r, sps)?,
            pic_struct: Self::read_pic_struct(&mut r, sps)?,
        };
        r.finish_sei_payload()?;
        Ok(pic_timing)
    }

    fn read_delays<R: BitRead>(
        r: &mut R,
        sps: &sps::SeqParameterSet,
    ) -> Result<Option<Delays>, PicTimingError> {
        Ok(if let Some(ref vui_params) = sps.vui_parameters {
            if let Some(ref hrd) = vui_params
                .nal_hrd_parameters
                .as_ref()
                .or_else(|| vui_params.nal_hrd_parameters.as_ref())
            {
                Some(Delays {
                    cpb_removal_delay: r.read(
                        u32::from(hrd.cpb_removal_delay_length_minus1) + 1,
                        "cpb_removal_delay",
                    )?,
                    dpb_output_delay: r.read(
                        u32::from(hrd.dpb_output_delay_length_minus1) + 1,
                        "dpb_output_delay",
                    )?,
                })
            } else {
                None
            }
        } else {
            None
        })
    }

    fn read_pic_struct<R: BitRead>(
        r: &mut R,
        sps: &sps::SeqParameterSet,
    ) -> Result<Option<PicStruct>, PicTimingError> {
        Ok(if let Some(ref vui_params) = sps.vui_parameters {
            if vui_params.pic_struct_present_flag {
                let pic_struct = PicStructType::from_id(r.read(4, "pic_struct")?)?;
                let clock_timestamps = Self::read_clock_timestamps(r, &pic_struct, sps)?;

                Some(PicStruct {
                    pic_struct,
                    clock_timestamps,
                })
            } else {
                None
            }
        } else {
            None
        })
    }

    fn read_clock_timestamps<R: BitRead>(
        r: &mut R,
        pic_struct: &PicStructType,
        sps: &sps::SeqParameterSet,
    ) -> Result<Vec<Option<ClockTimestamp>>, PicTimingError> {
        let mut res = Vec::new();
        for _ in 0..pic_struct.num_clock_timestamps() {
            res.push(if r.read_bool("clock_timestamp_flag")? {
                Some(ClockTimestamp::read(r, sps)?)
            } else {
                None
            });
        }
        Ok(res)
    }
}
#[cfg(test)]
mod test {
    use crate::rbsp;
    use hex_literal::hex;

    use super::*;

    #[test]
    fn parse() {
        // https://standards.iso.org/ittf/PubliclyAvailableStandards/ISO_IEC_14496-4_2004_Amd_6_2005_Bitstreams/
        // This example taken from CVSEFDFT3_Sony_E.zip.
        let sps = hex!(
            "
            4d 60 15 8d 8d 28 58 9d 08 00 00 0f a0 00 07 53
            07 00 00 00 92 7c 00 00 12 4f 80 fb dc 18 00 00
            0f 42 40 00 07 a1 20 7d ee 07 c6 0c 62 60
        "
        );
        let sps = sps::SeqParameterSet::from_bits(rbsp::BitReader::new(&sps[..])).unwrap();
        let msg = SeiMessage {
            payload_type: HeaderType::PicTiming,
            payload: &hex!("00 00 00 00 00 0c 72")[..],
        };
        let pic_timing = PicTiming::read(&sps, &msg).unwrap();
        assert_eq!(
            pic_timing,
            PicTiming {
                delays: Some(Delays {
                    cpb_removal_delay: 0,
                    dpb_output_delay: 12,
                }),
                pic_struct: Some(PicStruct {
                    pic_struct: PicStructType::FrameDoubling,
                    clock_timestamps: vec![None, None],
                }),
            }
        );
    }
}
