//! Macroblock type definitions for H.264 baseline profile.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MbPartPredMode {
    Intra4x4,
    Intra8x8,
    Intra16x16,
    PredL0,
    Direct,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MbTypeError {
    InvalidIMbType(u32),
    InvalidPMbType(u32),
    InvalidPSubMbType(u32),
    InvalidCodedBlockPatternCodeNum(u32),
}
impl std::error::Error for MbTypeError {}
impl fmt::Display for MbTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MbTypeError::InvalidIMbType(v) => write!(f, "invalid I mb_type value {}", v),
            MbTypeError::InvalidPMbType(v) => write!(f, "invalid P mb_type value {}", v),
            MbTypeError::InvalidPSubMbType(v) => write!(f, "invalid P sub_mb_type value {}", v),
            MbTypeError::InvalidCodedBlockPatternCodeNum(v) => {
                write!(f, "invalid coded_block_pattern code_num {}", v)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IMbTypeInfo {
    INxN,
    I16x16 {
        intra16x16_pred_mode: u8,
        coded_block_pattern_chroma: u8,
        coded_block_pattern_luma: u8,
    },
    IPCM,
}

impl IMbTypeInfo {
    pub fn pred_mode(&self) -> MbPartPredMode {
        match self {
            IMbTypeInfo::INxN => MbPartPredMode::Intra4x4,
            IMbTypeInfo::I16x16 { .. } => MbPartPredMode::Intra16x16,
            IMbTypeInfo::IPCM => MbPartPredMode::Intra4x4,
        }
    }

    pub fn num_parts(&self) -> u8 {
        1
    }

    pub fn part_width(&self) -> u8 {
        16
    }

    pub fn part_height(&self) -> u8 {
        16
    }
}

pub fn i_mb_type_info(mb_type: u32) -> Result<IMbTypeInfo, MbTypeError> {
    match mb_type {
        0 => Ok(IMbTypeInfo::INxN),
        1..=24 => {
            let val = mb_type - 1;
            let (coded_block_pattern_luma, idx) = if val < 12 { (0, val) } else { (15, val - 12) };
            let intra16x16_pred_mode = (idx % 4) as u8;
            let coded_block_pattern_chroma = (idx / 4) as u8;
            Ok(IMbTypeInfo::I16x16 {
                intra16x16_pred_mode,
                coded_block_pattern_chroma,
                coded_block_pattern_luma,
            })
        }
        25 => Ok(IMbTypeInfo::IPCM),
        _ => Err(MbTypeError::InvalidIMbType(mb_type)),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PMbTypeInfo {
    P {
        num_parts: u8,
        pred_mode: MbPartPredMode,
        part_width: u8,
        part_height: u8,
        ref_idx_forced_zero: bool,
    },
    I(IMbTypeInfo),
}

impl PMbTypeInfo {
    pub fn num_parts(&self) -> u8 {
        match self {
            PMbTypeInfo::P { num_parts, .. } => *num_parts,
            PMbTypeInfo::I(info) => info.num_parts(),
        }
    }

    pub fn part_width(&self) -> u8 {
        match self {
            PMbTypeInfo::P { part_width, .. } => *part_width,
            PMbTypeInfo::I(info) => info.part_width(),
        }
    }

    pub fn part_height(&self) -> u8 {
        match self {
            PMbTypeInfo::P { part_height, .. } => *part_height,
            PMbTypeInfo::I(info) => info.part_height(),
        }
    }

    pub fn pred_mode(&self) -> MbPartPredMode {
        match self {
            PMbTypeInfo::P { pred_mode, .. } => *pred_mode,
            PMbTypeInfo::I(info) => info.pred_mode(),
        }
    }
}

pub fn p_mb_type_info(mb_type: u32) -> Result<PMbTypeInfo, MbTypeError> {
    match mb_type {
        0 => Ok(PMbTypeInfo::P {
            num_parts: 1,
            pred_mode: MbPartPredMode::PredL0,
            part_width: 16,
            part_height: 16,
            ref_idx_forced_zero: false,
        }),
        1 => Ok(PMbTypeInfo::P {
            num_parts: 2,
            pred_mode: MbPartPredMode::PredL0,
            part_width: 16,
            part_height: 8,
            ref_idx_forced_zero: false,
        }),
        2 => Ok(PMbTypeInfo::P {
            num_parts: 2,
            pred_mode: MbPartPredMode::PredL0,
            part_width: 8,
            part_height: 16,
            ref_idx_forced_zero: false,
        }),
        3 => Ok(PMbTypeInfo::P {
            num_parts: 4,
            pred_mode: MbPartPredMode::PredL0,
            part_width: 8,
            part_height: 8,
            ref_idx_forced_zero: false,
        }),
        4 => Ok(PMbTypeInfo::P {
            num_parts: 4,
            pred_mode: MbPartPredMode::PredL0,
            part_width: 8,
            part_height: 8,
            ref_idx_forced_zero: true,
        }),
        5..=30 => {
            let i_type =
                i_mb_type_info(mb_type - 5).map_err(|_| MbTypeError::InvalidPMbType(mb_type))?;
            Ok(PMbTypeInfo::I(i_type))
        }
        _ => Err(MbTypeError::InvalidPMbType(mb_type)),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubMbTypeInfo {
    pub num_sub_parts: u8,
    pub sub_part_width: u8,
    pub sub_part_height: u8,
    pub pred_mode: MbPartPredMode,
}

pub fn p_sub_mb_type_info(sub_mb_type: u32) -> Result<SubMbTypeInfo, MbTypeError> {
    match sub_mb_type {
        0 => Ok(SubMbTypeInfo {
            num_sub_parts: 1,
            sub_part_width: 8,
            sub_part_height: 8,
            pred_mode: MbPartPredMode::PredL0,
        }),
        1 => Ok(SubMbTypeInfo {
            num_sub_parts: 2,
            sub_part_width: 8,
            sub_part_height: 4,
            pred_mode: MbPartPredMode::PredL0,
        }),
        2 => Ok(SubMbTypeInfo {
            num_sub_parts: 2,
            sub_part_width: 4,
            sub_part_height: 8,
            pred_mode: MbPartPredMode::PredL0,
        }),
        3 => Ok(SubMbTypeInfo {
            num_sub_parts: 4,
            sub_part_width: 4,
            sub_part_height: 4,
            pred_mode: MbPartPredMode::PredL0,
        }),
        _ => Err(MbTypeError::InvalidPSubMbType(sub_mb_type)),
    }
}

const CBP_INTRA_MAP: [u8; 48] = [
    47, 31, 15, 0, 23, 27, 29, 30, 7, 11, 13, 14, 39, 43, 45, 46, 16, 3, 5, 10, 12, 19, 21, 26, 28,
    35, 37, 42, 44, 1, 2, 4, 8, 17, 18, 20, 24, 6, 9, 22, 25, 32, 33, 34, 36, 40, 38, 41,
];

const CBP_INTER_MAP: [u8; 48] = [
    0, 16, 1, 2, 4, 8, 32, 3, 5, 10, 12, 15, 47, 7, 11, 13, 14, 6, 9, 31, 35, 37, 42, 44, 33, 34,
    36, 40, 39, 43, 45, 46, 17, 18, 20, 24, 19, 21, 26, 28, 23, 27, 29, 30, 22, 25, 38, 41,
];

/// Convert a exp-golomb `code_num` into `(coded_block_pattern_luma, coded_block_pattern_chroma)`
/// for 4:2:0 chroma format (Table 9-4).
///
/// - `code_num`: the decoded me(v) value, must be in range 0..=47
/// - `is_intra`: `true` for Intra macroblocks, `false` for Inter macroblocks
///
/// Returns `None` if `code_num` is out of range.
pub fn coded_block_pattern_from_me(code_num: u32, is_intra: bool) -> Option<(u8, u8)> {
    if code_num > 47 {
        return None;
    }
    let table = if is_intra {
        &CBP_INTRA_MAP
    } else {
        &CBP_INTER_MAP
    };
    let cbp = table[code_num as usize];
    let coded_block_pattern_luma = cbp % 16;
    let coded_block_pattern_chroma = cbp / 16;
    Some((coded_block_pattern_luma, coded_block_pattern_chroma))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn i_mb_type_0_is_inxn() {
        let info = i_mb_type_info(0).unwrap();
        assert_eq!(info, IMbTypeInfo::INxN);
        assert_eq!(info.pred_mode(), MbPartPredMode::Intra4x4);
    }

    #[test]
    fn i_mb_type_1_is_i16x16_0_0_0() {
        let info = i_mb_type_info(1).unwrap();
        assert_eq!(
            info,
            IMbTypeInfo::I16x16 {
                intra16x16_pred_mode: 0,
                coded_block_pattern_chroma: 0,
                coded_block_pattern_luma: 0,
            }
        );
        assert_eq!(info.pred_mode(), MbPartPredMode::Intra16x16);
    }

    #[test]
    fn i_mb_type_2_is_i16x16_1_0_0() {
        let info = i_mb_type_info(2).unwrap();
        assert_eq!(
            info,
            IMbTypeInfo::I16x16 {
                intra16x16_pred_mode: 1,
                coded_block_pattern_chroma: 0,
                coded_block_pattern_luma: 0,
            }
        );
    }

    #[test]
    fn i_mb_type_5_is_i16x16_0_1_0() {
        let info = i_mb_type_info(5).unwrap();
        assert_eq!(
            info,
            IMbTypeInfo::I16x16 {
                intra16x16_pred_mode: 0,
                coded_block_pattern_chroma: 1,
                coded_block_pattern_luma: 0,
            }
        );
    }

    #[test]
    fn i_mb_type_13_is_i16x16_0_0_1() {
        let info = i_mb_type_info(13).unwrap();
        assert_eq!(
            info,
            IMbTypeInfo::I16x16 {
                intra16x16_pred_mode: 0,
                coded_block_pattern_chroma: 0,
                coded_block_pattern_luma: 15,
            }
        );
    }

    #[test]
    fn i_mb_type_24_is_i16x16_3_2_1() {
        let info = i_mb_type_info(24).unwrap();
        assert_eq!(
            info,
            IMbTypeInfo::I16x16 {
                intra16x16_pred_mode: 3,
                coded_block_pattern_chroma: 2,
                coded_block_pattern_luma: 15,
            }
        );
    }

    #[test]
    fn i_mb_type_25_is_ipcm() {
        let info = i_mb_type_info(25).unwrap();
        assert_eq!(info, IMbTypeInfo::IPCM);
    }

    #[test]
    fn i_mb_type_26_is_invalid() {
        assert_eq!(i_mb_type_info(26), Err(MbTypeError::InvalidIMbType(26)));
    }

    #[test]
    fn p_mb_type_0_is_p_l0_16x16() {
        let info = p_mb_type_info(0).unwrap();
        assert_eq!(
            info,
            PMbTypeInfo::P {
                num_parts: 1,
                pred_mode: MbPartPredMode::PredL0,
                part_width: 16,
                part_height: 16,
                ref_idx_forced_zero: false,
            }
        );
        assert_eq!(info.num_parts(), 1);
    }

    #[test]
    fn p_mb_type_1_is_p_l0_l0_16x8() {
        let info = p_mb_type_info(1).unwrap();
        assert_eq!(info.num_parts(), 2);
        assert_eq!(info.part_width(), 16);
        assert_eq!(info.part_height(), 8);
    }

    #[test]
    fn p_mb_type_2_is_p_l0_l0_8x16() {
        let info = p_mb_type_info(2).unwrap();
        assert_eq!(info.num_parts(), 2);
        assert_eq!(info.part_width(), 8);
        assert_eq!(info.part_height(), 16);
    }

    #[test]
    fn p_mb_type_3_is_p_8x8() {
        let info = p_mb_type_info(3).unwrap();
        assert_eq!(info.num_parts(), 4);
        assert_eq!(info.part_width(), 8);
        assert_eq!(info.part_height(), 8);
    }

    #[test]
    fn p_mb_type_4_is_p_8x8ref0() {
        let info = p_mb_type_info(4).unwrap();
        match info {
            PMbTypeInfo::P {
                ref_idx_forced_zero,
                ..
            } => assert!(ref_idx_forced_zero),
            _ => panic!("expected P type"),
        }
    }

    #[test]
    fn p_mb_type_5_maps_to_inxn() {
        let info = p_mb_type_info(5).unwrap();
        assert_eq!(info, PMbTypeInfo::I(IMbTypeInfo::INxN));
    }

    #[test]
    fn p_mb_type_6_maps_to_i16x16_0_0_0() {
        let info = p_mb_type_info(6).unwrap();
        assert_eq!(
            info,
            PMbTypeInfo::I(IMbTypeInfo::I16x16 {
                intra16x16_pred_mode: 0,
                coded_block_pattern_chroma: 0,
                coded_block_pattern_luma: 0,
            })
        );
    }

    #[test]
    fn p_mb_type_30_maps_to_ipcm() {
        let info = p_mb_type_info(30).unwrap();
        assert_eq!(info, PMbTypeInfo::I(IMbTypeInfo::IPCM));
    }

    #[test]
    fn p_mb_type_31_is_invalid() {
        assert_eq!(p_mb_type_info(31), Err(MbTypeError::InvalidPMbType(31)));
    }

    #[test]
    fn p_sub_mb_type_0_is_8x8() {
        let info = p_sub_mb_type_info(0).unwrap();
        assert_eq!(
            info,
            SubMbTypeInfo {
                num_sub_parts: 1,
                sub_part_width: 8,
                sub_part_height: 8,
                pred_mode: MbPartPredMode::PredL0,
            }
        );
    }

    #[test]
    fn p_sub_mb_type_1_is_8x4() {
        let info = p_sub_mb_type_info(1).unwrap();
        assert_eq!(info.num_sub_parts, 2);
        assert_eq!(info.sub_part_width, 8);
        assert_eq!(info.sub_part_height, 4);
    }

    #[test]
    fn p_sub_mb_type_2_is_4x8() {
        let info = p_sub_mb_type_info(2).unwrap();
        assert_eq!(info.num_sub_parts, 2);
        assert_eq!(info.sub_part_width, 4);
        assert_eq!(info.sub_part_height, 8);
    }

    #[test]
    fn p_sub_mb_type_3_is_4x4() {
        let info = p_sub_mb_type_info(3).unwrap();
        assert_eq!(info.num_sub_parts, 4);
        assert_eq!(info.sub_part_width, 4);
        assert_eq!(info.sub_part_height, 4);
    }

    #[test]
    fn p_sub_mb_type_4_is_invalid() {
        assert_eq!(
            p_sub_mb_type_info(4),
            Err(MbTypeError::InvalidPSubMbType(4))
        );
    }

    #[test]
    fn cbp_intra_code_num_0_gives_15_2() {
        let (luma, chroma) = coded_block_pattern_from_me(0, true).unwrap();
        assert_eq!(luma, 15);
        assert_eq!(chroma, 2);
    }

    #[test]
    fn cbp_inter_code_num_0_gives_0_0() {
        let (luma, chroma) = coded_block_pattern_from_me(0, false).unwrap();
        assert_eq!(luma, 0);
        assert_eq!(chroma, 0);
    }

    #[test]
    fn cbp_intra_code_num_3_gives_0_0() {
        let (luma, chroma) = coded_block_pattern_from_me(3, true).unwrap();
        assert_eq!(luma, 0);
        assert_eq!(chroma, 0);
    }

    #[test]
    fn cbp_inter_code_num_1_gives_0_1() {
        let (luma, chroma) = coded_block_pattern_from_me(1, false).unwrap();
        assert_eq!(luma, 0);
        assert_eq!(chroma, 1);
    }

    #[test]
    fn cbp_inter_code_num_11_gives_15_0() {
        let (luma, chroma) = coded_block_pattern_from_me(11, false).unwrap();
        assert_eq!(luma, 15);
        assert_eq!(chroma, 0);
    }

    #[test]
    fn cbp_code_num_47_is_valid() {
        assert!(coded_block_pattern_from_me(47, true).is_some());
        assert!(coded_block_pattern_from_me(47, false).is_some());
    }

    #[test]
    fn cbp_code_num_48_is_invalid() {
        assert!(coded_block_pattern_from_me(48, true).is_none());
        assert!(coded_block_pattern_from_me(48, false).is_none());
    }

    #[test]
    fn all_i_mb_types_valid() {
        for mb_type in 0..=25 {
            assert!(
                i_mb_type_info(mb_type).is_ok(),
                "mb_type {} should be valid",
                mb_type
            );
        }
    }

    #[test]
    fn all_i16x16_pred_modes_in_range() {
        for mb_type in 1..=24 {
            if let IMbTypeInfo::I16x16 {
                intra16x16_pred_mode,
                coded_block_pattern_chroma,
                ..
            } = i_mb_type_info(mb_type).unwrap()
            {
                assert!(
                    intra16x16_pred_mode <= 3,
                    "pred_mode out of range for mb_type {}",
                    mb_type
                );
                assert!(
                    coded_block_pattern_chroma <= 2,
                    "cbp_chroma out of range for mb_type {}",
                    mb_type
                );
            }
        }
    }
}
