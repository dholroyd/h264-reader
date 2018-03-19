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
use ::annexb::NalReader;

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
        R: NalReader
{
    state: ParseState,
    nal_reader: R,
}
impl<R> RbspDecoder<R>
    where
        R: NalReader
{
    fn new(nal_reader: R) -> RbspDecoder<R> {
        RbspDecoder {
            state: ParseState::Start,
            nal_reader,
        }
    }

    fn to(&mut self, new_state: ParseState) {
        self.state = new_state;
    }

    fn emit(&mut self, buf:&[u8], start_index: Option<usize>, end_index: usize) {
        //println!("emit {:?}", &buf[start_index.unwrap()..end_index]);
        if let Some(start) = start_index {
            self.nal_reader.push(&buf[start..end_index])
        } else {
            eprintln!("RbspDecoder: no start_index");
        }
    }

    fn err(&mut self, b: u8) {
        eprintln!("RbspDecoder: state={:?}, invalid byte {:#x}", self.state, b);
        self.state = ParseState::Start;
    }
}
impl<R> NalReader for RbspDecoder<R>
    where
        R: NalReader
{
    fn start(&mut self) {
        unimplemented!()
    }

    fn push(&mut self, buf: &[u8]) {
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
                        _ => self.to(ParseState::Start),
                    }
                },
                ParseState::TwoZero => {
                    match b {
                        0x03 => {
                            // found an 'emulation prevention' byte; skip it,
                            self.emit(buf, rbsp_start, i);
                            self.nal_reader.end();
                            rbsp_start = Some(i + 1);
                            // TODO: per spec, the next byte should be either 0x00, 0x1, 0x02 or
                            // 0x03, but at the moment we assume this without checking for
                            // correctness
                            self.to(ParseState::Start);
                        },
                        0x00 => self.err(b),
                        _ => self.to(ParseState::Start),
                    }
                },
            }
        }
        if let Some(start) = rbsp_start {
            self.nal_reader.push(&buf[start..buf.len()-self.state.end_backtrack_bytes()])
        }
    }

    /// To be invoked when calling code knows that the end of a sequence of NAL Unit data has been
    /// reached.
    ///
    /// For example, if the containing data structure demarcates the end of a sequence of NAL
    /// Units explicitly, the parser for that structure should call `end_units()` once all data
    /// has been passed to the `push()` function.
    fn end(&mut self) {
        let backtrack = self.state.end_backtrack_bytes();
        if backtrack > 0 {
            // if we were in the middle of parsing a sequence of 0x00 bytes that might have become
            // a start-code, but actually reached the end of input, then we will now need to emit
            // those 0x00 bytes that we had been holding back,
            let tmp = [0u8; 3];
            self.nal_reader.push(&tmp[0..backtrack]);
        }
        self.to(ParseState::Start);
        self.nal_reader.end();
    }
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
    while !r.read_bool()? && count < 31 {
        count += 1;
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
    impl NalReader for MockReader {
        fn start(&mut self) {
            self.state.borrow_mut().started = true;
        }

        fn push(&mut self, buf: &[u8]) {
            self.state.borrow_mut().data.extend_from_slice(buf);
        }

        fn end(&mut self) {
            self.state.borrow_mut().ended = true;
        }
    }

    #[test]
    fn it_works() {
        let data = hex!(
           "67 64 00 0A AC 72 84 44 26 84 00 00 03
            00 04 00 00 03 00 CA 3C 48 96 11 80");
        let state = Rc::new(RefCell::new(State {
            started: false,
            ended: false,
            data: Vec::new(),
        }));
        let mock = MockReader::new(Rc::clone(&state));
        let mut r = RbspDecoder::new(mock);
        r.push(&data[..]);
        let expected = hex!(
           "67 64 00 0A AC 72 84 44 26 84 00 00
            00 04 00 00 00 CA 3C 48 96 11 80");
        let s = state.borrow();
        assert_eq!(s.data[..], expected);
    }
}