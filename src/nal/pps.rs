use super::sps;
use crate::nal::sps::{SeqParamSetId, SeqParamSetIdError};
use crate::rbsp::BitRead;
use crate::{rbsp, Context};

#[derive(Debug)]
pub enum PpsError {
    RbspReaderError(rbsp::BitReaderError),
    InvalidSliceGroupMapType(u32),
    InvalidNumSliceGroupsMinus1(u32),
    InvalidNumRefIdx(&'static str, u32),
    InvalidSliceGroupChangeType(u32),
    UnknownSeqParamSetId(SeqParamSetId),
    BadPicParamSetId(PicParamSetIdError),
    BadSeqParamSetId(SeqParamSetIdError),
    ScalingMatrix(sps::ScalingMatrixError),
}

impl From<rbsp::BitReaderError> for PpsError {
    fn from(e: rbsp::BitReaderError) -> Self {
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
    fn from_id(id: u32) -> Result<SliceGroupChangeType, PpsError> {
        match id {
            3 => Ok(SliceGroupChangeType::BoxOut),
            4 => Ok(SliceGroupChangeType::RasterScan),
            5 => Ok(SliceGroupChangeType::WipeOut),
            _ => Err(PpsError::InvalidSliceGroupChangeType(id)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SliceRect {
    top_left: u32,
    bottom_right: u32,
}
impl SliceRect {
    fn read<R: BitRead>(r: &mut R) -> Result<SliceRect, PpsError> {
        Ok(SliceRect {
            top_left: r.read_ue("top_left")?,
            bottom_right: r.read_ue("bottom_right")?,
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
        slice_group_id: Vec<u32>,
    },
}
impl SliceGroup {
    fn read<R: BitRead>(r: &mut R, num_slice_groups_minus1: u32) -> Result<SliceGroup, PpsError> {
        let slice_group_map_type = r.read_ue("slice_group_map_type")?;
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
            3 | 4 | 5 => Ok(SliceGroup::Changing {
                change_type: SliceGroupChangeType::from_id(slice_group_map_type)?,
                num_slice_groups_minus1,
                slice_group_change_direction_flag: r
                    .read_bool("slice_group_change_direction_flag")?,
                slice_group_change_rate_minus1: r.read_ue("slice_group_change_rate_minus1")?,
            }),
            6 => Ok(SliceGroup::ExplicitAssignment {
                num_slice_groups_minus1,
                slice_group_id: Self::read_group_ids(r, num_slice_groups_minus1)?,
            }),
            _ => Err(PpsError::InvalidSliceGroupMapType(slice_group_map_type)),
        }
    }

    fn read_run_lengths<R: BitRead>(
        r: &mut R,
        num_slice_groups_minus1: u32,
    ) -> Result<Vec<u32>, PpsError> {
        let mut run_length_minus1 = Vec::with_capacity(num_slice_groups_minus1 as usize + 1);
        for _ in 0..num_slice_groups_minus1 + 1 {
            run_length_minus1.push(r.read_ue("run_length_minus1")?);
        }
        Ok(run_length_minus1)
    }

    fn read_rectangles<R: BitRead>(
        r: &mut R,
        num_slice_groups_minus1: u32,
    ) -> Result<Vec<SliceRect>, PpsError> {
        let mut run_length_minus1 = Vec::with_capacity(num_slice_groups_minus1 as usize + 1);
        for _ in 0..num_slice_groups_minus1 + 1 {
            run_length_minus1.push(SliceRect::read(r)?);
        }
        Ok(run_length_minus1)
    }

    fn read_group_ids<R: BitRead>(
        r: &mut R,
        num_slice_groups_minus1: u32,
    ) -> Result<Vec<u32>, PpsError> {
        let pic_size_in_map_units_minus1 = r.read_ue("pic_size_in_map_units_minus1")?;
        // TODO: avoid any panics due to failed conversions
        let size = (1f64 + f64::from(num_slice_groups_minus1)).log2().ceil() as u32;
        let mut run_length_minus1 = Vec::with_capacity(num_slice_groups_minus1 as usize + 1);
        for _ in 0..pic_size_in_map_units_minus1 + 1 {
            run_length_minus1.push(r.read(size, "slice_group_id")?);
        }
        Ok(run_length_minus1)
    }
}

#[derive(Debug, Clone)]
pub struct PicScalingMatrix {
    // TODO
}
impl PicScalingMatrix {
    fn read<R: BitRead>(
        r: &mut R,
        sps: &sps::SeqParameterSet,
        transform_8x8_mode_flag: bool,
    ) -> Result<Option<PicScalingMatrix>, PpsError> {
        let pic_scaling_matrix_present_flag = r.read_bool("pic_scaling_matrix_present_flag")?;
        Ok(if pic_scaling_matrix_present_flag {
            let mut scaling_list4x4 = vec![];
            let mut scaling_list8x8 = vec![];

            let count = if transform_8x8_mode_flag {
                if sps.chroma_info.chroma_format == sps::ChromaFormat::YUV444 {
                    6
                } else {
                    2
                }
            } else {
                0
            };
            for i in 0..6 + count {
                let seq_scaling_list_present_flag = r.read_bool("seq_scaling_list_present_flag")?;
                if seq_scaling_list_present_flag {
                    if i < 6 {
                        scaling_list4x4
                            .push(sps::ScalingList::read(r, 16).map_err(PpsError::ScalingMatrix)?);
                    } else {
                        scaling_list8x8
                            .push(sps::ScalingList::read(r, 64).map_err(PpsError::ScalingMatrix)?);
                    }
                }
            }
            Some(PicScalingMatrix {})
        } else {
            None
        })
    }
}

#[derive(Debug, Clone)]
pub struct PicParameterSetExtra {
    pub transform_8x8_mode_flag: bool,
    pub pic_scaling_matrix: Option<PicScalingMatrix>,
    pub second_chroma_qp_index_offset: i32,
}
impl PicParameterSetExtra {
    fn read<R: BitRead>(
        r: &mut R,
        sps: &sps::SeqParameterSet,
    ) -> Result<Option<PicParameterSetExtra>, PpsError> {
        Ok(if r.has_more_rbsp_data("transform_8x8_mode_flag")? {
            let transform_8x8_mode_flag = r.read_bool("transform_8x8_mode_flag")?;
            Some(PicParameterSetExtra {
                transform_8x8_mode_flag,
                pic_scaling_matrix: PicScalingMatrix::read(r, sps, transform_8x8_mode_flag)?,
                second_chroma_qp_index_offset: r.read_se("second_chroma_qp_index_offset")?,
            })
        } else {
            None
        })
    }
}

#[derive(Debug, PartialEq)]
pub enum PicParamSetIdError {
    IdTooLarge(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PicParamSetId(u8);
impl PicParamSetId {
    pub fn from_u32(id: u32) -> Result<PicParamSetId, PicParamSetIdError> {
        if id > 255 {
            Err(PicParamSetIdError::IdTooLarge(id))
        } else {
            Ok(PicParamSetId(id as u8))
        }
    }
    pub fn id(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Debug)]
pub struct PicParameterSet {
    pub pic_parameter_set_id: PicParamSetId,
    pub seq_parameter_set_id: SeqParamSetId,
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
    pub fn from_bits<R: BitRead>(ctx: &Context, mut r: R) -> Result<PicParameterSet, PpsError> {
        let pic_parameter_set_id = PicParamSetId::from_u32(r.read_ue("pic_parameter_set_id")?)
            .map_err(PpsError::BadPicParamSetId)?;
        let seq_parameter_set_id = SeqParamSetId::from_u32(r.read_ue("seq_parameter_set_id")?)
            .map_err(PpsError::BadSeqParamSetId)?;
        let seq_parameter_set = ctx
            .sps_by_id(seq_parameter_set_id)
            .ok_or_else(|| PpsError::UnknownSeqParamSetId(seq_parameter_set_id))?;
        let pps = PicParameterSet {
            pic_parameter_set_id,
            seq_parameter_set_id,
            entropy_coding_mode_flag: r.read_bool("entropy_coding_mode_flag")?,
            bottom_field_pic_order_in_frame_present_flag: r
                .read_bool("bottom_field_pic_order_in_frame_present_flag")?,
            slice_groups: Self::read_slice_groups(&mut r)?,
            num_ref_idx_l0_default_active_minus1: read_num_ref_idx(
                &mut r,
                "num_ref_idx_l0_default_active_minus1",
            )?,
            num_ref_idx_l1_default_active_minus1: read_num_ref_idx(
                &mut r,
                "num_ref_idx_l1_default_active_minus1",
            )?,
            weighted_pred_flag: r.read_bool("weighted_pred_flag")?,
            weighted_bipred_idc: r.read(2, "weighted_bipred_idc")?,
            pic_init_qp_minus26: r.read_se("pic_init_qp_minus26")?,
            pic_init_qs_minus26: r.read_se("pic_init_qs_minus26")?,
            chroma_qp_index_offset: r.read_se("chroma_qp_index_offset")?,
            deblocking_filter_control_present_flag: r
                .read_bool("deblocking_filter_control_present_flag")?,
            constrained_intra_pred_flag: r.read_bool("constrained_intra_pred_flag")?,
            redundant_pic_cnt_present_flag: r.read_bool("redundant_pic_cnt_present_flag")?,
            extension: PicParameterSetExtra::read(&mut r, seq_parameter_set)?,
        };
        r.finish_rbsp()?;
        Ok(pps)
    }

    fn read_slice_groups<R: BitRead>(r: &mut R) -> Result<Option<SliceGroup>, PpsError> {
        let num_slice_groups_minus1 = r.read_ue("num_slice_groups_minus1")?;
        if num_slice_groups_minus1 > 7 {
            // 7 is the maximum allowed in any profile; some profiles restrict it to 0.
            return Err(PpsError::InvalidNumSliceGroupsMinus1(
                num_slice_groups_minus1,
            ));
        }
        Ok(if num_slice_groups_minus1 > 0 {
            Some(SliceGroup::read(r, num_slice_groups_minus1)?)
        } else {
            None
        })
    }
}

fn read_num_ref_idx<R: BitRead>(r: &mut R, name: &'static str) -> Result<u32, PpsError> {
    let val = r.read_ue(name)?;
    if val > 31 {
        return Err(PpsError::InvalidNumRefIdx(name, val));
    }
    Ok(val)
}

#[cfg(test)]
mod test {
    use super::*;
    use hex_literal::*;

    #[test]
    fn test_it() {
        let data = hex!(
            "64 00 0A AC 72 84 44 26 84 00 00
            00 04 00 00 00 CA 3C 48 96 11 80"
        );
        let sps = super::sps::SeqParameterSet::from_bits(rbsp::BitReader::new(&data[..]))
            .expect("unexpected test data");
        let mut ctx = Context::default();
        ctx.put_seq_param_set(sps);
        let data = hex!("E8 43 8F 13 21 30");
        match PicParameterSet::from_bits(&ctx, rbsp::BitReader::new(&data[..])) {
            Err(e) => panic!("failed: {:?}", e),
            Ok(pps) => {
                println!("pps: {:#?}", pps);
                assert_eq!(pps.pic_parameter_set_id.id(), 0);
                assert_eq!(pps.seq_parameter_set_id.id(), 0);
            }
        }
    }

    #[test]
    fn test_transform_8x8_mode_with_scaling_matrix() {
        let sps = hex!(
            "64 00 29 ac 1b 1a 50 1e 00 89 f9 70 11 00 00 03 e9 00 00 bb 80 e2 60 00 04 c3 7a 00 00
             72 70 e8 c4 b8 c4 c0 00 09 86 f4 00 00 e4 e1 d1 89 70 f8 e1 85 2c"
        );
        let pps = hex!(
            "ea 8d ce 50 94 8d 18 b2 5a 55 28 4a 46 8c 59 2d 2a 50 c9 1a 31 64 b4 aa 85 48 d2 75 d5
             25 1d 23 49 d2 7a 23 74 93 7a 49 be 95 da ad d5 3d 7a 6b 54 22 9a 4e 93 d6 ea 9f a4 ee
             aa fd 6e bf f5 f7"
        );
        let sps = super::sps::SeqParameterSet::from_bits(rbsp::BitReader::new(&sps[..]))
            .expect("unexpected test data");
        let mut ctx = Context::default();
        ctx.put_seq_param_set(sps);

        let pps = PicParameterSet::from_bits(&ctx, rbsp::BitReader::new(&pps[..]))
            .expect("we mis-parsed pic_scaling_matrix when transform_8x8_mode_flag is active");

        // if transform_8x8_mode_flag were false or pic_scaling_matrix were None then we wouldn't
        // be recreating the required conditions for the test
        assert!(matches!(
            pps.extension,
            Some(PicParameterSetExtra {
                transform_8x8_mode_flag: true,
                pic_scaling_matrix: Some(_),
                ..
            })
        ));
    }

    // Earlier versions of h264-reader incorrectly limited pic_parameter_set_id to at most 32,
    // while the spec allows up to 255.  Test that a value over 32 is accepted.
    #[test]
    fn pps_id_greater32() {
        // test SPS/PPS values courtesy of @astraw
        let sps = hex!("42c01643235010020b3cf00f08846a");
        let pps = hex!("0448e3c8");
        let sps = sps::SeqParameterSet::from_bits(rbsp::BitReader::new(&sps[..])).unwrap();
        let mut ctx = Context::default();
        ctx.put_seq_param_set(sps);

        let pps = PicParameterSet::from_bits(&ctx, rbsp::BitReader::new(&pps[..])).unwrap();

        assert_eq!(pps.pic_parameter_set_id, PicParamSetId(33));
    }
}
