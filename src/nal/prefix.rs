//! Parser for `prefix_nal_unit_rbsp()` (NAL unit type 14, spec 7.3.2.12).
//!
//! A prefix NAL unit carries an MVC/SVC NAL header extension. When the
//! extension is SVC and `nal_ref_idc != 0`, it also carries reference
//! picture base marking information (`prefix_nal_unit_svc()`, spec
//! F.7.3.2.12.1). MVC prefix NALs have an empty RBSP body.

use crate::nal::{parse_nal_header_extension, Nal, NalHeaderError, NalHeaderExtension};
use crate::rbsp::{BitRead, BitReaderError};

/// Parsed `prefix_nal_unit_rbsp()` (NAL unit type 14).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrefixNalUnit {
    pub header_extension: NalHeaderExtension,
    /// Present when `nal_ref_idc != 0` and the extension is SVC.
    pub ref_base_pic: Option<PrefixNalUnitRef>,
}

/// Reference base picture information within an SVC prefix NAL unit
/// (spec F.7.3.2.12.1, `prefix_nal_unit_svc()` when `nal_ref_idc != 0`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrefixNalUnitRef {
    pub store_ref_base_pic_flag: bool,
    pub dec_ref_base_pic_marking: Option<DecRefBasePicMarking>,
    pub additional_prefix_nal_unit_extension_flag: bool,
}

/// Decoded reference base picture marking syntax (spec G.7.3.3.5,
/// `dec_ref_base_pic_marking`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecRefBasePicMarking {
    pub operations: Vec<DecRefBasePicMarkingOp>,
}

/// A single operation in `dec_ref_base_pic_marking()`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecRefBasePicMarkingOp {
    /// `memory_management_base_control_operation == 1`:
    /// Mark a short-term reference base picture as unused.
    ShortTermUnusedForRef {
        difference_of_base_pic_nums_minus1: u32,
    },
    /// `memory_management_base_control_operation == 2`:
    /// Mark a long-term reference base picture as unused.
    LongTermUnusedForRef { long_term_base_pic_num: u32 },
}

#[derive(Debug)]
pub enum PrefixNalUnitError {
    RbspError(BitReaderError),
    IoError(std::io::Error),
    HeaderError(NalHeaderError),
    /// `memory_management_base_control_operation` had an invalid value.
    InvalidMemoryManagementBaseControlOperation(u32),
}
impl From<BitReaderError> for PrefixNalUnitError {
    fn from(e: BitReaderError) -> Self {
        PrefixNalUnitError::RbspError(e)
    }
}
impl From<NalHeaderError> for PrefixNalUnitError {
    fn from(e: NalHeaderError) -> Self {
        PrefixNalUnitError::HeaderError(e)
    }
}
impl From<std::io::Error> for PrefixNalUnitError {
    fn from(e: std::io::Error) -> Self {
        PrefixNalUnitError::IoError(e)
    }
}

impl PrefixNalUnit {
    /// Parse a prefix NAL unit from its raw NAL representation.
    ///
    /// The NAL must have `nal_unit_type == 14`. The 3-byte header extension
    /// is read first, then the RBSP body is parsed if applicable.
    pub fn from_nal<N: Nal>(nal: &N) -> Result<PrefixNalUnit, PrefixNalUnitError> {
        let header = nal.header()?;
        let (header_extension, rbsp_reader) = parse_nal_header_extension(nal)?;
        let mut r = crate::rbsp::BitReader::new(rbsp_reader);

        let ref_base_pic = match &header_extension {
            NalHeaderExtension::Svc(_) => {
                // SVC prefix NAL: parse prefix_nal_unit_svc() per F.7.3.2.12.1
                if header.nal_ref_idc() != 0 {
                    let store_ref_base_pic_flag = r.read_bool("store_ref_base_pic_flag")?;
                    let dec_ref_base_pic_marking = if store_ref_base_pic_flag {
                        Some(read_dec_ref_base_pic_marking(&mut r)?)
                    } else {
                        None
                    };
                    let additional_prefix_nal_unit_extension_flag =
                        r.read_bool("additional_prefix_nal_unit_extension_flag")?;
                    Some(PrefixNalUnitRef {
                        store_ref_base_pic_flag,
                        dec_ref_base_pic_marking,
                        additional_prefix_nal_unit_extension_flag,
                    })
                } else {
                    None
                }
            }
            NalHeaderExtension::Mvc(_) => {
                // MVC prefix NAL: RBSP body is empty per 7.3.2.12
                None
            }
        };

        Ok(PrefixNalUnit {
            header_extension,
            ref_base_pic,
        })
    }
}

/// Read `dec_ref_base_pic_marking()` syntax (spec G.7.3.3.5).
fn read_dec_ref_base_pic_marking<R: BitRead>(
    r: &mut R,
) -> Result<DecRefBasePicMarking, PrefixNalUnitError> {
    let mut operations = Vec::new();
    loop {
        let op = r.read_ue("memory_management_base_control_operation")?;
        match op {
            0 => break,
            1 => {
                let difference_of_base_pic_nums_minus1 =
                    r.read_ue("difference_of_base_pic_nums_minus1")?;
                operations.push(DecRefBasePicMarkingOp::ShortTermUnusedForRef {
                    difference_of_base_pic_nums_minus1,
                });
            }
            2 => {
                let long_term_base_pic_num = r.read_ue("long_term_base_pic_num")?;
                operations.push(DecRefBasePicMarkingOp::LongTermUnusedForRef {
                    long_term_base_pic_num,
                });
            }
            _ => {
                return Err(PrefixNalUnitError::InvalidMemoryManagementBaseControlOperation(op));
            }
        }
    }
    Ok(DecRefBasePicMarking { operations })
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::nal::RefNal;

    #[test]
    fn parse_prefix_nal_mvc_no_ref() {
        // NAL header: nal_ref_idc=0, nal_unit_type=14
        // Header byte: 0b0_00_01110 = 0x0E
        // MVC extension: svc=0, non_idr=1, priority_id=0, view_id=1, temporal_id=0,
        //   anchor=0, inter_view=1, reserved=1
        // No RBSP body since nal_ref_idc=0
        let data: &[u8] = &[
            0x0E,           // NAL header: ref_idc=0, type=14
            0b0100_0000,   // svc=0, non_idr=1, priority_id=0
            0x00,           // view_id high 8 = 0
            0b0100_0011, // view_id low 2 = 01 (view_id=1), temporal=0, anchor=0, inter_view=1, reserved=1
        ];
        let nal = RefNal::new(data, &[], true);
        let prefix = PrefixNalUnit::from_nal(&nal).unwrap();
        match prefix.header_extension {
            NalHeaderExtension::Mvc(mvc) => {
                assert!(mvc.non_idr_flag());
                assert_eq!(mvc.view_id(), 1);
                assert!(mvc.inter_view_flag());
            }
            _ => panic!("expected MVC extension"),
        }
        assert!(prefix.ref_base_pic.is_none());
    }

    #[test]
    fn parse_prefix_nal_mvc_with_ref() {
        // NAL header: nal_ref_idc=3, nal_unit_type=14
        // Header byte: 0b0_11_01110 = 0x6E
        // MVC extension: svc=0, all zeros, view_id=0, reserved=1
        // MVC prefix NALs have no RBSP body, so ref_base_pic should be None.
        let data: &[u8] = &[
            0x6E,           // NAL header: ref_idc=3, type=14
            0x00,           // svc=0, non_idr=0, priority_id=0
            0x00,           // view_id high 8 = 0
            0b0000_0001, // view_id=0, temporal=0, anchor=0, inter_view=0, reserved=1
        ];
        let nal = RefNal::new(data, &[], true);
        let prefix = PrefixNalUnit::from_nal(&nal).unwrap();
        assert!(prefix.ref_base_pic.is_none());
    }

    #[test]
    fn parse_prefix_nal_svc_no_ref() {
        // NAL header: nal_ref_idc=0, nal_unit_type=14 → 0b0_00_01110 = 0x0E
        // SVC extension: svc=1, idr=0, priority_id=0
        //   no_inter_layer_pred=0, dependency_id=0, quality_id=0
        //   temporal_id=0, use_ref_base=0, discardable=0, output=0, reserved=0b11
        let data: &[u8] = &[
            0x0E, // NAL header: ref_idc=0
            0x80, // svc=1, idr=0, priority_id=0
            0x00, // no_inter_layer=0, dep_id=0, quality_id=0
            0x03, // temporal=0, use_ref=0, discard=0, output=0, reserved=0b11
        ];
        let nal = RefNal::new(data, &[], true);
        let prefix = PrefixNalUnit::from_nal(&nal).unwrap();
        match prefix.header_extension {
            NalHeaderExtension::Svc(svc) => {
                assert!(!svc.idr_flag());
                assert_eq!(svc.priority_id(), 0);
            }
            _ => panic!("expected SVC extension"),
        }
        // nal_ref_idc=0, so ref_base_pic is None
        assert!(prefix.ref_base_pic.is_none());
    }

    #[test]
    fn parse_prefix_nal_svc_with_ref_no_marking() {
        // NAL header: nal_ref_idc=3, nal_unit_type=14
        // Header byte: 0b0_11_01110 = 0x6E
        // SVC extension: svc=1, idr=0, priority_id=0
        // RBSP: store_ref_base_pic_flag=0, additional_prefix_nal_unit_extension_flag=0
        //       then rbsp_trailing_bits (1 + padding)
        let data: &[u8] = &[
            0x6E, // NAL header: ref_idc=3, type=14
            0x80, // svc=1, idr=0, priority_id=0
            0x00, // no_inter_layer=0, dep_id=0, quality_id=0
            0x03, // temporal=0, use_ref=0, discard=0, output=0, reserved=0b11
            // RBSP body: store_ref_base_pic_flag=0, additional=0, rbsp_trailing=1, padding=00000
            0b0010_0000,
        ];
        let nal = RefNal::new(data, &[], true);
        let prefix = PrefixNalUnit::from_nal(&nal).unwrap();
        let ref_base = prefix.ref_base_pic.unwrap();
        assert!(!ref_base.store_ref_base_pic_flag);
        assert!(ref_base.dec_ref_base_pic_marking.is_none());
        assert!(!ref_base.additional_prefix_nal_unit_extension_flag);
    }

    #[test]
    fn parse_prefix_nal_svc_with_marking() {
        // NAL header: nal_ref_idc=2, nal_unit_type=14 → 0b0_10_01110 = 0x4E
        // SVC extension: svc=1, idr=0, priority_id=0
        // RBSP body:
        //   store_ref_base_pic_flag=1
        //   dec_ref_base_pic_marking:
        //     op=1 (ue: 010), difference_of_base_pic_nums_minus1=0 (ue: 1)
        //     op=0 (ue: 1) -- end
        //   additional_prefix_nal_unit_extension_flag=0
        //   rbsp_trailing_bits: 1 + padding
        //
        // Bits: 1  010 1  1  0  1 00000
        //       ^  ^^^ ^  ^  ^  ^ ^^^^^
        //       |  |   |  |  |  | padding
        //       |  |   |  |  |  rbsp stop bit
        //       |  |   |  |  additional=0
        //       |  |   |  op=0 (end)
        //       |  |   diff_minus1=0
        //       |  op=1
        //       store=1
        let data: &[u8] = &[
            0x4E, // NAL header
            0x80, // svc=1, idr=0, priority_id=0
            0x00, // no_inter_layer=0, dep_id=0, quality_id=0
            0x03, // temporal=0, use_ref=0, discard=0, output=0, reserved=0b11
            // RBSP: 1_010_1_1_0_1_00000 = 0b1010_1101 0b0000_0xxx
            0b1010_1101,
            0b0000_0000, // trailing bits with padding
        ];
        let nal = RefNal::new(data, &[], true);
        let prefix = PrefixNalUnit::from_nal(&nal).unwrap();
        let ref_base = prefix.ref_base_pic.unwrap();
        assert!(ref_base.store_ref_base_pic_flag);
        assert!(!ref_base.additional_prefix_nal_unit_extension_flag);
        let marking = ref_base.dec_ref_base_pic_marking.unwrap();
        assert_eq!(marking.operations.len(), 1);
        match &marking.operations[0] {
            DecRefBasePicMarkingOp::ShortTermUnusedForRef {
                difference_of_base_pic_nums_minus1,
            } => {
                assert_eq!(*difference_of_base_pic_nums_minus1, 0);
            }
            _ => panic!("expected ShortTermUnusedForRef"),
        }
    }
}
