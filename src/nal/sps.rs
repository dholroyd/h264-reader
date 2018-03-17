
use super::NalHandler;
use super::NalHeader;
use bitreader;

trait H264BitReader {
    fn read_ue(&mut self) -> Result<u32,bitreader::BitReaderError>;
    fn read_se(&mut self) -> Result<i32,bitreader::BitReaderError>;
    fn has_more_rbsp_data(&self) -> bool;
    fn golomb_to_signed(val: u32) -> i32;
}
impl<'a> H264BitReader for bitreader::BitReader<'a> {
    fn read_ue(&mut self) -> Result<u32,bitreader::BitReaderError> {
        let count = count_zero_bits(self)?;
        if count > 0 {
            let val = self.read_u32(count)?;
            Ok((1 << count) -1 + val)
        } else {
            Ok(0)
        }
    }

    fn read_se(&mut self) -> Result<i32,bitreader::BitReaderError> {
        Ok(Self::golomb_to_signed(self.read_ue()?))
    }

    fn has_more_rbsp_data(&self) -> bool {
        unimplemented!()
    }

    fn golomb_to_signed(val: u32) -> i32 {
        let sign = (((val & 0x1) as i32) << 1) - 1;
        ((val >> 1) as i32 + (val & 0x1) as i32) * sign
    }
}
fn count_zero_bits(r: &mut bitreader::BitReader) -> Result<u8,bitreader::BitReaderError> {
    let mut count = 0;
    while !r.read_bool()? && count < 31 {
        count += 1;
    }
    Ok(count)
}

#[derive(Debug)]
pub enum SpsError {
    /// Signals that bit_depth_luma_minus8 was greater than the max value, 6
    BitDepthOutOfRange(u32),
    ReaderError(bitreader::BitReaderError),
    PicOrderCnt(PicOrderCntError)
}

impl From<bitreader::BitReaderError> for SpsError {
    fn from(e: bitreader::BitReaderError) -> Self {
        SpsError::ReaderError(e)
    }
}

pub struct SeqParameterSetNalHandler {
    buf: Vec<u8>,
}

impl SeqParameterSetNalHandler {
    fn new() -> SeqParameterSetNalHandler {
        SeqParameterSetNalHandler {
            buf: Vec::new(),
        }
    }
}
impl NalHandler for SeqParameterSetNalHandler {
    fn start(&mut self, header: &NalHeader) {
        assert_eq!(header.nal_unit_type(), super::UnitType::SeqParameterSet);
    }

    fn push(&mut self, buf: &[u8]) {
        self.buf.extend_from_slice(buf);
    }

    fn end(&mut self) {
        let sps = SeqParameterSet::from_bytes(&self.buf[..]);
        println!("sps: {:#?}", sps);
        self.buf.clear();
    }
}

#[derive(Debug)]
enum Profile {
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
    fn from_profile_idc(profile_idc: ProfileIdc) -> Profile {
        // TODO: accept constraint_flags too, as Level does?
        match profile_idc.0 {
            66  => Profile::Baseline,
            77  => Profile::Main,
            100 => Profile::High,
            122 => Profile::High422,
            110 => Profile::High10,
            144 => Profile::High444,
            88  => Profile::Extended,
            83  => Profile::ScalableBase,
            86  => Profile::ScalableHigh,
            118 => Profile::MultiviewHigh,
            128 => Profile::StereoHigh,
            135 => Profile::MFCDepthHigh,
            138 => Profile::MultiviewDepthHigh,
            139 => Profile::EnhancedMultiviewDepthHigh,
            other   => Profile::Unknown(other),
        }
    }
    fn profile_idc(&self) -> u8 {
        match *self {
            Profile::Baseline                   => 66,
            Profile::Main                       => 77,
            Profile::High                       => 100,
            Profile::High422                    => 122,
            Profile::High10                     => 110,
            Profile::High444                    => 144,
            Profile::Extended                   => 88,
            Profile::ScalableBase               => 83,
            Profile::ScalableHigh               => 86,
            Profile::MultiviewHigh              => 118,
            Profile::StereoHigh                 => 128,
            Profile::MFCDepthHigh               => 135,
            Profile::MultiviewDepthHigh         => 138,
            Profile::EnhancedMultiviewDepthHigh => 139,
            Profile::Unknown(profile_idc)       => profile_idc,
        }
    }
}

#[derive(Debug)]
enum Level {
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
}
impl Level {
    fn from_constraint_flags_and_level_idc(constraint_flags: &[bool; 6], level_idc: u8) -> Level {
        match level_idc {
            10 => Level::L1,
            11 => {
                if constraint_flags[3] {
                    Level::L1_b
                } else {
                    Level::L1_1
                }
            },
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
            _  => Level::Unknown(level_idc)
        }
    }
    fn level_idc(&self) -> u8{
        match *self {
            Level::L1    => 10,
            Level::L1_1 | Level::L1_b  => 11,
            Level::L1_2  => 12,
            Level::L1_3  => 13,
            Level::L2    => 20,
            Level::L2_1  => 21,
            Level::L2_2  => 22,
            Level::L3    => 30,
            Level::L3_1  => 31,
            Level::L3_2  => 32,
            Level::L4    => 40,
            Level::L4_1  => 41,
            Level::L4_2  => 42,
            Level::L5    => 50,
            Level::L5_1  => 51,
            Level::L5_2  => 52,
            Level::Unknown(level_idc) => level_idc,
        }
    }
}

#[derive(Debug)]
enum ChromaFormat {
    Monochrome,
    YUV420,
    YUV422,
    YUV444,
    Invalid(u32),
}
impl ChromaFormat {
    fn from_chroma_format_idc(chroma_format_idc: u32) -> ChromaFormat{
        match chroma_format_idc {
            0 => ChromaFormat::Monochrome,
            1 => ChromaFormat::YUV420,
            2 => ChromaFormat::YUV422,
            3 => ChromaFormat::YUV444,
            _ => ChromaFormat::Invalid(chroma_format_idc)
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct ProfileIdc(u8);
impl ProfileIdc {
    pub fn has_chroma_info(&self) -> bool {
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

struct ScalingList {
    // TODO
}
impl ScalingList {
    fn read(r: &mut bitreader::BitReader, size: u8) -> Result<ScalingList,bitreader::BitReaderError> {
        let mut scaling_list = vec!();
        let mut last_scale = 8;
        let mut next_scale = 8;
        let mut use_default_scaling_matrix_flag = false;
        for j in 0..size {
            if next_scale != 0 {
                let delta_scale = r.read_se()?;
                next_scale = (last_scale + delta_scale + 256) % 256;
                use_default_scaling_matrix_flag = j == 0 && next_scale == 0;
            }
            let new_value = if next_scale == 0 { last_scale } else { next_scale };
            scaling_list.push(new_value);
            last_scale = new_value;
        }
        Ok(ScalingList { })
    }
}
#[derive(Debug)]
struct ScalingMatrix {
    // TODO
}
impl Default for ScalingMatrix {
    fn default() -> Self {
        ScalingMatrix { }
    }
}
impl ScalingMatrix {
    fn read(r: &mut bitreader::BitReader, chroma_format_idc: u32) -> Result<ScalingMatrix,bitreader::BitReaderError> {
        let mut scaling_list4x4 = vec!();
        let mut scaling_list8x8 = vec!();

        let count = if chroma_format_idc == 3 { 12 } else { 8 };
        for i in 0..count {
            let seq_scaling_list_present_flag = r.read_bool()?;
            if seq_scaling_list_present_flag {
                if i < 6 {
                    scaling_list4x4.push(ScalingList::read(r, 16)?);
                } else {
                    scaling_list8x8.push(ScalingList::read(r, 64)?);
                }
            }
        }
        Ok(ScalingMatrix { })
    }
}

#[derive(Debug)]
struct ChromaInfo {
    chroma_format: ChromaFormat,
    separate_colour_plane_flag: bool,
    bit_depth_luma_minus8: u8,
    bit_depth_chroma_minus8: u8,
    qpprime_y_zero_transform_bypass_flag: bool,
    scaling_matrix: ScalingMatrix,
}
impl ChromaInfo {
    pub fn read(r: &mut bitreader::BitReader, profile_idc: ProfileIdc) -> Result<ChromaInfo, SpsError> {
        if profile_idc.has_chroma_info() {
            let chroma_format_idc = r.read_ue()?;
            Ok(ChromaInfo {
                chroma_format: ChromaFormat::from_chroma_format_idc(chroma_format_idc),
                separate_colour_plane_flag: if chroma_format_idc == 3 { r.read_bool()? } else { false },
                bit_depth_luma_minus8: Self::read_bit_depth_minus8(r)?,
                bit_depth_chroma_minus8: Self::read_bit_depth_minus8(r)?,
                qpprime_y_zero_transform_bypass_flag: r.read_bool()?,
                scaling_matrix: Self::read_scaling_matrix(r, chroma_format_idc)?
            })
        } else {
            Ok(ChromaInfo {
                chroma_format: ChromaFormat::YUV420,
                separate_colour_plane_flag: false,
                bit_depth_luma_minus8: 0,
                bit_depth_chroma_minus8: 0,
                qpprime_y_zero_transform_bypass_flag: false,
                scaling_matrix: ScalingMatrix::default(),
            })
        }
    }
    fn read_bit_depth_minus8(r: &mut bitreader::BitReader) -> Result<u8, SpsError> {
        let value = r.read_ue()?;
        if value > 6 {
            Err(SpsError::BitDepthOutOfRange(value))
        } else {
            Ok(value as u8)
        }
    }
    fn read_scaling_matrix(r: &mut bitreader::BitReader, chroma_format_idc: u32) -> Result<ScalingMatrix, SpsError> {
        let scaling_matrix_present_flag = r.read_bool()?;
        if scaling_matrix_present_flag {
            ScalingMatrix::read(r, chroma_format_idc).map_err(|e| e.into())
        } else {
            Ok(ScalingMatrix::default())
        }
    }
}

#[derive(Debug)]
enum PicOrderCntError {
    InvalidPicOrderCountType(u32),
    ReaderError(bitreader::BitReaderError),
}

impl From<bitreader::BitReaderError> for PicOrderCntError {
    fn from(e: bitreader::BitReaderError) -> Self {
        PicOrderCntError::ReaderError(e)
    }
}

#[derive(Debug)]
enum PicOrderCntType {
    TypeZero {
        log2_max_pic_order_cnt_lsb_minus4: u32
    },
    TypeOne {
        delta_pic_order_always_zero_flag: u32,
        offset_for_non_ref_pic: i32,
        offset_for_top_to_bottom_field: i32,
        offsets_for_ref_frame: Vec<i32>
    },
    TypeTwo
}
impl PicOrderCntType {
    fn read(r: &mut bitreader::BitReader) -> Result<PicOrderCntType, PicOrderCntError> {
        let pic_order_cnt_type = r.read_ue()?;
        match pic_order_cnt_type {
            0 => {
                Ok(PicOrderCntType::TypeZero {
                    log2_max_pic_order_cnt_lsb_minus4: r.read_ue()?
                })
            },
            1 => {
                Ok(PicOrderCntType::TypeOne {
                    delta_pic_order_always_zero_flag: r.read_ue()?,
                    offset_for_non_ref_pic: r.read_se()?,
                    offset_for_top_to_bottom_field: r.read_se()?,
                    offsets_for_ref_frame: Self::read_offsets_for_ref_frame(r)?,
                })
            },
            2 => {
                Ok(PicOrderCntType::TypeTwo)
            },
            _ => {
                Err(PicOrderCntError::InvalidPicOrderCountType(pic_order_cnt_type))
            }
        }
    }

    fn read_offsets_for_ref_frame(r: &mut bitreader::BitReader) -> Result<Vec<i32>, PicOrderCntError> {
        let num_ref_frames_in_pic_order_cnt_cycle = r.read_ue()?;
        let mut offsets = Vec::with_capacity(num_ref_frames_in_pic_order_cnt_cycle as usize);
        for i in 0..num_ref_frames_in_pic_order_cnt_cycle {
            offsets.push(r.read_se()?);
        }
        Ok(offsets)
    }
}

#[derive(Debug)]
enum FrameMbsFlags {
    Frames,
    Fields {
        mb_adaptive_frame_field_flag: bool,
    }
}
impl FrameMbsFlags {
    fn read(r: &mut bitreader::BitReader) -> Result<FrameMbsFlags, bitreader::BitReaderError> {
        let frame_mbs_only_flag = r.read_bool()?;
        if frame_mbs_only_flag {
            Ok(FrameMbsFlags::Frames)
        } else {
            Ok(FrameMbsFlags::Fields {
                mb_adaptive_frame_field_flag: r.read_bool()?
            })
        }
    }
}

#[derive(Debug)]
struct FrameCropping {
    left_offset: u32,
    right_offset: u32,
    top_offset: u32,
    bottom_offset: u32,
}
impl FrameCropping {
    fn read(r: &mut bitreader::BitReader) -> Result<Option<FrameCropping>,bitreader::BitReaderError> {
        let frame_cropping_flag = r.read_bool()?;
        Ok(if frame_cropping_flag {
            Some(FrameCropping {
                left_offset: r.read_ue()?,
                right_offset: r.read_ue()?,
                top_offset: r.read_ue()?,
                bottom_offset: r.read_ue()?,
            })
        } else {
            None
        })
    }
}

#[derive(Debug)]
enum AspectRatioInfo {
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
    Extended(u16,u16),

}
impl AspectRatioInfo {
    fn read(r: &mut bitreader::BitReader) -> Result<Option<AspectRatioInfo>,bitreader::BitReaderError> {
        let aspect_ratio_info_present_flag = r.read_bool()?;
        Ok(if aspect_ratio_info_present_flag {
            let aspect_ratio_idc = r.read_u8(8)?;
            Some(match aspect_ratio_idc {
                1 => AspectRatioInfo::Unspecified,
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
                255 => AspectRatioInfo::Extended(r.read_u16(16)?, r.read_u16(16)?),
                _ => AspectRatioInfo::Reserved(aspect_ratio_idc),
            })
        } else {
            None
        })
    }
}

#[derive(Debug)]
enum OverscanAppropriate {
    Unspecified,
    Appropriate,
    Inappropriate,
}
impl OverscanAppropriate {
    fn read(r: &mut bitreader::BitReader) -> Result<OverscanAppropriate,bitreader::BitReaderError> {
        let overscan_info_present_flag = r.read_bool()?;
        Ok(if overscan_info_present_flag {
            let overscan_appropriate_flag = r.read_bool()?;
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

#[derive(Debug)]
enum VideoFormat {
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
            6|7 => VideoFormat::Reserved(video_format),
            _ => panic!("unsupported video_format value {}", video_format),
        }
    }
}

#[derive(Debug)]
struct ColourDescription {
    colour_primaries: u8,
    transfer_characteristics: u8,
    matrix_coefficients: u8,
}
impl ColourDescription {
    fn read(r: &mut bitreader::BitReader) -> Result<Option<ColourDescription>,bitreader::BitReaderError> {
        let colour_description_present_flag = r.read_bool()?;
        Ok(if colour_description_present_flag {
            Some(ColourDescription {
                colour_primaries: r.read_u8(8)?,
                transfer_characteristics: r.read_u8(8)?,
                matrix_coefficients: r.read_u8(8)?,
            })
        } else {
            None
        })
    }
}

#[derive(Debug)]
struct VideoSignalType {
    video_format: VideoFormat,
    video_full_range_flag: bool,
    colour_description: Option<ColourDescription>,
}
impl VideoSignalType {
    fn read(r: &mut bitreader::BitReader) -> Result<Option<VideoSignalType>,bitreader::BitReaderError> {
        let video_signal_type_present_flag = r.read_bool()?;
        Ok(if video_signal_type_present_flag {
            Some(VideoSignalType {
                video_format: VideoFormat::from(r.read_u8(3)?),
                video_full_range_flag: r.read_bool()?,
                colour_description: ColourDescription::read(r)?,
            })
        } else {
            None
        })
    }
}

#[derive(Debug)]
struct ChromaLocInfo {
    chroma_sample_loc_type_top_field: u32,
    chroma_sample_loc_type_bottom_field: u32,
}
impl ChromaLocInfo {
    fn read(r: &mut bitreader::BitReader) -> Result<Option<ChromaLocInfo>,bitreader::BitReaderError> {
        let chroma_loc_info_present_flag = r.read_bool()?;
        Ok(if chroma_loc_info_present_flag {
            Some(ChromaLocInfo {
                chroma_sample_loc_type_top_field: r.read_ue()?,
                chroma_sample_loc_type_bottom_field: r.read_ue()?,
            })
        } else {
            None
        })
    }
}

#[derive(Debug)]
struct TimingInfo {
    num_units_in_tick: u32,
    time_scale: u32,
    fixed_frame_rate_flag: bool,
}
impl TimingInfo {
    fn read(r: &mut bitreader::BitReader) -> Result<Option<TimingInfo>,bitreader::BitReaderError> {
        let timing_info_present_flag = r.read_bool()?;
        Ok(if timing_info_present_flag {
            Some(TimingInfo {
                num_units_in_tick: r.read_u32(32)?,
                time_scale: r.read_u32(32)?,
                fixed_frame_rate_flag: r.read_bool()?,
            })
        } else {
            None
        })
    }
}

#[derive(Debug)]
struct CpbSpec {
    bit_rate_value_minus1: u32,
    cpb_size_value_minus1: u32,
    cbr_flag: bool,
}
impl CpbSpec {
    fn read(r: &mut bitreader::BitReader) -> Result<CpbSpec,bitreader::BitReaderError> {
        Ok(CpbSpec {
            bit_rate_value_minus1: r.read_ue()?,
            cpb_size_value_minus1: r.read_ue()?,
            cbr_flag: r.read_bool()?,
        })
    }
}


#[derive(Debug)]
struct HrdParameters {
    bit_rate_scale: u8,
    cpb_size_scale: u8,
    cpb_specs: Vec<CpbSpec>,
    initial_cpb_removal_delay_length_minus1: u8,
    cpb_removal_delay_length_minus1: u8,
    dpb_output_delay_length_minus1: u8,
    time_offset_length: u8,
}
impl HrdParameters {
    fn read(r: &mut bitreader::BitReader, hrd_parameters_present: &bool) -> Result<Option<HrdParameters>,bitreader::BitReaderError> {
        let hrd_parameters_present_flag = r.read_bool()?;
        *hrd_parameters_present != hrd_parameters_present_flag;
        Ok(if hrd_parameters_present_flag {
            let cpb_cnt_minus1 = r.read_ue()?;
            let cpb_cnt = cpb_cnt_minus1 + 1;
            Some(HrdParameters {
                bit_rate_scale: r.read_u8(4)?,
                cpb_size_scale: r.read_u8(4)?,
                cpb_specs: Self::read_cpb_specs(r, cpb_cnt)?,
                initial_cpb_removal_delay_length_minus1: r.read_u8(5)?,
                cpb_removal_delay_length_minus1: r.read_u8(5)?,
                dpb_output_delay_length_minus1: r.read_u8(5)?,
                time_offset_length: r.read_u8(5)?,
            })
        } else {
            None
        })
    }
    fn read_cpb_specs(r: &mut bitreader::BitReader, cpb_cnt: u32) -> Result<Vec<CpbSpec>,bitreader::BitReaderError> {
        let mut cpb_specs = Vec::with_capacity(cpb_cnt as usize);
        for _ in 0..cpb_cnt {
            cpb_specs.push(CpbSpec::read(r)?);
        }
        Ok(cpb_specs)
    }
}

#[derive(Debug)]
struct BitstreamRestrictions {
    motion_vectors_over_pic_boundaries_flag: bool,
    max_bytes_per_pic_denom: u32,
    max_bits_per_mb_denom: u32,
    log2_max_mv_length_horizontal: u32,
    log2_max_mv_length_vertical: u32,
    max_num_reorder_frames: u32,
    max_dec_frame_buffering: u32,
}
impl BitstreamRestrictions {
    fn read(r: &mut bitreader::BitReader) -> Result<Option<BitstreamRestrictions>,bitreader::BitReaderError> {
        let bitstream_restriction_flag = r.read_bool()?;
        Ok(if bitstream_restriction_flag {
            Some(BitstreamRestrictions {
                motion_vectors_over_pic_boundaries_flag: r.read_bool()?,
                max_bytes_per_pic_denom: r.read_ue()?,
                max_bits_per_mb_denom: r.read_ue()?,
                log2_max_mv_length_horizontal: r.read_ue()?,
                log2_max_mv_length_vertical: r.read_ue()?,
                max_num_reorder_frames: r.read_ue()?,
                max_dec_frame_buffering: r.read_ue()?,
            })
        } else {
            None
        })
    }
}

#[derive(Debug)]
struct VuiParameters {
    aspect_ratio_info: Option<AspectRatioInfo>,
    overscan_appropriate: OverscanAppropriate,
    video_signal_type: Option<VideoSignalType>,
    chroma_loc_info: Option<ChromaLocInfo>,
    timing_info: Option<TimingInfo>,
    nal_hrd_parameters: Option<HrdParameters>,
    vcl_hrd_parameters: Option<HrdParameters>,
    low_delay_hrd_flag: Option<bool>,
    pic_struct_present_flag: bool,
    bitstream_restrictions: Option<BitstreamRestrictions>,
}
impl VuiParameters {
    fn read(r: &mut bitreader::BitReader) -> Result<Option<VuiParameters>,bitreader::BitReaderError> {
        let vui_parameters_present_flag = r.read_bool()?;
        Ok(if vui_parameters_present_flag {
            let hrd_parameters_present = false;
            Some(VuiParameters {
                aspect_ratio_info: AspectRatioInfo::read(r)?,
                overscan_appropriate: OverscanAppropriate::read(r)?,
                video_signal_type: VideoSignalType::read(r)?,
                chroma_loc_info: ChromaLocInfo::read(r)?,
                timing_info: TimingInfo::read(r)?,
                nal_hrd_parameters: HrdParameters::read(r, &hrd_parameters_present)?,
                vcl_hrd_parameters: HrdParameters::read(r, &hrd_parameters_present)?,
                low_delay_hrd_flag: if hrd_parameters_present { Some(r.read_bool()?) } else { None },
                pic_struct_present_flag: r.read_bool()?,
                bitstream_restrictions: BitstreamRestrictions::read(r)?,
            })
        } else {
            None
        })
    }
}

#[derive(Debug)]
struct SeqParameterSet {
    profile_idc: ProfileIdc,
    constraint_flags: [bool; 6],
    reserved_zero_two_bits: u8,
    level_idc: u8,
    seq_parameter_set_id: u32,
    chroma_info: ChromaInfo,
    log2_max_frame_num_minus4: u32,
    pic_order_cnt: PicOrderCntType,
    max_num_ref_frames: u32,
    gaps_in_frame_num_value_allowed_flag: bool,
    pic_width_in_mbs_minus1: u32,
    pic_height_in_map_units_minus1: u32,
    frame_mbs_flags: FrameMbsFlags,
    direct_8x8_inference_flag: bool,
    frame_cropping: Option<FrameCropping>,
    vui_parameters: Option<VuiParameters>,
}
impl SeqParameterSet {
    fn from_bytes(buf: &[u8]) -> Result<SeqParameterSet, SpsError> {
        let mut r = bitreader::BitReader::new(buf);
        let profile_idc = r.read_u8(8)?.into();
        let constraint_flags = [
                r.read_bool()?,
                r.read_bool()?,
                r.read_bool()?,
                r.read_bool()?,
                r.read_bool()?,
                r.read_bool()?,
            ];
        let sps = SeqParameterSet {
            profile_idc,
            constraint_flags,
            reserved_zero_two_bits: r.read_u8(2)?,
            level_idc: r.read_u8(8)?,
            seq_parameter_set_id: r.read_ue()?,
            chroma_info: ChromaInfo::read(&mut r, profile_idc)?,
            log2_max_frame_num_minus4: r.read_ue()?,
            pic_order_cnt: PicOrderCntType::read(&mut r).map_err(|e| SpsError::PicOrderCnt(e))?,
            max_num_ref_frames: r.read_ue()?,
            gaps_in_frame_num_value_allowed_flag: r.read_bool()?,
            pic_width_in_mbs_minus1: r.read_ue()?,
            pic_height_in_map_units_minus1: r.read_ue()?,
            frame_mbs_flags: FrameMbsFlags::read(&mut r)?,
            direct_8x8_inference_flag: r.read_bool()?,
            frame_cropping: FrameCropping::read(&mut r)?,
            vui_parameters: VuiParameters::read(&mut r)?,
        };
        Ok(sps)
    }

    pub fn profile(&self) -> Profile {
        Profile::from_profile_idc(self.profile_idc)
    }

    pub fn level(&self) -> Level {
        Level::from_constraint_flags_and_level_idc(&self.constraint_flags, self.level_idc)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_it() {
        let data = hex!(
           "64 00 0A AC 72 84 44 26 84 00 00
            00 04 00 00 00 CA 3C 48 96 11 80");
        match SeqParameterSet::from_bytes(&data[..]) {
            Err(e) => panic!("failed: {:?}", e),
            Ok(sps) => {
                println!("sps: {:#?}", sps);
                assert_eq!(100, sps.profile_idc.0);
                assert_eq!(0, sps.reserved_zero_two_bits);
            }
        }
    }
}