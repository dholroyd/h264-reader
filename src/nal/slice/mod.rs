
use super::{Context, NalHeader};
use rbsp::RbspBitReader;
use rbsp::RbspBitReaderError;
use nal::pps::ParamSetId;
use nal::pps;
use bitreader::BitReaderError;
use nal::sps;

enum ParseState {
    Unstarted,
    Start(NalHeader),
    Continue(NalHeader),
}

#[derive(Debug,PartialEq)]
enum SliceFamily {
    P,
    B,
    I,
    SP,
    SI
}
#[derive(Debug,PartialEq)]
enum SliceExclusive {
    /// All slices in the picture have the same type
    Exclusive,
    /// Other slices in the picture may have a different type than the current slice
    NonExclusive,
}
#[derive(Debug, PartialEq)]
struct SliceType {
    family: SliceFamily,
    exclusive: SliceExclusive,
}
impl SliceType {
    fn from_id(id: u32) -> Result<SliceType, SliceHeaderError> {
        match id {
            0 => Ok(SliceType { family: SliceFamily::P, exclusive: SliceExclusive::NonExclusive }),
            1 => Ok(SliceType { family: SliceFamily::B, exclusive: SliceExclusive::NonExclusive }),
            2 => Ok(SliceType { family: SliceFamily::I, exclusive: SliceExclusive::NonExclusive }),
            3 => Ok(SliceType { family: SliceFamily::SP, exclusive: SliceExclusive::NonExclusive }),
            4 => Ok(SliceType { family: SliceFamily::SI, exclusive: SliceExclusive::NonExclusive }),
            5 => Ok(SliceType { family: SliceFamily::P, exclusive: SliceExclusive::Exclusive }),
            6 => Ok(SliceType { family: SliceFamily::B, exclusive: SliceExclusive::Exclusive }),
            7 => Ok(SliceType { family: SliceFamily::I, exclusive: SliceExclusive::Exclusive }),
            8 => Ok(SliceType { family: SliceFamily::SP, exclusive: SliceExclusive::Exclusive }),
            9 => Ok(SliceType { family: SliceFamily::SI, exclusive: SliceExclusive::Exclusive }),
            _ => Err(SliceHeaderError::InvalidSliceType(id))
        }
    }
}

#[derive(Debug)]
enum SliceHeaderError {
    ReaderError(BitReaderError),
    RbspError(RbspBitReaderError),
    InvalidSliceType(u32),
    InvalidSeqParamSetId(pps::ParamSetIdError),
    UndefinedPicParamSetId(pps::ParamSetId),
    UndefinedSeqParamSetId(pps::ParamSetId),
    ColourPlaneError(ColourPlaneError),
    InvalidModificationOfPicNumIdc(u32),
    InvalidMemoryManagementControlOperation(u32),
    InvalidSliceQpDelta(i32),
    InvalidSliceQsDelta(i32),
    InvalidDisableDeblockingFilterIdc(u32),
    /// `slice_alpha_c0_offset_div2` was outside the expected range of `-6` to `+6`
    InvalidSliceAlphaC0OffsetDiv2(i32),
    /// The header contained syntax elements that the parser isn't able to handle yet
    UnsupportedSyntax(&'static str),
}
impl From<BitReaderError> for SliceHeaderError {
    fn from(e: BitReaderError) -> Self {
        SliceHeaderError::ReaderError(e)
    }
}
impl From<RbspBitReaderError> for SliceHeaderError {
    fn from(e: RbspBitReaderError) -> Self {
        SliceHeaderError::RbspError(e)
    }
}
impl From<pps::ParamSetIdError> for SliceHeaderError {
    fn from(e: pps::ParamSetIdError) -> Self {
        SliceHeaderError::InvalidSeqParamSetId(e)
    }
}
impl From<ColourPlaneError> for SliceHeaderError {
    fn from(e: ColourPlaneError) -> Self {
        SliceHeaderError::ColourPlaneError(e)
    }
}

#[derive(Debug)]
enum ColourPlane {
    /// Indicates the _chroma_ colour plane
    Y,
    /// Indicates the _blue-difference_ colour plane
    Cb,
    /// Indicates the _red-difference_ colour plane
    Cr,
}
#[derive(Debug)]
enum ColourPlaneError {
    InvalidId(u8),
}
impl ColourPlane {
    fn from_id(id: u8) -> Result<ColourPlane, ColourPlaneError> {
        match id {
            0 => Ok(ColourPlane::Y),
            1 => Ok(ColourPlane::Cb),
            2 => Ok(ColourPlane::Cr),
            _ => Err(ColourPlaneError::InvalidId(id))
        }
    }
}

#[derive(Debug,PartialEq)]
enum Field {
    Top,
    Bottom,
}

#[derive(Debug,PartialEq)]
enum FieldPic {
    Frame,
    Field(Field),
}

#[derive(Debug,PartialEq)]
enum PicOrderCountLsb {
    Frame(u32),
    FieldsAbsolute { top: u32, bottom_delta: i32 },
    FieldsDelta([i32; 2]),
}

#[derive(Debug)]
enum NumRefIdxActive {
    P{ num_ref_idx_l0_active_minus1: u32 },
    B{ num_ref_idx_l0_active_minus1: u32, num_ref_idx_l1_active_minus1: u32 }
}
impl NumRefIdxActive {
    fn num_ref_idx_l0_active_minus1(&self) -> u32 {
        match *self {
            NumRefIdxActive::P { num_ref_idx_l0_active_minus1 } => num_ref_idx_l0_active_minus1,
            NumRefIdxActive::B { num_ref_idx_l0_active_minus1, .. } => num_ref_idx_l0_active_minus1,
        }
    }
}

#[derive(Debug)]
enum ModificationOfPicNums {
    Subtract(u32),
    Add(u32),
    LongTermRef(u32),
}
#[derive(Debug)]
enum RefPicListModifications {
    I,
    P {
        ref_pic_list_modification_l0: Vec<ModificationOfPicNums>,
    },
    B {
        ref_pic_list_modification_l0: Vec<ModificationOfPicNums>,
        ref_pic_list_modification_l1: Vec<ModificationOfPicNums>,
    },
}
impl RefPicListModifications {
    fn read(slice_family: &SliceFamily, r: &mut RbspBitReader) -> Result<RefPicListModifications, SliceHeaderError> {
        Ok(match slice_family {
            SliceFamily::I | SliceFamily::SI => RefPicListModifications::I,
            SliceFamily::B => RefPicListModifications::B {
                ref_pic_list_modification_l0: Self::read_list(r)?,
                ref_pic_list_modification_l1: Self::read_list(r)?,
            },
            SliceFamily::P | SliceFamily::SP => RefPicListModifications::P {
                ref_pic_list_modification_l0: Self::read_list(r)?,
            }
        })
    }

    fn read_list(r: &mut RbspBitReader) -> Result<Vec<ModificationOfPicNums>, SliceHeaderError> {
        let mut result = vec![];
        // either ref_pic_list_modification_flag_l0 or ref_pic_list_modification_flag_l1 depending
        // on call-site,
        if !r.read_bool_named("ref_pic_list_modification_flag")? {
            return Ok(result);
        }
        loop {
            match r.read_ue_named("modification_of_pic_nums_idc")? {
                0 => result.push(ModificationOfPicNums::Subtract(r.read_ue_named("abs_diff_pic_num_minus1")?)),
                1 => result.push(ModificationOfPicNums::Add(r.read_ue_named("abs_diff_pic_num_minus1")?)),
                2 => result.push(ModificationOfPicNums::LongTermRef(r.read_ue_named("long_term_pic_num")?)),
                3 => break,
                v => return Err(SliceHeaderError::InvalidModificationOfPicNumIdc(v)),
            }
        }
        Ok(result)
    }
}

#[derive(Debug)]
struct PredWeight {
    weight: i32,
    offset: i32,
}
#[derive(Debug)]
struct PredWeightTable {
    luma_log2_weight_denom: u32,
    chroma_log2_weight_denom: Option<u32>,
    luma_weights: Vec<Option<PredWeight>>,
    chroma_weights: Vec<Vec<PredWeight>>,
}
impl PredWeightTable {
    fn read(r: &mut RbspBitReader, slice_type: &SliceType, pps: &pps::PicParameterSet, sps: &sps::SeqParameterSet, num_ref_active: &Option<NumRefIdxActive>) -> Result<PredWeightTable, SliceHeaderError> {
        let chroma_array_type = if sps.chroma_info.separate_colour_plane_flag {
            // TODO: "Otherwise (separate_colour_plane_flag is equal to 1), ChromaArrayType is
            //       set equal to 0."  ...does this mean ChromaFormat::Monochrome then?
            sps::ChromaFormat::Monochrome
        } else {
            sps.chroma_info.chroma_format
        };
        let luma_log2_weight_denom = r.read_ue_named("luma_log2_weight_denom")?;
        let chroma_log2_weight_denom = if chroma_array_type != sps::ChromaFormat::Monochrome {
            Some(r.read_ue_named("chroma_log2_weight_denom")?)
        } else {
            None
        };
        let num_ref_idx_l0_active_minus1 = num_ref_active
            .as_ref()
            .map(|n| n.num_ref_idx_l0_active_minus1() )
            .unwrap_or_else(|| {
                pps.num_ref_idx_l0_default_active_minus1
            });
        let mut luma_weights = Vec::with_capacity((num_ref_idx_l0_active_minus1 + 1) as usize);
        let mut chroma_weights = Vec::with_capacity((num_ref_idx_l0_active_minus1 + 1) as usize);
        for i in 0..=num_ref_idx_l0_active_minus1 {
            if r.read_bool_named("luma_weight_l0_flag")? {
                luma_weights.push(Some(PredWeight {
                    weight: r.read_se_named("luma_weight_l0")?,
                    offset: r.read_se_named("luma_offset_l0")?,
                }));
            } else {
                luma_weights.push(None);
            }
            if chroma_array_type != sps::ChromaFormat::Monochrome {
                let mut weights = Vec::with_capacity(2); // TODO: just an array?
                if r.read_bool_named("chroma_weight_l0_flag")? {
                    for _j in 0..2 {
                        weights.push(PredWeight {
                            weight: r.read_se_named("chroma_weight_l0")?,
                            offset: r.read_se_named("chroma_offset_l0")?,
                        });

                    }
                }
                chroma_weights.push(weights);
            }
        }
        if slice_type.family == SliceFamily::B {
            unimplemented!("B Frame")
        }
        Ok(PredWeightTable {
            luma_log2_weight_denom,
            chroma_log2_weight_denom,
            luma_weights,
            chroma_weights,
        })
    }
}

#[derive(Debug)]
enum MemoryManagementControlOperation {
    /// `memory_management_control_operation` value of `1`
    ShortTermUnusedForRef { difference_of_pic_nums_minus1: u32 },
    /// `memory_management_control_operation` value of `2`
    LongTermUnusedForRef { long_term_pic_num: u32 },
    /// `memory_management_control_operation` value of `3`
    ShortTermUsedForLongTerm { difference_of_pic_nums_minus1: u32, long_term_frame_idx: u32 },
    /// `memory_management_control_operation` value of `4`
    MaxUsedLongTermFrameRef { max_long_term_frame_idx_plus1: u32 },
    /// `memory_management_control_operation` value of `5`
    AllRefPicturesUnused,
    /// `memory_management_control_operation` value of `6`
    CurrentUsedForLongTerm { long_term_frame_idx: u32 },
}

/// Decoded reference picture marking
#[derive(Debug)]
enum DecRefPicMarking {
    Idr {
        no_output_of_prior_pics_flag: bool,
        long_term_reference_flag: bool,
    },
    /// `adaptive_ref_pic_marking_mode_flag` equal to `0`
    SlidingWindow,
    /// `adaptive_ref_pic_marking_mode_flag` equal to `1`
    Adaptive(Vec<MemoryManagementControlOperation>),
}
impl DecRefPicMarking {
    fn read(r: &mut RbspBitReader, header: NalHeader) -> Result<DecRefPicMarking, SliceHeaderError> {
        Ok(if header.nal_unit_type() == ::nal::UnitType::SliceLayerWithoutPartitioningIdr {
            DecRefPicMarking::Idr {
                no_output_of_prior_pics_flag: r.read_bool_named("no_output_of_prior_pics_flag")?,
                long_term_reference_flag: r.read_bool_named("long_term_reference_flag")?,
            }
        } else {
            if r.read_bool_named("adaptive_ref_pic_marking_mode_flag")? {
                let mut ctl = vec![];
                loop {
                    let op = match r.read_ue_named("memory_management_control_operation")? {
                        0 => break,
                        1 => {
                            let difference_of_pic_nums_minus1 = r.read_ue_named("difference_of_pic_nums_minus1")?;
                            MemoryManagementControlOperation::ShortTermUnusedForRef{difference_of_pic_nums_minus1}
                        },
                        2 => {
                            let long_term_pic_num = r.read_ue_named("long_term_pic_num")?;
                            MemoryManagementControlOperation::LongTermUnusedForRef{long_term_pic_num}
                        },
                        3 => {
                            let difference_of_pic_nums_minus1 = r.read_ue_named("difference_of_pic_nums_minus1")?;
                            let long_term_frame_idx = r.read_ue_named("long_term_frame_idx")?;
                            MemoryManagementControlOperation::ShortTermUsedForLongTerm{difference_of_pic_nums_minus1, long_term_frame_idx}
                        },
                        4 => {
                            let max_long_term_frame_idx_plus1 = r.read_ue_named("max_long_term_frame_idx_plus1")?;
                            MemoryManagementControlOperation::MaxUsedLongTermFrameRef{max_long_term_frame_idx_plus1}
                        },
                        5 => {
                            MemoryManagementControlOperation::AllRefPicturesUnused
                        },
                        6 => {
                            let long_term_frame_idx = r.read_ue_named("long_term_frame_idx")?;
                            MemoryManagementControlOperation::CurrentUsedForLongTerm{long_term_frame_idx}
                        },
                        other => return Err(SliceHeaderError::InvalidMemoryManagementControlOperation(other))
                    };
                    ctl.push(op);
                }
                DecRefPicMarking::Adaptive(ctl)
            } else {
                DecRefPicMarking::SlidingWindow
            }
        })
    }
}

#[derive(Debug)]
struct SliceHeader {
    first_mb_in_slice: u32,
    slice_type: SliceType,
    colour_plane: Option<ColourPlane>,
    frame_num: u16,
    field_pic: FieldPic,
    idr_pic_id: Option<u32>,
    pic_order_cnt_lsb: Option<PicOrderCountLsb>,
    redundant_pic_cnt: Option<u32>,
    direct_spatial_mv_pred_flag: Option<bool>,
    num_ref_idx_active: Option<NumRefIdxActive>,
    ref_pic_list_modification: Option<RefPicListModifications>,  // may become an enum rather than Option in future (for ref_pic_list_mvc_modification)
    pred_weight_table: Option<PredWeightTable>,
    dec_ref_pic_marking: Option<DecRefPicMarking>,
    cabac_init_idc: Option<u32>,
    slice_qp_delta: i32,
    sp_for_switch_flag: Option<bool>,
    slice_qs: Option<u32>,
    disable_deblocking_filter_idc: u8,
}
impl SliceHeader {
    fn read(ctx: &mut Context, r: &mut RbspBitReader, header: NalHeader) -> Result<SliceHeader, SliceHeaderError> {
        let first_mb_in_slice = r.read_ue_named("first_mb_in_slice")?;
        let slice_type = SliceType::from_id(r.read_ue_named("slice_type")?)?;
        let pic_parameter_set_id = ParamSetId::from_u32(r.read_ue_named("pic_parameter_set_id")?)?;
        let pps = ctx.pps_by_id(pic_parameter_set_id)
            .ok_or(SliceHeaderError::UndefinedPicParamSetId(pic_parameter_set_id))?;
        let sps = ctx.sps_by_id(pps.seq_parameter_set_id)
            .ok_or(SliceHeaderError::UndefinedSeqParamSetId(pps.seq_parameter_set_id))?;
        let colour_plane = if sps.chroma_info.separate_colour_plane_flag {
            Some(ColourPlane::from_id(r.read_u8(2)?)?)
        } else {
            None
        };
        let frame_num = r.read_u16(sps.log2_max_frame_num())?;
        let field_pic = if let sps::FrameMbsFlags::Fields { .. } = sps.frame_mbs_flags {
            if r.read_bool_named("field_pic_flag")? {
                if r.read_bool_named("bottom_field_flag")? {
                    FieldPic::Field(Field::Bottom)
                } else {
                    FieldPic::Field(Field::Top)
                }
            } else {
                FieldPic::Frame
            }
        } else {
            FieldPic::Frame
        };
        let idr_pic_id = if header.nal_unit_type() == ::nal::UnitType::SliceLayerWithoutPartitioningIdr {
            Some(r.read_ue_named("idr_pic_id")?)
        } else {
            None
        };
        let pic_order_cnt_lsb = match sps.pic_order_cnt {
            sps::PicOrderCntType::TypeZero { log2_max_pic_order_cnt_lsb_minus4 } => {
                let pic_order_cnt_lsb = r.read_u32(log2_max_pic_order_cnt_lsb_minus4 + 4)?;
                Some(if pps.bottom_field_pic_order_in_frame_present_flag && field_pic == FieldPic::Frame {
                    let delta_pic_order_cnt_bottom = r.read_se()?;
                    PicOrderCountLsb::FieldsAbsolute {
                        top: pic_order_cnt_lsb,
                        bottom_delta: pic_order_cnt_lsb as i32 + delta_pic_order_cnt_bottom,
                    }
                } else {
                    PicOrderCountLsb::Frame(pic_order_cnt_lsb)
                })
            },
            sps::PicOrderCntType::TypeOne { delta_pic_order_always_zero_flag, offset_for_non_ref_pic, offset_for_top_to_bottom_field, ref offsets_for_ref_frame } => {
                if delta_pic_order_always_zero_flag {
                    None
                } else {
                    Some(PicOrderCountLsb::FieldsDelta([
                        r.read_se()?,
                        r.read_se()?
                    ]))
                }
            },
            sps::PicOrderCntType::TypeTwo => None,
        };
        let redundant_pic_cnt = if pps.redundant_pic_cnt_present_flag {
            Some(r.read_ue_named("redundant_pic_cnt ")?)
        } else {
            None
        };
        let direct_spatial_mv_pred_flag = if slice_type.family == SliceFamily::B {
            Some(r.read_bool_named("direct_spatial_mv_pred_flag")?)
        } else {
            None
        };
        let num_ref_idx_active = if slice_type.family == SliceFamily::P || slice_type.family == SliceFamily::SP || slice_type.family == SliceFamily::B {
            if r.read_bool_named("num_ref_idx_active_override_flag")? {
                let num_ref_idx_l0_active_minus1 = r.read_ue_named("num_ref_idx_l0_active_minus1")?;
                Some(if slice_type.family == SliceFamily::B {
                    NumRefIdxActive::B {
                        num_ref_idx_l0_active_minus1,
                        num_ref_idx_l1_active_minus1: r.read_ue_named("num_ref_idx_l1_active_minus1")?,
                    }
                } else {
                    NumRefIdxActive::P { num_ref_idx_l0_active_minus1 }
                })
            } else {
                None
            }
        } else {
            None
        };
        let ref_pic_list_modification = if header.nal_unit_type() == ::nal::UnitType::SliceExtension || header.nal_unit_type() == ::nal::UnitType::SliceExtensionViewComponent {
            return Err(SliceHeaderError::UnsupportedSyntax("NALU types 20 and 21 not yet supported"));
        } else {
            RefPicListModifications::read(&slice_type.family, r)?
        };
        let pred_weight_table = if (pps.weighted_pred_flag && slice_type.family == SliceFamily::P || slice_type.family == SliceFamily::SP)
            || (pps.weighted_bipred_idc == 1 && slice_type.family == SliceFamily::B) {
            Some(PredWeightTable::read(r, &slice_type, pps, sps, &num_ref_idx_active)?)
        } else {
            None
        };
        let dec_ref_pic_marking = if header.nal_ref_idc() == 0 {
            None
        } else {
            Some(DecRefPicMarking::read(r, header)?)
        };
        let cabac_init_idc = if pps.entropy_coding_mode_flag && slice_type.family != SliceFamily::I && slice_type.family != SliceFamily::SI {
            Some(r.read_ue_named("cabac_init_idc")?)
        } else {
            None
        };
        let slice_qp_delta = r.read_se()?;
        if slice_qp_delta > 51 {  // TODO: or less than -qp_bd_offset
            return Err(SliceHeaderError::InvalidSliceQpDelta(slice_qp_delta))
        }
        let mut sp_for_switch_flag = None;
        let mut slice_qs = None;
        if slice_type.family == SliceFamily::SP || slice_type.family != SliceFamily::SI {
            if slice_type.family == SliceFamily::SP {
                sp_for_switch_flag = Some(r.read_bool_named("sp_for_switch_flag")?);
            }
            let slice_qs_delta = r.read_se()?;
            let qs_y = 26 + pps.pic_init_qs_minus26 + slice_qs_delta;
            if qs_y < 0 || 51 < qs_y {
                return Err(SliceHeaderError::InvalidSliceQsDelta(slice_qs_delta))
            }
            slice_qs = Some(qs_y as u32);
        }
        let mut disable_deblocking_filter_idc = 0;
        if pps.deblocking_filter_control_present_flag {
            disable_deblocking_filter_idc = {
                let v = r.read_ue_named("disable_deblocking_filter_idc")?;
                if v > 6 {
                    return Err(SliceHeaderError::InvalidDisableDeblockingFilterIdc(v));
                }
                v as u8
            };
            if disable_deblocking_filter_idc != 1 {
                let slice_alpha_c0_offset_div2 = r.read_se()?;
                if slice_alpha_c0_offset_div2 < -6 || 6 < slice_alpha_c0_offset_div2 {
                    return Err(SliceHeaderError::InvalidSliceAlphaC0OffsetDiv2(slice_alpha_c0_offset_div2));
                }
                let slice_beta_offset_div2 = r.read_se()?;
            }
        }
        Ok(SliceHeader {
            first_mb_in_slice,
            slice_type,
            colour_plane,
            frame_num,
            field_pic,
            idr_pic_id,
            pic_order_cnt_lsb,
            redundant_pic_cnt,
            direct_spatial_mv_pred_flag,
            num_ref_idx_active,
            ref_pic_list_modification: Some(ref_pic_list_modification),
            pred_weight_table,
            dec_ref_pic_marking,
            cabac_init_idc,
            slice_qp_delta,
            sp_for_switch_flag,
            slice_qs,
            disable_deblocking_filter_idc,
        })
    }
}

pub struct SliceLayerWithoutPartitioningRbsp {
    state: ParseState,
}
impl super::NalHandler for SliceLayerWithoutPartitioningRbsp {
    fn start(&mut self, _ctx: &mut Context, header: &NalHeader) {
        println!("SliceLayerWithoutPartitioningRbsp: start()");
        self.state = ParseState::Start(*header);
    }

    fn push(&mut self, ctx: &mut Context, buf: &[u8]) {
        match self.state {
            ParseState::Unstarted => panic!("start() not yet called"),
            ParseState::Start(header) => {
                let mut r = RbspBitReader::new(buf);
                match SliceHeader::read(ctx, &mut r, header) {
                    Ok(header) => println!("{:#?}", header),
                    Err(e) => println!("slice_header() error: SliceHeaderError::{:?}", e),
                }
                self.state = ParseState::Continue(header);
            },
            ParseState::Continue(_header) => {
                // TODO
            }
        }
    }

    fn end(&mut self, _ctx: &mut Context) {
        println!("SliceLayerWithoutPartitioningRbsp: end()");
    }
}
impl Default for SliceLayerWithoutPartitioningRbsp {
    fn default() -> Self {
        SliceLayerWithoutPartitioningRbsp {
            state: ParseState::Unstarted
        }
    }
}