//! Types for reading H264 _Network Abstraction Layer_ Units (NAL Units).
//!
//! The data presented must already be in _RBSP_ form (i.e. have been passed through
//! [`RbspDecoder`](../rbsp/struct.RbspDecoder.html)), where it has been encoded with
//! 'emulation prevention bytes'.

pub mod aud;
pub mod pps;
pub mod prefix;
pub mod sei;
pub mod slice;
pub mod sps;
pub mod sps_extension;
pub mod subset_sps;

use crate::rbsp;
use hex_slice::AsHex;
use std::fmt;
use std::io::Read;
use std::num::NonZeroUsize;

#[derive(PartialEq, Hash, Debug, Copy, Clone)]
pub enum UnitType {
    /// The values `0` and `24`-`31` are unspecified in the H264 spec
    Unspecified(u8),
    SliceLayerWithoutPartitioningNonIdr,
    SliceDataPartitionALayer,
    SliceDataPartitionBLayer,
    SliceDataPartitionCLayer,
    SliceLayerWithoutPartitioningIdr,
    /// Supplemental enhancement information
    SEI,
    SeqParameterSet,
    PicParameterSet,
    AccessUnitDelimiter,
    EndOfSeq,
    EndOfStream,
    FillerData,
    SeqParameterSetExtension,
    PrefixNALUnit,
    SubsetSeqParameterSet,
    DepthParameterSet,
    SliceLayerWithoutPartitioningAux,
    SliceExtension,
    SliceExtensionViewComponent,
    /// The values `17`, `18`, `22` and `23` are reserved for future use by the H264 spec
    Reserved(u8),
}
impl UnitType {
    pub fn for_id(id: u8) -> Result<UnitType, UnitTypeError> {
        if id > 31 {
            Err(UnitTypeError::ValueOutOfRange(id))
        } else {
            let t = match id {
                0 => UnitType::Unspecified(0),
                1 => UnitType::SliceLayerWithoutPartitioningNonIdr,
                2 => UnitType::SliceDataPartitionALayer,
                3 => UnitType::SliceDataPartitionBLayer,
                4 => UnitType::SliceDataPartitionCLayer,
                5 => UnitType::SliceLayerWithoutPartitioningIdr,
                6 => UnitType::SEI,
                7 => UnitType::SeqParameterSet,
                8 => UnitType::PicParameterSet,
                9 => UnitType::AccessUnitDelimiter,
                10 => UnitType::EndOfSeq,
                11 => UnitType::EndOfStream,
                12 => UnitType::FillerData,
                13 => UnitType::SeqParameterSetExtension,
                14 => UnitType::PrefixNALUnit,
                15 => UnitType::SubsetSeqParameterSet,
                16 => UnitType::DepthParameterSet,
                17..=18 => UnitType::Reserved(id),
                19 => UnitType::SliceLayerWithoutPartitioningAux,
                20 => UnitType::SliceExtension,
                21 => UnitType::SliceExtensionViewComponent,
                22..=23 => UnitType::Reserved(id),
                24..=31 => UnitType::Unspecified(id),
                _ => panic!("unexpected {}", id), // shouldn't happen
            };
            Ok(t)
        }
    }

    pub fn id(self) -> u8 {
        match self {
            UnitType::Unspecified(v) => v,
            UnitType::SliceLayerWithoutPartitioningNonIdr => 1,
            UnitType::SliceDataPartitionALayer => 2,
            UnitType::SliceDataPartitionBLayer => 3,
            UnitType::SliceDataPartitionCLayer => 4,
            UnitType::SliceLayerWithoutPartitioningIdr => 5,
            UnitType::SEI => 6,
            UnitType::SeqParameterSet => 7,
            UnitType::PicParameterSet => 8,
            UnitType::AccessUnitDelimiter => 9,
            UnitType::EndOfSeq => 10,
            UnitType::EndOfStream => 11,
            UnitType::FillerData => 12,
            UnitType::SeqParameterSetExtension => 13,
            UnitType::PrefixNALUnit => 14,
            UnitType::SubsetSeqParameterSet => 15,
            UnitType::DepthParameterSet => 16,
            UnitType::SliceLayerWithoutPartitioningAux => 19,
            UnitType::SliceExtension => 20,
            UnitType::SliceExtensionViewComponent => 21,
            UnitType::Reserved(v) => v,
        }
    }
}

#[derive(Debug)]
pub enum UnitTypeError {
    /// if the value was outside the range `0` - `31`.
    ValueOutOfRange(u8),
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct NalHeader(u8);

#[derive(Debug)]
pub enum NalHeaderError {
    /// The most significant bit of the header, called `forbidden_zero_bit`, was set to 1.
    ForbiddenZeroBit,
}
impl NalHeader {
    pub fn new(header_value: u8) -> Result<NalHeader, NalHeaderError> {
        if header_value & 0b1000_0000 != 0 {
            Err(NalHeaderError::ForbiddenZeroBit)
        } else {
            Ok(NalHeader(header_value))
        }
    }

    pub fn nal_ref_idc(self) -> u8 {
        (self.0 & 0b0110_0000) >> 5
    }

    pub fn nal_unit_type(self) -> UnitType {
        UnitType::for_id(self.0 & 0b0001_1111).unwrap()
    }
}
impl From<NalHeader> for u8 {
    fn from(v: NalHeader) -> Self {
        v.0
    }
}

impl fmt::Debug for NalHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("NalHeader")
            .field("nal_ref_idc", &self.nal_ref_idc())
            .field("nal_unit_type", &self.nal_unit_type())
            .finish()
    }
}

/// MVC NAL unit header extension (spec G.7.3.1.1).
///
/// Wraps the 3 raw extension bytes with accessor methods, following the same
/// pattern as [`NalHeader`]. All fields are at fixed bit positions:
///
/// ```text
/// Byte 0: svc_extension_flag(1) | non_idr_flag(1) | priority_id(6)
/// Byte 1: view_id[9:2] (high 8 bits of 10-bit view_id)
/// Byte 2: view_id[1:0](2) | temporal_id(3) | anchor_pic_flag(1) | inter_view_flag(1) | reserved_one_bit(1)
/// ```
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct NalHeaderMvcExtension([u8; 3]);

impl NalHeaderMvcExtension {
    pub fn non_idr_flag(&self) -> bool {
        self.0[0] & 0x40 != 0
    }
    pub fn priority_id(&self) -> u8 {
        self.0[0] & 0x3F
    }
    pub fn view_id(&self) -> u16 {
        ((self.0[1] as u16) << 2) | ((self.0[2] as u16) >> 6)
    }
    pub fn temporal_id(&self) -> u8 {
        (self.0[2] >> 3) & 0x07
    }
    pub fn anchor_pic_flag(&self) -> bool {
        self.0[2] & 0x04 != 0
    }
    pub fn inter_view_flag(&self) -> bool {
        self.0[2] & 0x02 != 0
    }
}
impl fmt::Debug for NalHeaderMvcExtension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NalHeaderMvcExtension")
            .field("non_idr_flag", &self.non_idr_flag())
            .field("priority_id", &self.priority_id())
            .field("view_id", &self.view_id())
            .field("temporal_id", &self.temporal_id())
            .field("anchor_pic_flag", &self.anchor_pic_flag())
            .field("inter_view_flag", &self.inter_view_flag())
            .finish()
    }
}

/// SVC NAL unit header extension (spec F.7.3.1.1).
///
/// Wraps the 3 raw extension bytes with accessor methods:
///
/// ```text
/// Byte 0: svc_extension_flag(1) | idr_flag(1) | priority_id(6)
/// Byte 1: no_inter_layer_pred_flag(1) | dependency_id(3) | quality_id(4)
/// Byte 2: temporal_id(3) | use_ref_base_pic_flag(1) | discardable_flag(1) | output_flag(1) | reserved_three_2bits(2)
/// ```
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct NalHeaderSvcExtension([u8; 3]);

impl NalHeaderSvcExtension {
    pub fn idr_flag(&self) -> bool {
        self.0[0] & 0x40 != 0
    }
    pub fn priority_id(&self) -> u8 {
        self.0[0] & 0x3F
    }
    pub fn no_inter_layer_pred_flag(&self) -> bool {
        self.0[1] & 0x80 != 0
    }
    pub fn dependency_id(&self) -> u8 {
        (self.0[1] >> 4) & 0x07
    }
    pub fn quality_id(&self) -> u8 {
        self.0[1] & 0x0F
    }
    pub fn temporal_id(&self) -> u8 {
        (self.0[2] >> 5) & 0x07
    }
    pub fn use_ref_base_pic_flag(&self) -> bool {
        self.0[2] & 0x10 != 0
    }
    pub fn discardable_flag(&self) -> bool {
        self.0[2] & 0x08 != 0
    }
    pub fn output_flag(&self) -> bool {
        self.0[2] & 0x04 != 0
    }
}
impl fmt::Debug for NalHeaderSvcExtension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NalHeaderSvcExtension")
            .field("idr_flag", &self.idr_flag())
            .field("priority_id", &self.priority_id())
            .field("no_inter_layer_pred_flag", &self.no_inter_layer_pred_flag())
            .field("dependency_id", &self.dependency_id())
            .field("quality_id", &self.quality_id())
            .field("temporal_id", &self.temporal_id())
            .field("use_ref_base_pic_flag", &self.use_ref_base_pic_flag())
            .field("discardable_flag", &self.discardable_flag())
            .field("output_flag", &self.output_flag())
            .finish()
    }
}

/// Extended NAL unit header data for `nal_unit_type` 14 and 20 (spec 7.3.1).
///
/// The first bit of the 3-byte extension is `svc_extension_flag`:
/// - `1` → SVC extension ([`NalHeaderSvcExtension`])
/// - `0` → MVC extension ([`NalHeaderMvcExtension`])
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NalHeaderExtension {
    Mvc(NalHeaderMvcExtension),
    Svc(NalHeaderSvcExtension),
}

impl NalHeaderExtension {
    /// Parse a 3-byte NAL header extension. The first bit is `svc_extension_flag`.
    pub fn from_bytes(bytes: [u8; 3]) -> Self {
        if bytes[0] & 0x80 != 0 {
            NalHeaderExtension::Svc(NalHeaderSvcExtension(bytes))
        } else {
            NalHeaderExtension::Mvc(NalHeaderMvcExtension(bytes))
        }
    }
}

/// Read the 3-byte header extension from a NAL with extended header (types 14, 20).
///
/// Returns the parsed extension and a [`rbsp::ByteReader`] positioned after the 4-byte
/// extended header (1-byte NAL header + 3-byte extension), ready for RBSP processing
/// of the NAL body. The extension bytes are not subject to emulation prevention.
pub fn parse_nal_header_extension<N: Nal>(
    nal: &N,
) -> Result<(NalHeaderExtension, rbsp::ByteReader<N::BufRead>), std::io::Error> {
    let mut reader = nal.reader();
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    let ext = NalHeaderExtension::from_bytes([buf[1], buf[2], buf[3]]);
    let rbsp = rbsp::ByteReader::without_skip(reader);
    Ok((ext, rbsp))
}

/// Read the 3-byte header extension and return an RBSP byte reader that skips the
/// full 4-byte extended header. Unlike [`parse_nal_header_extension`], this creates
/// the reader from a fresh `nal.reader()` call, so nothing is consumed.
pub fn extended_rbsp_bytes<N: Nal>(nal: &N) -> rbsp::ByteReader<N::BufRead> {
    // Safety: 4 is non-zero
    let skip = NonZeroUsize::new(4).unwrap();
    rbsp::ByteReader::skipping_bytes(nal.reader(), skip)
}

/// A partially- or completely-buffered encoded NAL.

/// Must have at least one byte (the header). Partially-encoded NALs are *prefixes*
/// of a complete NAL. They can always be parsed from the beginning.
///
///
/// ```
/// use h264_reader::nal::{Nal, RefNal, UnitType};
/// use h264_reader::rbsp::BitRead;
/// use std::io::{ErrorKind, Read};
/// let nal_bytes = &b"\x68\x12\x34\x00\x00\x03\x00\x86"[..];
/// let nal = RefNal::new(nal_bytes, &[], true);
///
/// // Basic inspection:
/// assert!(nal.is_complete());
/// assert_eq!(nal.header().unwrap().nal_unit_type(), UnitType::PicParameterSet);
///
/// // Reading NAL bytes:
/// let mut buf = Vec::new();
/// nal.reader().read_to_end(&mut buf);
/// assert_eq!(buf, nal_bytes);
///
/// // Reading from a partial NAL:
/// let partial_nal = RefNal::new(&nal_bytes[0..2], &[], false);
/// assert!(!partial_nal.is_complete());
/// let mut r = partial_nal.reader();
/// buf.resize(2, 0u8);
/// r.read_exact(&mut buf).unwrap(); // reading buffered bytes works.
/// assert_eq!(&buf[..], &b"\x68\x12"[..]);
/// buf.resize(1, 0u8);
/// let e = r.read_exact(&mut buf).unwrap_err(); // beyond returns WouldBlock.
/// assert_eq!(e.kind(), ErrorKind::WouldBlock);
///
/// // Reading RBSP bytes (no header byte, `03` removed from `00 00 03` sequences):
/// buf.clear();
/// nal.rbsp_bytes().read_to_end(&mut buf);
/// assert_eq!(buf, &b"\x12\x34\x00\x00\x00\x86"[..]);
///
/// // Reading RBSP bytes of invalid NALs:
/// let invalid_nal = RefNal::new(&b"\x68\x12\x34\x00\x00\x00\x86"[..], &[], true);
/// buf.clear();
/// assert_eq!(invalid_nal.rbsp_bytes().read_to_end(&mut buf).unwrap_err().kind(),
///            ErrorKind::InvalidData);
///
/// // Reading RBSP as a bit sequence:
/// let mut r = nal.rbsp_bits();
/// assert_eq!(r.read::<4, u8>("first nibble").unwrap(), 0x1);
/// assert_eq!(r.read::<4, u8>("second nibble").unwrap(), 0x2);
/// assert_eq!(r.read::<23, u32>("23 bits at a time").unwrap(), 0x1a_00_00);
/// assert!(r.has_more_rbsp_data("more left").unwrap());
/// ```
pub trait Nal {
    type BufRead: std::io::BufRead + Clone;

    /// Returns whether the NAL is completely buffered.
    fn is_complete(&self) -> bool;

    /// Returns the NAL header or error if corrupt.
    fn header(&self) -> Result<NalHeader, NalHeaderError>;

    /// Reads the bytes in NAL form (including the header byte and
    /// any emulation-prevention-three-bytes) as a [`std::io::BufRead`].
    /// If the NAL is incomplete, reads may fail with [`std::io::ErrorKind::WouldBlock`].
    fn reader(&self) -> Self::BufRead;

    /// Reads the bytes in RBSP form (skipping header byte and
    /// emulation-prevention-three-bytes).
    #[inline]
    fn rbsp_bytes(&self) -> rbsp::ByteReader<Self::BufRead> {
        rbsp::ByteReader::skipping_h264_header(self.reader())
    }

    /// Reads bits within the RBSP form.
    #[inline]
    fn rbsp_bits(&self) -> rbsp::BitReader<rbsp::ByteReader<Self::BufRead>> {
        rbsp::BitReader::new(self.rbsp_bytes())
    }
}

/// A partially- or completely-buffered [`Nal`] backed by borrowed `&[u8]`s. See [`Nal`] docs.
#[derive(Clone, Eq, PartialEq)]
pub struct RefNal<'a> {
    header: u8,
    complete: bool,

    // Non-empty chunks.
    head: &'a [u8],
    tail: &'a [&'a [u8]],
}
impl<'a> RefNal<'a> {
    /// The caller must ensure that each provided chunk is non-empty.
    #[inline]
    pub fn new(head: &'a [u8], tail: &'a [&'a [u8]], complete: bool) -> Self {
        for buf in tail {
            debug_assert!(!buf.is_empty());
        }
        Self {
            header: *head.first().expect("RefNal must be non-empty"),
            head,
            tail,
            complete,
        }
    }
}
impl<'a> Nal for RefNal<'a> {
    type BufRead = RefNalReader<'a>;

    #[inline]
    fn is_complete(&self) -> bool {
        self.complete
    }

    #[inline]
    fn header(&self) -> Result<NalHeader, NalHeaderError> {
        NalHeader::new(self.header)
    }

    #[inline]
    fn reader(&self) -> Self::BufRead {
        RefNalReader {
            cur: self.head,
            tail: self.tail,
            complete: self.complete,
        }
    }
}
impl<'a> std::fmt::Debug for RefNal<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Interpret the NAL header and display the data as a hex string.
        f.debug_struct("RefNal")
            .field("header", &self.header())
            .field(
                "data",
                &RefNalReader {
                    cur: self.head,
                    tail: self.tail,
                    complete: self.complete,
                },
            )
            .finish()
    }
}

/// A reader through the bytes of a partially- or fully-buffered [`RefNal`]
/// that implements [`std::io::BufRead`].
///
/// Returns [`std::io::ErrorKind::WouldBlock`] on reaching the end of partially-buffered NAL.
/// Construct via [`Nal::reader`].
#[derive(Clone)]
pub struct RefNalReader<'a> {
    /// Empty only if at end.
    cur: &'a [u8],
    tail: &'a [&'a [u8]],
    complete: bool,
}
impl<'a> RefNalReader<'a> {
    fn next_chunk(&mut self) {
        match self.tail {
            [first, tail @ ..] => {
                self.cur = first;
                self.tail = tail;
            }
            _ => self.cur = &[], // EOF.
        }
    }
}
impl<'a> std::io::Read for RefNalReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let len;
        if buf.is_empty() {
            len = 0;
        } else if self.cur.is_empty() && !self.complete {
            return Err(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "reached end of partially-buffered NAL",
            ));
        } else if buf.len() < self.cur.len() {
            len = buf.len();
            let (copy, keep) = self.cur.split_at(len);
            buf.copy_from_slice(copy);
            self.cur = keep;
        } else {
            len = self.cur.len();
            buf[..len].copy_from_slice(self.cur);
            self.next_chunk();
        }
        Ok(len)
    }
}
impl<'a> std::io::BufRead for RefNalReader<'a> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        if self.cur.is_empty() && !self.complete {
            return Err(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "reached end of partially-buffered NAL",
            ));
        }
        Ok(self.cur)
    }
    fn consume(&mut self, amt: usize) {
        self.cur = &self.cur[amt..];
        if self.cur.is_empty() {
            self.next_chunk();
        }
    }
}
impl<'a> std::fmt::Debug for RefNalReader<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02x}", self.cur.plain_hex(true))?;
        for buf in self.tail {
            write!(f, " {:02x}", buf.plain_hex(true))?;
        }
        if !self.complete {
            f.write_str(" ...")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::io::{BufRead, Read};

    use super::*;

    #[test]
    fn header() {
        let h = NalHeader::new(0b0101_0001).unwrap();
        assert_eq!(0b10, h.nal_ref_idc());
        assert_eq!(UnitType::Reserved(17), h.nal_unit_type());
    }

    #[test]
    fn ref_nal() {
        fn common<'a>(head: &'a [u8], tail: &'a [&'a [u8]], complete: bool) -> RefNal<'a> {
            let nal = RefNal::new(head, tail, complete);
            assert_eq!(NalHeader::new(0b0101_0001).unwrap(), nal.header().unwrap());

            // Try the Read impl.
            let mut r = nal.reader();
            let mut buf = [0u8; 5];
            r.read_exact(&mut buf).unwrap();
            assert_eq!(&buf[..], &[0b0101_0001, 1, 2, 3, 4]);
            if complete {
                assert_eq!(r.read(&mut buf[..]).unwrap(), 0);

                // Also try read_to_end.
                let mut buf = Vec::new();
                nal.reader().read_to_end(&mut buf).unwrap();
                assert_eq!(buf, &[0b0101_0001, 1, 2, 3, 4]);
            } else {
                assert_eq!(
                    r.read(&mut buf[..]).unwrap_err().kind(),
                    std::io::ErrorKind::WouldBlock
                );
            }

            // Let the caller try the BufRead impl.
            nal
        }

        // Incomplete NAL with a first chunk only.
        let nal = common(&[0b0101_0001, 1, 2, 3, 4], &[], false);
        let mut r = nal.reader();
        assert_eq!(r.fill_buf().unwrap(), &[0b0101_0001, 1, 2, 3, 4]);
        r.consume(1);
        assert_eq!(r.fill_buf().unwrap(), &[1, 2, 3, 4]);
        r.consume(4);
        assert_eq!(
            r.fill_buf().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );

        // Incomplete NAL with multiple chunks.
        let nal = common(&[0b0101_0001], &[&[1, 2], &[3, 4]], false);
        let mut r = nal.reader();
        assert_eq!(r.fill_buf().unwrap(), &[0b0101_0001]);
        r.consume(1);
        assert_eq!(r.fill_buf().unwrap(), &[1, 2]);
        r.consume(2);
        assert_eq!(r.fill_buf().unwrap(), &[3, 4]);
        r.consume(1);
        assert_eq!(r.fill_buf().unwrap(), &[4]);
        r.consume(1);
        assert_eq!(
            r.fill_buf().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );

        // Complete NAL with first chunk only.
        let nal = common(&[0b0101_0001, 1, 2, 3, 4], &[], true);
        let mut r = nal.reader();
        assert_eq!(r.fill_buf().unwrap(), &[0b0101_0001, 1, 2, 3, 4]);
        r.consume(1);
        assert_eq!(r.fill_buf().unwrap(), &[1, 2, 3, 4]);
        r.consume(4);
        assert!(r.fill_buf().unwrap().is_empty());
    }

    #[test]
    fn mvc_header_extension() {
        // svc_extension_flag=0, non_idr_flag=1, priority_id=0b00_0011 = 3
        // view_id = 0b01_1000_0010 = 386
        // temporal_id = 0b101 = 5
        // anchor_pic_flag=1, inter_view_flag=0, reserved_one_bit=1
        let bytes: [u8; 3] = [
            0b0100_0011, // svc=0, non_idr=1, priority_id=3
            0b01100000,  // view_id high 8 bits = 0x60
            0b1010_1101, // view_id low 2 = 0b10, temporal=5, anchor=1, inter_view=0, reserved=1
        ];
        let ext = NalHeaderExtension::from_bytes(bytes);
        match ext {
            NalHeaderExtension::Mvc(mvc) => {
                assert!(mvc.non_idr_flag());
                assert_eq!(mvc.priority_id(), 3);
                // view_id = (0x60 << 2) | (0b10) = 0x180 | 0x02 = 386
                assert_eq!(mvc.view_id(), 386);
                assert_eq!(mvc.temporal_id(), 5);
                assert!(mvc.anchor_pic_flag());
                assert!(!mvc.inter_view_flag());
            }
            _ => panic!("expected MVC extension"),
        }
    }

    #[test]
    fn mvc_header_extension_view_id_zero() {
        // Minimal: all zeros except svc_extension_flag=0, reserved_one_bit=1
        let bytes: [u8; 3] = [0x00, 0x00, 0x01];
        let ext = NalHeaderExtension::from_bytes(bytes);
        match ext {
            NalHeaderExtension::Mvc(mvc) => {
                assert!(!mvc.non_idr_flag());
                assert_eq!(mvc.priority_id(), 0);
                assert_eq!(mvc.view_id(), 0);
                assert_eq!(mvc.temporal_id(), 0);
                assert!(!mvc.anchor_pic_flag());
                assert!(!mvc.inter_view_flag());
            }
            _ => panic!("expected MVC extension"),
        }
    }

    #[test]
    fn mvc_header_extension_max_view_id() {
        // view_id = 1023 (max 10-bit value) = 0b11_1111_1111
        // byte1 = 0xFF (high 8 bits), byte2 high 2 bits = 0b11
        let bytes: [u8; 3] = [0x00, 0xFF, 0b1100_0001];
        let ext = NalHeaderExtension::from_bytes(bytes);
        match ext {
            NalHeaderExtension::Mvc(mvc) => {
                assert_eq!(mvc.view_id(), 1023);
            }
            _ => panic!("expected MVC extension"),
        }
    }

    #[test]
    fn svc_header_extension() {
        // svc_extension_flag=1, idr_flag=0, priority_id=0b10_1010 = 42
        // no_inter_layer_pred_flag=1, dependency_id=0b110 = 6, quality_id=0b0011 = 3
        // temporal_id=0b010 = 2, use_ref_base_pic_flag=1, discardable_flag=0, output_flag=1
        // reserved_three_2bits=0b11
        let bytes: [u8; 3] = [
            0b1010_1010, // svc=1, idr=0, priority_id=42
            0b1110_0011, // no_inter_layer=1, dep_id=6, quality_id=3
            0b0101_0111, // temporal=2, use_ref=1, discard=0, output=1, reserved=3
        ];
        let ext = NalHeaderExtension::from_bytes(bytes);
        match ext {
            NalHeaderExtension::Svc(svc) => {
                assert!(!svc.idr_flag());
                assert_eq!(svc.priority_id(), 42);
                assert!(svc.no_inter_layer_pred_flag());
                assert_eq!(svc.dependency_id(), 6);
                assert_eq!(svc.quality_id(), 3);
                assert_eq!(svc.temporal_id(), 2);
                assert!(svc.use_ref_base_pic_flag());
                assert!(!svc.discardable_flag());
                assert!(svc.output_flag());
            }
            _ => panic!("expected SVC extension"),
        }
    }

    #[test]
    fn svc_header_extension_idr() {
        // svc_extension_flag=1, idr_flag=1, all other fields zero except reserved
        let bytes: [u8; 3] = [0b1100_0000, 0b0000_0000, 0b0000_0011];
        let ext = NalHeaderExtension::from_bytes(bytes);
        match ext {
            NalHeaderExtension::Svc(svc) => {
                assert!(svc.idr_flag());
                assert_eq!(svc.priority_id(), 0);
                assert!(!svc.no_inter_layer_pred_flag());
                assert_eq!(svc.dependency_id(), 0);
                assert_eq!(svc.quality_id(), 0);
                assert_eq!(svc.temporal_id(), 0);
                assert!(!svc.use_ref_base_pic_flag());
                assert!(!svc.discardable_flag());
                assert!(!svc.output_flag());
            }
            _ => panic!("expected SVC extension"),
        }
    }

    #[test]
    fn parse_nal_header_extension_from_refnal() {
        // NAL type 14 (PrefixNALUnit), nal_ref_idc=3
        // Header byte: 0b0_11_01110 = 0x6E
        // Extension: MVC with view_id=1, all other fields 0 except reserved
        let nal_bytes: &[u8] = &[
            0x6E,        // NAL header: ref_idc=3, type=14
            0x00,        // svc=0, non_idr=0, priority_id=0
            0x00,        // view_id high 8 = 0
            0b0100_0001, // view_id low 2 = 01, temporal=0, anchor=0, inter_view=0, reserved=1
            0xAA,
            0xBB, // body bytes
        ];
        let nal = RefNal::new(nal_bytes, &[], true);
        let (ext, _rbsp) = parse_nal_header_extension(&nal).unwrap();
        match ext {
            NalHeaderExtension::Mvc(mvc) => {
                assert_eq!(mvc.view_id(), 1);
                assert!(!mvc.non_idr_flag());
            }
            _ => panic!("expected MVC extension"),
        }
    }

    #[test]
    fn reader_debug() {
        assert_eq!(
            format!(
                "{:?}",
                RefNalReader {
                    cur: &b"\x00"[..],
                    tail: &[&b"\x01"[..], &b"\x02\x03"[..]],
                    complete: false,
                }
            ),
            "00 01 02 03 ..."
        );
    }
}
