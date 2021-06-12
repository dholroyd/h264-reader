//! A reader for the NAL Unit framing format defined in _ITU-T Recommendation H.264 - Annex B_,
//! as used when H264 data is embedded in an MPEG2 Transport Stream

use crate::Context;
use memchr;
use log::*;

#[derive(Debug)]
enum ParseState {
    Start,
    StartOneZero,
    StartTwoZero,
    InUnitStart,
    InUnit,
    InUnitOneZero,
    InUnitTwoZero,
    Error,
    End,
}
impl ParseState {
    fn in_unit(&self) -> bool {
        match *self {
            ParseState::Start => false,
            ParseState::StartOneZero => false,
            ParseState::StartTwoZero => false,
            ParseState::InUnitStart => true,
            ParseState::InUnit => true,
            ParseState::InUnitOneZero => true,
            ParseState::InUnitTwoZero => true,
            ParseState::Error => false,
            ParseState::End => false,
        }
    }

    fn end_backtrack_bytes(&self) -> Option<usize> {
        match *self {
            ParseState::Start => None,
            ParseState::StartOneZero => None,
            ParseState::StartTwoZero => None,
            ParseState::InUnitStart => Some(0),
            ParseState::InUnit => Some(0),
            ParseState::InUnitOneZero => Some(1),
            ParseState::InUnitTwoZero => Some(2),
            ParseState::Error => None,
            ParseState::End => None,
        }
    }
}


pub trait NalReader {
    type Ctx;

    fn start(&mut self, ctx: &mut Context<Self::Ctx>);
    fn push(&mut self, ctx: &mut Context<Self::Ctx>, buf: &[u8]);
    fn end(&mut self, ctx: &mut Context<Self::Ctx>);
}

pub struct AnnexBReader<R, Ctx>
    where
        R: NalReader<Ctx=Ctx>
{
    state: ParseState,
    nal_reader: R,
}
impl<R, Ctx> AnnexBReader<R, Ctx>
    where
        R: NalReader<Ctx=Ctx>
{
    pub fn new(nal_reader: R) -> Self {
        AnnexBReader {
            state: ParseState::End,
            nal_reader,
        }
    }

    pub fn start(&mut self, ctx: &mut Context<Ctx>) {
        if self.state.in_unit() {
            // TODO: or reset()?
            self.nal_reader.end(ctx);
        }
        self.to(ParseState::Start);
    }

    pub fn push(&mut self, ctx: &mut Context<Ctx>, buf: &[u8]) {
        let mut unit_start: Option<isize> = self.state.end_backtrack_bytes().map(|v| -(v as isize));

        let mut i = 0;
        while i < buf.len() {
            let b = buf[i];
            match self.state {
                ParseState::End => {
                    error!("no previous call to start()");
                    self.state = ParseState::Error;
                    return;
                },
                ParseState::Error => return,
                ParseState::Start => {
                    match b {
                        0x00 => self.to(ParseState::StartOneZero),
                        _ => self.err(b),
                    }
                },
                ParseState::StartOneZero => {
                    match b {
                        0x00 => self.to(ParseState::StartTwoZero),
                        _ => self.err(b),
                    }
                },
                ParseState::StartTwoZero => {
                    match b {
                        0x00 => (),   // keep ignoring further 0x00 bytes
                        0x01 => {
                            self.to(ParseState::InUnitStart);
                            unit_start = Some(i as isize + 1);
                        },
                        _ => self.err(b),
                    }
                },
                ParseState::InUnitStart => {
                    self.nal_reader.start(ctx);
                    match b {
                        0x00 => self.to(ParseState::InUnitOneZero),
                        _ => self.to(ParseState::InUnit),
                    }
                },
                ParseState::InUnit => {
                    let remaining = &buf[i..];
                    match memchr::memchr(0x00, remaining) {
                        Some(pos) => {
                            self.to(ParseState::InUnitOneZero);
                            i += pos;
                        },
                        None => {
                            // skip to end
                            i = buf.len();
                        }
                    }
                },
                ParseState::InUnitOneZero => {
                    match b {
                        0x00 => self.to(ParseState::InUnitTwoZero),
                        _ => {
                            if i < 1 { self.emit_fake(ctx, 1) }
                            self.to(ParseState::InUnit)
                        },
                    }
                },
                ParseState::InUnitTwoZero => {
                    match b {
                        0x00 => {
                            if unit_start.is_some() && (unit_start.unwrap() > 0 || i > 2) {
                                self.emit(ctx, buf, unit_start, i - 2);
                            }
                            self.nal_reader.end(ctx);
                            unit_start = None;
                            self.to(ParseState::StartTwoZero);
                        },
                        0x01 => {
                            if unit_start.is_some() && (unit_start.unwrap() > 0 || i > 2) {
                                self.emit(ctx, buf, unit_start, i - 2);
                            }
                            self.nal_reader.end(ctx);
                            unit_start = Some(i as isize + 1);
                            self.to(ParseState::InUnitStart);
                        },
                        _ => {
                            if i < 2 { self.emit_fake(ctx, 2-i) }
                            self.to(ParseState::InUnit)
                        },
                    }
                },
            }
            i += 1;
        }
        if let (Some(start), Some(backtrack)) = (unit_start, self.state.end_backtrack_bytes()) {
            let adjusted_start = if start < 0 {
                0usize
            } else {
                start as usize
            };
            if buf.len() > backtrack {
                self.nal_reader.push(ctx, &buf[adjusted_start..buf.len() - backtrack])
            }
        }
    }

    /// To be invoked when calling code knows that the end of a sequence of NAL Unit data has been
    /// reached.
    ///
    /// For example, if the containing data structure demarcates the end of a sequence of NAL
    /// Units explicitly, the parser for that structure should call `end_units()` once all data
    /// has been passed to the `push()` function.
    pub fn end_units(&mut self, ctx: &mut Context<Ctx>) {
        if let Some(backtrack) = self.state.end_backtrack_bytes() {
            // if we were in the middle of parsing a sequence of 0x00 bytes that might have become
            // a start-code, but actually reached the end of input, then we will now need to emit
            // those 0x00 bytes that we had been holding back,
            if backtrack > 0 {
                self.emit_fake(ctx, backtrack);
            }
        }
        self.to(ParseState::End);
        self.nal_reader.end(ctx);
    }

    fn to(&mut self, new_state: ParseState) {
        self.state = new_state;
    }

    /// count must be 2 or less
    fn emit_fake(&mut self, ctx: &mut Context<Ctx>, count: usize) {
        let fake = [0u8; 2];
        self.nal_reader.push(ctx, &fake[..count]);
    }

    fn emit(&mut self, ctx: &mut Context<Ctx>, buf:&[u8], start_index: Option<isize>, end_index: usize) {
        if let Some(start) = start_index {
            let start = if start < 0 {
                0usize
            } else {
                start as usize
            };
            self.nal_reader.push(ctx, &buf[start..end_index])
        } else {
            error!("AnnexBReader: no start_index");
        }
    }

    fn err(&mut self, b: u8) {
        error!("AnnexBReader: state={:?}, invalid byte {:#x}", self.state, b);
        self.state = ParseState::Start;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use std::cell::RefCell;
    use hex_literal::*;

    struct State {
        started: u32,
        ended: u32,
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
        type Ctx = ();

        fn start(&mut self, _ctx: &mut Context<Self::Ctx>) {
            self.state.borrow_mut().started += 1;
        }

        fn push(&mut self, _ctx: &mut Context<Self::Ctx>, buf: &[u8]) {
            self.state.borrow_mut().data.extend_from_slice(buf);
        }

        fn end(&mut self, _ctx: &mut Context<Self::Ctx>) {
            self.state.borrow_mut().ended += 1;
        }
    }

    #[test]
    fn simple_nal() {
        let state = Rc::new(RefCell::new(State {
            started: 0,
            ended: 0,
            data: Vec::new(),
        }));
        let mock = MockReader::new(Rc::clone(&state));
        let mut r = AnnexBReader::new(mock);
        let data = vec!(
            0, 0, 0, 1,  // start-code
            3,           // NAL data
            0, 0, 1      // end-code
        );
        let mut ctx = Context::default();
        r.start(&mut ctx);
        r.push(&mut ctx, &data[..]);
        {
            let s = state.borrow();
            assert_eq!(1, s.started);
            assert_eq!(&s.data[..], &[3u8][..]);
            assert_eq!(1, s.ended);
        }
    }

    #[test]
    fn short_start_code() {
        let state = Rc::new(RefCell::new(State {
            started: 0,
            ended: 0,
            data: Vec::new(),
        }));
        let mock = MockReader::new(Rc::clone(&state));
        let mut r = AnnexBReader::new(mock);
        let data = vec!(
            0, 0, 1,  // start-code -- only three bytes rather than the usual 4
            3,        // NAL data
            0, 0, 1   // end-code
        );
        let mut ctx = Context::default();
        r.start(&mut ctx);
        r.push(&mut ctx, &data[..]);
        {
            let s = state.borrow();
            assert_eq!(1, s.started);
            assert_eq!(&s.data[..], &[3u8][..]);
            assert_eq!(1, s.ended);
        }
    }

    // Several trailing 0x00 0x00 0x03 bytes
    #[test]
    fn rbsp_cabac() {
        let state = Rc::new(RefCell::new(State {
            started: 0,
            ended: 0,
            data: Vec::new(),
        }));
        let mock = MockReader::new(Rc::clone(&state));
        let mut r = AnnexBReader::new(mock);
        let data = vec!(
            0, 0, 0, 1,  // start-code
            3,           // NAL data
            0x80,        // 1 stop-bit + 7 alignment-zero-bits
            0, 0, 3,     // cabac_zero_word + emulation_prevention_three_byte
            0, 0, 3,     // cabac_zero_word + emulation_prevention_three_byte
            0, 0, 0, 1,  // start-code
        );
        let mut ctx = Context::default();
        r.start(&mut ctx);
        r.push(&mut ctx, &data[..]);
        {
            let s = state.borrow();
            assert_eq!(1, s.started);
            assert_eq!(&s.data[..], &[3, 0x80, 0, 0, 3, 0, 0, 3][..]);
            assert_eq!(1, s.ended);
        }
    }

    // Several trailing 0x00 bytes
    #[test]
    fn trailing_zero() {
        let state = Rc::new(RefCell::new(State {
            started: 0,
            ended: 0,
            data: Vec::new(),
        }));
        let mock = MockReader::new(Rc::clone(&state));
        let mut r = AnnexBReader::new(mock);
        let data = vec!(
            0, 0, 0, 1,  // start-code
            3,           // NAL data
            0x80,        // 1 stop-bit + 7 alignment-zero-bits
            0,           // trailing_zero_8bits
            0,           // trailing_zero_8bits
            0, 0, 0, 1,  // start-code
        );
        let mut ctx = Context::default();
        r.start(&mut ctx);
        r.push(&mut ctx, &data[..]);
        {
            let s = state.borrow();
            assert_eq!(1, s.started);
            assert_eq!(&s.data[..], &[3, 0x80][..]);
            assert_eq!(1, s.ended);
        }
    }

    // If there's bad data after a trailing zero, the parser recovers after the next start code.
    #[test]
    fn recovery_on_corrupt_trailing_zero() {
        let state = Rc::new(RefCell::new(State {
            started: 0,
            ended: 0,
            data: Vec::new(),
        }));
        let mock = MockReader::new(Rc::clone(&state));
        let mut r = AnnexBReader::new(mock);
        let data = vec!(
            0, 0, 0, 1,  // start-code
            3,           // NAL data
            0x80,        // 1 stop-bit + 7 alignment-zero-bits
            0, 0, 0,     // trailing_zero_8bits
            42,          // unexpected byte
            0, 0, 1,     // start-code
            2, 3,        // NAL data
            0x80,        // 1 stop-bit + 7 alignment-zero-bits
            0, 0, 1,     // start-code
        );
        let mut ctx = Context::default();
        r.start(&mut ctx);
        r.push(&mut ctx, &data[..]);
        {
            let s = state.borrow();
            assert_eq!(2, s.started);
            assert_eq!(&s.data[..], &[3, 0x80, 2, 3, 0x80][..]);
            assert_eq!(2, s.ended);
        }
    }

    #[test]
    fn implicit_end() {
        let state = Rc::new(RefCell::new(State {
            started: 0,
            ended: 0,
            data: Vec::new(),
        }));
        let mock = MockReader::new(Rc::clone(&state));
        let mut r = AnnexBReader::new(mock);
        let data = vec!(
            0, 0, 0, 1,  // start-code
            3, 0         // NAL data
        );
        let mut ctx = Context::default();
        r.start(&mut ctx);
        r.push(&mut ctx, &data[..]);
        r.end_units(&mut ctx);
        {
            let s = state.borrow();
            assert_eq!(1, s.started);
            assert_eq!(&s.data[..], &[3u8, 0u8][..]);
            assert_eq!(1, s.ended);
        }
    }

    #[test]
    fn split_nal() {
        let state = Rc::new(RefCell::new(State {
            started: 0,
            ended: 0,
            data: Vec::new(),
        }));
        let mock = MockReader::new(Rc::clone(&state));
        let mut r = AnnexBReader::new(mock);
        let data = vec!(
            0, 0, 0, 1,  // start-code
            2, 3,        // NAL data
            0, 0, 1      // nd-code
        );
        let mut ctx = Context::default();
        r.start(&mut ctx);
        r.push(&mut ctx, &data[..5]);  // half-way through the NAL Unit
        {
            let s = state.borrow();
            assert_eq!(1, s.started);
            assert_eq!(&s.data[..], &[2u8][..]);
            assert_eq!(0, s.ended);
        }
        r.push(&mut ctx, &data[5..]);  // second half of the NAL Unit
        {
            let s = state.borrow();
            assert_eq!(1, s.started);
            assert_eq!(&s.data[..], &[2u8, 3u8][..]);
            assert_eq!(1, s.ended);
        }
    }

    #[test]
    fn split_large() {
        let data = hex!(
           "00 00 00 01 67 64 00 0A AC 72 84 44 26 84 00 00
            03 00 04 00 00 03 00 CA 3C 48 96 11 80 00 00 00
            01 68 E8 43 8F 13 21 30 00 00 01 65 88 81 00 05
            4E 7F 87 DF 61 A5 8B 95 EE A4 E9 38 B7 6A 30 6A
            71 B9 55 60 0B 76 2E B5 0E E4 80 59 27 B8 67 A9
            63 37 5E 82 20 55 FB E4 6A E9 37 35 72 E2 22 91
            9E 4D FF 60 86 CE 7E 42 B7 95 CE 2A E1 26 BE 87
            73 84 26 BA 16 36 F4 E6 9F 17 DA D8 64 75 54 B1
            F3 45 0C 0B 3C 74 B3 9D BC EB 53 73 87 C3 0E 62
            47 48 62 CA 59 EB 86 3F 3A FA 86 B5 BF A8 6D 06
            16 50 82 C4 CE 62 9E 4E E6 4C C7 30 3E DE A1 0B
            D8 83 0B B6 B8 28 BC A9 EB 77 43 FC 7A 17 94 85
            21 CA 37 6B 30 95 B5 46 77 30 60 B7 12 D6 8C C5
            54 85 29 D8 69 A9 6F 12 4E 71 DF E3 E2 B1 6B 6B
            BF 9F FB 2E 57 30 A9 69 76 C4 46 A2 DF FA 91 D9
            50 74 55 1D 49 04 5A 1C D6 86 68 7C B6 61 48 6C
            96 E6 12 4C 27 AD BA C7 51 99 8E D0 F0 ED 8E F6
            65 79 79 A6 12 A1 95 DB C8 AE E3 B6 35 E6 8D BC
            48 A3 7F AF 4A 28 8A 53 E2 7E 68 08 9F 67 77 98
            52 DB 50 84 D6 5E 25 E1 4A 99 58 34 C7 11 D6 43
            FF C4 FD 9A 44 16 D1 B2 FB 02 DB A1 89 69 34 C2
            32 55 98 F9 9B B2 31 3F 49 59 0C 06 8C DB A5 B2
            9D 7E 12 2F D0 87 94 44 E4 0A 76 EF 99 2D 91 18
            39 50 3B 29 3B F5 2C 97 73 48 91 83 B0 A6 F3 4B
            70 2F 1C 8F 3B 78 23 C6 AA 86 46 43 1D D7 2A 23
            5E 2C D9 48 0A F5 F5 2C D1 FB 3F F0 4B 78 37 E9
            45 DD 72 CF 80 35 C3 95 07 F3 D9 06 E5 4A 58 76
            03 6C 81 20 62 45 65 44 73 BC FE C1 9F 31 E5 DB
            89 5C 6B 79 D8 68 90 D7 26 A8 A1 88 86 81 DC 9A
            4F 40 A5 23 C7 DE BE 6F 76 AB 79 16 51 21 67 83
            2E F3 D6 27 1A 42 C2 94 D1 5D 6C DB 4A 7A E2 CB
            0B B0 68 0B BE 19 59 00 50 FC C0 BD 9D F5 F5 F8
            A8 17 19 D6 B3 E9 74 BA 50 E5 2C 45 7B F9 93 EA
            5A F9 A9 30 B1 6F 5B 36 24 1E 8D 55 57 F4 CC 67
            B2 65 6A A9 36 26 D0 06 B8 E2 E3 73 8B D1 C0 1C
            52 15 CA B5 AC 60 3E 36 42 F1 2C BD 99 77 AB A8
            A9 A4 8E 9C 8B 84 DE 73 F0 91 29 97 AE DB AF D6
            F8 5E 9B 86 B3 B3 03 B3 AC 75 6F A6 11 69 2F 3D
            3A CE FA 53 86 60 95 6C BB C5 4E F3");
        let expected = hex!(
           "67 64 00 0A AC 72 84 44 26 84 00 00
            03 00 04 00 00 03 00 CA 3C 48 96 11 80
            68 E8 43 8F 13 21 30 65 88 81 00 05
            4E 7F 87 DF 61 A5 8B 95 EE A4 E9 38 B7 6A 30 6A
            71 B9 55 60 0B 76 2E B5 0E E4 80 59 27 B8 67 A9
            63 37 5E 82 20 55 FB E4 6A E9 37 35 72 E2 22 91
            9E 4D FF 60 86 CE 7E 42 B7 95 CE 2A E1 26 BE 87
            73 84 26 BA 16 36 F4 E6 9F 17 DA D8 64 75 54 B1
            F3 45 0C 0B 3C 74 B3 9D BC EB 53 73 87 C3 0E 62
            47 48 62 CA 59 EB 86 3F 3A FA 86 B5 BF A8 6D 06
            16 50 82 C4 CE 62 9E 4E E6 4C C7 30 3E DE A1 0B
            D8 83 0B B6 B8 28 BC A9 EB 77 43 FC 7A 17 94 85
            21 CA 37 6B 30 95 B5 46 77 30 60 B7 12 D6 8C C5
            54 85 29 D8 69 A9 6F 12 4E 71 DF E3 E2 B1 6B 6B
            BF 9F FB 2E 57 30 A9 69 76 C4 46 A2 DF FA 91 D9
            50 74 55 1D 49 04 5A 1C D6 86 68 7C B6 61 48 6C
            96 E6 12 4C 27 AD BA C7 51 99 8E D0 F0 ED 8E F6
            65 79 79 A6 12 A1 95 DB C8 AE E3 B6 35 E6 8D BC
            48 A3 7F AF 4A 28 8A 53 E2 7E 68 08 9F 67 77 98
            52 DB 50 84 D6 5E 25 E1 4A 99 58 34 C7 11 D6 43
            FF C4 FD 9A 44 16 D1 B2 FB 02 DB A1 89 69 34 C2
            32 55 98 F9 9B B2 31 3F 49 59 0C 06 8C DB A5 B2
            9D 7E 12 2F D0 87 94 44 E4 0A 76 EF 99 2D 91 18
            39 50 3B 29 3B F5 2C 97 73 48 91 83 B0 A6 F3 4B
            70 2F 1C 8F 3B 78 23 C6 AA 86 46 43 1D D7 2A 23
            5E 2C D9 48 0A F5 F5 2C D1 FB 3F F0 4B 78 37 E9
            45 DD 72 CF 80 35 C3 95 07 F3 D9 06 E5 4A 58 76
            03 6C 81 20 62 45 65 44 73 BC FE C1 9F 31 E5 DB
            89 5C 6B 79 D8 68 90 D7 26 A8 A1 88 86 81 DC 9A
            4F 40 A5 23 C7 DE BE 6F 76 AB 79 16 51 21 67 83
            2E F3 D6 27 1A 42 C2 94 D1 5D 6C DB 4A 7A E2 CB
            0B B0 68 0B BE 19 59 00 50 FC C0 BD 9D F5 F5 F8
            A8 17 19 D6 B3 E9 74 BA 50 E5 2C 45 7B F9 93 EA
            5A F9 A9 30 B1 6F 5B 36 24 1E 8D 55 57 F4 CC 67
            B2 65 6A A9 36 26 D0 06 B8 E2 E3 73 8B D1 C0 1C
            52 15 CA B5 AC 60 3E 36 42 F1 2C BD 99 77 AB A8
            A9 A4 8E 9C 8B 84 DE 73 F0 91 29 97 AE DB AF D6
            F8 5E 9B 86 B3 B3 03 B3 AC 75 6F A6 11 69 2F 3D
            3A CE FA 53 86 60 95 6C BB C5 4E F3");
        for i in 1..data.len()-1 {
            let state = Rc::new(RefCell::new(State {
                started: 0,
                ended: 0,
                data: Vec::new(),
            }));
            let mock = MockReader::new(Rc::clone(&state));
            let mut r = AnnexBReader::new(mock);
            let mut ctx = Context::default();
            let (head, tail) = data.split_at(i);
            r.start(&mut ctx);
            r.push(&mut ctx, &head[..]);
            r.push(&mut ctx, &tail[..]);
            r.end_units(&mut ctx);
            assert_eq!(3, state.borrow().started);
            assert_eq!(3, state.borrow().ended);
            assert_eq!(&state.borrow().data[..], &expected[..]);
        }
    }
    #[test]
    fn onebyte_large() {
        let data = hex!(
           "00 00 00 01 67 64 00 0A AC 72 84 44 26 84 00 00
            03 00 04 00 00 03 00 CA 3C 48 96 11 80 00 00 00
            01 68 E8 43 8F 13 21 30 00 00 01 65 88 81 00 05
            4E 7F 87 DF 61 A5 8B 95 EE A4 E9 38 B7 6A 30 6A
            71 B9 55 60 0B 76 2E B5 0E E4 80 59 27 B8 67 A9
            63 37 5E 82 20 55 FB E4 6A E9 37 35 72 E2 22 91
            9E 4D FF 60 86 CE 7E 42 B7 95 CE 2A E1 26 BE 87
            73 84 26 BA 16 36 F4 E6 9F 17 DA D8 64 75 54 B1
            F3 45 0C 0B 3C 74 B3 9D BC EB 53 73 87 C3 0E 62
            47 48 62 CA 59 EB 86 3F 3A FA 86 B5 BF A8 6D 06
            16 50 82 C4 CE 62 9E 4E E6 4C C7 30 3E DE A1 0B
            D8 83 0B B6 B8 28 BC A9 EB 77 43 FC 7A 17 94 85
            21 CA 37 6B 30 95 B5 46 77 30 60 B7 12 D6 8C C5
            54 85 29 D8 69 A9 6F 12 4E 71 DF E3 E2 B1 6B 6B
            BF 9F FB 2E 57 30 A9 69 76 C4 46 A2 DF FA 91 D9
            50 74 55 1D 49 04 5A 1C D6 86 68 7C B6 61 48 6C
            96 E6 12 4C 27 AD BA C7 51 99 8E D0 F0 ED 8E F6
            65 79 79 A6 12 A1 95 DB C8 AE E3 B6 35 E6 8D BC
            48 A3 7F AF 4A 28 8A 53 E2 7E 68 08 9F 67 77 98
            52 DB 50 84 D6 5E 25 E1 4A 99 58 34 C7 11 D6 43
            FF C4 FD 9A 44 16 D1 B2 FB 02 DB A1 89 69 34 C2
            32 55 98 F9 9B B2 31 3F 49 59 0C 06 8C DB A5 B2
            9D 7E 12 2F D0 87 94 44 E4 0A 76 EF 99 2D 91 18
            39 50 3B 29 3B F5 2C 97 73 48 91 83 B0 A6 F3 4B
            70 2F 1C 8F 3B 78 23 C6 AA 86 46 43 1D D7 2A 23
            5E 2C D9 48 0A F5 F5 2C D1 FB 3F F0 4B 78 37 E9
            45 DD 72 CF 80 35 C3 95 07 F3 D9 06 E5 4A 58 76
            03 6C 81 20 62 45 65 44 73 BC FE C1 9F 31 E5 DB
            89 5C 6B 79 D8 68 90 D7 26 A8 A1 88 86 81 DC 9A
            4F 40 A5 23 C7 DE BE 6F 76 AB 79 16 51 21 67 83
            2E F3 D6 27 1A 42 C2 94 D1 5D 6C DB 4A 7A E2 CB
            0B B0 68 0B BE 19 59 00 50 FC C0 BD 9D F5 F5 F8
            A8 17 19 D6 B3 E9 74 BA 50 E5 2C 45 7B F9 93 EA
            5A F9 A9 30 B1 6F 5B 36 24 1E 8D 55 57 F4 CC 67
            B2 65 6A A9 36 26 D0 06 B8 E2 E3 73 8B D1 C0 1C
            52 15 CA B5 AC 60 3E 36 42 F1 2C BD 99 77 AB A8
            A9 A4 8E 9C 8B 84 DE 73 F0 91 29 97 AE DB AF D6
            F8 5E 9B 86 B3 B3 03 B3 AC 75 6F A6 11 69 2F 3D
            3A CE FA 53 86 60 95 6C BB C5 4E F3");
        let expected = hex!(
           "67 64 00 0A AC 72 84 44 26 84 00 00
            03 00 04 00 00 03 00 CA 3C 48 96 11 80
            68 E8 43 8F 13 21 30 65 88 81 00 05
            4E 7F 87 DF 61 A5 8B 95 EE A4 E9 38 B7 6A 30 6A
            71 B9 55 60 0B 76 2E B5 0E E4 80 59 27 B8 67 A9
            63 37 5E 82 20 55 FB E4 6A E9 37 35 72 E2 22 91
            9E 4D FF 60 86 CE 7E 42 B7 95 CE 2A E1 26 BE 87
            73 84 26 BA 16 36 F4 E6 9F 17 DA D8 64 75 54 B1
            F3 45 0C 0B 3C 74 B3 9D BC EB 53 73 87 C3 0E 62
            47 48 62 CA 59 EB 86 3F 3A FA 86 B5 BF A8 6D 06
            16 50 82 C4 CE 62 9E 4E E6 4C C7 30 3E DE A1 0B
            D8 83 0B B6 B8 28 BC A9 EB 77 43 FC 7A 17 94 85
            21 CA 37 6B 30 95 B5 46 77 30 60 B7 12 D6 8C C5
            54 85 29 D8 69 A9 6F 12 4E 71 DF E3 E2 B1 6B 6B
            BF 9F FB 2E 57 30 A9 69 76 C4 46 A2 DF FA 91 D9
            50 74 55 1D 49 04 5A 1C D6 86 68 7C B6 61 48 6C
            96 E6 12 4C 27 AD BA C7 51 99 8E D0 F0 ED 8E F6
            65 79 79 A6 12 A1 95 DB C8 AE E3 B6 35 E6 8D BC
            48 A3 7F AF 4A 28 8A 53 E2 7E 68 08 9F 67 77 98
            52 DB 50 84 D6 5E 25 E1 4A 99 58 34 C7 11 D6 43
            FF C4 FD 9A 44 16 D1 B2 FB 02 DB A1 89 69 34 C2
            32 55 98 F9 9B B2 31 3F 49 59 0C 06 8C DB A5 B2
            9D 7E 12 2F D0 87 94 44 E4 0A 76 EF 99 2D 91 18
            39 50 3B 29 3B F5 2C 97 73 48 91 83 B0 A6 F3 4B
            70 2F 1C 8F 3B 78 23 C6 AA 86 46 43 1D D7 2A 23
            5E 2C D9 48 0A F5 F5 2C D1 FB 3F F0 4B 78 37 E9
            45 DD 72 CF 80 35 C3 95 07 F3 D9 06 E5 4A 58 76
            03 6C 81 20 62 45 65 44 73 BC FE C1 9F 31 E5 DB
            89 5C 6B 79 D8 68 90 D7 26 A8 A1 88 86 81 DC 9A
            4F 40 A5 23 C7 DE BE 6F 76 AB 79 16 51 21 67 83
            2E F3 D6 27 1A 42 C2 94 D1 5D 6C DB 4A 7A E2 CB
            0B B0 68 0B BE 19 59 00 50 FC C0 BD 9D F5 F5 F8
            A8 17 19 D6 B3 E9 74 BA 50 E5 2C 45 7B F9 93 EA
            5A F9 A9 30 B1 6F 5B 36 24 1E 8D 55 57 F4 CC 67
            B2 65 6A A9 36 26 D0 06 B8 E2 E3 73 8B D1 C0 1C
            52 15 CA B5 AC 60 3E 36 42 F1 2C BD 99 77 AB A8
            A9 A4 8E 9C 8B 84 DE 73 F0 91 29 97 AE DB AF D6
            F8 5E 9B 86 B3 B3 03 B3 AC 75 6F A6 11 69 2F 3D
            3A CE FA 53 86 60 95 6C BB C5 4E F3");
        let state = Rc::new(RefCell::new(State {
            started: 0,
            ended: 0,
            data: Vec::new(),
        }));
        let mock = MockReader::new(Rc::clone(&state));
        let mut r = AnnexBReader::new(mock);
        let mut ctx = Context::default();
        r.start(&mut ctx);
        for i in 0..data.len() {
            r.push(&mut ctx, &data[i..i+1]);
        }
        r.end_units(&mut ctx);
        assert_eq!(3, state.borrow().started);
        assert_eq!(3, state.borrow().ended);
        assert_eq!(&state.borrow().data[..], &expected[..]);
    }
}
