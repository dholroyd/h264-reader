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

use std::ops::{Deref, DerefMut};
use bitreader;
use ::nal::NalHandler;
use ::nal::NalHeader;
use Context;

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
    pub fn new(nal_reader: R) -> RbspDecoder<R> {
        RbspDecoder {
            state: ParseState::Start,
            nal_reader,
        }
    }

    fn to(&mut self, new_state: ParseState) {
        self.state = new_state;
    }

    fn emit(&mut self, ctx: &mut Context, buf:&[u8], start_index: Option<usize>, end_index: usize) {
        //println!("emit {:?}", &buf[start_index.unwrap()..end_index]);
        if let Some(start) = start_index {
            self.nal_reader.push(ctx, &buf[start..end_index])
        } else {
            eprintln!("RbspDecoder: no start_index");
        }
    }

    fn err(&mut self, b: u8) {
        eprintln!("RbspDecoder: state={:?}, invalid byte {:#x}", self.state, b);
        self.state = ParseState::Start;
    }
}
impl<R> NalHandler for RbspDecoder<R>
    where
        R: NalHandler
{
    fn start(&mut self, ctx: &mut Context, header: &NalHeader) {
        self.state = ParseState::Start;
        self.nal_reader.start(ctx, header);
    }

    fn push(&mut self, ctx: &mut Context, buf: &[u8]) {
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
    fn end(&mut self, ctx: &mut Context) {
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


impl From<bitreader::BitReaderError> for RbspBitReaderError {
    fn from(e: bitreader::BitReaderError) -> Self {
        RbspBitReaderError::ReaderError(e)
    }
}

#[derive(Debug)]
pub enum RbspBitReaderError {
    ReaderError(bitreader::BitReaderError),
    ReaderErrorFor(&'static str, bitreader::BitReaderError),
}

pub struct RbspBitReader<'a> {
    total_size: usize,
    reader: bitreader::BitReader<'a>,
}
impl<'a> RbspBitReader<'a> {
    pub fn new(buf: &'a[u8]) -> RbspBitReader<'a> {
        RbspBitReader {
            total_size: buf.len() * 8,
            reader: bitreader::BitReader::new(buf),
        }
    }
    pub fn read_ue(&mut self) -> Result<u32,bitreader::BitReaderError> {
        let count = count_zero_bits(&mut self.reader)?;
        if count > 0 {
            let val = self.reader.read_u32(count)?;
            Ok((1 << count) -1 + val)
        } else {
            Ok(0)
        }
    }

    pub fn read_ue_named(&mut self, name: &'static str) -> Result<u32,RbspBitReaderError> {
        self.read_ue().map_err( |e| RbspBitReaderError::ReaderErrorFor(name, e) )
    }

    pub fn read_bool_named(&mut self, name: &'static str) -> Result<bool, RbspBitReaderError> {
        self.read_bool().map_err( |e| RbspBitReaderError::ReaderErrorFor(name, e) )
    }

    pub fn read_se(&mut self) -> Result<i32,bitreader::BitReaderError> {
        Ok(Self::golomb_to_signed(self.read_ue()?))
    }

    pub fn has_more_rbsp_data(&self) -> bool {
        self.position() < self.total_size as u64
    }

    fn golomb_to_signed(val: u32) -> i32 {
        let sign = (((val & 0x1) as i32) << 1) - 1;
        ((val >> 1) as i32 + (val & 0x1) as i32) * sign
    }
}
fn count_zero_bits(r: &mut bitreader::BitReader) -> Result<u8,bitreader::BitReaderError> {
    let mut count = 0;
    while !r.read_bool()? {
        count += 1;
        if count > 31 {
            return Err(bitreader::BitReaderError::TooManyBitsForType {
                position: r.position(),
                requested: 32,
                allowed: 31,
            })
        }
    }
    Ok(count)
}
impl<'a> Deref for RbspBitReader<'a> {
    type Target = bitreader::BitReader<'a>;

    fn deref(&self) -> &Self::Target {
        &self.reader
    }
}
impl<'a> DerefMut for RbspBitReader<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.reader
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use std::cell::RefCell;

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
        fn start(&mut self, ctx: &mut Context, header: &NalHeader) {
            self.state.borrow_mut().started = true;
        }

        fn push(&mut self, ctx: &mut Context, buf: &[u8]) {
            self.state.borrow_mut().data.extend_from_slice(buf);
        }

        fn end(&mut self, ctx: &mut Context) {
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
}