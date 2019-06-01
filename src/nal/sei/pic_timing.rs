use nal::sei::SeiCompletePayloadReader;
use Context;
use nal::sei::HeaderType;
use nal::pps::ParamSetId;
use rbsp::RbspBitReader;
use nal::sps;
use rbsp::RbspBitReaderError;
use bitreader::BitReaderError;

// FIXME: SPS selection
//      We should really wait until we know what SPS is in use by the frame which follows the
//      pic_timing header, before trying to decode the pic_timing header.  This would require
//      storing away the header bytes.  As a bodge, for now we assume frames always use SPS 0.

#[derive(Debug)]
pub enum PicTimingError {
    ReaderError(BitReaderError),
    RbspError(RbspBitReaderError),
    UndefinedSeqParamSetId(ParamSetId),
    InvalidPicStructId(u8),
}
impl From<BitReaderError> for PicTimingError {
    fn from(e: BitReaderError) -> Self {
        PicTimingError::ReaderError(e)
    }
}
impl From<RbspBitReaderError> for PicTimingError {
    fn from(e: RbspBitReaderError) -> Self {
        PicTimingError::RbspError(e)
    }
}

#[derive(Debug)]
pub struct Delays {
    cpb_removal_delay: u32,
    dpb_output_delay: u32,
}

#[derive(Debug)]
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
            9...15 => Ok(PicStructType::Reserved(id)),
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
            PicStructType::Reserved(id) => 0,
        }
    }
}

#[derive(Debug)]
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

#[derive(Debug)]
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
            7...31 => CountingType::Reserved(id),
            _ => panic!("unexpected counting_type {}", id),
        }
    }
}

#[derive(Debug)]
pub enum SecMinHour {
    None,
    S(u8),
    SM(u8, u8),
    SMH(u8, u8, u8)
}

#[derive(Debug)]
pub struct ClockTimestamp {
    ct_type: CtType,
    nuit_field_based_flag: bool,
    counting_type: CountingType,
    discontinuity_flag: bool,
    cnt_dropped_flag: bool,
    n_frames: u8,
    smh: SecMinHour,
    time_offset: i32,
}
impl ClockTimestamp {
    fn read(r: &mut RbspBitReader, sps: &sps::SeqParameterSet) -> Result<ClockTimestamp, PicTimingError> {
        let ct_type = CtType::from_id(r.read_u8(2)?);
        let nuit_field_based_flag = r.read_bool_named("nuit_field_based_flag")?;
        let counting_type = CountingType::from_id(r.read_u8(5)?);
        let full_timestamp_flag = r.read_bool_named("full_timestamp_flag")?;
        let discontinuity_flag = r.read_bool_named("discontinuity_flag")?;
        let cnt_dropped_flag = r.read_bool_named("cnt_dropped_flag")?;
        let n_frames = r.read_u8(8)?;
        let smh = if full_timestamp_flag {
            SecMinHour::SMH(
                r.read_u8(6)?,
                r.read_u8(6)?,
                r.read_u8(5)?,
            )
        } else {
            if r.read_bool_named("seconds_flag")? {
                let seconds = r.read_u8(6)?;
                if r.read_bool_named("minutes_flag")? {
                    let minutes = r.read_u8(6)?;
                    if r.read_bool_named("hours_flag")? {
                        let hours = r.read_u8(5)?;
                        SecMinHour::SMH(seconds, minutes, hours)
                    } else {
                        SecMinHour::SM(seconds, minutes)
                    }
                } else {
                    SecMinHour::S(seconds)
                }
            } else {
                SecMinHour::None
            }
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
        Ok(ClockTimestamp {
            ct_type,
            nuit_field_based_flag,
            counting_type,
            discontinuity_flag,
            cnt_dropped_flag,
            n_frames,
            smh,
            time_offset: r.read_i32(time_offset_length)?,
        })
    }
}

#[derive(Debug)]
struct PicStruct {
    pic_struct: PicStructType,
    clock_timestamps: Vec<Option<ClockTimestamp>>,
}

#[derive(Debug)]
pub struct PicTiming {
    delays: Option<Delays>,
    pic_struct: Option<PicStruct>,
}
impl PicTiming {
    pub fn read<Ctx>(ctx: &mut Context<Ctx>, buf: &[u8]) -> Result<PicTiming, PicTimingError> {
        let mut r = RbspBitReader::new(buf);
        let seq_parameter_set_id = ParamSetId::from_u32(0).unwrap();
        match ctx.sps_by_id(seq_parameter_set_id) {
            None => Err(PicTimingError::UndefinedSeqParamSetId(seq_parameter_set_id)),
            Some(sps) => {
                Ok(PicTiming {
                    delays: Self::read_delays(&mut r, sps)?,
                    pic_struct: Self::read_pic_struct(&mut r, sps)?,
                })
            }
        }
    }

    fn read_delays(r: &mut RbspBitReader, sps: &sps::SeqParameterSet) -> Result<Option<Delays>,PicTimingError> {
        Ok(if let Some(ref vui_params) = sps.vui_parameters {
            if let Some(ref hrd) = vui_params.nal_hrd_parameters.as_ref().or_else(|| vui_params.nal_hrd_parameters.as_ref() ) {
                Some(Delays {
                    cpb_removal_delay: r.read_u32(hrd.cpb_removal_delay_length_minus1+1)?,
                    dpb_output_delay: r.read_u32(hrd.dpb_output_delay_length_minus1+1)?,
                })
            } else {
                None
            }
        } else {
            None
        })
    }

    fn read_pic_struct(r: &mut RbspBitReader, sps: &sps::SeqParameterSet) -> Result<Option<PicStruct>,PicTimingError> {
        Ok(if let Some(ref vui_params) = sps.vui_parameters {
            if vui_params.pic_struct_present_flag {
                let pic_struct = PicStructType::from_id(r.read_u8(4)?)?;
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

    fn read_clock_timestamps(r: &mut RbspBitReader, pic_struct: &PicStructType, sps: &sps::SeqParameterSet) -> Result<Vec<Option<ClockTimestamp>>,PicTimingError> {
        let mut res = Vec::new();
        for i in 0..pic_struct.num_clock_timestamps() {
            res.push(if r.read_bool_named("clock_timestamp_flag")? {
                Some(ClockTimestamp::read(r, sps)?)
            } else {
                None
            });
        }
        Ok(res)
    }
}
pub trait PicTimingHandler {
    type Ctx;
    fn handle(&mut self, ctx: &mut Context<Self::Ctx>, pic_timing: PicTiming);
}
pub struct PicTimingReader<H: PicTimingHandler> {
    handler: H,
}
impl<H: PicTimingHandler> PicTimingReader<H> {
    pub fn new(handler: H) -> Self {
        PicTimingReader {
            handler,
        }
    }
}
impl<H: PicTimingHandler> SeiCompletePayloadReader for PicTimingReader<H> {
    type Ctx = H::Ctx;

    fn header(&mut self, ctx: &mut Context<Self::Ctx>, payload_type: HeaderType, buf: &[u8]) {
        assert_eq!(payload_type, HeaderType::PicTiming);
        match PicTiming::read(ctx, buf) {
            Err(e) => eprintln!("Failure reading pic_timing: {:?}", e),
            Ok(pic_timing) => {
                self.handler.handle(ctx, pic_timing);
            }
        }
    }
}
