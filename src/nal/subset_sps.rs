//! Parser for `subset_seq_parameter_set_rbsp()` (NAL type 15, spec 7.3.2.1.3).
//!
//! A subset SPS wraps a base `SeqParameterSet` plus a profile-dependent extension:
//! - SVC extension (profiles 83/86, spec Annex F)
//! - MVC extension (profiles 118/128/134, spec Annex G)
//!
//! VUI parameter extensions are detected but not parsed; when present, `finish_rbsp()`
//! validation is skipped and `additional_extension2_flag` defaults to `false`.

use crate::nal::sps::{SeqParameterSet, SpsError};
use crate::rbsp::BitRead;

/// Profile-dependent extension data within a subset SPS.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubsetSpsExtension {
    Svc(SvcSpsExtension),
    Mvc {
        ext: MvcSpsExtension,
        mvc_vui_parameters_present_flag: bool,
    },
    /// MVCD extension (profiles 135/138/139). Parsing not implemented - fields not read.
    Mvcd,
}

/// SVC SPS extension (spec F.7.3.2.1.4, `seq_parameter_set_svc_extension`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SvcSpsExtension {
    pub inter_layer_deblocking_filter_control_present_flag: bool,
    pub extended_spatial_scalability_idc: u8,
    pub chroma_phase_x_plus1_flag: bool,
    pub chroma_phase_y_plus1: u8,
    pub seq_ref_layer_chroma_phase_x_plus1_flag: bool,
    pub seq_ref_layer_chroma_phase_y_plus1: u8,
    pub seq_scaled_ref_layer_left_offset: i32,
    pub seq_scaled_ref_layer_top_offset: i32,
    pub seq_scaled_ref_layer_right_offset: i32,
    pub seq_scaled_ref_layer_bottom_offset: i32,
    pub seq_tcoeff_level_prediction_flag: bool,
    pub adaptive_tcoeff_level_prediction_flag: bool,
    pub slice_header_restriction_flag: bool,
    pub svc_vui_parameters_present_flag: bool,
}

/// A single view in the MVC SPS extension.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MvcView {
    pub view_id: u16,
    pub anchor_refs_l0: Vec<u16>,
    pub anchor_refs_l1: Vec<u16>,
    pub non_anchor_refs_l0: Vec<u16>,
    pub non_anchor_refs_l1: Vec<u16>,
}

/// A single level-value entry with its applicable operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MvcLevelValue {
    pub level_idc: u8,
    pub applicable_ops: Vec<MvcApplicableOp>,
}

/// An applicable operation within an MVC level value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MvcApplicableOp {
    pub temporal_id: u8,
    pub num_target_views_minus1: u16,
    pub target_view_ids: Vec<u16>,
    pub num_views_minus1: u16,
}

/// MVC SPS extension (spec G.7.3.2.1.4, `seq_parameter_set_mvc_extension`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MvcSpsExtension {
    pub views: Vec<MvcView>,
    pub level_values: Vec<MvcLevelValue>,
}

/// Parsed `subset_seq_parameter_set_rbsp()` (NAL unit type 15).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubsetSps {
    pub sps: SeqParameterSet,
    pub extension: Option<SubsetSpsExtension>,
    pub additional_extension2_flag: bool,
}

/// Read a ue value and validate it fits in u16 with given max.
fn read_ue_bounded<R: BitRead>(
    r: &mut R,
    name: &'static str,
    max: u32,
) -> Result<u16, SpsError> {
    let val = r.read_ue(name)?;
    if val > max {
        return Err(SpsError::FieldValueTooLarge { name, value: val });
    }
    Ok(val as u16)
}

impl SubsetSps {
    pub fn from_bits<R: BitRead>(mut r: R) -> Result<SubsetSps, SpsError> {
        let sps = SeqParameterSet::read_seq_parameter_set_data(&mut r)?;
        let profile_idc: u8 = sps.profile_idc.into();

        let (extension, has_unparsed_vui) = match profile_idc {
            83 | 86 => {
                // bit_equal_to_one f(1) per spec F.7.3.2.1.3
                let _bit_equal_to_one = r.read_bool("bit_equal_to_one")?;
                let ext = read_svc_extension(&mut r, &sps)?;
                let has_vui = ext.svc_vui_parameters_present_flag;
                (Some(SubsetSpsExtension::Svc(ext)), has_vui)
            }
            118 | 128 | 134 => {
                // bit_equal_to_one f(1) per spec G.7.3.2.1.3
                let _bit_equal_to_one = r.read_bool("bit_equal_to_one")?;
                let ext = read_mvc_extension(&mut r)?;
                let mvc_vui_parameters_present_flag =
                    r.read_bool("mvc_vui_parameters_present_flag")?;
                (
                    Some(SubsetSpsExtension::Mvc {
                        ext,
                        mvc_vui_parameters_present_flag,
                    }),
                    mvc_vui_parameters_present_flag,
                )
            }
            135 | 138 | 139 => {
                // bit_equal_to_one f(1) per spec I.7.3.2.1.3
                let _bit_equal_to_one = r.read_bool("bit_equal_to_one")?;
                // MVCD extension -- parsing deferred, skip remaining data.
                (Some(SubsetSpsExtension::Mvcd), true)
            }
            _ => (None, false),
        };

        let additional_extension2_flag = if has_unparsed_vui {
            // VUI extension data follows but is not parsed; skip finish_rbsp() validation.
            false
        } else {
            let flag = r.read_bool("additional_extension2_flag")?;
            r.finish_rbsp()?;
            flag
        };

        Ok(SubsetSps {
            sps,
            extension,
            additional_extension2_flag,
        })
    }
}

fn read_svc_extension<R: BitRead>(
    r: &mut R,
    sps: &SeqParameterSet,
) -> Result<SvcSpsExtension, SpsError> {
    let inter_layer_deblocking_filter_control_present_flag =
        r.read_bool("inter_layer_deblocking_filter_control_present_flag")?;
    let extended_spatial_scalability_idc: u8 = r.read(2, "extended_spatial_scalability_idc")?;

    let chroma_array_type = sps.chroma_info.chroma_array_type();

    let chroma_phase_x_plus1_flag = if chroma_array_type == 1 || chroma_array_type == 2 {
        r.read_bool("chroma_phase_x_plus1_flag")?
    } else {
        false
    };
    let chroma_phase_y_plus1 = if chroma_array_type == 1 {
        r.read(2, "chroma_phase_y_plus1")?
    } else {
        // Default: 0 for Monochrome, 1 for YUV422/444
        if chroma_array_type == 0 { 0 } else { 1 }
    };

    let (
        seq_ref_layer_chroma_phase_x_plus1_flag,
        seq_ref_layer_chroma_phase_y_plus1,
        seq_scaled_ref_layer_left_offset,
        seq_scaled_ref_layer_top_offset,
        seq_scaled_ref_layer_right_offset,
        seq_scaled_ref_layer_bottom_offset,
    ) = if extended_spatial_scalability_idc == 1 {
        let ref_phase_x = if chroma_array_type == 1 || chroma_array_type == 2 {
            r.read_bool("seq_ref_layer_chroma_phase_x_plus1_flag")?
        } else {
            false
        };
        let ref_phase_y = if chroma_array_type == 1 {
            r.read(2, "seq_ref_layer_chroma_phase_y_plus1")?
        } else {
            if chroma_array_type == 0 { 0 } else { 1 }
        };
        (
            ref_phase_x,
            ref_phase_y,
            r.read_se("seq_scaled_ref_layer_left_offset")?,
            r.read_se("seq_scaled_ref_layer_top_offset")?,
            r.read_se("seq_scaled_ref_layer_right_offset")?,
            r.read_se("seq_scaled_ref_layer_bottom_offset")?,
        )
    } else {
        (false, if chroma_array_type == 0 { 0 } else { 1 }, 0, 0, 0, 0)
    };

    let seq_tcoeff_level_prediction_flag =
        r.read_bool("seq_tcoeff_level_prediction_flag")?;
    let adaptive_tcoeff_level_prediction_flag = if seq_tcoeff_level_prediction_flag {
        r.read_bool("adaptive_tcoeff_level_prediction_flag")?
    } else {
        false
    };
    let slice_header_restriction_flag = r.read_bool("slice_header_restriction_flag")?;
    let svc_vui_parameters_present_flag =
        r.read_bool("svc_vui_parameters_present_flag")?;

    Ok(SvcSpsExtension {
        inter_layer_deblocking_filter_control_present_flag,
        extended_spatial_scalability_idc,
        chroma_phase_x_plus1_flag,
        chroma_phase_y_plus1,
        seq_ref_layer_chroma_phase_x_plus1_flag,
        seq_ref_layer_chroma_phase_y_plus1,
        seq_scaled_ref_layer_left_offset,
        seq_scaled_ref_layer_top_offset,
        seq_scaled_ref_layer_right_offset,
        seq_scaled_ref_layer_bottom_offset,
        seq_tcoeff_level_prediction_flag,
        adaptive_tcoeff_level_prediction_flag,
        slice_header_restriction_flag,
        svc_vui_parameters_present_flag,
    })
}

fn read_mvc_extension<R: BitRead>(r: &mut R) -> Result<MvcSpsExtension, SpsError> {
    let num_views_minus1 = r.read_ue("num_views_minus1")?;
    if num_views_minus1 > 1023 {
        return Err(SpsError::FieldValueTooLarge {
            name: "num_views_minus1",
            value: num_views_minus1,
        });
    }

    let mut views = Vec::with_capacity(num_views_minus1 as usize + 1);
    for _ in 0..=num_views_minus1 {
        let view_id = read_ue_bounded(r, "view_id", 1023)?;
        views.push(MvcView {
            view_id,
            anchor_refs_l0: Vec::new(),
            anchor_refs_l1: Vec::new(),
            non_anchor_refs_l0: Vec::new(),
            non_anchor_refs_l1: Vec::new(),
        });
    }

    // anchor refs
    for i in 1..=num_views_minus1 as usize {
        let num_anchor_refs_l0 = r.read_ue("num_anchor_refs_l0")?;
        if num_anchor_refs_l0 > 15 {
            return Err(SpsError::FieldValueTooLarge {
                name: "num_anchor_refs_l0",
                value: num_anchor_refs_l0,
            });
        }
        for _ in 0..num_anchor_refs_l0 {
            views[i]
                .anchor_refs_l0
                .push(read_ue_bounded(r, "anchor_ref_l0", 1023)?);
        }
        let num_anchor_refs_l1 = r.read_ue("num_anchor_refs_l1")?;
        if num_anchor_refs_l1 > 15 {
            return Err(SpsError::FieldValueTooLarge {
                name: "num_anchor_refs_l1",
                value: num_anchor_refs_l1,
            });
        }
        for _ in 0..num_anchor_refs_l1 {
            views[i]
                .anchor_refs_l1
                .push(read_ue_bounded(r, "anchor_ref_l1", 1023)?);
        }
    }

    // non-anchor refs
    for i in 1..=num_views_minus1 as usize {
        let num_non_anchor_refs_l0 = r.read_ue("num_non_anchor_refs_l0")?;
        if num_non_anchor_refs_l0 > 15 {
            return Err(SpsError::FieldValueTooLarge {
                name: "num_non_anchor_refs_l0",
                value: num_non_anchor_refs_l0,
            });
        }
        for _ in 0..num_non_anchor_refs_l0 {
            views[i]
                .non_anchor_refs_l0
                .push(read_ue_bounded(r, "non_anchor_ref_l0", 1023)?);
        }
        let num_non_anchor_refs_l1 = r.read_ue("num_non_anchor_refs_l1")?;
        if num_non_anchor_refs_l1 > 15 {
            return Err(SpsError::FieldValueTooLarge {
                name: "num_non_anchor_refs_l1",
                value: num_non_anchor_refs_l1,
            });
        }
        for _ in 0..num_non_anchor_refs_l1 {
            views[i]
                .non_anchor_refs_l1
                .push(read_ue_bounded(r, "non_anchor_ref_l1", 1023)?);
        }
    }

    // level values
    let num_level_values_signalled_minus1 = r.read_ue("num_level_values_signalled_minus1")?;
    if num_level_values_signalled_minus1 > 63 {
        return Err(SpsError::FieldValueTooLarge {
            name: "num_level_values_signalled_minus1",
            value: num_level_values_signalled_minus1,
        });
    }

    let mut level_values =
        Vec::with_capacity(num_level_values_signalled_minus1 as usize + 1);
    for _ in 0..=num_level_values_signalled_minus1 {
        let level_idc: u8 = r.read(8, "level_idc")?;
        let num_applicable_ops_minus1 = r.read_ue("num_applicable_ops_minus1")?;
        if num_applicable_ops_minus1 > 1023 {
            return Err(SpsError::FieldValueTooLarge {
                name: "num_applicable_ops_minus1",
                value: num_applicable_ops_minus1,
            });
        }
        let mut applicable_ops =
            Vec::with_capacity(num_applicable_ops_minus1 as usize + 1);
        for _ in 0..=num_applicable_ops_minus1 {
            let temporal_id: u8 = r.read(3, "applicable_op_temporal_id")?;
            let num_target_views_minus1 =
                read_ue_bounded(r, "applicable_op_num_target_views_minus1", 1023)?;
            let mut target_view_ids =
                Vec::with_capacity(num_target_views_minus1 as usize + 1);
            for _ in 0..=num_target_views_minus1 {
                target_view_ids
                    .push(read_ue_bounded(r, "applicable_op_target_view_id", 1023)?);
            }
            let num_views_minus1 =
                read_ue_bounded(r, "applicable_op_num_views_minus1", 1023)?;
            applicable_ops.push(MvcApplicableOp {
                temporal_id,
                num_target_views_minus1,
                target_view_ids,
                num_views_minus1,
            });
        }
        level_values.push(MvcLevelValue {
            level_idc,
            applicable_ops,
        });
    }

    Ok(MvcSpsExtension {
        views,
        level_values,
    })
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::rbsp::BitReader;

    #[test]
    fn parse_subset_sps_unknown_profile() {
        // profile_idc=66 (Baseline, not an extension profile)
        // constraint_flags=0xC0
        // level_idc=30
        // seq_parameter_set_id=0 (ue: '1')
        // log2_max_frame_num_minus4=0 (ue: '1')
        // pic_order_cnt_type=0 (ue: '1')
        // log2_max_pic_order_cnt_lsb_minus4=0 (ue: '1')
        // max_num_ref_frames=0 (ue: '1')
        // gaps_in_frame_num_value_allowed_flag=0
        // pic_width_in_mbs_minus1=0 (ue: '1')
        // pic_height_in_map_units_minus1=0 (ue: '1')
        // frame_mbs_only_flag=1
        // direct_8x8_inference_flag=0
        // frame_cropping_flag=0
        // vui_parameters_present_flag=0
        // additional_extension2_flag=0
        // rbsp_stop_one_bit=1
        #[rustfmt::skip]
        let data = [
            0x42, // profile_idc=66
            0xC0, // constraint_flags
            0x1E, // level_idc=30
            // ue(0) x5: sps_id, log2_max_frame_num, poc_type, log2_poc_lsb, max_ref
            // 0: gaps_in_frame_num
            // ue(0) x2: pic_width, pic_height
            // bits so far: 1 1 1 1 1 0 1 1 = 0xFB
            0xFB,
            // 1: frame_mbs_only_flag (Frames)
            // 0: direct_8x8_inference_flag
            // 0: frame_cropping_flag
            // 0: vui_parameters_present_flag
            // 0: additional_extension2_flag
            // 1: rbsp_stop_one_bit
            // 00: padding
            // bits: 1 0 0 0 0 1 0 0 = 0x84
            0x84,
        ];
        let subset = SubsetSps::from_bits(BitReader::new(&data[..])).unwrap();
        assert_eq!(u8::from(subset.sps.profile_idc), 66);
        assert!(subset.extension.is_none());
        assert!(!subset.additional_extension2_flag);
    }
}
