//! Decoder that will remove NAL header bytes and _Emulation Prevention_ byte
//! values from encoded NAL Units, to produce the _Raw Byte Sequence Payload_
//! (RBSP).
//!
//! The following byte sequences are not allowed to appear in a framed H264 bitstream,
//!
//!  - `0x00` `0x00` `0x00`
//!  - `0x00` `0x00` `0x01`
//!  - `0x00` `0x00` `0x02`
//!  - `0x00` `0x00` `0x03`
//!
//! therefore if these byte sequences do appear in the raw bitstream, an 'escaping' mechanism
//! (called 'emulation prevention' in the spec) is applied by adding a `0x03` byte between the
//! second and third bytes in the above sequence, resulting in the following encoded versions,
//!
//!  - `0x00` `0x00` **`0x03`** `0x00`
//!  - `0x00` `0x00` **`0x03`** `0x01`
//!  - `0x00` `0x00` **`0x03`** `0x02`
//!  - `0x00` `0x00` **`0x03`** `0x03`
//!
//! The [`ByteReader`] type will accept byte sequences that have had this encoding applied, and will
//! yield byte sequences where the encoding is removed (i.e. the decoder will replace instances of
//! the sequence `0x00 0x00 0x03` with `0x00 0x00`).

use bitstream_io::read::BitRead as _;
use bitstream_io::write::BitWrite as _;
use std::borrow::Cow;
use std::io::BufRead;
use std::io::Read;
use std::io::Write;
use std::num::NonZeroUsize;

#[derive(Copy, Clone, Debug)]
enum ParseState {
    /// Scanning for emulation prevention bytes; `zero_count` (0, 1, or 2)
    /// tracks consecutive trailing `0x00` bytes.
    Start(u8),
    Skip(NonZeroUsize),
    Three,
    PostThree,
}

const H264_HEADER_LEN: NonZeroUsize = match NonZeroUsize::new(1) {
    Some(one) => one,
    None => panic!("1 should be non-zero"),
};

/// [`BufRead`] adapter which returns RBSP from NAL bytes.
///
/// This optionally skips a given number of leading bytes, then returns any bytes except the
/// `emulation-prevention-three` bytes.
///
/// See also [module docs](self).
///
/// Typically used via a [`h264_reader::nal::Nal`]. Returns error on encountering
/// invalid byte sequences.
#[derive(Clone)]
pub struct ByteReader<R: BufRead> {
    // self.inner[0..self.i] hasn't yet been emitted and is RBSP (has no
    // emulation_prevention_three_bytes).
    //
    // self.state describes the state before self.inner[self.i].
    //
    // self.inner[self.i..] has yet to be examined.
    inner: R,
    state: ParseState,
    i: usize,

    /// The maximum number of bytes in a fresh chunk. Surprisingly, it's
    /// significantly faster to limit this, maybe due to CPU cache effects, or
    /// maybe because it's common to examine at most the headers of large slice NALs.
    max_fill: usize,

    /// Pre-built SIMD searcher for `0x00 0x00` pairs, reused across calls
    zero_pair_finder: memchr::memmem::Finder<'static>,
}
impl<R: BufRead> ByteReader<R> {
    /// Constructs an adapter from the given [`BufRead`] which does not skip any initial bytes.
    pub fn without_skip(inner: R) -> Self {
        Self {
            inner,
            state: ParseState::Start(0),
            i: 0,
            max_fill: 128,
            zero_pair_finder: memchr::memmem::Finder::new(b"\x00\x00"),
        }
    }

    /// Constructs an adapter from the given [`BufRead`] which skips the 1-byte H.264 header.
    pub fn skipping_h264_header(inner: R) -> Self {
        Self {
            inner,
            state: ParseState::Skip(H264_HEADER_LEN),
            i: 0,
            max_fill: 128,
            zero_pair_finder: memchr::memmem::Finder::new(b"\x00\x00"),
        }
    }

    /// Constructs an adapter from the given [`BufRead`] which will skip over the first `skip` bytes.
    ///
    /// This can be useful for parsing H.265, which uses the same
    /// `emulation-prevention-three-bytes` convention but two-byte NAL headers.
    pub fn skipping_bytes(inner: R, skip: NonZeroUsize) -> Self {
        Self {
            inner,
            state: ParseState::Skip(skip),
            i: 0,
            max_fill: 128,
            zero_pair_finder: memchr::memmem::Finder::new(b"\x00\x00"),
        }
    }

    /// Called when self.i == 0 only; returns false at EOF.
    /// Doesn't return actual buffer contents due to borrow checker limitations;
    /// caller will need to call fill_buf again.
    fn try_fill_buf_slow(&mut self) -> std::io::Result<bool> {
        debug_assert_eq!(self.i, 0);
        let chunk = self.inner.fill_buf()?;
        if chunk.is_empty() {
            return Ok(false);
        }

        let limit = std::cmp::min(chunk.len(), self.max_fill);
        while self.i < limit {
            match self.state {
                ParseState::Start(zero_count) => {
                    // Find the index of the byte right after a 0x00 0x00 pair,
                    // or None if no complete pair is available in this chunk.
                    let after_pair = if zero_count >= 2 {
                        // Two trailing zeros carried from previous buffer.
                        Some(self.i)
                    } else if zero_count == 1 && chunk[self.i] == 0x00 {
                        // One trailing zero + current 0x00 = cross-buffer pair.
                        if self.i + 1 < limit {
                            Some(self.i + 1)
                        } else {
                            self.state = ParseState::Start(2);
                            self.i += 1;
                            None
                        }
                    } else {
                        // Bulk scan for the next 0x00 0x00 pair.
                        match self.zero_pair_finder.find(&chunk[self.i..limit]) {
                            Some(offset) => {
                                let ap = self.i + offset + 2;
                                if ap < limit {
                                    Some(ap)
                                } else {
                                    self.state = ParseState::Start(2);
                                    self.i = ap;
                                    None
                                }
                            }
                            None => {
                                let trailing = if limit > self.i && chunk[limit - 1] == 0x00 {
                                    1
                                } else {
                                    0
                                };
                                self.state = ParseState::Start(trailing);
                                self.i = limit;
                                None
                            }
                        }
                    };
                    let Some(after_pair) = after_pair else { break };
                    // Check the byte after the 0x00 0x00 pair.
                    match chunk[after_pair] {
                        0x03 => {
                            self.i = after_pair;
                            self.state = ParseState::Three;
                            break;
                        }
                        0x00 => {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                format!("invalid RBSP byte {:#x} in state {:?}", 0x00, &self.state,),
                            ));
                        }
                        _ => {
                            self.i = after_pair + 1;
                            self.state = ParseState::Start(0);
                            continue;
                        }
                    }
                }
                ParseState::Skip(remaining) => {
                    debug_assert_eq!(self.i, 0);
                    let skip = std::cmp::min(chunk.len(), remaining.get());
                    self.inner.consume(skip);
                    self.state = NonZeroUsize::new(remaining.get() - skip)
                        .map(ParseState::Skip)
                        .unwrap_or(ParseState::Start(0));
                    break;
                }
                ParseState::Three => {
                    debug_assert_eq!(self.i, 0);
                    self.inner.consume(1);
                    self.state = ParseState::PostThree;
                    break;
                }
                ParseState::PostThree => {
                    match chunk[self.i] {
                        0x00 => self.state = ParseState::Start(1),
                        0x01 | 0x02 | 0x03 => self.state = ParseState::Start(0),
                        o => {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                format!("invalid RBSP byte {:#x} in state {:?}", o, &self.state),
                            ))
                        }
                    }
                    self.i += 1;
                }
            }
        }
        Ok(true)
    }

    /// Borrows the underlying reader
    pub fn reader(&mut self) -> &mut R {
        &mut self.inner
    }
}
impl<R: BufRead> Read for ByteReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let chunk = self.fill_buf()?;
        let amt = std::cmp::min(buf.len(), chunk.len());
        if amt == 1 {
            // Stolen from std::io::Read implementation for &[u8]:
            // apparently this is faster to special-case. (And this is the
            // common case for BitReader.)
            buf[0] = chunk[0];
        } else {
            buf[..amt].copy_from_slice(&chunk[..amt]);
        }
        self.consume(amt);
        Ok(amt)
    }
}
impl<R: BufRead> BufRead for ByteReader<R> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        while self.i == 0 && self.try_fill_buf_slow()? {}
        Ok(&self.inner.fill_buf()?[0..self.i])
    }

    fn consume(&mut self, amt: usize) {
        self.i = self.i.checked_sub(amt).unwrap();
        self.inner.consume(amt);
    }
}

/// Returns RBSP from a NAL by removing the NAL header and `emulation-prevention-three` bytes.
///
/// See also [module docs](self).
///
/// Returns error on invalid byte sequences. Returns a borrowed pointer if possible.
///
/// ```
/// # use h264_reader::rbsp::decode_nal;
/// # use std::borrow::Cow;
/// # use std::io::ErrorKind;
/// let nal_with_escape = &b"\x68\x12\x34\x00\x00\x03\x00\x86"[..];
/// assert!(matches!(
///     decode_nal(nal_with_escape).unwrap(),
///     Cow::Owned(s) if s == &b"\x12\x34\x00\x00\x00\x86"[..]));
///
/// let nal_without_escape = &b"\x68\xE8\x43\x8F\x13\x21\x30"[..];
/// assert_eq!(decode_nal(nal_without_escape).unwrap(), Cow::Borrowed(&nal_without_escape[1..]));
///
/// let invalid_nal = &b"\x68\x12\x34\x00\x00\x00\x86"[..];
/// assert_eq!(decode_nal(invalid_nal).unwrap_err().kind(), ErrorKind::InvalidData);
/// ```
pub fn decode_nal<'a>(nal_unit: &'a [u8]) -> Result<Cow<'a, [u8]>, std::io::Error> {
    let mut reader = ByteReader {
        inner: nal_unit,
        state: ParseState::Skip(H264_HEADER_LEN),
        i: 0,
        max_fill: usize::MAX, // to borrow if at all possible.
        zero_pair_finder: memchr::memmem::Finder::new(b"\x00\x00"),
    };
    let buf = reader.fill_buf()?;
    if buf.len() + 1 == nal_unit.len() {
        return Ok(Cow::Borrowed(&nal_unit[1..]));
    }
    // Upper bound estimate; skipping the NAL header and at least one emulation prevention byte.
    let mut dst = Vec::with_capacity(nal_unit.len() - 2);
    loop {
        let buf = reader.fill_buf()?;
        if buf.is_empty() {
            break;
        }
        dst.extend_from_slice(buf);
        let len = buf.len();
        reader.consume(len);
    }
    Ok(Cow::Owned(dst))
}

#[derive(Debug)]
pub enum BitReaderError {
    /// An I/O error occurred reading the given field.
    ReaderError(&'static str, std::io::Error),

    /// An Exp-Golomb-coded syntax elements value has more than 32 bits.
    ExpGolombTooLarge(&'static str),

    /// The stream was positioned before the final one bit on [BitRead::finish_rbsp].
    RemainingData,
}

pub trait Integer: bitstream_io::Integer + std::fmt::Debug {}
impl<I: bitstream_io::Integer + std::fmt::Debug> Integer for I {}

pub trait Primitive: bitstream_io::Primitive + std::fmt::Debug {}
impl<P: bitstream_io::Primitive + std::fmt::Debug> Primitive for P {}

/// Writes H.26x bitstream syntax elements.
///
/// This is the write counterpart to [`BitRead`].
pub trait BitWrite {
    /// Writes an unsigned Exp-Golomb-coded value, as defined in the H.264 spec.
    fn write_ue(&mut self, value: u32) -> std::io::Result<()>;

    /// Writes a signed Exp-Golomb-coded value, as defined in the H.264 spec.
    fn write_se(&mut self, value: i32) -> std::io::Result<()>;

    /// Writes a single bit.
    fn write_bit(&mut self, bit: bool) -> std::io::Result<()>;

    /// Writes `BITS` bits of `value`.
    fn write<const BITS: u32, I: Integer>(&mut self, value: I) -> std::io::Result<()>;

    /// Writes `bit_count` bits of `value`.
    fn write_var<I: Integer>(&mut self, bit_count: u32, value: I) -> std::io::Result<()>;

    /// Writes the RBSP trailing bits: a stop bit (1) and then zero-padding to byte boundary.
    fn write_rbsp_trailing_bits(&mut self) -> std::io::Result<()>;
}

/// Reads H.26x bitstream elements as specified in H.264 section 7.2.
pub trait BitRead {
    /// Reads an unsigned Exp-Golomb-coded value, as defined in the H.264 spec.
    fn read_ue(&mut self, name: &'static str) -> Result<u32, BitReaderError>;

    /// Reads a signed Exp-Golomb-coded value, as defined in the H.264 spec.
    fn read_se(&mut self, name: &'static str) -> Result<i32, BitReaderError>;

    /// Reads a single bit, as in [`crate::bitstream_io::read::BitRead::read_bit`].
    fn read_bit(&mut self, name: &'static str) -> Result<bool, BitReaderError>;

    /// Reads a value from the bitstream with a statically-known number of bits, as in
    /// [`crate::bitstream_io::read::BitRead::read`]. This matches the `u(BITS)`
    /// and `i(BITS)` syntax elements.
    fn read<const BITS: u32, I: Integer>(
        &mut self,
        name: &'static str,
    ) -> Result<I, BitReaderError>;

    /// Reads a value from the bitstream with a runtime-determined number of bits, as in
    /// [`crate::bitstream_io::read::BitRead::read_var`]. This matches the
    /// `u(bit_count)` and `i(bit_count)` syntax elements.
    fn read_var<I: Integer>(
        &mut self,
        bit_count: u32,
        name: &'static str,
    ) -> Result<I, BitReaderError>;

    /// Reads a whole value from the bitstream whose size is equal to its byte size, as in
    /// [`crate::bitstream_io::read::BitRead::read_to`].
    fn read_to<V: Primitive>(&mut self, name: &'static str) -> Result<V, BitReaderError>;

    /// Skips the given number of bits in the bitstream, as in
    /// [`crate::bitstream_io::read::BitRead::skip`].
    fn skip(&mut self, bit_count: u32, name: &'static str) -> Result<(), BitReaderError>;

    /// Returns true if positioned before the RBSP trailing bits.
    ///
    /// This matches the definition of `more_rbsp_data()` in Rec. ITU-T H.264
    /// (03/2010) section 7.2.
    fn has_more_rbsp_data(&mut self, name: &'static str) -> Result<bool, BitReaderError>;

    /// Consumes the reader, returning error if it's not positioned at the RBSP trailing bits.
    fn finish_rbsp(self) -> Result<(), BitReaderError>;

    /// Consumes the reader, returning error if this `sei_payload` message is unfinished.
    ///
    /// This is similar to `finish_rbsp`, but SEI payloads have no trailing bits if
    /// already byte-aligned.
    fn finish_sei_payload(self) -> Result<(), BitReaderError>;
}

/// Reads H.264 bitstream syntax elements from an RBSP representation (no NAL
/// header byte or emulation prevention three bytes).
///
/// Use `BitReader::new(ByteReader::skipping_h264_header(nal))` to read the bit stream
/// from a complete NAL representation, including header and emulation prevention three bytes.
pub struct BitReader<R: std::io::BufRead + Clone> {
    reader: bitstream_io::read::BitReader<R, bitstream_io::BigEndian>,
}
impl<R: std::io::BufRead + Clone> BitReader<R> {
    pub fn new(inner: R) -> Self {
        Self {
            reader: bitstream_io::read::BitReader::new(inner),
        }
    }

    /// Borrows the underlying reader if byte-aligned.
    pub fn reader(&mut self) -> Option<&mut R> {
        self.reader.reader()
    }

    /// Unwraps internal reader and disposes of BitReader.
    ///
    /// # Warning
    ///
    /// Any unread partial bits are discarded.
    pub fn into_reader(self) -> R {
        self.reader.into_reader()
    }
}

impl<R: std::io::BufRead + Clone> BitRead for BitReader<R> {
    fn read_ue(&mut self, name: &'static str) -> Result<u32, BitReaderError> {
        let count = self
            .reader
            .read_unary::<1>()
            .map_err(|e| BitReaderError::ReaderError(name, e))?;
        if count > 31 {
            return Err(BitReaderError::ExpGolombTooLarge(name));
        } else if count > 0 {
            let val: u32 = self.read_var(count, name)?;
            Ok((1 << count) - 1 + val)
        } else {
            Ok(0)
        }
    }

    fn read_se(&mut self, name: &'static str) -> Result<i32, BitReaderError> {
        Ok(golomb_to_signed(self.read_ue(name)?))
    }

    fn read_bit(&mut self, name: &'static str) -> Result<bool, BitReaderError> {
        self.reader
            .read_bit()
            .map_err(|e| BitReaderError::ReaderError(name, e))
    }

    fn read<const BITS: u32, I: Integer>(
        &mut self,
        name: &'static str,
    ) -> Result<I, BitReaderError> {
        self.reader
            .read::<BITS, I>()
            .map_err(|e| BitReaderError::ReaderError(name, e))
    }

    fn read_var<I: Integer>(
        &mut self,
        bit_count: u32,
        name: &'static str,
    ) -> Result<I, BitReaderError> {
        self.reader
            .read_var(bit_count)
            .map_err(|e| BitReaderError::ReaderError(name, e))
    }

    fn read_to<V: Primitive>(&mut self, name: &'static str) -> Result<V, BitReaderError> {
        self.reader
            .read_to()
            .map_err(|e| BitReaderError::ReaderError(name, e))
    }

    fn skip(&mut self, bit_count: u32, name: &'static str) -> Result<(), BitReaderError> {
        self.reader
            .skip(bit_count)
            .map_err(|e| BitReaderError::ReaderError(name, e))
    }

    fn has_more_rbsp_data(&mut self, name: &'static str) -> Result<bool, BitReaderError> {
        let mut throwaway = self.reader.clone();
        let r = (move || {
            throwaway.skip(1)?;
            throwaway.read_unary::<1>()?;
            Ok::<_, std::io::Error>(())
        })();
        match r {
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(false),
            Err(e) => Err(BitReaderError::ReaderError(name, e)),
            Ok(_) => Ok(true),
        }
    }

    fn finish_rbsp(mut self) -> Result<(), BitReaderError> {
        // The next bit is expected to be the final one bit.
        if !self
            .reader
            .read_bit()
            .map_err(|e| BitReaderError::ReaderError("finish", e))?
        {
            // It was a zero! Determine if we're past the end or haven't reached it yet.
            match self.reader.read_unary::<1>() {
                Err(e) => return Err(BitReaderError::ReaderError("finish", e)),
                Ok(_) => return Err(BitReaderError::RemainingData),
            }
        }
        // All remaining bits in the stream must then be zeros.
        match self.reader.read_unary::<1>() {
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(()),
            Err(e) => Err(BitReaderError::ReaderError("finish", e)),
            Ok(_) => Err(BitReaderError::RemainingData),
        }
    }

    fn finish_sei_payload(mut self) -> Result<(), BitReaderError> {
        match self.reader.read_bit() {
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(e) => return Err(BitReaderError::ReaderError("finish", e)),
            Ok(false) => return Err(BitReaderError::RemainingData),
            Ok(true) => {}
        }
        match self.reader.read_unary::<1>() {
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(()),
            Err(e) => Err(BitReaderError::ReaderError("finish", e)),
            Ok(_) => Err(BitReaderError::RemainingData),
        }
    }
}
fn golomb_to_signed(val: u32) -> i32 {
    let sign = (((val & 0x1) as i32) << 1) - 1;
    ((val >> 1) as i32 + (val & 0x1) as i32) * sign
}

/// Writes H.264 bitstream syntax elements to RBSP writer.
pub struct BitWriter<W: std::io::Write> {
    inner: bitstream_io::write::BitWriter<W, bitstream_io::BigEndian>,
}

impl<W: std::io::Write> BitWriter<W> {
    /// Creates a new `BitWriter` writing to `writer`.
    pub fn new(writer: W) -> Self {
        Self {
            inner: bitstream_io::write::BitWriter::new(writer),
        }
    }

    /// Returns a mutable reference to the underlying writer, if the stream is byte-aligned.
    pub fn writer(&mut self) -> Option<&mut W> {
        self.inner.writer()
    }

    /// Returns the underlying writer.
    ///
    /// # Warning
    ///
    /// Any unwritten partial bits are discarded.
    pub fn into_writer(self) -> W {
        self.inner.into_writer()
    }
}

impl<W: std::io::Write> BitWrite for BitWriter<W> {
    fn write_ue(&mut self, value: u32) -> std::io::Result<()> {
        // Exp-Golomb: write (count) zero bits, then 1 bit, then (count) data bits.
        // code_num = value, M = floor(log2(value+1)), prefix is (M+1) bits = M zeros + 1 one,
        // suffix is M bits = value+1 - 2^M.
        if value == 0 {
            self.inner.write_bit(true)
        } else {
            let code_num = value + 1;
            let bits = 32 - code_num.leading_zeros(); // = floor(log2(code_num)) + 1
            let zeros = bits - 1;
            // write (zeros) zero bits
            for _ in 0..zeros {
                self.inner.write_bit(false)?;
            }
            // write (bits) bits of code_num
            self.inner.write_var(bits, code_num)
        }
    }

    fn write_se(&mut self, value: i32) -> std::io::Result<()> {
        // Map signed -> unsigned: 0->0, 1->1, -1->2, 2->3, -2->4, ...
        let code_num = if value > 0 {
            (value as u32) * 2 - 1
        } else {
            (-value as u32) * 2
        };
        self.write_ue(code_num)
    }

    fn write_bit(&mut self, bit: bool) -> std::io::Result<()> {
        self.inner.write_bit(bit)
    }

    fn write<const BITS: u32, I: Integer>(&mut self, value: I) -> std::io::Result<()> {
        self.inner.write::<BITS, I>(value)
    }

    fn write_var<I: Integer>(&mut self, bit_count: u32, value: I) -> std::io::Result<()> {
        self.inner.write_var::<I>(bit_count, value)
    }

    fn write_rbsp_trailing_bits(&mut self) -> std::io::Result<()> {
        self.inner.write_bit(true)?; // stop bit
        self.inner.byte_align()?; // zero-pad to byte boundary
        Ok(())
    }
}

/// [`Write`] adapter which inserts emulation-prevention-three bytes into RBSP.
///
/// The caller writes raw RBSP bytes; this inserts `0x03` bytes wherever the
/// sequence `0x00 0x00` would be followed by `0x00`, `0x01`, `0x02`, or `0x03`.
///
/// See also [module docs](self).
pub struct ByteWriter<W: Write> {
    inner: W,
    /// Number of consecutive `0x00` bytes at the tail of what has been written
    /// so far. Always 0, 1, or 2.
    zero_count: u8,
}

impl<W: Write> ByteWriter<W> {
    /// Creates a new `ByteWriter` wrapping the given [`Write`].
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            zero_count: 0,
        }
    }

    /// Returns the underlying writer.
    pub fn into_writer(self) -> W {
        self.inner
    }
}

impl<W: Write> Write for ByteWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut i = 0;
        let mut chunk_start = 0;
        while i < buf.len() {
            // When two trailing zeros have been written, the current byte may
            // require an emulation-prevention byte inserted before it.
            if self.zero_count == 2 {
                let b = buf[i];
                if b <= 3 {
                    self.inner.write_all(&buf[chunk_start..i])?;
                    chunk_start = i;
                    self.inner.write_all(&[0x03])?;
                }
                self.zero_count = 0;
                // Fall through; the memchr scan below processes buf[i].
            }
            // zero_count is 0 or 1 here. Use memchr to skip non-zero bytes.
            match memchr::memchr(0x00, &buf[i..]) {
                None => {
                    self.zero_count = 0;
                    break;
                }
                Some(rel) => {
                    // buf[i..i+rel] are non-zero (any of them resets zero_count),
                    // buf[i+rel] is 0x00.
                    self.zero_count = if rel > 0 { 1 } else { self.zero_count + 1 };
                    i += rel + 1;
                }
            }
        }
        self.inner.write_all(&buf[chunk_start..])?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex_literal::*;
    use hex_slice::AsHex;

    #[test]
    fn byte_reader() {
        let data = hex!(
            "67 64 00 0A AC 72 84 44 26 84 00 00 03
            00 04 00 00 03 00 CA 3C 48 96 11 80"
        );
        for i in 1..data.len() - 1 {
            let (head, tail) = data.split_at(i);
            let r = head.chain(tail);
            let mut r = ByteReader::skipping_h264_header(r);
            let mut rbsp = Vec::new();
            r.read_to_end(&mut rbsp).unwrap();
            let expected = hex!(
                "64 00 0A AC 72 84 44 26 84 00 00
            00 04 00 00 00 CA 3C 48 96 11 80"
            );
            assert!(
                rbsp == &expected[..],
                "Mismatch with on split_at({}):\nrbsp     {:02x}\nexpected {:02x}",
                i,
                rbsp.as_hex(),
                expected.as_hex()
            );
        }
    }

    #[test]
    fn bitreader_has_more_data() {
        // Should work when the end bit is byte-aligned.
        let mut reader = BitReader::new(&[0x12, 0x80][..]);
        assert!(reader.has_more_rbsp_data("call 1").unwrap());
        assert_eq!(reader.read::<8, u8>("u8 1").unwrap(), 0x12);
        assert!(!reader.has_more_rbsp_data("call 2").unwrap());

        // and when it's not.
        let mut reader = BitReader::new(&[0x18][..]);
        assert!(reader.has_more_rbsp_data("call 3").unwrap());
        assert_eq!(reader.read::<4, u8>("u8 2").unwrap(), 0x1);
        assert!(!reader.has_more_rbsp_data("call 4").unwrap());

        // should also work when there are cabac-zero-words.
        let mut reader = BitReader::new(&[0x80, 0x00, 0x00][..]);
        assert!(!reader
            .has_more_rbsp_data("at end with cabac-zero-words")
            .unwrap());
    }

    #[test]
    fn byte_reader_emulation_prevention_beyond_max_fill() {
        // Input: 129 non-zero bytes followed by an emulation prevention
        // sequence (00 00 03 01). With max_fill=128, the initial memchr scan
        // only covers the first 128 bytes. A bug caused bytes beyond max_fill
        // to be returned as RBSP without being checked, so the 0x03 emulation
        // prevention byte would not be stripped.
        let mut input = vec![0xFF; 129];
        input.extend_from_slice(&[0x00, 0x00, 0x03, 0x01]);
        let mut r = ByteReader::without_skip(&input[..]);
        let mut rbsp = Vec::new();
        r.read_to_end(&mut rbsp).unwrap();
        let mut expected = vec![0xFF; 129];
        expected.extend_from_slice(&[0x00, 0x00, 0x01]);
        assert_eq!(rbsp, expected, "emulation prevention byte was not stripped");
    }

    #[test]
    fn read_ue_overflow() {
        let mut reader = BitReader::new(&[0, 0, 0, 0, 255, 255, 255, 255, 255][..]);
        assert!(matches!(
            reader.read_ue("test"),
            Err(BitReaderError::ExpGolombTooLarge("test"))
        ));
    }

    /// Writes `rbsp` through a `ByteWriter` and returns the emulation-prevention-encoded bytes.
    fn byte_writer_encode(rbsp: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        ByteWriter::new(&mut out).write_all(rbsp).unwrap();
        out
    }

    #[test]
    fn byte_writer_no_escaping_needed() {
        // Bytes that never trigger emulation prevention.
        assert_eq!(byte_writer_encode(b"hello"), b"hello");
        assert_eq!(
            byte_writer_encode(&[0xFF, 0xFE, 0x01, 0x02]),
            &[0xFF, 0xFE, 0x01, 0x02]
        );
        // Single zero: no escape.
        assert_eq!(byte_writer_encode(&[0x00, 0x04]), &[0x00, 0x04]);
        // Two zeros followed by non-trigger byte: no escape.
        assert_eq!(byte_writer_encode(&[0x00, 0x00, 0x04]), &[0x00, 0x00, 0x04]);
    }

    #[test]
    fn byte_writer_escaping() {
        // The four trigger bytes after two zeros.
        assert_eq!(
            byte_writer_encode(&[0x00, 0x00, 0x00]),
            &[0x00, 0x00, 0x03, 0x00]
        );
        assert_eq!(
            byte_writer_encode(&[0x00, 0x00, 0x01]),
            &[0x00, 0x00, 0x03, 0x01]
        );
        assert_eq!(
            byte_writer_encode(&[0x00, 0x00, 0x02]),
            &[0x00, 0x00, 0x03, 0x02]
        );
        assert_eq!(
            byte_writer_encode(&[0x00, 0x00, 0x03]),
            &[0x00, 0x00, 0x03, 0x03]
        );
    }

    #[test]
    fn byte_writer_multiple_escapes() {
        // Five zeros then 0x01: the third zero triggers one escape (leaving one
        // trailing zero), then the fifth zero makes two trailing zeros again so
        // 0x01 triggers a second escape.
        assert_eq!(
            byte_writer_encode(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x01]),
            &[0x00, 0x00, 0x03, 0x00, 0x00, 0x03, 0x00, 0x01],
        );
    }

    #[test]
    fn byte_writer_split_writes() {
        // Verify state is maintained across separate write() calls.
        let mut out = Vec::new();
        let mut w = ByteWriter::new(&mut out);
        w.write_all(&[0x00, 0x00]).unwrap();
        w.write_all(&[0x03]).unwrap(); // should be escaped
        drop(w);
        assert_eq!(out, &[0x00, 0x00, 0x03, 0x03]);

        let mut out2 = Vec::new();
        let mut w2 = ByteWriter::new(&mut out2);
        w2.write_all(&[0x00]).unwrap();
        w2.write_all(&[0x00]).unwrap();
        w2.write_all(&[0x01]).unwrap(); // should be escaped
        drop(w2);
        assert_eq!(out2, &[0x00, 0x00, 0x03, 0x01]);
    }

    /// Builds a complete NAL unit from `hdr` and `rbsp` using `ByteWriter`.
    fn make_nal(hdr: u8, rbsp: &[u8]) -> Vec<u8> {
        // Capacity: at most 1 escape per 3 RBSP bytes.
        let mut out = Vec::with_capacity(1 + rbsp.len() + rbsp.len() / 3);
        out.push(hdr);
        ByteWriter::new(&mut out).write_all(rbsp).unwrap();
        out
    }

    #[test]
    fn byte_writer_roundtrip() {
        // Roundtrip: decode(make_nal(hdr, rbsp)) == rbsp.
        let rbsp = hex!(
            "64 00 0A AC 72 84 44 26 84 00 00
            00 04 00 00 00 CA 3C 48 96 11 80"
        );
        let nal = make_nal(0x67, &rbsp);
        let decoded = decode_nal(&nal).unwrap();
        assert_eq!(&*decoded, &rbsp[..]);
    }

    #[test]
    fn byte_writer_escape_inserted_in_nal() {
        // RBSP: 12 34 00 00 00 86 -> NAL: 68 12 34 00 00 03 00 86
        assert_eq!(
            make_nal(0x68, &hex!("12 34 00 00 00 86")),
            hex!("68 12 34 00 00 03 00 86"),
        );
    }
}
