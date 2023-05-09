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
use std::borrow::Cow;
use std::io::BufRead;
use std::io::Read;

#[derive(Copy, Clone, Debug)]
enum ParseState {
    Start,
    OneZero,
    TwoZero,
    HeaderByte,
    Three,
    PostThree,
}

/// [`BufRead`] adapter which returns RBSP bytes given NAL bytes by removing
/// the NAL header and `emulation-prevention-three` bytes.
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
    /// significantly faster to limit this, maybe due to CPU cache effects.
    max_fill: usize,
}
impl<R: BufRead> ByteReader<R> {
    /// Constructs an adapter from the given [BufRead]. The NAL header byte is
    /// expected to be present.
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            state: ParseState::HeaderByte,
            i: 0,
            max_fill: 128,
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
                ParseState::Start => match memchr::memchr(0x00, &chunk[self.i..limit]) {
                    Some(nonzero_len) => {
                        self.i += nonzero_len;
                        self.state = ParseState::OneZero;
                    }
                    None => {
                        self.i = chunk.len();
                        break;
                    }
                },
                ParseState::OneZero => match chunk[self.i] {
                    0x00 => self.state = ParseState::TwoZero,
                    _ => self.state = ParseState::Start,
                },
                ParseState::TwoZero => match chunk[self.i] {
                    0x03 => {
                        self.state = ParseState::Three;
                        break;
                    }
                    0x00 => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("invalid RBSP byte {:#x} in state {:?}", 0x00, &self.state),
                        ))
                    }
                    _ => self.state = ParseState::Start,
                },
                ParseState::HeaderByte => {
                    debug_assert_eq!(self.i, 0);
                    self.inner.consume(1);
                    self.state = ParseState::Start;
                    break;
                }
                ParseState::Three => {
                    debug_assert_eq!(self.i, 0);
                    self.inner.consume(1);
                    self.state = ParseState::PostThree;
                    break;
                }
                ParseState::PostThree => match chunk[self.i] {
                    0x00 => self.state = ParseState::OneZero,
                    0x01 | 0x02 | 0x03 => self.state = ParseState::Start,
                    o => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("invalid RBSP byte {:#x} in state {:?}", o, &self.state),
                        ))
                    }
                },
            }
            self.i += 1;
        }
        Ok(true)
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
        state: ParseState::HeaderByte,
        i: 0,
        max_fill: usize::MAX, // to borrow if at all possible.
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
    ReaderError(std::io::Error),
    ReaderErrorFor(&'static str, std::io::Error),

    /// An Exp-Golomb-coded syntax elements value has more than 32 bits.
    ExpGolombTooLarge(&'static str),

    /// The stream was positioned before the final one bit on [BitRead::finish_rbsp].
    RemainingData,

    Unaligned,
}

pub trait BitRead {
    fn read_ue(&mut self, name: &'static str) -> Result<u32, BitReaderError>;
    fn read_se(&mut self, name: &'static str) -> Result<i32, BitReaderError>;
    fn read_bool(&mut self, name: &'static str) -> Result<bool, BitReaderError>;
    fn read_u8(&mut self, bit_count: u32, name: &'static str) -> Result<u8, BitReaderError>;
    fn read_u16(&mut self, bit_count: u32, name: &'static str) -> Result<u16, BitReaderError>;
    fn read_u32(&mut self, bit_count: u32, name: &'static str) -> Result<u32, BitReaderError>;
    fn read_i32(&mut self, bit_count: u32, name: &'static str) -> Result<i32, BitReaderError>;

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
}

impl<R: std::io::BufRead + Clone> BitRead for BitReader<R> {
    fn read_ue(&mut self, name: &'static str) -> Result<u32, BitReaderError> {
        let count = self
            .reader
            .read_unary1()
            .map_err(|e| BitReaderError::ReaderErrorFor(name, e))?;
        if count > 31 {
            return Err(BitReaderError::ExpGolombTooLarge(name));
        } else if count > 0 {
            let val = self.read_u32(count, name)?;
            Ok((1 << count) - 1 + val)
        } else {
            Ok(0)
        }
    }

    fn read_se(&mut self, name: &'static str) -> Result<i32, BitReaderError> {
        Ok(golomb_to_signed(self.read_ue(name)?))
    }

    fn read_bool(&mut self, name: &'static str) -> Result<bool, BitReaderError> {
        self.reader
            .read_bit()
            .map_err(|e| BitReaderError::ReaderErrorFor(name, e))
    }

    fn read_u8(&mut self, bit_count: u32, name: &'static str) -> Result<u8, BitReaderError> {
        self.reader
            .read(bit_count)
            .map_err(|e| BitReaderError::ReaderErrorFor(name, e))
    }

    fn read_u16(&mut self, bit_count: u32, name: &'static str) -> Result<u16, BitReaderError> {
        self.reader
            .read(bit_count)
            .map_err(|e| BitReaderError::ReaderErrorFor(name, e))
    }

    fn read_u32(&mut self, bit_count: u32, name: &'static str) -> Result<u32, BitReaderError> {
        self.reader
            .read(bit_count)
            .map_err(|e| BitReaderError::ReaderErrorFor(name, e))
    }

    fn read_i32(&mut self, bit_count: u32, name: &'static str) -> Result<i32, BitReaderError> {
        self.reader
            .read(bit_count)
            .map_err(|e| BitReaderError::ReaderErrorFor(name, e))
    }

    fn has_more_rbsp_data(&mut self, name: &'static str) -> Result<bool, BitReaderError> {
        let mut throwaway = self.reader.clone();
        let r = (move || {
            throwaway.skip(1)?;
            throwaway.read_unary1()?;
            Ok::<_, std::io::Error>(())
        })();
        match r {
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(false),
            Err(e) => Err(BitReaderError::ReaderErrorFor(name, e)),
            Ok(_) => Ok(true),
        }
    }

    fn finish_rbsp(mut self) -> Result<(), BitReaderError> {
        // The next bit is expected to be the final one bit.
        if !self
            .reader
            .read_bit()
            .map_err(|e| BitReaderError::ReaderErrorFor("finish", e))?
        {
            // It was a zero! Determine if we're past the end or haven't reached it yet.
            match self.reader.read_unary1() {
                Err(e) => return Err(BitReaderError::ReaderErrorFor("finish", e)),
                Ok(_) => return Err(BitReaderError::RemainingData),
            }
        }
        // All remaining bits in the stream must then be zeros.
        match self.reader.read_unary1() {
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(()),
            Err(e) => Err(BitReaderError::ReaderErrorFor("finish", e)),
            Ok(_) => Err(BitReaderError::RemainingData),
        }
    }

    fn finish_sei_payload(mut self) -> Result<(), BitReaderError> {
        match self.reader.read_bit() {
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(e) => return Err(BitReaderError::ReaderErrorFor("finish", e)),
            Ok(false) => return Err(BitReaderError::RemainingData),
            Ok(true) => {}
        }
        match self.reader.read_unary1() {
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(()),
            Err(e) => Err(BitReaderError::ReaderErrorFor("finish", e)),
            Ok(_) => Err(BitReaderError::RemainingData),
        }
    }
}
fn golomb_to_signed(val: u32) -> i32 {
    let sign = (((val & 0x1) as i32) << 1) - 1;
    ((val >> 1) as i32 + (val & 0x1) as i32) * sign
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
            let mut r = ByteReader::new(r);
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
        assert_eq!(reader.read_u8(8, "u8 1").unwrap(), 0x12);
        assert!(!reader.has_more_rbsp_data("call 2").unwrap());

        // and when it's not.
        let mut reader = BitReader::new(&[0x18][..]);
        assert!(reader.has_more_rbsp_data("call 3").unwrap());
        assert_eq!(reader.read_u8(4, "u8 2").unwrap(), 0x1);
        assert!(!reader.has_more_rbsp_data("call 4").unwrap());

        // should also work when there are cabac-zero-words.
        let mut reader = BitReader::new(&[0x80, 0x00, 0x00][..]);
        assert!(!reader
            .has_more_rbsp_data("at end with cabac-zero-words")
            .unwrap());
    }

    #[test]
    fn read_ue_overflow() {
        let mut reader = BitReader::new(&[0, 0, 0, 0, 255, 255, 255, 255, 255][..]);
        assert!(matches!(
            reader.read_ue("test"),
            Err(BitReaderError::ExpGolombTooLarge("test"))
        ));
    }
}
