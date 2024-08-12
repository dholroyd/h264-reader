//! Types for reading H264 _Network Abstraction Layer_ Units (NAL Units).
//!
//! The data presented must already be in _RBSP_ form (i.e. have been passed through
//! [`RbspDecoder`](../rbsp/struct.RbspDecoder.html)), where it has been encoded with
//! 'emulation prevention bytes'.

pub mod pps;
pub mod sei;
pub mod slice;
pub mod sps;

use crate::rbsp;
use hex_slice::AsHex;
use std::fmt;

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
/// assert_eq!(r.read::<u8>(4, "first nibble").unwrap(), 0x1);
/// assert_eq!(r.read::<u8>(4, "second nibble").unwrap(), 0x2);
/// assert_eq!(r.read::<u32>(23, "23 bits at a time").unwrap(), 0x1a_00_00);
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
