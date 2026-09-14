//! Slice data access (currently baseline profile - CAVLC only, no MBAFF).
//!
//! Implements the `slice_data()` syntax from spec section 7.3.4.
//!
//! 1. Parse context-free parts of `macroblock_layer()` — `mb_type`, `mb_pred`/`sub_mb_pred`,
//!    `coded_block_pattern`, `mb_qp_delta`.
//! 2. The caller takes over the `BitRead` stream to parse `residual()` using CAVLC with
//!    decoder-computed `nC` values.

pub use crate::nal::slice::cavlc::{CavlcContext, CavlcError};
use crate::nal::slice::macroblock::{
    coded_block_pattern_from_me, i_mb_type_info, p_mb_type_info, p_sub_mb_type_info, IMbTypeInfo,
    MbPartPredMode, MbTypeError, PMbTypeInfo, SubMbTypeInfo,
};
use crate::nal::slice::{cavlc, SliceFamily};
use crate::rbsp::{BitRead, BitReaderError};
use std::convert::TryFrom;
use std::fmt;

#[derive(Debug)]
pub enum SliceDataError {
    BitReaderError(BitReaderError),
    InvalidMbType(MbTypeError),
    InvalidSubMbType(MbTypeError),
    InvalidCodedBlockPattern(u32),
    InvalidParameter { field: &'static str, value: i64 },
}

impl fmt::Display for SliceDataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SliceDataError::BitReaderError(e) => write!(f, "bitstream I/O: {e:?}"),
            SliceDataError::InvalidMbType(e) => write!(f, "{e}"),
            SliceDataError::InvalidSubMbType(e) => write!(f, "{e}"),
            SliceDataError::InvalidCodedBlockPattern(v) => {
                write!(f, "invalid coded_block_pattern code_num {v}")
            }
            SliceDataError::InvalidParameter { field, value } => {
                write!(f, "{field} out of range: {value}")
            }
        }
    }
}

impl std::error::Error for SliceDataError {}

impl From<BitReaderError> for SliceDataError {
    fn from(e: BitReaderError) -> Self {
        SliceDataError::BitReaderError(e)
    }
}

impl From<MbTypeError> for SliceDataError {
    fn from(e: MbTypeError) -> Self {
        match e {
            MbTypeError::InvalidPSubMbType(_) => SliceDataError::InvalidSubMbType(e),
            _ => SliceDataError::InvalidMbType(e),
        }
    }
}

#[derive(Debug)]
pub struct InterPrediction {
    pub num_parts: u8,
    pub part_width: u8,
    pub part_height: u8,
    pub ref_idx_l0: [i8; 4],
    pub mvd_l0: [[[i16; 2]; 4]; 4],
    pub sub_mb_info: Option<SubMbInfo>,
}

#[derive(Debug)]
pub struct SubMbInfo {
    pub sub_mb_type: [u8; 4],
    pub info: [SubMbTypeInfo; 4],
    pub ref_idx_l0: [i8; 4],
}

#[derive(Debug)]
pub enum MbPrediction {
    Intra4x4 {
        prev_intra4x4_pred_mode_flag: [bool; 16],
        rem_intra4x4_pred_mode: [u8; 16],
        intra_chroma_pred_mode: u8,
    },
    Intra16x16 {
        intra_chroma_pred_mode: u8,
    },
    IntraPCM {
        pcm_sample_luma: [u8; 256],
        pcm_sample_chroma: [u8; 128],
    },
    Inter(InterPrediction),
}

#[derive(Debug)]
pub enum MbTypeInfo {
    I(IMbTypeInfo),
    P(PMbTypeInfo),
}

#[derive(Debug)]
pub struct MacroblockHeader {
    pub mb_addr: u32,
    pub mb_type: MbTypeInfo,
    pub coded_block_pattern_luma: u8,
    pub coded_block_pattern_chroma: u8,
    pub mb_qp_delta: i32,
    pub mb_pred: MbPrediction,
}

pub enum SliceEvent<'a, R: BitRead> {
    End,
    Skip {
        mb_addr: u32,
        next: SliceDataReader<'a, R>,
    },
    Macroblock(MacroblockResidual<'a, R>),
}

struct SliceDataCore<'a, R: BitRead> {
    reader: &'a mut R,
    slice_family: SliceFamily,
    curr_mb_addr: u32,
    pic_size_in_mbs: u32,
    num_ref_idx_l0_active_minus1: u32,
    pending_skips: u32,
    after_skip_run: bool,
    done: bool,
}

impl<R: BitRead> SliceDataCore<'_, R> {
    fn next_mb_address(&self, addr: u32) -> u32 {
        addr + 1
    }

    fn parse_macroblock_header(
        &mut self,
        mb_addr: u32,
    ) -> Result<MacroblockHeader, SliceDataError> {
        let raw_mb_type = self.reader.read_ue("mb_type")?;
        log::trace!(
            "MB {}: raw_mb_type={}, slice_family={:?}",
            mb_addr,
            raw_mb_type,
            self.slice_family
        );

        match self.slice_family {
            SliceFamily::I | SliceFamily::SI => {
                let info = i_mb_type_info(raw_mb_type)?;
                self.parse_i_macroblock(mb_addr, info)
            }
            SliceFamily::P | SliceFamily::SP => {
                let info = p_mb_type_info(raw_mb_type)?;
                self.parse_p_macroblock(mb_addr, info)
            }
            SliceFamily::B => {
                // B slices not supported yet (baseline profile only)
                Err(SliceDataError::InvalidParameter {
                    field: "slice_type",
                    value: 1,
                })
            }
        }
    }

    fn parse_i_macroblock(
        &mut self,
        mb_addr: u32,
        info: IMbTypeInfo,
    ) -> Result<MacroblockHeader, SliceDataError> {
        let (mb_pred, cbp_luma, cbp_chroma) = self.parse_i_mb_body(&info)?;

        let mb_qp_delta =
            if cbp_luma > 0 || cbp_chroma > 0 || matches!(info, IMbTypeInfo::I16x16 { .. }) {
                self.read_mb_qp_delta()?
            } else {
                0
            };

        Ok(MacroblockHeader {
            mb_addr,
            mb_type: MbTypeInfo::I(info),
            coded_block_pattern_luma: cbp_luma,
            coded_block_pattern_chroma: cbp_chroma,
            mb_qp_delta,
            mb_pred,
        })
    }

    fn parse_p_macroblock(
        &mut self,
        mb_addr: u32,
        info: PMbTypeInfo,
    ) -> Result<MacroblockHeader, SliceDataError> {
        match info {
            PMbTypeInfo::I(ref i_info) => {
                let (mb_pred, cbp_luma, cbp_chroma) = self.parse_i_mb_body(i_info)?;

                let mb_qp_delta = if cbp_luma > 0
                    || cbp_chroma > 0
                    || matches!(i_info, IMbTypeInfo::I16x16 { .. })
                {
                    self.read_mb_qp_delta()?
                } else {
                    0
                };

                Ok(MacroblockHeader {
                    mb_addr,
                    mb_type: MbTypeInfo::P(info),
                    coded_block_pattern_luma: cbp_luma,
                    coded_block_pattern_chroma: cbp_chroma,
                    mb_qp_delta,
                    mb_pred,
                })
            }
            PMbTypeInfo::P {
                num_parts,
                part_width,
                part_height,
                ref_idx_forced_zero,
                ..
            } => {
                let is_8x8 = num_parts == 4;

                let mb_pred = if is_8x8 {
                    self.parse_sub_mb_pred(ref_idx_forced_zero)?
                } else {
                    self.parse_inter_mb_pred(num_parts, part_width, part_height)?
                };

                // coded_block_pattern is read as ue(v), then mapped.
                let code_num = self.reader.read_ue("coded_block_pattern")?;
                let (cbp_luma, cbp_chroma) = coded_block_pattern_from_me(code_num, false)
                    .ok_or(SliceDataError::InvalidCodedBlockPattern(code_num))?;

                let mb_qp_delta = if cbp_luma > 0 || cbp_chroma > 0 {
                    self.read_mb_qp_delta()?
                } else {
                    0
                };

                Ok(MacroblockHeader {
                    mb_addr,
                    mb_type: MbTypeInfo::P(info),
                    coded_block_pattern_luma: cbp_luma,
                    coded_block_pattern_chroma: cbp_chroma,
                    mb_qp_delta,
                    mb_pred,
                })
            }
        }
    }

    fn parse_i_mb_body(
        &mut self,
        info: &IMbTypeInfo,
    ) -> Result<(MbPrediction, u8, u8), SliceDataError> {
        match info {
            IMbTypeInfo::INxN => {
                let mut prev_flags = [false; 16];
                let mut rem_modes = [0u8; 16];
                for i in 0..16 {
                    prev_flags[i] = self.reader.read_bit("prev_intra4x4_pred_mode_flag")?;
                    if !prev_flags[i] {
                        rem_modes[i] = self.reader.read::<3, u8>("rem_intra4x4_pred_mode")?;
                    }
                }
                let intra_chroma_pred_mode_val = self.reader.read_ue("intra_chroma_pred_mode")?;
                if intra_chroma_pred_mode_val > 3 {
                    return Err(SliceDataError::InvalidParameter {
                        field: "intra_chroma_pred_mode",
                        value: intra_chroma_pred_mode_val as i64,
                    });
                }
                let intra_chroma_pred_mode = intra_chroma_pred_mode_val as u8;

                let code_num = self.reader.read_ue("coded_block_pattern")?;
                let (cbp_luma, cbp_chroma) = coded_block_pattern_from_me(code_num, true)
                    .ok_or(SliceDataError::InvalidCodedBlockPattern(code_num))?;

                Ok((
                    MbPrediction::Intra4x4 {
                        prev_intra4x4_pred_mode_flag: prev_flags,
                        rem_intra4x4_pred_mode: rem_modes,
                        intra_chroma_pred_mode,
                    },
                    cbp_luma,
                    cbp_chroma,
                ))
            }
            IMbTypeInfo::I16x16 {
                coded_block_pattern_luma,
                coded_block_pattern_chroma,
                ..
            } => {
                let intra_chroma_pred_mode_val = self.reader.read_ue("intra_chroma_pred_mode")?;
                if intra_chroma_pred_mode_val > 3 {
                    return Err(SliceDataError::InvalidParameter {
                        field: "intra_chroma_pred_mode",
                        value: intra_chroma_pred_mode_val as i64,
                    });
                }
                let intra_chroma_pred_mode = intra_chroma_pred_mode_val as u8;

                Ok((
                    MbPrediction::Intra16x16 {
                        intra_chroma_pred_mode,
                    },
                    *coded_block_pattern_luma,
                    *coded_block_pattern_chroma,
                ))
            }
            IMbTypeInfo::IPCM => {
                while !self.reader.byte_aligned() {
                    if self.reader.read_bit("pcm_alignment_zero_bit")? {
                        return Err(SliceDataError::InvalidParameter {
                            field: "pcm_alignment_zero_bit",
                            value: 1,
                        });
                    }
                }

                let mut pcm_sample_luma = [0u8; 256];
                for sample in &mut pcm_sample_luma {
                    *sample = self.reader.read::<8, u8>("pcm_sample_luma")?;
                }
                let mut pcm_sample_chroma = [0u8; 128];
                for sample in &mut pcm_sample_chroma {
                    *sample = self.reader.read::<8, u8>("pcm_sample_chroma")?;
                }

                Ok((
                    MbPrediction::IntraPCM {
                        pcm_sample_luma,
                        pcm_sample_chroma,
                    },
                    0,
                    0,
                ))
            }
        }
    }

    fn parse_inter_mb_pred(
        &mut self,
        num_parts: u8,
        part_width: u8,
        part_height: u8,
    ) -> Result<MbPrediction, SliceDataError> {
        let mut ref_idx_l0 = [0i8; 4];
        let mut mvd_l0 = [[[0i16; 2]; 4]; 4];

        for item in ref_idx_l0.iter_mut().take(num_parts as usize) {
            *item = self.read_te("ref_idx_l0", self.num_ref_idx_l0_active_minus1)?;
        }

        for item in mvd_l0.iter_mut().take(num_parts as usize) {
            let x = self.reader.read_se("mvd_l0_x")?;
            item[0][0] = i16::try_from(x).map_err(|_| SliceDataError::InvalidParameter {
                field: "mvd_l0_x",
                value: x as i64,
            })?;
            let y = self.reader.read_se("mvd_l0_y")?;
            item[0][1] = i16::try_from(y).map_err(|_| SliceDataError::InvalidParameter {
                field: "mvd_l0_y",
                value: y as i64,
            })?;
        }

        Ok(MbPrediction::Inter(InterPrediction {
            num_parts,
            part_width,
            part_height,
            ref_idx_l0,
            mvd_l0,
            sub_mb_info: None,
        }))
    }

    fn parse_sub_mb_pred(
        &mut self,
        ref_idx_forced_zero: bool,
    ) -> Result<MbPrediction, SliceDataError> {
        let mut sub_mb_type_raw = [0u8; 4];
        let mut sub_info: [SubMbTypeInfo; 4] = [
            SubMbTypeInfo {
                num_sub_parts: 1,
                sub_part_width: 8,
                sub_part_height: 8,
                pred_mode: MbPartPredMode::PredL0,
            },
            SubMbTypeInfo {
                num_sub_parts: 1,
                sub_part_width: 8,
                sub_part_height: 8,
                pred_mode: MbPartPredMode::PredL0,
            },
            SubMbTypeInfo {
                num_sub_parts: 1,
                sub_part_width: 8,
                sub_part_height: 8,
                pred_mode: MbPartPredMode::PredL0,
            },
            SubMbTypeInfo {
                num_sub_parts: 1,
                sub_part_width: 8,
                sub_part_height: 8,
                pred_mode: MbPartPredMode::PredL0,
            },
        ];
        for i in 0..4 {
            let raw = self.reader.read_ue("sub_mb_type")?;
            sub_mb_type_raw[i] = raw as u8;
            sub_info[i] = p_sub_mb_type_info(raw)?;
        }

        let mut sub_ref_idx_l0 = [0i8; 4];
        if !ref_idx_forced_zero {
            for item in &mut sub_ref_idx_l0 {
                *item = self.read_te("ref_idx_l0", self.num_ref_idx_l0_active_minus1)?;
            }
        }

        let mut mvd_l0 = [[[0i16; 2]; 4]; 4];
        for mb_part in 0..4 {
            for sub_part in 0..sub_info[mb_part].num_sub_parts as usize {
                let x = self.reader.read_se("mvd_l0_x")?;
                mvd_l0[mb_part][sub_part][0] =
                    i16::try_from(x).map_err(|_| SliceDataError::InvalidParameter {
                        field: "mvd_l0_x",
                        value: x as i64,
                    })?;
                let y = self.reader.read_se("mvd_l0_y")?;
                mvd_l0[mb_part][sub_part][1] =
                    i16::try_from(y).map_err(|_| SliceDataError::InvalidParameter {
                        field: "mvd_l0_y",
                        value: y as i64,
                    })?;
            }
        }

        Ok(MbPrediction::Inter(InterPrediction {
            num_parts: 4,
            part_width: 8,
            part_height: 8,
            ref_idx_l0: [0; 4],
            mvd_l0,
            sub_mb_info: Some(SubMbInfo {
                sub_mb_type: sub_mb_type_raw,
                info: sub_info,
                ref_idx_l0: sub_ref_idx_l0,
            }),
        }))
    }

    fn read_mb_qp_delta(&mut self) -> Result<i32, SliceDataError> {
        let val = self.reader.read_se("mb_qp_delta")?;
        if !(-26..=25).contains(&val) {
            return Err(SliceDataError::InvalidParameter {
                field: "mb_qp_delta",
                value: val as i64,
            });
        }
        Ok(val)
    }

    /// Read a `te(v)` (truncated Exp-Golomb) value.
    ///
    /// - If `max == 0`: the value is always 0 (nothing is read).
    /// - If `max == 1`: read 1 bit and invert (0 -> 1, 1 -> 0).
    /// - If `max > 1`: read as `ue(v)`.
    fn read_te(&mut self, name: &'static str, max: u32) -> Result<i8, SliceDataError> {
        if max == 0 {
            Ok(0)
        } else if max == 1 {
            let bit = self.reader.read_bit(name)?;
            Ok(if bit { 0 } else { 1 })
        } else {
            let val = self.reader.read_ue(name)?;
            if val > max {
                return Err(SliceDataError::InvalidParameter {
                    field: name,
                    value: val as i64,
                });
            }
            Ok(val as i8)
        }
    }
}

pub struct SliceDataReader<'a, R: BitRead> {
    core: SliceDataCore<'a, R>,
}

impl<'a, R: BitRead> SliceDataReader<'a, R> {
    pub fn new(
        reader: &'a mut R,
        slice_family: SliceFamily,
        first_mb_in_slice: u32,
        pic_size_in_mbs: u32,
        num_ref_idx_l0_active_minus1: u32,
    ) -> Self {
        SliceDataReader {
            core: SliceDataCore {
                reader,
                slice_family,
                curr_mb_addr: first_mb_in_slice,
                pic_size_in_mbs,
                num_ref_idx_l0_active_minus1,
                pending_skips: 0,
                after_skip_run: false,
                done: false,
            },
        }
    }

    pub fn next(mut self) -> Result<SliceEvent<'a, R>, SliceDataError> {
        if self.core.done {
            return Ok(SliceEvent::End);
        }

        if self.core.curr_mb_addr >= self.core.pic_size_in_mbs {
            self.core.done = true;
            return Ok(SliceEvent::End);
        }

        if self.core.pending_skips > 0 {
            self.core.pending_skips -= 1;
            let addr = self.core.curr_mb_addr;
            self.core.curr_mb_addr = self.core.next_mb_address(addr);
            if self.core.pending_skips == 0 {
                if !self.core.reader.has_more_rbsp_data("slice_data")? {
                    self.core.done = true;
                } else {
                    self.core.after_skip_run = true;
                }
            }
            return Ok(SliceEvent::Skip {
                mb_addr: addr,
                next: self,
            });
        }

        if self.core.after_skip_run {
            self.core.after_skip_run = false;
        } else if self.core.slice_family != SliceFamily::I
            && self.core.slice_family != SliceFamily::SI
        {
            let mb_skip_run = self.core.reader.read_ue("mb_skip_run")?;
            if mb_skip_run > 0 {
                self.core.pending_skips = mb_skip_run;
                self.core.pending_skips -= 1;
                let addr = self.core.curr_mb_addr;
                self.core.curr_mb_addr = self.core.next_mb_address(addr);
                if self.core.pending_skips == 0 {
                    if !self.core.reader.has_more_rbsp_data("slice_data")? {
                        self.core.done = true;
                    } else {
                        self.core.after_skip_run = true;
                    }
                }
                return Ok(SliceEvent::Skip {
                    mb_addr: addr,
                    next: self,
                });
            }
            if !self.core.reader.has_more_rbsp_data("slice_data")? {
                self.core.done = true;
                return Ok(SliceEvent::End);
            }
        }

        let mb_addr = self.core.curr_mb_addr;
        let header = self.core.parse_macroblock_header(mb_addr)?;
        self.core.curr_mb_addr = self.core.next_mb_address(mb_addr);

        Ok(SliceEvent::Macroblock(MacroblockResidual {
            core: self.core,
            header,
        }))
    }

    pub fn curr_mb_addr(&self) -> u32 {
        self.core.curr_mb_addr
    }

    pub fn pic_size_in_mbs(&self) -> u32 {
        self.core.pic_size_in_mbs
    }
}

pub struct MacroblockResidual<'a, R: BitRead> {
    core: SliceDataCore<'a, R>,
    header: MacroblockHeader,
}

impl<'a, R: BitRead> MacroblockResidual<'a, R> {
    pub fn header(&self) -> &MacroblockHeader {
        &self.header
    }

    pub fn residual_block_cavlc(
        &mut self,
        coeff_level: &mut [i32],
        start_idx: usize,
        end_idx: usize,
        max_num_coeff: usize,
        nc: CavlcContext,
    ) -> Result<u8, CavlcError> {
        cavlc::residual_block_cavlc(
            self.core.reader,
            coeff_level,
            start_idx,
            end_idx,
            max_num_coeff,
            nc,
        )
    }

    pub fn curr_mb_addr(&self) -> u32 {
        self.core.curr_mb_addr
    }

    pub fn finish(mut self) -> Result<SliceDataReader<'a, R>, SliceDataError> {
        match self.core.reader.has_more_rbsp_data("slice_data") {
            Ok(false) => {
                self.core.done = true;
            }
            Ok(true) => {}
            Err(_) => {
                self.core.done = true;
            }
        }
        Ok(SliceDataReader { core: self.core })
    }
}
