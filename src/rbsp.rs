//! Decoder that will remove _Emulation Prevention_ byte values from encoded NAL Units, to produce
//! the _Raw Byte Sequence Payload_ (RBSP).
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
//! The `RbspDecoder` type will accept byte sequences that have had this encoding applied, and will
//! yield byte sequences where the encoding is removed (i.e. the decoder will replace instances of
//! the sequence `0x00 0x00 0x03` with `0x00 0x00`).

use bitstream_io::read::BitRead;
use std::borrow::Cow;
use crate::nal::{NalHandler, NalHeader};
use crate::Context;
use log::*;

#[derive(Debug)]
enum ParseState {
    Start,
    OneZero,
    TwoZero,
}
impl ParseState {
    fn in_rbsp(&self) -> bool {
        match *self {
            ParseState::Start => true,
            ParseState::OneZero => false,
            ParseState::TwoZero => false,
        }
    }

    fn end_backtrack_bytes(&self) -> usize {
        match *self {
            ParseState::Start => 0,
            ParseState::OneZero => 1,
            ParseState::TwoZero => 2,
        }
    }
}
pub struct RbspDecoder<R>
    where
        R: NalHandler
{
    state: ParseState,
    nal_reader: R,
}
impl<R> RbspDecoder<R>
    where
        R: NalHandler
{
    pub fn new(nal_reader: R) -> Self {
        RbspDecoder {
            state: ParseState::Start,
            nal_reader,
        }
    }

    fn to(&mut self, new_state: ParseState) {
        self.state = new_state;
    }

    fn emit(&mut self, ctx: &mut Context<R::Ctx>, buf:&[u8], start_index: Option<usize>, end_index: usize) {
        if let Some(start) = start_index {
            self.nal_reader.push(ctx, &buf[start..end_index])
        } else {
            error!("RbspDecoder: no start_index");
        }
    }

    pub fn into_handler(self) -> R {
        self.nal_reader
    }
}
impl<R> NalHandler for RbspDecoder<R>
    where
        R: NalHandler
{
    type Ctx = R::Ctx;

    fn start(&mut self, ctx: &mut Context<Self::Ctx>, header: NalHeader) {
        self.state = ParseState::Start;
        self.nal_reader.start(ctx, header);
    }

    fn push(&mut self, ctx: &mut Context<Self::Ctx>, buf: &[u8]) {
        let mut rbsp_start: Option<usize> = if self.state.in_rbsp() {
            Some(0)
        } else {
            None
        };

        for i in 0..buf.len() {
            let b = buf[i];
            match self.state {
                ParseState::Start => {
                    match b {
                        0x00 => self.to(ParseState::OneZero),
                        _ => self.to(ParseState::Start),
                    }
                },
                ParseState::OneZero => {
                    match b {
                        0x00 => self.to(ParseState::TwoZero),
                        _ => {
                            if rbsp_start.is_none() {
                                let fake = [0x00];
                                self.emit(ctx, &fake[..], Some(0), 1);
                                rbsp_start = Some(i);
                            }
                            self.to(ParseState::Start)
                        },
                    }
                },
                ParseState::TwoZero => {
                    match b {
                        0x03 => {
                            // found an 'emulation prevention' byte; skip it,
                            if rbsp_start.is_none() {
                                let fake = [0x00, 0x00];
                                self.emit(ctx, &fake[..], Some(0), 2);
                            } else {
                                self.emit(ctx, buf, rbsp_start, i);
                            }
                            rbsp_start = Some(i + 1);
                            // TODO: per spec, the next byte should be either 0x00, 0x1, 0x02 or
                            // 0x03, but at the moment we assume this without checking for
                            // correctness
                            self.to(ParseState::Start);
                        },
                        // I see example PES packet payloads that end with 0x80 0x00 0x00 0x00,
                        // which triggered this error; guess the example is correct and this code
                        // was wrong, but not sure why!
                        // 0x00 => { self.err(b); },
                        _ => {
                            if rbsp_start.is_none() {
                                let fake = [0x00, 0x00];
                                self.emit(ctx, &fake[..], Some(0), 2);
                                rbsp_start = Some(i);
                            }
                            self.to(ParseState::Start)
                        },
                    }
                },
            }
        }
        if let Some(start) = rbsp_start {
            let end = buf.len() - self.state.end_backtrack_bytes();
            if start != end {
                self.nal_reader.push(ctx, &buf[start..end])
            }
        }
    }

    /// To be invoked when calling code knows that the end of a sequence of NAL Unit data has been
    /// reached.
    ///
    /// For example, if the containing data structure demarcates the end of a sequence of NAL
    /// Units explicitly, the parser for that structure should call `end_units()` once all data
    /// has been passed to the `push()` function.
    fn end(&mut self, ctx: &mut Context<Self::Ctx>) {
        let backtrack = self.state.end_backtrack_bytes();
        if backtrack > 0 {
            // if we were in the middle of parsing a sequence of 0x00 bytes that might have become
            // a start-code, but actually reached the end of input, then we will now need to emit
            // those 0x00 bytes that we had been holding back,
            let tmp = [0u8; 3];
            self.nal_reader.push(ctx, &tmp[0..backtrack]);
        }
        self.to(ParseState::Start);
        self.nal_reader.end(ctx);
    }
}

/// Removes _Emulation Prevention_ from the given byte sequence of a single NAL unit, returning the
/// NAL units _Raw Byte Sequence Payload_ (RBSP).
pub fn decode_nal<'a>(nal_unit: &'a [u8]) -> Cow<'a, [u8]> {
    struct DecoderState<'b> {
        data: Cow<'b, [u8]>,
        index: usize,
    }

    impl<'b> DecoderState<'b> {
        pub fn new(data: Cow<'b, [u8]>) -> Self {
            DecoderState { 
                data,
                index: 0,
            }
        }
    }

    impl<'b> NalHandler for DecoderState<'b> {
        type Ctx = ();

        fn start(&mut self, _ctx: &mut Context<Self::Ctx>, _header: NalHeader) {}

        fn push(&mut self, _ctx: &mut Context<Self::Ctx>, buf: &[u8]) {
            let dest = self.index..(self.index + buf.len());

            if &self.data[dest.clone()] != buf {
                self.data.to_mut()[dest].copy_from_slice(buf);
            }

            self.index += buf.len();
        }

        fn end(&mut self, _ctx: &mut Context<Self::Ctx>) {
            if let Cow::Owned(vec) = &mut self.data {
                vec.truncate(self.index);
            }
        }
    }

    let state = DecoderState::new(Cow::Borrowed(nal_unit));

    let mut decoder = RbspDecoder::new(state);
    let mut ctx = Context::default();

    decoder.push(&mut ctx, nal_unit);
    decoder.end(&mut ctx);

    decoder.into_handler().data
}

impl From<std::io::Error> for RbspBitReaderError {
    fn from(e: std::io::Error) -> Self {
        RbspBitReaderError::ReaderError(e)
    }
}

#[derive(Debug)]
pub enum RbspBitReaderError {
    ReaderError(std::io::Error),
    ReaderErrorFor(&'static str, std::io::Error),

    /// An Exp-Golomb-coded syntax elements value has more than 32 bits.
    ExpGolombTooLarge(&'static str),
}

pub struct RbspBitReader<'buf> {
    reader: bitstream_io::read::BitReader<std::io::Cursor<&'buf [u8]>, bitstream_io::BigEndian>,
}
impl<'buf> RbspBitReader<'buf> {
    pub fn new(buf: &'buf [u8]) -> Self {
        RbspBitReader {
            reader: bitstream_io::read::BitReader::new(std::io::Cursor::new(buf)),
        }
    }

    pub fn read_ue_named(&mut self, name: &'static str) -> Result<u32,RbspBitReaderError> {
        let count = count_zero_bits(&mut self.reader, name)?;
        if count > 0 {
            let val = self.read_u32(count)?;
            Ok((1 << count) -1 + val)
        } else {
            Ok(0)
        }
    }

    pub fn read_se_named(&mut self, name: &'static str) -> Result<i32, RbspBitReaderError> {
        Ok(Self::golomb_to_signed(self.read_ue_named(name)?))
    }

    pub fn read_bool(&mut self) -> Result<bool, RbspBitReaderError> {
        self.reader.read_bit().map_err( |e| RbspBitReaderError::ReaderError(e) )
    }

    pub fn read_bool_named(&mut self, name: &'static str) -> Result<bool, RbspBitReaderError> {
        self.reader.read_bit().map_err( |e| RbspBitReaderError::ReaderErrorFor(name, e) )
    }

    pub fn read_u8(&mut self, bit_count: u32) -> Result<u8, RbspBitReaderError> {
        self.reader.read(u32::from(bit_count)).map_err( |e| RbspBitReaderError::ReaderError(e) )
    }

    pub fn read_u16(&mut self, bit_count: u8) -> Result<u16, RbspBitReaderError> {
        self.reader.read(u32::from(bit_count)).map_err( |e| RbspBitReaderError::ReaderError(e) )
    }

    pub fn read_u32(&mut self, bit_count: u8) -> Result<u32, RbspBitReaderError> {
        self.reader.read(u32::from(bit_count)).map_err( |e| RbspBitReaderError::ReaderError(e) )
    }

    pub fn read_i32(&mut self, bit_count: u8) -> Result<i32, RbspBitReaderError> {
        self.reader.read(u32::from(bit_count)).map_err( |e| RbspBitReaderError::ReaderError(e) )
    }

    pub fn has_more_rbsp_data(&mut self) -> bool {
        // BitReader returns its reader iff at an aligned position.
        self.reader.reader().map(|r| (r.position() as usize) < r.get_ref().len()).unwrap_or(true)
    }

    fn golomb_to_signed(val: u32) -> i32 {
        let sign = (((val & 0x1) as i32) << 1) - 1;
        ((val >> 1) as i32 + (val & 0x1) as i32) * sign
    }
}
fn count_zero_bits<R: BitRead>(r: &mut R, name: &'static str) -> Result<u8, RbspBitReaderError> {
    let mut count = 0;
    while !r.read_bit()? {
        count += 1;
        if count > 31 {
            return Err(RbspBitReaderError::ExpGolombTooLarge(name));
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use std::cell::RefCell;
    use hex_literal::*;

    struct State {
        started: bool,
        ended: bool,
        data: Vec<u8>,
    }
    struct MockReader {
        state: Rc<RefCell<State>>
    }
    impl MockReader {
        fn new(state: Rc<RefCell<State>>) -> MockReader {
            MockReader {
                state
            }
        }
    }
    impl NalHandler for MockReader {
        type Ctx = ();

        fn start(&mut self, _ctx: &mut Context<Self::Ctx>, _header: NalHeader) {
            self.state.borrow_mut().started = true;
        }

        fn push(&mut self, _ctx: &mut Context<Self::Ctx>, buf: &[u8]) {
            self.state.borrow_mut().data.extend_from_slice(buf);
        }

        fn end(&mut self, _ctx: &mut Context<Self::Ctx>) {
            self.state.borrow_mut().ended = true;
        }
    }

    #[test]
    fn it_works() {
        let data = hex!(
           "67 64 00 0A AC 72 84 44 26 84 00 00 03
            00 04 00 00 03 00 CA 3C 48 96 11 80");
        for i in 1..data.len()-1 {
            let state = Rc::new(RefCell::new(State {
                started: false,
                ended: false,
                data: Vec::new(),
            }));
            let mock = MockReader::new(Rc::clone(&state));
            let mut r = RbspDecoder::new(mock);
            let mut ctx = Context::default();
            let (head, tail) = data.split_at(i);
            r.push(&mut ctx, head);
            r.push(&mut ctx, tail);
            let expected = hex!(
           "67 64 00 0A AC 72 84 44 26 84 00 00
            00 04 00 00 00 CA 3C 48 96 11 80");
            let s = state.borrow();
            assert_eq!(&s.data[..], &expected[..], "on split_at({})", i);
        }
    }

    #[test]
    fn decode_single_nal() {
        let data = hex!(
           "67 42 c0 15 d9 01 41 fb 01 6a 0c 02 0b
            4a 00 00 03 00 02 00 00 03 00 79 1e 2c
            5c 90");
        let expected = hex!(
           "67 42 c0 15 d9 01 41 fb 01 6a 0c 02 0b
            4a 00 00 00 02 00 00 00 79 1e 2c 5c 90");

        let decoded = decode_nal(&data);

        assert_eq!(decoded, &expected[..]);
        assert!(matches!(decoded, Cow::Owned(..)));
    }

    #[test]
    fn decode_single_nal_no_emulation() {
        let data = hex!(
           "64 00 0A AC 72 84 44 26 84 00 00
            00 04 00 00 00 CA 3C 48 96 11 80");
        let expected = hex!(
           "64 00 0A AC 72 84 44 26 84 00 00
            00 04 00 00 00 CA 3C 48 96 11 80");

        let decoded = decode_nal(&data);

        assert_eq!(decoded, &expected[..]);
        assert!(matches!(decoded, Cow::Borrowed(..)));
    }

    #[test]
    fn bitreader_has_more_data() {
        let mut reader = RbspBitReader::new(&[0x12, 0x34]);
        assert!(reader.has_more_rbsp_data());
        assert_eq!(reader.read_u8(4).unwrap(), 0x1);
        assert!(reader.has_more_rbsp_data()); // unaligned, backing reader not at EOF
        assert_eq!(reader.read_u8(4).unwrap(), 0x2);
        assert!(reader.has_more_rbsp_data()); // aligned, backing reader not at EOF
        assert_eq!(reader.read_u8(4).unwrap(), 0x3);
        assert!(reader.has_more_rbsp_data()); // unaligned, backing reader at EOF
        assert_eq!(reader.read_u8(4).unwrap(), 0x4);
        assert!(!reader.has_more_rbsp_data()); // aligned, backing reader at EOF
    }
}
