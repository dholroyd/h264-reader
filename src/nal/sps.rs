use crate::rbsp::{BitRead, BitReaderError};
use std::fmt::{self, Debug};

#[derive(Debug, PartialEq)]
pub enum SeqParamSetIdError {
    IdTooLarge(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SeqParamSetId(u8);
impl SeqParamSetId {
    pub fn from_u32(id: u32) -> Result<SeqParamSetId, SeqParamSetIdError> {
        if id > 31 {
            Err(SeqParamSetIdError::IdTooLarge(id))
        } else {
            Ok(SeqParamSetId(id as u8))
        }
    }
    pub fn id(self) -> u8 {
        self.0
    }
}

#[derive(Debug)]
pub enum SpsError {
    /// Signals that bit_depth_luma_minus8 was greater than the max value, 6
    BitDepthOutOfRange(u32),
    RbspReaderError(BitReaderError),
    PicOrderCnt(PicOrderCntError),
    ScalingMatrix(ScalingMatrixError),
    /// log2_max_frame_num_minus4 must be between 0 and 12
    Log2MaxFrameNumMinus4OutOfRange(u32),
    BadSeqParamSetId(SeqParamSetIdError),
    UnknownSeqParamSetId(SeqParamSetId),
    /// A field in the bitstream had a value too large for a subsequent calculation
    FieldValueTooLarge {
        name: &'static str,
        value: u32,
    },
    /// A field in the bitstream had a value that is too small
    FieldValueTooSmall {
        name: &'static str,
        value: u32,
    },
    /// The frame-cropping values are too large vs. the coded picture size,
    CroppingError(FrameCropping),
    /// The `cpb_cnt_minus1` field must be between 0 and 31 inclusive.
    CpbCountOutOfRange(u32),
}

impl From<BitReaderError> for SpsError {
    fn from(e: BitReaderError) -> Self {
        SpsError::RbspReaderError(e)
    }
}

#[derive(Debug)]
pub enum Profile {
    Unknown(u8),
    Baseline,
    Main,
    High,
    High422,
    High10,
    High444,
    Extended,
    ScalableBase,
    ScalableHigh,
    MultiviewHigh,
    StereoHigh,
    MFCDepthHigh,
    MultiviewDepthHigh,
    EnhancedMultiviewDepthHigh,
}

impl Profile {
    pub fn from_profile_idc(profile_idc: ProfileIdc) -> Profile {
        // TODO: accept constraint_flags too, as Level does?
        match profile_idc.0 {
            66 => Profile::Baseline,
            77 => Profile::Main,
            100 => Profile::High,
            122 => Profile::High422,
            110 => Profile::High10,
            244 => Profile::High444,
            88 => Profile::Extended,
            83 => Profile::ScalableBase,
            86 => Profile::ScalableHigh,
            118 => Profile::MultiviewHigh,
            128 => Profile::StereoHigh,
            135 => Profile::MFCDepthHigh,
            138 => Profile::MultiviewDepthHigh,
            139 => Profile::EnhancedMultiviewDepthHigh,
            other => Profile::Unknown(other),
        }
    }
    pub fn profile_idc(&self) -> u8 {
        match *self {
            Profile::Baseline => 66,
            Profile::Main => 77,
            Profile::High => 100,
            Profile::High422 => 122,
            Profile::High10 => 110,
            Profile::High444 => 144,
            Profile::Extended => 88,
            Profile::ScalableBase => 83,
            Profile::ScalableHigh => 86,
            Profile::MultiviewHigh => 118,
            Profile::StereoHigh => 128,
            Profile::MFCDepthHigh => 135,
            Profile::MultiviewDepthHigh => 138,
            Profile::EnhancedMultiviewDepthHigh => 139,
            Profile::Unknown(profile_idc) => profile_idc,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct ConstraintFlags(u8);
impl From<u8> for ConstraintFlags {
    fn from(v: u8) -> Self {
        ConstraintFlags(v)
    }
}
impl From<ConstraintFlags> for u8 {
    fn from(v: ConstraintFlags) -> Self {
        v.0
    }
}
impl ConstraintFlags {
    pub fn flag0(self) -> bool {
        self.0 & 0b1000_0000 != 0
    }
    pub fn flag1(self) -> bool {
        self.0 & 0b0100_0000 != 0
    }
    pub fn flag2(self) -> bool {
        self.0 & 0b0010_0000 != 0
    }
    pub fn flag3(self) -> bool {
        self.0 & 0b0001_0000 != 0
    }
    pub fn flag4(self) -> bool {
        self.0 & 0b0000_1000 != 0
    }
    pub fn flag5(self) -> bool {
        self.0 & 0b0000_0100 != 0
    }
    pub fn reserved_zero_two_bits(self) -> u8 {
        self.0 & 0b0000_0011
    }
}
impl Debug for ConstraintFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("ConstraintFlags")
            .field("flag0", &self.flag0())
            .field("flag1", &self.flag1())
            .field("flag2", &self.flag2())
            .field("flag3", &self.flag3())
            .field("flag4", &self.flag4())
            .field("flag5", &self.flag5())
            .field("reserved_zero_two_bits", &self.reserved_zero_two_bits())
            .finish()
    }
}

#[derive(Debug, PartialEq, Hash, Eq)]
#[allow(non_camel_case_types)]
pub enum Level {
    Unknown(u8),
    L1,
    L1_b,
    L1_1,
    L1_2,
    L1_3,
    L2,
    L2_1,
    L2_2,
    L3,
    L3_1,
    L3_2,
    L4,
    L4_1,
    L4_2,
    L5,
    L5_1,
    L5_2,
    L6,
    L6_1,
    L6_2,
}
impl Level {
    pub fn from_constraint_flags_and_level_idc(
        constraint_flags: ConstraintFlags,
        level_idc: u8,
    ) -> Level {
        match level_idc {
            10 => Level::L1,
            11 => {
                if constraint_flags.flag3() {
                    Level::L1_b
                } else {
                    Level::L1_1
                }
            }
            12 => Level::L1_2,
            13 => Level::L1_3,
            20 => Level::L2,
            21 => Level::L2_1,
            22 => Level::L2_2,
            30 => Level::L3,
            31 => Level::L3_1,
            32 => Level::L3_2,
            40 => Level::L4,
            41 => Level::L4_1,
            42 => Level::L4_2,
            50 => Level::L5,
            51 => Level::L5_1,
            52 => Level::L5_2,
            60 => Level::L6,
            61 => Level::L6_1,
            62 => Level::L6_2,
            _ => Level::Unknown(level_idc),
        }
    }
    pub fn level_idc(&self) -> u8 {
        match *self {
            Level::L1 => 10,
            Level::L1_1 | Level::L1_b => 11,
            Level::L1_2 => 12,
            Level::L1_3 => 13,
            Level::L2 => 20,
            Level::L2_1 => 21,
            Level::L2_2 => 22,
            Level::L3 => 30,
            Level::L3_1 => 31,
            Level::L3_2 => 32,
            Level::L4 => 40,
            Level::L4_1 => 41,
            Level::L4_2 => 42,
            Level::L5 => 50,
            Level::L5_1 => 51,
            Level::L5_2 => 52,
            Level::L6 => 60,
            Level::L6_1 => 61,
            Level::L6_2 => 62,
            Level::Unknown(level_idc) => level_idc,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChromaFormat {
    Monochrome,
    #[default]
    YUV420,
    YUV422,
    YUV444,
    Invalid(u32),
}
impl ChromaFormat {
    fn from_chroma_format_idc(chroma_format_idc: u32) -> ChromaFormat {
        match chroma_format_idc {
            0 => ChromaFormat::Monochrome,
            1 => ChromaFormat::YUV420,
            2 => ChromaFormat::YUV422,
            3 => ChromaFormat::YUV444,
            _ => ChromaFormat::Invalid(chroma_format_idc),
        }
    }
}

// _Profile Indication_ value
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ProfileIdc(u8);
impl ProfileIdc {
    pub fn has_chroma_info(self) -> bool {
        match self.0 {
            100 | 110 | 122 | 244 | 44 | 83 | 86 => true,
            _ => false,
        }
    }
}
impl From<u8> for ProfileIdc {
    fn from(v: u8) -> Self {
        ProfileIdc(v)
    }
}
impl From<ProfileIdc> for u8 {
    fn from(v: ProfileIdc) -> Self {
        v.0
    }
}

pub struct ScalingList {
    // TODO
}
impl ScalingList {
    pub fn read<R: BitRead>(r: &mut R, size: u8) -> Result<ScalingList, ScalingMatrixError> {
        let mut scaling_list = vec![];
        let mut last_scale = 8;
        let mut next_scale = 8;
        let mut _use_default_scaling_matrix_flag = false;
        for j in 0..size {
            if next_scale != 0 {
                let delta_scale = r.read_se("delta_scale")?;
                if delta_scale < -128 || delta_scale > 127 {
                    return Err(ScalingMatrixError::DeltaScaleOutOfRange(delta_scale));
                }
                next_scale = (last_scale + delta_scale + 256) % 256;
                _use_default_scaling_matrix_flag = j == 0 && next_scale == 0;
            }
            let new_value = if next_scale == 0 {
                last_scale
            } else {
                next_scale
            };
            scaling_list.push(new_value);
            last_scale = new_value;
        }
        Ok(ScalingList {})
    }
}

#[derive(Debug)]
pub enum ScalingMatrixError {
    ReaderError(BitReaderError),
    /// The `delta_scale` field must be between -128 and 127 inclusive.
    DeltaScaleOutOfRange(i32),
}

impl From<BitReaderError> for ScalingMatrixError {
    fn from(e: BitReaderError) -> Self {
        ScalingMatrixError::ReaderError(e)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SeqScalingMatrix {
    // TODO
}

impl SeqScalingMatrix {
    fn read<R: BitRead>(
        r: &mut R,
        chroma_format_idc: u32,
    ) -> Result<SeqScalingMatrix, ScalingMatrixError> {
        let mut scaling_list4x4 = vec![];
        let mut scaling_list8x8 = vec![];

        let count = if chroma_format_idc == 3 { 12 } else { 8 };
        for i in 0..count {
            let seq_scaling_list_present_flag = r.read_bool("seq_scaling_list_present_flag")?;
            if seq_scaling_list_present_flag {
                if i < 6 {
                    scaling_list4x4.push(ScalingList::read(r, 16)?);
                } else {
                    scaling_list8x8.push(ScalingList::read(r, 64)?);
                }
            }
        }
        Ok(SeqScalingMatrix {})
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ChromaInfo {
    pub chroma_format: ChromaFormat,
    pub separate_colour_plane_flag: bool,
    pub bit_depth_luma_minus8: u8,
    pub bit_depth_chroma_minus8: u8,
    pub qpprime_y_zero_transform_bypass_flag: bool,
    pub scaling_matrix: SeqScalingMatrix,
}
impl ChromaInfo {
    pub fn read<R: BitRead>(r: &mut R, profile_idc: ProfileIdc) -> Result<ChromaInfo, SpsError> {
        if profile_idc.has_chroma_info() {
            let chroma_format_idc = r.read_ue("chroma_format_idc")?;
            Ok(ChromaInfo {
                chroma_format: ChromaFormat::from_chroma_format_idc(chroma_format_idc),
                separate_colour_plane_flag: if chroma_format_idc == 3 {
                    r.read_bool("separate_colour_plane_flag")?
                } else {
                    false
                },
                bit_depth_luma_minus8: Self::read_bit_depth_minus8(r)?,
                bit_depth_chroma_minus8: Self::read_bit_depth_minus8(r)?,
                qpprime_y_zero_transform_bypass_flag: r
                    .read_bool("qpprime_y_zero_transform_bypass_flag")?,
                scaling_matrix: Self::read_scaling_matrix(r, chroma_format_idc)?,
            })
        } else {
            Ok(ChromaInfo::default())
        }
    }
    fn read_bit_depth_minus8<R: BitRead>(r: &mut R) -> Result<u8, SpsError> {
        let value = r.read_ue("read_bit_depth_minus8")?;
        if value > 6 {
            Err(SpsError::BitDepthOutOfRange(value))
        } else {
            Ok(value as u8)
        }
    }
    fn read_scaling_matrix<R: BitRead>(
        r: &mut R,
        chroma_format_idc: u32,
    ) -> Result<SeqScalingMatrix, SpsError> {
        let scaling_matrix_present_flag = r.read_bool("scaling_matrix_present_flag")?;
        if scaling_matrix_present_flag {
            SeqScalingMatrix::read(r, chroma_format_idc).map_err(SpsError::ScalingMatrix)
        } else {
            Ok(SeqScalingMatrix::default())
        }
    }
}

#[derive(Debug)]
pub enum PicOrderCntError {
    InvalidPicOrderCountType(u32),
    ReaderError(BitReaderError),
    /// log2_max_pic_order_cnt_lsb_minus4 must be between 0 and 12
    Log2MaxPicOrderCntLsbMinus4OutOfRange(u32),
    /// num_ref_frames_in_pic_order_cnt_cycle must be between 0 and 255
    NumRefFramesInPicOrderCntCycleOutOfRange(u32),
}

impl From<BitReaderError> for PicOrderCntError {
    fn from(e: BitReaderError) -> Self {
        PicOrderCntError::ReaderError(e)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PicOrderCntType {
    TypeZero {
        log2_max_pic_order_cnt_lsb_minus4: u8,
    },
    TypeOne {
        delta_pic_order_always_zero_flag: bool,
        offset_for_non_ref_pic: i32,
        offset_for_top_to_bottom_field: i32,
        offsets_for_ref_frame: Vec<i32>,
    },
    TypeTwo,
}
impl PicOrderCntType {
    fn read<R: BitRead>(r: &mut R) -> Result<PicOrderCntType, PicOrderCntError> {
        let pic_order_cnt_type = r.read_ue("pic_order_cnt_type")?;
        match pic_order_cnt_type {
            0 => Ok(PicOrderCntType::TypeZero {
                log2_max_pic_order_cnt_lsb_minus4: Self::read_log2_max_pic_order_cnt_lsb_minus4(r)?,
            }),
            1 => Ok(PicOrderCntType::TypeOne {
                delta_pic_order_always_zero_flag: r
                    .read_bool("delta_pic_order_always_zero_flag")?,
                offset_for_non_ref_pic: r.read_se("offset_for_non_ref_pic")?,
                offset_for_top_to_bottom_field: r.read_se("offset_for_top_to_bottom_field")?,
                offsets_for_ref_frame: Self::read_offsets_for_ref_frame(r)?,
            }),
            2 => Ok(PicOrderCntType::TypeTwo),
            _ => Err(PicOrderCntError::InvalidPicOrderCountType(
                pic_order_cnt_type,
            )),
        }
    }

    fn read_log2_max_pic_order_cnt_lsb_minus4<R: BitRead>(
        r: &mut R,
    ) -> Result<u8, PicOrderCntError> {
        let val = r.read_ue("log2_max_pic_order_cnt_lsb_minus4")?;
        if val > 12 {
            Err(PicOrderCntError::Log2MaxPicOrderCntLsbMinus4OutOfRange(val))
        } else {
            Ok(val as u8)
        }
    }

    fn read_offsets_for_ref_frame<R: BitRead>(r: &mut R) -> Result<Vec<i32>, PicOrderCntError> {
        let num_ref_frames_in_pic_order_cnt_cycle =
            r.read_ue("num_ref_frames_in_pic_order_cnt_cycle")?;
        if num_ref_frames_in_pic_order_cnt_cycle > 255 {
            return Err(PicOrderCntError::NumRefFramesInPicOrderCntCycleOutOfRange(
                num_ref_frames_in_pic_order_cnt_cycle,
            ));
        }
        let mut offsets = Vec::with_capacity(num_ref_frames_in_pic_order_cnt_cycle as usize);
        for _ in 0..num_ref_frames_in_pic_order_cnt_cycle {
            offsets.push(r.read_se("offset_for_ref_frame")?);
        }
        Ok(offsets)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FrameMbsFlags {
    Frames,
    Fields { mb_adaptive_frame_field_flag: bool },
}
impl FrameMbsFlags {
    fn read<R: BitRead>(r: &mut R) -> Result<FrameMbsFlags, BitReaderError> {
        let frame_mbs_only_flag = r.read_bool("frame_mbs_only_flag")?;
        if frame_mbs_only_flag {
            Ok(FrameMbsFlags::Frames)
        } else {
            Ok(FrameMbsFlags::Fields {
                mb_adaptive_frame_field_flag: r.read_bool("mb_adaptive_frame_field_flag")?,
            })
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FrameCropping {
    pub left_offset: u32,
    pub right_offset: u32,
    pub top_offset: u32,
    pub bottom_offset: u32,
}
impl FrameCropping {
    fn read<R: BitRead>(r: &mut R) -> Result<Option<FrameCropping>, BitReaderError> {
        let frame_cropping_flag = r.read_bool("frame_cropping_flag")?;
        Ok(if frame_cropping_flag {
            Some(FrameCropping {
                left_offset: r.read_ue("left_offset")?,
                right_offset: r.read_ue("right_offset")?,
                top_offset: r.read_ue("top_offset")?,
                bottom_offset: r.read_ue("bottom_offset")?,
            })
        } else {
            None
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum AspectRatioInfo {
    #[default]
    Unspecified,
    Ratio1_1,
    Ratio12_11,
    Ratio10_11,
    Ratio16_11,
    Ratio40_33,
    Ratio24_11,
    Ratio20_11,
    Ratio32_11,
    Ratio80_33,
    Ratio18_11,
    Ratio15_11,
    Ratio64_33,
    Ratio160_99,
    Ratio4_3,
    Ratio3_2,
    Ratio2_1,
    Reserved(u8),
    Extended(u16, u16),
}
impl AspectRatioInfo {
    fn read<R: BitRead>(r: &mut R) -> Result<Option<AspectRatioInfo>, BitReaderError> {
        let aspect_ratio_info_present_flag = r.read_bool("aspect_ratio_info_present_flag")?;
        Ok(if aspect_ratio_info_present_flag {
            let aspect_ratio_idc = r.read(8, "aspect_ratio_idc")?;
            Some(match aspect_ratio_idc {
                0 => AspectRatioInfo::Unspecified,
                1 => AspectRatioInfo::Ratio1_1,
                2 => AspectRatioInfo::Ratio12_11,
                3 => AspectRatioInfo::Ratio10_11,
                4 => AspectRatioInfo::Ratio16_11,
                5 => AspectRatioInfo::Ratio40_33,
                6 => AspectRatioInfo::Ratio24_11,
                7 => AspectRatioInfo::Ratio20_11,
                8 => AspectRatioInfo::Ratio32_11,
                9 => AspectRatioInfo::Ratio80_33,
                10 => AspectRatioInfo::Ratio18_11,
                11 => AspectRatioInfo::Ratio15_11,
                12 => AspectRatioInfo::Ratio64_33,
                13 => AspectRatioInfo::Ratio160_99,
                14 => AspectRatioInfo::Ratio4_3,
                15 => AspectRatioInfo::Ratio3_2,
                16 => AspectRatioInfo::Ratio2_1,
                255 => {
                    AspectRatioInfo::Extended(r.read(16, "sar_width")?, r.read(16, "sar_height")?)
                }
                _ => AspectRatioInfo::Reserved(aspect_ratio_idc),
            })
        } else {
            None
        })
    }

    /// Returns the aspect ratio as `(width, height)`, if specified.
    pub fn get(&self) -> Option<(u16, u16)> {
        match self {
            AspectRatioInfo::Unspecified => None,
            AspectRatioInfo::Ratio1_1 => Some((1, 1)),
            AspectRatioInfo::Ratio12_11 => Some((12, 11)),
            AspectRatioInfo::Ratio10_11 => Some((10, 11)),
            AspectRatioInfo::Ratio16_11 => Some((16, 11)),
            AspectRatioInfo::Ratio40_33 => Some((40, 33)),
            AspectRatioInfo::Ratio24_11 => Some((24, 11)),
            AspectRatioInfo::Ratio20_11 => Some((20, 11)),
            AspectRatioInfo::Ratio32_11 => Some((32, 11)),
            AspectRatioInfo::Ratio80_33 => Some((80, 33)),
            AspectRatioInfo::Ratio18_11 => Some((18, 11)),
            AspectRatioInfo::Ratio15_11 => Some((15, 11)),
            AspectRatioInfo::Ratio64_33 => Some((64, 33)),
            AspectRatioInfo::Ratio160_99 => Some((160, 99)),
            AspectRatioInfo::Ratio4_3 => Some((4, 3)),
            AspectRatioInfo::Ratio3_2 => Some((3, 2)),
            AspectRatioInfo::Ratio2_1 => Some((2, 1)),
            AspectRatioInfo::Reserved(_) => None,
            &AspectRatioInfo::Extended(width, height) => {
                // ISO/IEC 14496-10 section E.2.1: "When ... sar_width is equal to 0 or sar_height
                // is equal to 0, the sample aspect ratio shall be considered unspecified by this
                // Recommendation | International Standard."
                if width == 0 || height == 0 {
                    None
                } else {
                    Some((width, height))
                }
            }
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum OverscanAppropriate {
    #[default]
    Unspecified,
    Appropriate,
    Inappropriate,
}
impl OverscanAppropriate {
    fn read<R: BitRead>(r: &mut R) -> Result<OverscanAppropriate, BitReaderError> {
        let overscan_info_present_flag = r.read_bool("overscan_info_present_flag")?;
        Ok(if overscan_info_present_flag {
            let overscan_appropriate_flag = r.read_bool("overscan_appropriate_flag")?;
            if overscan_appropriate_flag {
                OverscanAppropriate::Appropriate
            } else {
                OverscanAppropriate::Inappropriate
            }
        } else {
            OverscanAppropriate::Unspecified
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum VideoFormat {
    #[default]
    Component,
    PAL,
    NTSC,
    SECAM,
    MAC,
    Unspecified,
    Reserved(u8),
}
impl VideoFormat {
    fn from(video_format: u8) -> VideoFormat {
        match video_format {
            0 => VideoFormat::Component,
            1 => VideoFormat::PAL,
            2 => VideoFormat::NTSC,
            3 => VideoFormat::SECAM,
            4 => VideoFormat::MAC,
            5 => VideoFormat::Unspecified,
            6 | 7 => VideoFormat::Reserved(video_format),
            _ => panic!("unsupported video_format value {}", video_format),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ColourDescription {
    pub colour_primaries: u8,
    pub transfer_characteristics: u8,
    pub matrix_coefficients: u8,
}
impl ColourDescription {
    fn read<R: BitRead>(r: &mut R) -> Result<Option<ColourDescription>, BitReaderError> {
        let colour_description_present_flag = r.read_bool("colour_description_present_flag")?;
        Ok(if colour_description_present_flag {
            Some(ColourDescription {
                colour_primaries: r.read(8, "colour_primaries")?,
                transfer_characteristics: r.read(8, "transfer_characteristics")?,
                matrix_coefficients: r.read(8, "matrix_coefficients")?,
            })
        } else {
            None
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VideoSignalType {
    pub video_format: VideoFormat,
    pub video_full_range_flag: bool,
    pub colour_description: Option<ColourDescription>,
}
impl VideoSignalType {
    fn read<R: BitRead>(r: &mut R) -> Result<Option<VideoSignalType>, BitReaderError> {
        let video_signal_type_present_flag = r.read_bool("video_signal_type_present_flag")?;
        Ok(if video_signal_type_present_flag {
            Some(VideoSignalType {
                video_format: VideoFormat::from(r.read(3, "video_format")?),
                video_full_range_flag: r.read_bool("video_full_range_flag")?,
                colour_description: ColourDescription::read(r)?,
            })
        } else {
            None
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChromaLocInfo {
    pub chroma_sample_loc_type_top_field: u32,
    pub chroma_sample_loc_type_bottom_field: u32,
}
impl ChromaLocInfo {
    fn read<R: BitRead>(r: &mut R) -> Result<Option<ChromaLocInfo>, BitReaderError> {
        let chroma_loc_info_present_flag = r.read_bool("chroma_loc_info_present_flag")?;
        Ok(if chroma_loc_info_present_flag {
            Some(ChromaLocInfo {
                chroma_sample_loc_type_top_field: r.read_ue("chroma_sample_loc_type_top_field")?,
                chroma_sample_loc_type_bottom_field: r
                    .read_ue("chroma_sample_loc_type_bottom_field")?,
            })
        } else {
            None
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TimingInfo {
    pub num_units_in_tick: u32,
    pub time_scale: u32,
    pub fixed_frame_rate_flag: bool,
}
impl TimingInfo {
    fn read<R: BitRead>(r: &mut R) -> Result<Option<TimingInfo>, BitReaderError> {
        let timing_info_present_flag = r.read_bool("timing_info_present_flag")?;
        Ok(if timing_info_present_flag {
            Some(TimingInfo {
                num_units_in_tick: r.read(32, "num_units_in_tick")?,
                time_scale: r.read(32, "time_scale")?,
                fixed_frame_rate_flag: r.read_bool("fixed_frame_rate_flag")?,
            })
        } else {
            None
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CpbSpec {
    pub bit_rate_value_minus1: u32,
    pub cpb_size_value_minus1: u32,
    pub cbr_flag: bool,
}
impl CpbSpec {
    fn read<R: BitRead>(r: &mut R) -> Result<CpbSpec, BitReaderError> {
        Ok(CpbSpec {
            bit_rate_value_minus1: r.read_ue("bit_rate_value_minus1")?,
            cpb_size_value_minus1: r.read_ue("cpb_size_value_minus1")?,
            cbr_flag: r.read_bool("cbr_flag")?,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HrdParameters {
    pub bit_rate_scale: u8,
    pub cpb_size_scale: u8,
    pub cpb_specs: Vec<CpbSpec>,
    pub initial_cpb_removal_delay_length_minus1: u8,
    pub cpb_removal_delay_length_minus1: u8,
    pub dpb_output_delay_length_minus1: u8,
    pub time_offset_length: u8,
}
impl HrdParameters {
    fn read<R: BitRead>(
        r: &mut R,
        hrd_parameters_present: &mut bool,
    ) -> Result<Option<HrdParameters>, SpsError> {
        let hrd_parameters_present_flag = r.read_bool("hrd_parameters_present_flag")?;
        *hrd_parameters_present |= hrd_parameters_present_flag;
        Ok(if hrd_parameters_present_flag {
            let cpb_cnt_minus1 = r.read_ue("cpb_cnt_minus1")?;
            if cpb_cnt_minus1 > 31 {
                return Err(SpsError::CpbCountOutOfRange(cpb_cnt_minus1));
            }
            let cpb_cnt = cpb_cnt_minus1 + 1;
            Some(HrdParameters {
                bit_rate_scale: r.read(4, "bit_rate_scale")?,
                cpb_size_scale: r.read(4, "cpb_size_scale")?,
                cpb_specs: Self::read_cpb_specs(r, cpb_cnt)?,
                initial_cpb_removal_delay_length_minus1: r
                    .read(5, "initial_cpb_removal_delay_length_minus1")?,
                cpb_removal_delay_length_minus1: r.read(5, "cpb_removal_delay_length_minus1")?,
                dpb_output_delay_length_minus1: r.read(5, "dpb_output_delay_length_minus1")?,
                time_offset_length: r.read(5, "time_offset_length")?,
            })
        } else {
            None
        })
    }
    fn read_cpb_specs<R: BitRead>(r: &mut R, cpb_cnt: u32) -> Result<Vec<CpbSpec>, BitReaderError> {
        let mut cpb_specs = Vec::with_capacity(cpb_cnt as usize);
        for _ in 0..cpb_cnt {
            cpb_specs.push(CpbSpec::read(r)?);
        }
        Ok(cpb_specs)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BitstreamRestrictions {
    pub motion_vectors_over_pic_boundaries_flag: bool,
    pub max_bytes_per_pic_denom: u32,
    pub max_bits_per_mb_denom: u32,
    pub log2_max_mv_length_horizontal: u32,
    pub log2_max_mv_length_vertical: u32,
    pub max_num_reorder_frames: u32,
    pub max_dec_frame_buffering: u32,
}
impl BitstreamRestrictions {
    fn read<R: BitRead>(
        r: &mut R,
        sps: &SeqParameterSet,
    ) -> Result<Option<BitstreamRestrictions>, SpsError> {
        let bitstream_restriction_flag = r.read_bool("bitstream_restriction_flag")?;
        Ok(if bitstream_restriction_flag {
            let motion_vectors_over_pic_boundaries_flag =
                r.read_bool("motion_vectors_over_pic_boundaries_flag")?;
            let max_bytes_per_pic_denom = r.read_ue("max_bytes_per_pic_denom")?;
            if max_bytes_per_pic_denom > 16 {
                return Err(SpsError::FieldValueTooLarge {
                    name: "max_bytes_per_pic_denom",
                    value: max_bytes_per_pic_denom,
                });
            }
            let max_bits_per_mb_denom = r.read_ue("max_bits_per_mb_denom")?;
            if max_bits_per_mb_denom > 16 {
                return Err(SpsError::FieldValueTooLarge {
                    name: "max_bits_per_mb_denom",
                    value: max_bits_per_mb_denom,
                });
            }
            // more recent versions of the spec say log2_max_mv_length_horizontal and
            // log2_max_mv_length_vertical - "shall be in the range of 0 to 15, inclusive."
            // However, older versions of the spec say 0 to 16, and real bitstreams present 16, so
            // we apply the more-permissive check to avoid rejecting real files.
            let log2_max_mv_length_horizontal = r.read_ue("log2_max_mv_length_horizontal")?;
            if log2_max_mv_length_horizontal > 16 {
                return Err(SpsError::FieldValueTooLarge {
                    name: "log2_max_mv_length_horizontal",
                    value: log2_max_mv_length_horizontal,
                });
            }
            let log2_max_mv_length_vertical = r.read_ue("log2_max_mv_length_vertical")?;
            if log2_max_mv_length_vertical > 16 {
                return Err(SpsError::FieldValueTooLarge {
                    name: "log2_max_mv_length_vertical",
                    value: log2_max_mv_length_vertical,
                });
            }
            let max_num_reorder_frames = r.read_ue("max_num_reorder_frames")?;
            let max_dec_frame_buffering = r.read_ue("max_dec_frame_buffering")?;
            if max_num_reorder_frames > max_dec_frame_buffering {
                return Err(SpsError::FieldValueTooLarge {
                    name: "max_num_reorder_frames",
                    value: max_num_reorder_frames,
                });
            }
            // "The value of max_dec_frame_buffering shall be greater than or equal to
            // max_num_ref_frames."
            if max_dec_frame_buffering < sps.max_num_ref_frames {
                return Err(SpsError::FieldValueTooSmall {
                    name: "max_dec_frame_buffering",
                    value: max_dec_frame_buffering,
                });
            }
            /*let max = max_val_for_max_dec_frame_buffering(sps);
            if max_dec_frame_buffering > max {
                return Err(SpsError::FieldValueTooLarge {
                    name: "max_dec_frame_buffering",
                    value: max_dec_frame_buffering,
                });
            }*/
            Some(BitstreamRestrictions {
                motion_vectors_over_pic_boundaries_flag,
                max_bytes_per_pic_denom,
                max_bits_per_mb_denom,
                log2_max_mv_length_horizontal,
                log2_max_mv_length_vertical,
                max_num_reorder_frames,
                max_dec_frame_buffering,
            })
        } else {
            None
        })
    }
}

// calculates the maximum allowed value for the max_dec_frame_buffering field
/*fn max_val_for_max_dec_frame_buffering(sps: &SeqParameterSet) -> u32 {
    let level = Level::from_constraint_flags_and_level_idc(
        ConstraintFlags::from(sps.constraint_flags),
        sps.level_idc,
    );
    let profile = Profile::from_profile_idc(sps.profile_idc);
    let pic_width_in_mbs = sps.pic_width_in_mbs_minus1 + 1;
    let pic_height_in_map_units = sps.pic_height_in_map_units_minus1 + 1;
    let frame_height_in_mbs = if let FrameMbsFlags::Frames = sps.frame_mbs_flags {
        1
    } else {
        2
    } * pic_height_in_map_units;
    let max_dpb_mbs = LEVEL_LIMITS.get(&level).unwrap().max_dpb_mbs;
    match profile {
        // "A.3.1 - Level limits common to the Baseline, Constrained Baseline, Main, and Extended
        // profiles"
        Profile::Baseline | Profile::Main | Profile::Extended => {
            std::cmp::min(max_dpb_mbs / (pic_width_in_mbs * frame_height_in_mbs), 16)
        }
        // "A.3.2 - Level limits common to the High, Progressive High, Constrained High, High 10,
        // Progressive High 10, High 4:2:2, High 4:4:4 Predictive, High 10 Intra, High 4:2:2 Intra,
        // High 4:4:4 Intra, and CAVLC 4:4:4 Intra profiles"
        Profile::High | Profile::High422 | Profile::High10 | Profile::High444 => {
            std::cmp::min(max_dpb_mbs / (pic_width_in_mbs * frame_height_in_mbs), 16)
        }

        // "G.10.2.1 - Level limits common to Scalable Baseline, Scalable Constrained Baseline,
        // Scalable High, Scalable Constrained High, and Scalable High Intra profiles"
        Profile::ScalableBase | Profile::ScalableHigh => {
            // Min( MaxDpbMbs / ( PicWidthInMbs * FrameHeightInMbs ), 16 )
            std::cmp::min(max_dpb_mbs / (pic_width_in_mbs * frame_height_in_mbs), 16)
        }

        // "H.10.2.1 - Level limits common to Multiview High, Stereo High, and MFC High profiles"
        //Profile::MultiviewHigh | Profile::StereoHigh | Profile::MFCDepthHigh => {
        //    // Min( mvcScaleFactor * MaxDpbMbs / ( PicWidthInMbs * FrameHeightInMbs ), Max( 1, Ceil( log2( NumViews ) ) ) * 16 )
        //}

        // "I.10.2.1 - Level limits common to Multiview Depth High profiles"
        //Profile::MultiviewDepthHigh | Profile::EnhancedMultiviewDepthHigh => {
        //    let mvcd_scale_factor = 2.5;
        //    std::cmp::min( mvcd_scale_factor * max_dpb_mbs / ( TotalPicSizeInMbs / NumViews ) ), std::cmp::max(1, Ceil( log2( NumViews ) ) ) * 16 )
        //}
        _ => unimplemented!("{:?}", profile),
    }
}*/

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VuiParameters {
    pub aspect_ratio_info: Option<AspectRatioInfo>,
    pub overscan_appropriate: OverscanAppropriate,
    pub video_signal_type: Option<VideoSignalType>,
    pub chroma_loc_info: Option<ChromaLocInfo>,
    pub timing_info: Option<TimingInfo>,
    pub nal_hrd_parameters: Option<HrdParameters>,
    pub vcl_hrd_parameters: Option<HrdParameters>,
    pub low_delay_hrd_flag: Option<bool>,
    pub pic_struct_present_flag: bool,
    pub bitstream_restrictions: Option<BitstreamRestrictions>,
}
impl VuiParameters {
    fn read<R: BitRead>(
        r: &mut R,
        sps: &SeqParameterSet,
    ) -> Result<Option<VuiParameters>, SpsError> {
        let vui_parameters_present_flag = r.read_bool("vui_parameters_present_flag")?;
        Ok(if vui_parameters_present_flag {
            let mut hrd_parameters_present = false;
            Some(VuiParameters {
                aspect_ratio_info: AspectRatioInfo::read(r)?,
                overscan_appropriate: OverscanAppropriate::read(r)?,
                video_signal_type: VideoSignalType::read(r)?,
                chroma_loc_info: ChromaLocInfo::read(r)?,
                timing_info: TimingInfo::read(r)?,
                nal_hrd_parameters: HrdParameters::read(r, &mut hrd_parameters_present)?,
                vcl_hrd_parameters: HrdParameters::read(r, &mut hrd_parameters_present)?,
                low_delay_hrd_flag: if hrd_parameters_present {
                    Some(r.read_bool("low_delay_hrd_flag")?)
                } else {
                    None
                },
                pic_struct_present_flag: r.read_bool("pic_struct_present_flag")?,
                bitstream_restrictions: BitstreamRestrictions::read(r, sps)?,
            })
        } else {
            None
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeqParameterSet {
    pub profile_idc: ProfileIdc,
    pub constraint_flags: ConstraintFlags,
    pub level_idc: u8,
    pub seq_parameter_set_id: SeqParamSetId,
    pub chroma_info: ChromaInfo,
    pub log2_max_frame_num_minus4: u8,
    pub pic_order_cnt: PicOrderCntType,
    pub max_num_ref_frames: u32,
    pub gaps_in_frame_num_value_allowed_flag: bool,
    pub pic_width_in_mbs_minus1: u32,
    pub pic_height_in_map_units_minus1: u32,
    pub frame_mbs_flags: FrameMbsFlags,
    pub direct_8x8_inference_flag: bool,
    pub frame_cropping: Option<FrameCropping>,
    pub vui_parameters: Option<VuiParameters>,
}
impl SeqParameterSet {
    pub fn from_bits<R: BitRead>(mut r: R) -> Result<SeqParameterSet, SpsError> {
        let profile_idc = r.read::<u8>(8, "profile_idc")?.into();
        let constraint_flags = r.read::<u8>(8, "constraint_flags")?.into();
        let level_idc = r.read::<u8>(8, "level_idc")?;
        let mut sps = SeqParameterSet {
            profile_idc,
            constraint_flags,
            level_idc,
            seq_parameter_set_id: SeqParamSetId::from_u32(r.read_ue("seq_parameter_set_id")?)
                .map_err(SpsError::BadSeqParamSetId)?,
            chroma_info: ChromaInfo::read(&mut r, profile_idc)?,
            log2_max_frame_num_minus4: Self::read_log2_max_frame_num_minus4(&mut r)?,
            pic_order_cnt: PicOrderCntType::read(&mut r).map_err(SpsError::PicOrderCnt)?,
            max_num_ref_frames: r.read_ue("max_num_ref_frames")?,
            gaps_in_frame_num_value_allowed_flag: r
                .read_bool("gaps_in_frame_num_value_allowed_flag")?,
            pic_width_in_mbs_minus1: r.read_ue("pic_width_in_mbs_minus1")?,
            pic_height_in_map_units_minus1: r.read_ue("pic_height_in_map_units_minus1")?,
            frame_mbs_flags: FrameMbsFlags::read(&mut r)?,
            direct_8x8_inference_flag: r.read_bool("direct_8x8_inference_flag")?,
            frame_cropping: FrameCropping::read(&mut r)?,
            // read the basic SPS data without reading VUI parameters yet, since checks of the
            // bitstream restriction fields within the VUI parameters will need access to the
            // initial SPS data
            vui_parameters: None,
        };
        let vui_parameters = VuiParameters::read(&mut r, &sps)?;
        sps.vui_parameters = vui_parameters;
        r.finish_rbsp()?;
        Ok(sps)
    }

    pub fn id(&self) -> SeqParamSetId {
        self.seq_parameter_set_id
    }

    fn read_log2_max_frame_num_minus4<R: BitRead>(r: &mut R) -> Result<u8, SpsError> {
        let val = r.read_ue("log2_max_frame_num_minus4")?;
        if val > 12 {
            Err(SpsError::Log2MaxFrameNumMinus4OutOfRange(val))
        } else {
            Ok(val as u8)
        }
    }

    pub fn profile(&self) -> Profile {
        Profile::from_profile_idc(self.profile_idc)
    }

    pub fn level(&self) -> Level {
        Level::from_constraint_flags_and_level_idc(self.constraint_flags, self.level_idc)
    }
    /// returned value will be in the range 4 to 16 inclusive
    pub fn log2_max_frame_num(&self) -> u8 {
        self.log2_max_frame_num_minus4 + 4
    }

    /// Helper to calculate the pixel-dimensions of the video image specified by this SPS, taking
    /// into account sample-format, interlacing and cropping.
    pub fn pixel_dimensions(&self) -> Result<(u32, u32), SpsError> {
        let width = self
            .pic_width_in_mbs_minus1
            .checked_add(1)
            .and_then(|w| w.checked_mul(16))
            .ok_or_else(|| SpsError::FieldValueTooLarge {
                name: "pic_width_in_mbs_minus1",
                value: self.pic_width_in_mbs_minus1,
            })?;
        let mul = match self.frame_mbs_flags {
            FrameMbsFlags::Fields { .. } => 2,
            FrameMbsFlags::Frames => 1,
        };
        let vsub = if self.chroma_info.chroma_format == ChromaFormat::YUV420 {
            1
        } else {
            0
        };
        let hsub = if self.chroma_info.chroma_format == ChromaFormat::YUV420
            || self.chroma_info.chroma_format == ChromaFormat::YUV422
        {
            1
        } else {
            0
        };

        let step_x = 1 << hsub;
        let step_y = mul << vsub;

        let height = (self.pic_height_in_map_units_minus1 + 1)
            .checked_mul(mul * 16)
            .ok_or_else(|| SpsError::FieldValueTooLarge {
                name: "pic_height_in_map_units_minus1",
                value: self.pic_height_in_map_units_minus1,
            })?;
        if let Some(ref crop) = self.frame_cropping {
            let left_offset = crop.left_offset.checked_mul(step_x).ok_or_else(|| {
                SpsError::FieldValueTooLarge {
                    name: "left_offset",
                    value: crop.left_offset,
                }
            })?;
            let right_offset = crop.right_offset.checked_mul(step_x).ok_or_else(|| {
                SpsError::FieldValueTooLarge {
                    name: "right_offset",
                    value: crop.right_offset,
                }
            })?;
            let top_offset = crop.top_offset.checked_mul(step_y).ok_or_else(|| {
                SpsError::FieldValueTooLarge {
                    name: "top_offset",
                    value: crop.top_offset,
                }
            })?;
            let bottom_offset = crop.bottom_offset.checked_mul(step_y).ok_or_else(|| {
                SpsError::FieldValueTooLarge {
                    name: "bottom_offset",
                    value: crop.bottom_offset,
                }
            })?;
            let width = width
                .checked_sub(left_offset)
                .and_then(|w| w.checked_sub(right_offset));
            let height = height
                .checked_sub(top_offset)
                .and_then(|w| w.checked_sub(bottom_offset));
            if let (Some(width), Some(height)) = (width, height) {
                Ok((width, height))
            } else {
                Err(SpsError::CroppingError(crop.clone()))
            }
        } else {
            Ok((width, height))
        }
    }

    pub fn rfc6381(&self) -> rfc6381_codec::Codec {
        rfc6381_codec::Codec::avc1(self.profile_idc.0, self.constraint_flags.0, self.level_idc)
    }

    pub fn fps(&self) -> Option<f64> {
        let Some(vui) = &self.vui_parameters else {
            return None;
        };
        let Some(timing_info) = &vui.timing_info else {
            return None;
        };

        Some((timing_info.time_scale as f64) / (2.0 * (timing_info.num_units_in_tick as f64)))
    }

    pub fn pic_width_in_mbs(&self) -> u32 {
        self.pic_width_in_mbs_minus1 + 1
    }

    /// From the spec: `PicHeightInMapUnits = pic_height_in_map_units_minus1 + 1`
    pub fn pic_height_in_map_units(&self) -> u32 {
        self.pic_height_in_map_units_minus1 + 1
    }

    /// From the spec: `PicSizeInMapUnits = PicWidthInMbs * PicHeightInMapUnits`
    pub fn pic_size_in_map_units(&self) -> u32 {
        self.pic_width_in_mbs() * self.pic_height_in_map_units()
    }
}

/*struct LevelLimit {
    max_mbps: u32,
    max_fs: u32,
    max_dpb_mbs: u32,
    max_br: u32,
    max_cpb: u32,
    max_vmv_r: u32,
    min_cr: u8,
    max_mvs_per2mb: Option<NonZeroU8>,
}

lazy_static! {
    // "Table A-1 – Level limits" from the spec
    static ref LEVEL_LIMITS: std::collections::HashMap<Level, LevelLimit> = {
        let mut m = std::collections::HashMap::new();
        m.insert(
            Level::L1,
            LevelLimit {
                max_mbps: 1485,
                max_fs: 99,
                max_dpb_mbs: 396,
                max_br: 64,
                max_cpb: 175,
                max_vmv_r: 64,
                min_cr: 2,
                max_mvs_per2mb: None,
            },
        );
        m.insert(
            Level::L1_b,
            LevelLimit {
                max_mbps: 1485,
                max_fs: 99,
                max_dpb_mbs: 396,
                max_br: 128,
                max_cpb: 350,
                max_vmv_r: 64,
                min_cr: 2,
                max_mvs_per2mb: None,
            },
        );
        m.insert(
            Level::L1_1,
            LevelLimit {
                max_mbps: 3000,
                max_fs: 396,
                max_dpb_mbs: 900,
                max_br: 192,
                max_cpb: 500,
                max_vmv_r: 128,
                min_cr: 2,
                max_mvs_per2mb: None,
            },
        );
        m.insert(
            Level::L1_2,
            LevelLimit {
                max_mbps: 6000,
                max_fs: 396,
                max_dpb_mbs: 2376,
                max_br: 384,
                max_cpb: 1000,
                max_vmv_r: 128,
                min_cr: 2,
                max_mvs_per2mb: None,
            },
        );
        m.insert(
            Level::L1_3,
            LevelLimit {
                max_mbps: 11880,
                max_fs: 396,
                max_dpb_mbs: 2376,
                max_br: 768,
                max_cpb: 2000,
                max_vmv_r: 128,
                min_cr: 2,
                max_mvs_per2mb: None,
            },
        );
        m.insert(
            Level::L2,
            LevelLimit {
                max_mbps: 11880,
                max_fs: 396,
                max_dpb_mbs: 2376,
                max_br: 2000,
                max_cpb: 2000,
                max_vmv_r: 128,
                min_cr: 2,
                max_mvs_per2mb: None,
            },
        );
        m.insert(
            Level::L2_1,
            LevelLimit {
                max_mbps: 19800,
                max_fs: 792,
                max_dpb_mbs: 4752,
                max_br: 4000,
                max_cpb: 4000,
                max_vmv_r: 256,
                min_cr: 2,
                max_mvs_per2mb: None,
            },
        );
        m.insert(
            Level::L2_2,
            LevelLimit {
                max_mbps: 20250,
                max_fs: 1620,
                max_dpb_mbs: 8100,
                max_br: 4000,
                max_cpb: 4000,
                max_vmv_r: 256,
                min_cr: 2,
                max_mvs_per2mb: None,
            },
        );
        m.insert(
            Level::L3,
            LevelLimit {
                max_mbps: 40500,
                max_fs: 1620,
                max_dpb_mbs: 8100,
                max_br: 10000,
                max_cpb: 10000,
                max_vmv_r: 256,
                min_cr: 2,
                max_mvs_per2mb: NonZeroU8::new(32),
            },
        );
        m.insert(
            Level::L3_1,
            LevelLimit {
                max_mbps: 108000,
                max_fs: 3600,
                max_dpb_mbs: 18000,
                max_br: 14000,
                max_cpb: 14000,
                max_vmv_r: 512,
                min_cr: 4,
                max_mvs_per2mb: NonZeroU8::new(16),
            },
        );
        m.insert(
            Level::L3_2,
            LevelLimit {
                max_mbps: 216000,
                max_fs: 5120,
                max_dpb_mbs: 20480,
                max_br: 20000,
                max_cpb: 20000,
                max_vmv_r: 512,
                min_cr: 4,
                max_mvs_per2mb: NonZeroU8::new(16),
            },
        );
        m.insert(
            Level::L4,
            LevelLimit {
                max_mbps: 245760,
                max_fs: 8192,
                max_dpb_mbs: 32768,
                max_br: 20000,
                max_cpb: 25000,
                max_vmv_r: 512,
                min_cr: 4,
                max_mvs_per2mb: NonZeroU8::new(16),
            },
        );
        m.insert(
            Level::L4_1,
            LevelLimit {
                max_mbps: 245760,
                max_fs: 8192,
                max_dpb_mbs: 32768,
                max_br: 50000,
                max_cpb: 62500,
                max_vmv_r: 512,
                min_cr: 2,
                max_mvs_per2mb: NonZeroU8::new(16),
            },
        );
        m.insert(
            Level::L4_2,
            LevelLimit {
                max_mbps: 522240,
                max_fs: 8704,
                max_dpb_mbs: 34816,
                max_br: 50000,
                max_cpb: 62500,
                max_vmv_r: 512,
                min_cr: 2,
                max_mvs_per2mb: NonZeroU8::new(16),
            },
        );
        m.insert(
            Level::L5,
            LevelLimit {
                max_mbps: 589824,
                max_fs: 22080,
                max_dpb_mbs: 110400,
                max_br: 135000,
                max_cpb: 135000,
                max_vmv_r: 512,
                min_cr: 2,
                max_mvs_per2mb: NonZeroU8::new(16),
            },
        );
        m.insert(
            Level::L5_1,
            LevelLimit {
                max_mbps: 983040,
                max_fs: 36864,
                max_dpb_mbs: 184320,
                max_br: 240000,
                max_cpb: 240000,
                max_vmv_r: 512,
                min_cr: 2,
                max_mvs_per2mb: NonZeroU8::new(16),
            },
        );
        m.insert(
            Level::L5_2,
            LevelLimit {
                max_mbps: 2073600,
                max_fs: 36864,
                max_dpb_mbs: 184320,
                max_br: 240000,
                max_cpb: 240000,
                max_vmv_r: 512,
                min_cr: 2,
                max_mvs_per2mb: NonZeroU8::new(16),
            },
        );
        m.insert(
            Level::L6,
            LevelLimit {
                max_mbps: 4177920,
                max_fs: 139264,
                max_dpb_mbs: 696320,
                max_br: 240000,
                max_cpb: 240000,
                max_vmv_r: 8192,
                min_cr: 2,
                max_mvs_per2mb: NonZeroU8::new(16),
            },
        );
        m.insert(
            Level::L6_1,
            LevelLimit {
                max_mbps: 8355840,
                max_fs: 139264,
                max_dpb_mbs: 696320,
                max_br: 480000,
                max_cpb: 480000,
                max_vmv_r: 8192,
                min_cr: 2,
                max_mvs_per2mb: NonZeroU8::new(16),
            },
        );
        m.insert(
            Level::L6_2,
            LevelLimit {
                max_mbps: 16711680,
                max_fs: 139264,
                max_dpb_mbs: 696320,
                max_br: 800000,
                max_cpb: 800000,
                max_vmv_r: 8192,
                min_cr: 2,
                max_mvs_per2mb: NonZeroU8::new(16),
            },
        );
        m
    };
}*/

#[cfg(test)]
mod test {
    use crate::rbsp::{self, decode_nal, BitReader};

    use super::*;
    use hex_literal::*;
    use test_case::test_case;

    #[test]
    fn test_it() {
        let data = hex!(
            "64 00 0A AC 72 84 44 26 84 00 00
            00 04 00 00 00 CA 3C 48 96 11 80"
        );
        let sps = SeqParameterSet::from_bits(rbsp::BitReader::new(&data[..])).unwrap();
        assert!(!format!("{:?}", sps).is_empty());
        assert_eq!(100, sps.profile_idc.0);
        assert_eq!(0, sps.constraint_flags.reserved_zero_two_bits());
        assert_eq!((64, 64), sps.pixel_dimensions().unwrap());
        assert!(!sps.rfc6381().to_string().is_empty())
    }

    #[test]
    fn test_dahua() {
        // From a Dahua IPC-HDW5231R-Z's sub stream, which is anamorphic.
        let data = hex!(
            "64 00 16 AC 1B 1A 80 B0 3D FF FF
           00 28 00 21 6E 0C 0C 0C 80 00 01
           F4 00 00 27 10 74 30 07 D0 00 07
           A1 25 DE 5C 68 60 0F A0 00 0F 42
           4B BC B8 50"
        );
        let sps = SeqParameterSet::from_bits(rbsp::BitReader::new(&data[..])).unwrap();
        println!("sps: {:#?}", sps);
        assert_eq!(
            sps.vui_parameters.unwrap().aspect_ratio_info.unwrap().get(),
            Some((40, 33))
        );
    }

    #[test]
    fn crop_removes_all_pixels() {
        let sps = SeqParameterSet {
            profile_idc: ProfileIdc(0),
            constraint_flags: ConstraintFlags(0),
            level_idc: 0,
            seq_parameter_set_id: SeqParamSetId::from_u32(0).unwrap(),
            chroma_info: ChromaInfo {
                chroma_format: ChromaFormat::Monochrome,
                separate_colour_plane_flag: false,
                bit_depth_luma_minus8: 0,
                bit_depth_chroma_minus8: 0,
                qpprime_y_zero_transform_bypass_flag: false,
                scaling_matrix: Default::default(),
            },
            log2_max_frame_num_minus4: 0,
            pic_order_cnt: PicOrderCntType::TypeTwo,
            max_num_ref_frames: 0,
            frame_cropping: Some(FrameCropping {
                bottom_offset: 20,
                left_offset: 20,
                right_offset: 20,
                top_offset: 20,
            }),
            pic_width_in_mbs_minus1: 1,
            pic_height_in_map_units_minus1: 1,
            frame_mbs_flags: FrameMbsFlags::Frames,
            gaps_in_frame_num_value_allowed_flag: false,
            direct_8x8_inference_flag: false,
            vui_parameters: None,
        };
        // should return Err, rather than assert due to integer underflow for example,
        let dim = sps.pixel_dimensions();
        assert!(matches!(dim, Err(SpsError::CroppingError(_))));
    }

    #[test_case(
        vec![
            0x67, 0x64, 0x00, 0x0c, 0xac, 0x3b, 0x50, 0xb0,
            0x4b, 0x42, 0x00, 0x00, 0x03, 0x00, 0x02, 0x00,
            0x00, 0x03, 0x00, 0x3d, 0x08,
        ],
        SeqParameterSet{
            profile_idc: ProfileIdc::from(100),
            constraint_flags: ConstraintFlags::from(0),
            level_idc: 12,
            seq_parameter_set_id: SeqParamSetId::from_u32(0).unwrap(),
            chroma_info: ChromaInfo{
                chroma_format: ChromaFormat::YUV420,
                ..ChromaInfo::default()
            },
            log2_max_frame_num_minus4: 6,
            pic_order_cnt: PicOrderCntType::TypeTwo,
            max_num_ref_frames: 1,
            gaps_in_frame_num_value_allowed_flag: true,
            pic_width_in_mbs_minus1: 21,
            pic_height_in_map_units_minus1: 17,
            frame_mbs_flags: FrameMbsFlags::Frames,
            direct_8x8_inference_flag: true,
            frame_cropping: None,
            vui_parameters: Some(VuiParameters{
                timing_info: Some(TimingInfo{
                    num_units_in_tick: 1,
                    time_scale: 30,
                    fixed_frame_rate_flag: true,
                }),
                ..VuiParameters::default()
            }),
        },
        352,
        288,
        15.0; "352x288"
    )]
    #[test_case(
        vec![
            0x67, 0x64, 0x00, 0x1f, 0xac, 0xd9, 0x40, 0x50,
            0x05, 0xbb, 0x01, 0x6c, 0x80, 0x00, 0x00, 0x03,
            0x00, 0x80, 0x00, 0x00, 0x1e, 0x07, 0x8c, 0x18,
            0xcb,
        ],
        SeqParameterSet{
            profile_idc: ProfileIdc::from(100),
            constraint_flags: ConstraintFlags::from(0),
            level_idc: 31,
            seq_parameter_set_id: SeqParamSetId::from_u32(0).unwrap(),
            chroma_info: ChromaInfo{
                chroma_format: ChromaFormat::YUV420,
                ..ChromaInfo::default()
            },
            log2_max_frame_num_minus4: 0,
            pic_order_cnt: PicOrderCntType::TypeZero {
                log2_max_pic_order_cnt_lsb_minus4: 2
            },
            max_num_ref_frames: 4,
            gaps_in_frame_num_value_allowed_flag: false,
            pic_width_in_mbs_minus1: 79,
            pic_height_in_map_units_minus1: 44,
            frame_mbs_flags: FrameMbsFlags::Frames,
            direct_8x8_inference_flag: true,
            frame_cropping: None,
            vui_parameters: Some(VuiParameters{
                aspect_ratio_info: Some(AspectRatioInfo::Ratio1_1),
                video_signal_type: Some(VideoSignalType{
                    video_format: VideoFormat::Unspecified,
                    video_full_range_flag: true,
                    colour_description: None,
                }),
                timing_info: Some(TimingInfo{
                    num_units_in_tick: 1,
                    time_scale: 60,
                    fixed_frame_rate_flag: false,
                }),
                bitstream_restrictions: Some(BitstreamRestrictions{
                    motion_vectors_over_pic_boundaries_flag: true,
                    log2_max_mv_length_horizontal: 11,
                    log2_max_mv_length_vertical: 11,
                    max_num_reorder_frames: 2,
                    max_dec_frame_buffering: 4,
                    ..BitstreamRestrictions::default()
                }),
                ..VuiParameters::default()
            }),
        },
        1280,
        720,
        30.0; "1280x720"
    )]
    #[test_case(
        vec![
            0x67, 0x42, 0xc0, 0x28, 0xd9, 0x00, 0x78, 0x02,
            0x27, 0xe5, 0x84, 0x00, 0x00, 0x03, 0x00, 0x04,
            0x00, 0x00, 0x03, 0x00, 0xf0, 0x3c, 0x60, 0xc9, 0x20,
        ],
        SeqParameterSet{
            profile_idc: ProfileIdc::from(66),
            constraint_flags: ConstraintFlags::from(0b11000000),
            level_idc: 40,
            seq_parameter_set_id: SeqParamSetId::from_u32(0).unwrap(),
            chroma_info: ChromaInfo{
                chroma_format: ChromaFormat::YUV420,
                ..ChromaInfo::default()
            },
            log2_max_frame_num_minus4: 0,
            pic_order_cnt: PicOrderCntType::TypeTwo,
            max_num_ref_frames: 3,
            gaps_in_frame_num_value_allowed_flag: false,
            pic_width_in_mbs_minus1: 119,
            pic_height_in_map_units_minus1: 67,
            frame_mbs_flags: FrameMbsFlags::Frames,
            direct_8x8_inference_flag: true,
            frame_cropping: Some(FrameCropping{
                bottom_offset: 4,
                ..FrameCropping::default()
            }),
            vui_parameters: Some(VuiParameters{
                timing_info: Some(TimingInfo{
                    num_units_in_tick: 1,
                    time_scale:      60,
                    fixed_frame_rate_flag: false,
                }),
                bitstream_restrictions: Some(BitstreamRestrictions{
                    motion_vectors_over_pic_boundaries_flag: true,
                    log2_max_mv_length_horizontal: 11,
                    log2_max_mv_length_vertical: 11,
                    max_dec_frame_buffering: 3,
                    ..BitstreamRestrictions::default()
                }),
                ..VuiParameters::default()
            }),
        },
        1920,
        1080,
        30.0; "1920x1080 baseline"
    )]
    #[test_case(
        vec![
            0x67, 0x64, 0x00, 0x28, 0xac, 0xd9, 0x40, 0x78,
            0x02, 0x27, 0xe5, 0x84, 0x00, 0x00, 0x03, 0x00,
            0x04, 0x00, 0x00, 0x03, 0x00, 0xf0, 0x3c, 0x60,
            0xc6, 0x58,
        ],
        SeqParameterSet{
            profile_idc: ProfileIdc::from(100),
            constraint_flags: ConstraintFlags::from(0),
            level_idc: 40,
            seq_parameter_set_id: SeqParamSetId::from_u32(0).unwrap(),
            chroma_info: ChromaInfo{
                chroma_format: ChromaFormat::YUV420,
                ..ChromaInfo::default()
            },
            log2_max_frame_num_minus4: 0,
            pic_order_cnt: PicOrderCntType::TypeZero {
                log2_max_pic_order_cnt_lsb_minus4: 2
            },
            max_num_ref_frames: 4,
            gaps_in_frame_num_value_allowed_flag: false,
            pic_width_in_mbs_minus1: 119,
            pic_height_in_map_units_minus1: 67,
            frame_mbs_flags: FrameMbsFlags::Frames,
            direct_8x8_inference_flag: true,
            frame_cropping: Some(FrameCropping{
                bottom_offset: 4,
                ..FrameCropping::default()
            }),
            vui_parameters: Some(VuiParameters{
                timing_info: Some(TimingInfo{
                    num_units_in_tick: 1,
                    time_scale: 60,
                    fixed_frame_rate_flag: false,
                }),
                bitstream_restrictions: Some(BitstreamRestrictions{
                    motion_vectors_over_pic_boundaries_flag: true,
                    log2_max_mv_length_horizontal: 11,
                    log2_max_mv_length_vertical: 11,
                    max_num_reorder_frames: 2,
                    max_dec_frame_buffering: 4,
                    ..BitstreamRestrictions::default()
                }),
                ..VuiParameters::default()
            }),
        },
        1920,
        1080,
        30.0; "1920x1080 nvidia"
    )]
    // This fails.

    /*#[test_case(
        vec![
            0x67, 0x64, 0x00, 0x29, 0xac, 0x13, 0x31, 0x40,
            0x78, 0x04, 0x47, 0xde, 0x03, 0xea, 0x02, 0x02,
            0x03, 0xe0, 0x00, 0x00, 0x03, 0x00, 0x20, 0x00,
            0x00, 0x06, 0x52, // 0x80,
        ],
        SeqParameterSet{
            profile_idc: ProfileIdc::from(100),
            constraint_flags: ConstraintFlags::from(0),
            level_idc: 41,
            seq_parameter_set_id: SeqParamSetId::from_u32(0).unwrap(),
            chroma_info: ChromaInfo{
                chroma_format: ChromaFormat::YUV420,
                ..ChromaInfo::default()
            },
            log2_max_frame_num_minus4: 8,
            pic_order_cnt: PicOrderCntType::TypeZero {
                log2_max_pic_order_cnt_lsb_minus4: 5
            },
            max_num_ref_frames: 4,
            gaps_in_frame_num_value_allowed_flag: false,
            pic_width_in_mbs_minus1: 119,
            pic_height_in_map_units_minus1: 33,
            frame_mbs_flags: FrameMbsFlags::Fields{
                mb_adaptive_frame_field_flag: false,
            },
            direct_8x8_inference_flag: true,
            frame_cropping: Some(FrameCropping{
                bottom_offset: 2,
                ..FrameCropping::default()
            }),
            vui_parameters: Some(VuiParameters{
                aspect_ratio_info: Some(AspectRatioInfo::Ratio1_1),
                overscan_appropriate: OverscanAppropriate::Appropriate,
                video_signal_type: Some(VideoSignalType{
                    video_format: VideoFormat::Unspecified,
                    video_full_range_flag: false,
                    colour_description: Some(ColourDescription{
                        colour_primaries: 1,
                        transfer_characteristics: 1,
                        matrix_coefficients: 1,
                    }),
                }),
                chroma_loc_info: Some(ChromaLocInfo{
                    chroma_sample_loc_type_top_field: 0,
                    chroma_sample_loc_type_bottom_field: 0,
                }),
                timing_info: Some(TimingInfo{
                    num_units_in_tick: 1,
                    time_scale: 50,
                    fixed_frame_rate_flag: true,
                }),
                pic_struct_present_flag: true,
                ..VuiParameters::default()
            }),
        },
        1920,
        1084,
        25.0; "1920x1080"
    )]
    */
    #[test_case(
        vec![103, 100, 0, 32, 172, 23, 42, 1, 64, 30, 104, 64, 0, 1, 194, 0, 0, 87, 228, 33],
        SeqParameterSet{
            profile_idc: ProfileIdc::from(100),
            constraint_flags: ConstraintFlags::from(0),
            level_idc: 32,
            seq_parameter_set_id: SeqParamSetId::from_u32(0).unwrap(),
            chroma_info: ChromaInfo{
                chroma_format: ChromaFormat::YUV420,
                ..ChromaInfo::default()
            },
            log2_max_frame_num_minus4: 10,
            pic_order_cnt: PicOrderCntType::TypeZero {
                log2_max_pic_order_cnt_lsb_minus4: 4
            },
            max_num_ref_frames: 1,
            gaps_in_frame_num_value_allowed_flag: false,
            pic_width_in_mbs_minus1: 79,
            pic_height_in_map_units_minus1: 59,
            frame_mbs_flags: FrameMbsFlags::Frames,
            direct_8x8_inference_flag: true,
            frame_cropping: None,
            vui_parameters: Some(VuiParameters{
                timing_info: Some(TimingInfo{
                    num_units_in_tick: 1800,
                    time_scale: 90000,
                    fixed_frame_rate_flag: true,
                }),
                ..VuiParameters::default()
            }),
        },
        1280,
        960,
        25.0; "hikvision"
    )]
    #[test_case(
        vec![
            103, 100, 0, 50, 173, 132, 1, 12, 32, 8, 97, 0, 67, 8, 2,
            24, 64, 16, 194, 0, 132, 59, 80, 20, 0, 90, 211,
            112, 16, 16, 20, 0, 0, 3, 0, 4, 0, 0, 3, 0, 162, 16,
        ],
        SeqParameterSet{
            profile_idc: ProfileIdc::from(100),
            constraint_flags: ConstraintFlags::from(0),
            level_idc: 50,
            seq_parameter_set_id: SeqParamSetId::from_u32(0).unwrap(),
            chroma_info: ChromaInfo{
                chroma_format: ChromaFormat::YUV420,
                ..ChromaInfo::default()
            },
            /*seq_scaling_list: Some(SeqScalingList{
                scaling_list_4x4: vec![
                    vec![
                        16, 16, 16, 16, 16, 16, 16, 16,
                        16, 16, 16, 16, 16, 16, 16, 16,
                    ],
                    vec![
                        16, 16, 16, 16, 16, 16, 16, 16,
                        16, 16, 16, 16, 16, 16, 16, 16,
                    ],
                    vec![
                        16, 16, 16, 16, 16, 16, 16, 16,
                        16, 16, 16, 16, 16, 16, 16, 16,
                    ],
                    vec![
                        16, 16, 16, 16, 16, 16, 16, 16,
                        16, 16, 16, 16, 16, 16, 16, 16,
                    ],
                    vec![
                        16, 16, 16, 16, 16, 16, 16, 16,
                        16, 16, 16, 16, 16, 16, 16, 16,
                    ],
                    vec![
                        16, 16, 16, 16, 16, 16, 16, 16,
                        16, 16, 16, 16, 16, 16, 16, 16,
                    ],
                ],
                use_default_scaling_matrix_4x4_flag: vec![false, false, false, false, false, false],
                ..SeqScalingList::default()
            }),*/
            log2_max_frame_num_minus4: 6,
            pic_order_cnt: PicOrderCntType::TypeTwo,
            max_num_ref_frames: 1,
            gaps_in_frame_num_value_allowed_flag: true,
            pic_width_in_mbs_minus1: 159,
            pic_height_in_map_units_minus1: 89,
            frame_mbs_flags: FrameMbsFlags::Frames,
            direct_8x8_inference_flag: true,
            frame_cropping: None,
            vui_parameters: Some(VuiParameters{
                video_signal_type: Some(VideoSignalType{
                    video_format: VideoFormat::Unspecified,
                    video_full_range_flag: true,
                    colour_description: Some(ColourDescription{
                        colour_primaries: 1,
                        transfer_characteristics: 1,
                        matrix_coefficients: 1,
                    }),
                }),
                timing_info: Some(TimingInfo{
                    num_units_in_tick: 1,
                    time_scale: 40,
                    fixed_frame_rate_flag: true,
                }),
                ..VuiParameters::default()
            }),
        },
        2560,
        1440,
        20.0; "scaling matrix"
    )]
    #[test_case(
        vec![
            103, 100, 0, 42, 172, 44, 172, 7,
            128, 34, 126, 92, 5, 168, 8, 8,
            10, 0, 0, 7, 208, 0, 3, 169,
            129, 192, 0, 0, 76, 75, 0, 0,
            38, 37, 173, 222, 92, 20,
        ],
        SeqParameterSet{
            profile_idc: ProfileIdc::from(100),
            constraint_flags: ConstraintFlags::from(0),
            level_idc: 42,
            seq_parameter_set_id: SeqParamSetId::from_u32(0).unwrap(),
            chroma_info: ChromaInfo{
                chroma_format: ChromaFormat::YUV420,
                ..ChromaInfo::default()
            },
            log2_max_frame_num_minus4: 4,
            pic_order_cnt: PicOrderCntType::TypeZero {
                log2_max_pic_order_cnt_lsb_minus4: 4
            },
            max_num_ref_frames: 2,
            gaps_in_frame_num_value_allowed_flag: false,
            pic_width_in_mbs_minus1: 119,
            pic_height_in_map_units_minus1: 67,
            frame_mbs_flags: FrameMbsFlags::Frames,
            direct_8x8_inference_flag: true,
            frame_cropping: Some(FrameCropping{
                bottom_offset: 4,
                ..FrameCropping::default()
            }),
            vui_parameters: Some(VuiParameters{
                aspect_ratio_info: Some(AspectRatioInfo::Ratio1_1),
                video_signal_type: Some(VideoSignalType{
                    video_format: VideoFormat::Unspecified,
                    video_full_range_flag: false,
                    colour_description: Some(ColourDescription{
                        colour_primaries: 1,
                        transfer_characteristics: 1,
                        matrix_coefficients: 1,
                    }),
                }),
                timing_info: Some(TimingInfo{
                    num_units_in_tick: 1000,
                    time_scale: 120000,
                    fixed_frame_rate_flag: true,
                }),
                nal_hrd_parameters: Some(HrdParameters{
                    cpb_specs: vec![CpbSpec{
                        bit_rate_value_minus1: 39061,
                        cpb_size_value_minus1: 156249,
                        cbr_flag: true,
                    }],
                    initial_cpb_removal_delay_length_minus1: 23,
                    cpb_removal_delay_length_minus1: 15,
                    dpb_output_delay_length_minus1: 5,
                    time_offset_length: 24,
                    ..HrdParameters::default()
                }),
                low_delay_hrd_flag: Some(false),
                pic_struct_present_flag: true,
                ..VuiParameters::default()
            }),
        },
        1920,
        1080,
        60.0; "1920x1080 nvenc hrd"
    )]
    #[test_case(
        vec![
            103, 77, 0, 41, 154, 100, 3, 192,
            17, 63, 46, 2, 220, 4, 4, 5,
            0, 0, 3, 3, 232, 0, 0, 195,
            80, 232, 96, 0, 186, 180, 0, 2,
            234, 196, 187, 203, 141, 12, 0, 23,
            86, 128, 0, 93, 88, 151, 121, 112,
            160,
        ],
        SeqParameterSet{
            profile_idc: ProfileIdc::from(77),
            constraint_flags: ConstraintFlags::from(0),
            level_idc: 41,
            seq_parameter_set_id: SeqParamSetId::from_u32(0).unwrap(),
            chroma_info: ChromaInfo{
                chroma_format: ChromaFormat::YUV420,
                ..ChromaInfo::default()
            },
            log2_max_frame_num_minus4: 5,
            pic_order_cnt: PicOrderCntType::TypeZero {
                log2_max_pic_order_cnt_lsb_minus4: 5
            },
            max_num_ref_frames: 1,
            gaps_in_frame_num_value_allowed_flag: false,
            pic_width_in_mbs_minus1: 119,
            pic_height_in_map_units_minus1: 67,
            frame_mbs_flags: FrameMbsFlags::Frames,
            direct_8x8_inference_flag: true,
            frame_cropping: Some(FrameCropping{
                bottom_offset: 4,
                ..FrameCropping::default()
            }),
            vui_parameters: Some(VuiParameters{
                aspect_ratio_info: Some(AspectRatioInfo::Ratio1_1),
                video_signal_type: Some(VideoSignalType{
                    video_format: VideoFormat::Unspecified,
                    video_full_range_flag: true,
                    colour_description: Some(ColourDescription{
                        colour_primaries: 1,
                        transfer_characteristics: 1,
                        matrix_coefficients: 1,
                    }),
                }),
                timing_info: Some(TimingInfo{
                    num_units_in_tick: 1000,
                    time_scale: 50000,
                    fixed_frame_rate_flag: true,
                }),
                nal_hrd_parameters: Some(HrdParameters{
                    bit_rate_scale: 4,
                    cpb_size_scale: 3,
                    cpb_specs: vec![CpbSpec{
                        bit_rate_value_minus1: 11948,
                        cpb_size_value_minus1: 95585,
                        cbr_flag: false,
                    }],
                    initial_cpb_removal_delay_length_minus1: 23,
                    cpb_removal_delay_length_minus1: 15,
                    dpb_output_delay_length_minus1: 5,
                    time_offset_length: 24,
                }),
                vcl_hrd_parameters: Some(HrdParameters{
                    bit_rate_scale: 4,
                    cpb_size_scale: 3,
                    cpb_specs: vec![CpbSpec{
                        bit_rate_value_minus1: 11948,
                        cpb_size_value_minus1: 95585,
                        cbr_flag: false,
                    }],
                    initial_cpb_removal_delay_length_minus1: 23,
                    cpb_removal_delay_length_minus1: 15,
                    dpb_output_delay_length_minus1: 5,
                    time_offset_length: 24,
                    ..HrdParameters::default()
                }),
                low_delay_hrd_flag: Some(false),
                pic_struct_present_flag: true,
                ..VuiParameters::default()
            }),
        },
        1920,
        1080,
        25.0; "1920x1080 hikvision nal hrd + vcl hrd"
    )]
    fn test_sps(byts: Vec<u8>, sps: SeqParameterSet, width: u32, height: u32, fps: f64) {
        let sps_rbsp = decode_nal(&byts).unwrap();
        let sps2 = SeqParameterSet::from_bits(BitReader::new(&*sps_rbsp)).unwrap();

        let (width2, height2) = sps2.pixel_dimensions().unwrap();
        assert_eq!(sps, sps2);
        assert_eq!(width, width2);
        assert_eq!(height, height2);
        assert_eq!(fps, sps2.fps().unwrap());
    }
}
