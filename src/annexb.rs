//! A reader for the NAL Unit framing format defined in _ITU-T Recommendation H.264 - Annex B_,
//! as used when H264 data is embedded in an MPEG2 Transport Stream

use log::*;
use memchr;

use crate::push::{AccumulatedNalHandler, NalAccumulator, NalFragmentHandler};

/// The current state, named for the most recently examined byte.
#[derive(Debug)]
enum ParseState {
    Start,
    StartOneZero,
    StartTwoZero,
    InUnit,
    InUnitOneZero,
    InUnitTwoZero,
}
impl ParseState {
    /// If in a NAL unit (`NalReader`'s `start` has been called but not its `end`),
    /// returns an object describing the state.
    fn in_unit(&self) -> Option<InUnitState> {
        match *self {
            ParseState::Start => None,
            ParseState::StartOneZero => None,
            ParseState::StartTwoZero => None,
            ParseState::InUnit => Some(InUnitState { backtrack_bytes: 0 }),
            ParseState::InUnitOneZero => Some(InUnitState { backtrack_bytes: 1 }),
            ParseState::InUnitTwoZero => Some(InUnitState { backtrack_bytes: 2 }),
        }
    }
}

struct InUnitState {
    /// The number of bytes to backtrack if the current sequence of `0x00`s
    /// doesn't end the NAL unit.
    backtrack_bytes: usize,
}

/// Push parser for Annex B format which delegates to a [NalFragmentHandler], most commonly a
/// [NalAccumulator]:
///
/// ```
/// use h264_reader::annexb::AnnexBReader;
/// use h264_reader::nal::{Nal, RefNal, UnitType};
/// use h264_reader::push::NalInterest;
///
/// let mut calls = Vec::new();
/// let mut reader = AnnexBReader::accumulate(|nal: RefNal<'_>| {
///     let nal_unit_type = nal.header().unwrap().nal_unit_type();
///     calls.push((nal_unit_type, nal.is_complete()));
///     match nal_unit_type {
///         UnitType::SeqParameterSet => NalInterest::Buffer,
///         _ => NalInterest::Ignore,
///     }
/// });
///
/// // Push a couple NALs. Pushes don't have to match up to Annex B framing.
/// reader.push(&b"\x00\x00"[..]);
/// reader.push(&b"\x01\x67\x64\x00\x0A\xAC\x72\x84\x44\x26\x84\x00\x00"[..]);
/// reader.push(&b"\x03\x00\x04\x00\x00\x03\x00\xCA\x3C\x48\x96\x11\x80\x00\x00\x01"[..]);
/// reader.push(&b"\x68"[..]);
/// reader.push(&b"\xE8\x43\x8F\x13\x21\x30"[..]);
///
/// assert_eq!(calls, &[
///     (UnitType::SeqParameterSet, false),
///     (UnitType::SeqParameterSet, true),
///     (UnitType::PicParameterSet, false),
///     // no second call on the PicParameterSet because the handler returned Ignore.
/// ]);
/// ```
///
/// See [NalAccumulator] for an example with a handler that *owns* state.
///
/// When corruption is detected, the `AnnexbReader` logs error and recovers on
/// the next start code boundary.
///
/// Guarantees that the bytes supplied to [`NalFragmentHandler`]—the concatenation of all
/// `buf`s supplied to `NalFragmentHandler::nal_fragment`—will be exactly the same for a given
/// Annex B stream, regardless of boundaries of `AnnexBReader::push` calls.
pub struct AnnexBReader<H: NalFragmentHandler> {
    state: ParseState,
    inner: H,
}
impl<H: AccumulatedNalHandler> AnnexBReader<NalAccumulator<H>> {
    /// Constructs an `AnnexBReader` with a `NalAccumulator`.
    pub fn accumulate(inner: H) -> Self {
        Self::for_fragment_handler(NalAccumulator::new(inner))
    }

    /// Gets a reference to the underlying [AccumulatedNalHandler].
    pub fn nal_handler_ref(&self) -> &H {
        self.inner.handler()
    }

    /// Gets a mutable reference to the underlying [AccumulatedNalHandler].
    pub fn nal_handler_mut(&mut self) -> &mut H {
        self.inner.handler_mut()
    }

    /// Unwraps the `AnnexBReader<H>`, returning the inner [AccumulatedNalHandler].
    pub fn into_nal_handler(self) -> H {
        self.inner.into_handler()
    }
}
impl<H: NalFragmentHandler> AnnexBReader<H> {
    /// Constructs an `AnnexBReader` with a custom [`NalFragmentHandler`].
    pub fn for_fragment_handler(inner: H) -> Self {
        AnnexBReader {
            state: ParseState::Start,
            inner,
        }
    }

    /// Gets a reference to the underlying [NalFragmentHandler].
    pub fn fragment_handler_ref(&self) -> &H {
        &self.inner
    }

    /// Gets a mutable reference to the underlying [NalFragmentHandler].
    pub fn fragment_handler_mut(&mut self) -> &mut H {
        &mut self.inner
    }

    /// Unwraps the `AnnexBReader<H>`, returning the inner [NalFragmentHandler].
    pub fn into_fragment_handler(self) -> H {
        self.inner
    }

    pub fn push(&mut self, buf: &[u8]) {
        // When in a NAL unit, start is the first index in buf with a byte to
        // be pushed. Note that due to backtracking, sometimes 0x00 bytes
        // must be pushed that logically precede buf.
        let mut fake_and_start = self.state.in_unit().map(|s| (s.backtrack_bytes, 0));

        let mut i = 0;
        while i < buf.len() {
            debug_assert!(fake_and_start.is_some() == self.state.in_unit().is_some());
            let b = buf[i];
            match self.state {
                ParseState::Start => match b {
                    0x00 => self.to(ParseState::StartOneZero),
                    _ => self.err(b),
                },
                ParseState::StartOneZero => match b {
                    0x00 => self.to(ParseState::StartTwoZero),
                    _ => self.err(b),
                },
                ParseState::StartTwoZero => {
                    match b {
                        0x00 => (), // keep ignoring further 0x00 bytes
                        0x01 => {
                            fake_and_start = Some((0, i + 1));
                            self.to(ParseState::InUnit);
                        }
                        _ => self.err(b),
                    }
                }
                ParseState::InUnit => {
                    let remaining = &buf[i..];
                    match memchr::memchr(0x00, remaining) {
                        Some(pos) => {
                            self.to(ParseState::InUnitOneZero);
                            i += pos;
                        }
                        None => {
                            // skip to end
                            i = buf.len();
                        }
                    }
                }
                ParseState::InUnitOneZero => match b {
                    0x00 => self.to(ParseState::InUnitTwoZero),
                    _ => self.to(ParseState::InUnit),
                },
                ParseState::InUnitTwoZero => match b {
                    0x00 => {
                        self.maybe_emit(buf, fake_and_start, i, 2, true);
                        fake_and_start = None;
                        self.to(ParseState::StartTwoZero);
                    }
                    0x01 => {
                        self.maybe_emit(buf, fake_and_start, i, 2, true);
                        fake_and_start = Some((0, i + 1));
                        self.to(ParseState::InUnit);
                    }
                    _ => self.to(ParseState::InUnit),
                },
            }
            i += 1;
        }
        if let Some(in_unit) = self.state.in_unit() {
            self.maybe_emit(
                buf,
                fake_and_start,
                buf.len(),
                in_unit.backtrack_bytes,
                false,
            );
        }
    }

    /// To be invoked when calling code knows that the end of a sequence of NAL Unit data has been
    /// reached.
    ///
    /// For example, if the containing data structure demarcates the end of a sequence of NAL
    /// Units explicitly, the parser for that structure should call `end_units()` once all data
    /// has been passed to the `push()` function.
    pub fn reset(&mut self) {
        if let Some(in_unit) = self.state.in_unit() {
            // if we were in the middle of parsing a sequence of 0x00 bytes that might have become
            // a start-code, but actually reached the end of input, then we will now need to emit
            // those 0x00 bytes that we had been holding back,
            if in_unit.backtrack_bytes > 0 {
                self.inner
                    .nal_fragment(&[&[0u8; 2][..in_unit.backtrack_bytes]], true);
            } else {
                self.inner.nal_fragment(&[], true);
            }
        }
        self.to(ParseState::Start);
    }

    fn to(&mut self, new_state: ParseState) {
        self.state = new_state;
    }

    fn maybe_emit(
        &mut self,
        buf: &[u8],
        fake_and_start: Option<(usize, usize)>,
        end: usize,
        backtrack: usize,
        is_end: bool,
    ) {
        match fake_and_start {
            Some((fake, start)) if start + backtrack < end => {
                if fake > 0 {
                    self.inner.nal_fragment(
                        &[&[0u8; 2][..fake], &buf[start..end - backtrack]][..],
                        is_end,
                    );
                } else {
                    self.inner
                        .nal_fragment(&[&buf[start..end - backtrack]][..], is_end);
                };
            }
            Some(_) if is_end => self.inner.nal_fragment(&[], true),
            _ => {}
        }
    }

    fn err(&mut self, b: u8) {
        error!(
            "AnnexBReader: state={:?}, invalid byte {:#x}",
            self.state, b
        );
        self.state = ParseState::Start;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex_literal::*;

    #[derive(Default)]
    struct MockFragmentHandler {
        ended: u32,
        data: Vec<u8>,
    }
    impl NalFragmentHandler for MockFragmentHandler {
        fn nal_fragment(&mut self, bufs: &[&[u8]], end: bool) {
            assert!(!bufs.is_empty() || end);
            for buf in bufs {
                self.data.extend_from_slice(buf);
            }
            if end {
                self.ended += 1;
            }
        }
    }

    #[test]
    fn simple_nal() {
        let mock = MockFragmentHandler::default();
        let mut r = AnnexBReader::for_fragment_handler(mock);
        let data = vec![
            0, 0, 0, 1, // start-code
            3, // NAL data
            0, 0, 1, // end-code
        ];
        r.push(&data[..]);
        let mock = r.into_fragment_handler();
        assert_eq!(&mock.data[..], &[3u8][..]);
        assert_eq!(1, mock.ended);
    }

    #[test]
    fn short_start_code() {
        let mock = MockFragmentHandler::default();
        let mut r = AnnexBReader::for_fragment_handler(mock);
        let data = vec![
            0, 0, 1, // start-code -- only three bytes rather than the usual 4
            3, // NAL data
            0, 0, 1, // end-code
        ];
        r.push(&data[..]);
        let mock = r.into_fragment_handler();
        assert_eq!(&mock.data[..], &[3u8][..]);
        assert_eq!(1, mock.ended);
    }

    // Several trailing 0x00 0x00 0x03 bytes
    #[test]
    fn rbsp_cabac() {
        let mock = MockFragmentHandler::default();
        let mut r = AnnexBReader::for_fragment_handler(mock);
        let data = vec![
            0, 0, 0, 1,    // start-code
            3,    // NAL data
            0x80, // 1 stop-bit + 7 alignment-zero-bits
            0, 0, 3, // cabac_zero_word + emulation_prevention_three_byte
            0, 0, 3, // cabac_zero_word + emulation_prevention_three_byte
            0, 0, 0, 1, // start-code
        ];
        r.push(&data[..]);
        let mock = r.into_fragment_handler();
        assert_eq!(&mock.data[..], &[3, 0x80, 0, 0, 3, 0, 0, 3][..]);
        assert_eq!(1, mock.ended);
    }

    // Several trailing 0x00 bytes
    #[test]
    fn trailing_zero() {
        let mock = MockFragmentHandler::default();
        let mut r = AnnexBReader::for_fragment_handler(mock);
        let data = vec![
            0, 0, 0, 1,    // start-code
            3,    // NAL data
            0x80, // 1 stop-bit + 7 alignment-zero-bits
            0,    // trailing_zero_8bits
            0,    // trailing_zero_8bits
            0, 0, 0, 1, // start-code
        ];
        r.push(&data[..]);
        let mock = r.into_fragment_handler();
        assert_eq!(&mock.data[..], &[3, 0x80][..]);
        assert_eq!(1, mock.ended);
    }

    // If there's bad data after a trailing zero, the parser recovers after the next start code.
    #[test]
    fn recovery_on_corrupt_trailing_zero() {
        let mock = MockFragmentHandler::default();
        let mut r = AnnexBReader::for_fragment_handler(mock);
        let data = vec![
            0, 0, 0, 1,    // start-code
            3,    // NAL data
            0x80, // 1 stop-bit + 7 alignment-zero-bits
            0, 0, 0,  // trailing_zero_8bits
            42, // unexpected byte
            0, 0, 1, // start-code
            2, 3,    // NAL data
            0x80, // 1 stop-bit + 7 alignment-zero-bits
            0, 0, 1, // start-code
        ];
        r.push(&data[..]);
        let mock = r.into_fragment_handler();
        assert_eq!(&mock.data[..], &[3, 0x80, 2, 3, 0x80][..]);
        assert_eq!(2, mock.ended);
    }

    #[test]
    fn implicit_end() {
        let mock = MockFragmentHandler::default();
        let mut r = AnnexBReader::for_fragment_handler(mock);
        let data = vec![
            0, 0, 0, 1, // start-code
            3, 0, // NAL data
        ];
        r.push(&data[..]);
        r.reset();
        let mock = r.into_fragment_handler();
        assert_eq!(&mock.data[..], &[3u8, 0u8][..]);
        assert_eq!(1, mock.ended);
    }

    #[test]
    fn split_nal() {
        let mock = MockFragmentHandler::default();
        let mut r = AnnexBReader::for_fragment_handler(mock);
        let data = vec![
            0, 0, 0, 1, // start-code
            2, 3, // NAL data
            0, 0, 1, // nd-code
        ];
        r.push(&data[..5]); // half-way through the NAL Unit
        let mock = r.fragment_handler_ref();
        assert_eq!(&mock.data[..], &[2u8][..]);
        assert_eq!(0, mock.ended);
        r.push(&data[5..]); // second half of the NAL Unit
        let mock = r.fragment_handler_ref();
        assert_eq!(&mock.data[..], &[2u8, 3u8][..]);
        assert_eq!(1, mock.ended);
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
            3A CE FA 53 86 60 95 6C BB C5 4E F3"
        );
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
            3A CE FA 53 86 60 95 6C BB C5 4E F3"
        );
        for i in 1..data.len() - 1 {
            let mock = MockFragmentHandler::default();
            let mut r = AnnexBReader::for_fragment_handler(mock);
            let (head, tail) = data.split_at(i);
            r.push(&head[..]);
            r.push(&tail[..]);
            r.reset();
            let mock = r.into_fragment_handler();
            assert_eq!(3, mock.ended);
            assert_eq!(&mock.data[..], &expected[..]);
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
            3A CE FA 53 86 60 95 6C BB C5 4E F3"
        );
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
            3A CE FA 53 86 60 95 6C BB C5 4E F3"
        );
        let mock = MockFragmentHandler::default();
        let mut r = AnnexBReader::for_fragment_handler(mock);
        for i in 0..data.len() {
            r.push(&data[i..i + 1]);
        }
        r.reset();
        let mock = r.into_fragment_handler();
        assert_eq!(3, mock.ended);
        assert_eq!(&mock.data[..], &expected[..]);
    }
}
