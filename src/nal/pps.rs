use bitreader;
use super::NalHandler;
use super::NalHeader;
use super::sps;
use std::marker;
use crate::{rbsp, Context};
use crate::rbsp::RbspBitReader;

#[derive(Debug)]
pub enum PpsError {
    ReaderError(bitreader::BitReaderError),
    RbspReaderError(rbsp::RbspBitReaderError),
    InvalidSliceGroupMapType(u32),
    InvalidSliceGroupChangeType(u32),
    UnknownSeqParamSetId(ParamSetId),
    BadPicParamSetId(ParamSetIdError),
    BadSeqParamSetId(ParamSetIdError),
}

impl From<bitreader::BitReaderError> for PpsError {
    fn from(e: bitreader::BitReaderError) -> Self {
        PpsError::ReaderError(e)
    }
}
impl From<rbsp::RbspBitReaderError> for PpsError {
    fn from(e: rbsp::RbspBitReaderError) -> Self {
        PpsError::RbspReaderError(e)
    }
}

#[derive(Debug, Clone)]
pub enum SliceGroupChangeType {
    BoxOut,
    RasterScan,
    WipeOut,
}
impl SliceGroupChangeType {
    fn from_id(id: u32) -> Result<SliceGroupChangeType,PpsError> {
        match id {
            3 => Ok(SliceGroupChangeType::BoxOut),
            4 => Ok(SliceGroupChangeType::RasterScan),
            5 => Ok(SliceGroupChangeType::WipeOut),
            _ => Err(PpsError::InvalidSliceGroupChangeType(id))
        }
    }
}

#[derive(Debug, Clone)]
pub struct SliceRect {
    top_left: u32,
    bottom_right: u32,
}
impl SliceRect {
    fn read(r: &mut RbspBitReader<'_>) -> Result<SliceRect,PpsError> {
        Ok(SliceRect {
            top_left: r.read_ue()?,
            bottom_right: r.read_ue()?,
        })
    }
}

#[derive(Debug, Clone)]
pub enum SliceGroup {
    Interleaved {
        run_length_minus1: Vec<u32>,
    },
    Dispersed {
        num_slice_groups_minus1: u32,
    },
    ForegroundAndLeftover {
        rectangles: Vec<SliceRect>,
    },
    Changing {
        change_type: SliceGroupChangeType,
        num_slice_groups_minus1: u32,
        slice_group_change_direction_flag: bool,
        slice_group_change_rate_minus1: u32,
    },
    ExplicitAssignment {
        num_slice_groups_minus1: u32,
        slice_group_id: Vec<u32>
    },
}
impl SliceGroup {
    fn read(r: &mut RbspBitReader<'_>, num_slice_groups_minus1: u32) -> Result<SliceGroup,PpsError> {
        let slice_group_map_type = r.read_ue()?;
        match slice_group_map_type {
            0 => Ok(SliceGroup::Interleaved {
                run_length_minus1: Self::read_run_lengths(r, num_slice_groups_minus1)?,
            }),
            1 => Ok(SliceGroup::Dispersed {
                num_slice_groups_minus1,
            }),
            2 => Ok(SliceGroup::ForegroundAndLeftover {
                rectangles: Self::read_rectangles(r, num_slice_groups_minus1)?,
            }),
            3|4|5 => Ok(SliceGroup::Changing {
                change_type: SliceGroupChangeType::from_id(slice_group_map_type)?,
                num_slice_groups_minus1,
                slice_group_change_direction_flag: r.read_bool()?,
                slice_group_change_rate_minus1: r.read_ue()?,
            }),
            6 => Ok(SliceGroup::ExplicitAssignment {
                num_slice_groups_minus1,
                slice_group_id: Self::read_group_ids(r, num_slice_groups_minus1)?,
            }),
            _ => Err(PpsError::InvalidSliceGroupMapType(slice_group_map_type))
        }
    }

    fn read_run_lengths(r: &mut RbspBitReader<'_>, num_slice_groups_minus1: u32) -> Result<Vec<u32>,PpsError> {
        let mut run_length_minus1 = Vec::with_capacity(num_slice_groups_minus1 as usize + 1);
        for _ in 0..num_slice_groups_minus1+1 {
            run_length_minus1.push(r.read_ue()?);
        }
        Ok(run_length_minus1)
    }

    fn read_rectangles(r: &mut RbspBitReader<'_>, num_slice_groups_minus1: u32) -> Result<Vec<SliceRect>,PpsError> {
        let mut run_length_minus1 = Vec::with_capacity(num_slice_groups_minus1 as usize + 1);
        for _ in 0..num_slice_groups_minus1+1 {
            run_length_minus1.push(SliceRect::read(r)?);
        }
        Ok(run_length_minus1)
    }

    fn read_group_ids(r: &mut RbspBitReader<'_>, num_slice_groups_minus1: u32) -> Result<Vec<u32>,PpsError> {
        let pic_size_in_map_units_minus1 = r.read_ue()?;
        // TODO: avoid any panics due to failed conversions
        let size = ((1f64+f64::from(pic_size_in_map_units_minus1)).log2()) as u8;
        let mut run_length_minus1 = Vec::with_capacity(num_slice_groups_minus1 as usize + 1);
        for _ in 0..num_slice_groups_minus1+1 {
            run_length_minus1.push(r.read_u32(size)?);
        }
        Ok(run_length_minus1)
    }
}

#[derive(Debug, Clone)]
struct PicScalingMatrix {
    // TODO
}
impl PicScalingMatrix {
    fn read(r: &mut RbspBitReader<'_>, sps: &sps::SeqParameterSet, transform_8x8_mode_flag: bool) -> Result<Option<PicScalingMatrix>,PpsError> {
        let pic_scaling_matrix_present_flag = r.read_bool()?;
        Ok(if pic_scaling_matrix_present_flag {
            let mut scaling_list4x4 = vec!();
            let mut scaling_list8x8 = vec!();

            let count = if transform_8x8_mode_flag {
                if sps.chroma_info.chroma_format == sps::ChromaFormat::YUV444 { 12 } else { 8 }
            } else {
                0
            };
            for i in 0..6+count {
                let seq_scaling_list_present_flag = r.read_bool()?;
                if seq_scaling_list_present_flag {
                    if i < 6 {
                        scaling_list4x4.push(sps::ScalingList::read(r, 16)?);
                    } else {
                        scaling_list8x8.push(sps::ScalingList::read(r, 64)?);
                    }
                }
            }
            Some(PicScalingMatrix { })
        } else {
            None
        })
    }
}

#[derive(Debug, Clone)]
pub struct PicParameterSetExtra {
    transform_8x8_mode_flag: bool,
    pic_scaling_matrix: Option<PicScalingMatrix>,
    second_chroma_qp_index_offset: i32,
}
impl PicParameterSetExtra {
    fn read(r: &mut RbspBitReader<'_>, sps: &sps::SeqParameterSet) -> Result<Option<PicParameterSetExtra>,PpsError> {
        Ok(if r.has_more_rbsp_data() {
            let transform_8x8_mode_flag = r.read_bool()?;
            Some(PicParameterSetExtra {
                transform_8x8_mode_flag,
                pic_scaling_matrix: PicScalingMatrix::read(r, sps, transform_8x8_mode_flag)?,
                second_chroma_qp_index_offset: r.read_se_named("second_chroma_qp_index_offset")?,
            })
        } else {
            None
        })
    }
}

#[derive(Debug, PartialEq)]
pub enum ParamSetIdError {
    IdTooLarge(u32)
}

#[derive(Debug,Clone,Copy,PartialEq,Eq,Hash)]
pub struct ParamSetId(u8);
impl ParamSetId {
    pub fn from_u32(id: u32) -> Result<ParamSetId,ParamSetIdError> {
        if id > 31 {
            Err(ParamSetIdError::IdTooLarge(id))
        } else {
            Ok(ParamSetId(id as u8))
        }
    }
    pub fn id(self) -> u8 {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct PicParameterSet {
    pub pic_parameter_set_id: ParamSetId,
    pub seq_parameter_set_id: ParamSetId,
    pub entropy_coding_mode_flag: bool,
    pub bottom_field_pic_order_in_frame_present_flag: bool,
    pub slice_groups: Option<SliceGroup>,
    pub num_ref_idx_l0_default_active_minus1: u32,
    pub num_ref_idx_l1_default_active_minus1: u32,
    pub weighted_pred_flag: bool,
    pub weighted_bipred_idc: u8,
    pub pic_init_qp_minus26: i32,
    pub pic_init_qs_minus26: i32,
    pub chroma_qp_index_offset: i32,
    pub deblocking_filter_control_present_flag: bool,
    pub constrained_intra_pred_flag: bool,
    pub redundant_pic_cnt_present_flag: bool,
    pub extension: Option<PicParameterSetExtra>,
}
impl PicParameterSet {
    pub fn from_bytes<Ctx>(ctx: &Context<Ctx>, buf: &[u8]) -> Result<PicParameterSet, PpsError> {
        let mut r = RbspBitReader::new(buf);
        let pic_parameter_set_id = ParamSetId::from_u32(r.read_ue_named("pic_parameter_set_id")?)
            .map_err(PpsError::BadPicParamSetId)?;
        let seq_parameter_set_id = ParamSetId::from_u32(r.read_ue_named("seq_parameter_set_id")?)
            .map_err(PpsError::BadSeqParamSetId)?;
        let _seq_parameter_set = ctx.sps_by_id(seq_parameter_set_id)
            .ok_or_else(|| PpsError::UnknownSeqParamSetId(seq_parameter_set_id))?;
        Ok(PicParameterSet {
            pic_parameter_set_id,
            seq_parameter_set_id,
            entropy_coding_mode_flag: r.read_bool()?,
            bottom_field_pic_order_in_frame_present_flag: r.read_bool()?,
            slice_groups: Self::read_slice_groups(&mut r)?,
            num_ref_idx_l0_default_active_minus1: r.read_ue()?,
            num_ref_idx_l1_default_active_minus1: r.read_ue()?,
            weighted_pred_flag: r.read_bool()?,
            weighted_bipred_idc: r.read_u8(2)?,
            pic_init_qp_minus26: r.read_se()?,
            pic_init_qs_minus26: r.read_se()?,
            chroma_qp_index_offset: r.read_se()?,
            deblocking_filter_control_present_flag: r.read_bool()?,
            constrained_intra_pred_flag: r.read_bool()?,
            redundant_pic_cnt_present_flag: r.read_bool()?,
            extension: None, // TODO: buggy?: PicParameterSetExtra::read(&mut r, seq_parameter_set)?,
        })
    }

    fn read_slice_groups(r: &mut RbspBitReader<'_>) -> Result<Option<SliceGroup>,PpsError> {
        let num_slice_groups_minus1 = r.read_ue()?;
        Ok(if num_slice_groups_minus1 > 0 {
            Some(SliceGroup::read(r, num_slice_groups_minus1)?)
        } else {
            None
        })
    }
}

pub struct PicParameterSetNalHandler<Ctx> {
    buf: Vec<u8>,
    phantom: marker::PhantomData<Ctx>
}

impl<Ctx> Default for PicParameterSetNalHandler<Ctx> {
    fn default() -> Self {
        PicParameterSetNalHandler {
            buf: Vec::new(),
            phantom: marker::PhantomData,
        }
    }
}
impl<Ctx> NalHandler for PicParameterSetNalHandler<Ctx> {
    type Ctx = Ctx;

    fn start(&mut self, _ctx: &mut Context<Ctx>, header: NalHeader) {
        assert_eq!(header.nal_unit_type(), super::UnitType::PicParameterSet);
    }

    fn push(&mut self, _ctx: &mut Context<Ctx>, buf: &[u8]) {
        self.buf.extend_from_slice(buf);
    }

    fn end(&mut self, ctx: &mut Context<Ctx>) {
        let pps = PicParameterSet::from_bytes(ctx, &self.buf[..]);
        self.buf.clear();
        match pps {
            Ok(pps) => {
                ctx.put_pic_param_set(pps);
            },
            Err(e) => {
                eprintln!("pps: {:?}", e);
            },
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use hex_literal::*;

    #[test]
    fn test_it() {
        let sps_data = hex!(
           "64 00 0A AC 72 84 44 26 84 00 00
            00 04 00 00 00 CA 3C 48 96 11 80");
        let sps = super::sps::SeqParameterSet::from_bytes(&sps_data[..]).expect("unexpected test data");
        let mut ctx = Context::default();
        ctx.put_seq_param_set(sps);
        let data = hex!("E8 43 8F 13 21 30");
        match PicParameterSet::from_bytes(&mut ctx, &data[..]) {
            Err(e) => panic!("failed: {:?}", e),
            Ok(pps) => {
                println!("pps: {:#?}", pps);
                assert_eq!(pps.pic_parameter_set_id.id(), 0);
                assert_eq!(pps.seq_parameter_set_id.id(), 0);
            }
        }
    }
}
