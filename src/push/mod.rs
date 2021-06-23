//! Push parsing of encoded NALs.

use crate::nal::{NalHeader, RefNal};

/// [`AccumulatedNalHandler`]'s interest in receiving additional callbacks on a NAL.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum NalInterest {
    /// If this NAL is incomplete, buffer it and call again later.
    /// No effect if the NAL is complete.
    Buffer,

    /// Don't buffer any more of this NAL or make any more calls on it.
    Ignore,
}

/// [NalAccumulator] callback which handles partially- or completely-buffered NALs.
///
/// The simplest handler is a closure. Implement this type manually when you
/// your handler to own state which can be accessed via [NalAccumulator::handler],
/// [NalAccumulator::handler_mut], or [NalAccumulator::into_handler].
pub trait AccumulatedNalHandler {
    fn nal(&mut self, nal: RefNal<'_>) -> NalInterest;
}

impl<F: FnMut(RefNal<'_>) -> NalInterest> AccumulatedNalHandler for F {
    fn nal(&mut self, nal: RefNal<'_>) -> NalInterest {
        (self)(nal)
    }
}

/// Handles arbitrary fragments of NALs. See [NalAccumulator].
///
/// It's probably unnecessary to provide your own implementation of this trait
/// except when benchmarking or testing a parser.
pub trait NalFragmentHandler {
    /// Pushes a fragment of a NAL.
    ///
    /// The caller must ensure that each element of `bufs` (if there are any)
    /// is non-empty.
    fn nal_fragment(&mut self, bufs: &[&[u8]], end: bool);
}

/// NAL accumulator for push parsers.
///
/// This is meant to be used by parsers for a specific format: Annex B, AVC, MPEG-TS, RTP, etc.
/// Accumulates NALs in an internal buffer and delegates to an [AccumulatedNalHandler].
///
/// ```
/// use h264_reader::nal::{Nal, RefNal, UnitType};
/// use h264_reader::push::{NalAccumulator, NalFragmentHandler, NalInterest};
/// let mut calls = Vec::new();
/// let mut acc = NalAccumulator::new(|nal: RefNal<'_>| {
///     let nal_unit_type = nal.header().unwrap().nal_unit_type();
///     calls.push((nal_unit_type, nal.is_complete()));
///     match nal_unit_type {
///         UnitType::SeqParameterSet => NalInterest::Buffer,
///         _ => NalInterest::Ignore,
///     }
/// });
///
/// // Push a SeqParameterSet in two calls (the latter with two byte slices).
/// acc.nal_fragment(&[&b"\x67\x64\x00\x0A\xAC\x72\x84\x44\x26\x84\x00\x00\x03"[..]], false);
/// acc.nal_fragment(&[&b"\x00"[..], &b"\x04\x00\x00\x03\x00\xCA\x3C\x48\x96\x11\x80"[..]], true);
///
/// // Push a PicParameterSet in two calls.
/// acc.nal_fragment(&[&b"\x68"[..]], false);
/// acc.nal_fragment(&[&b"\xE8\x43\x8F\x13\x21\x30"[..]], true);
///
/// assert_eq!(calls, &[
///     (UnitType::SeqParameterSet, false),
///     (UnitType::SeqParameterSet, true),
///     (UnitType::PicParameterSet, false),
///     // no second call on the PicParameterSet because the handler returned Ignore.
/// ]);
/// ```
///
/// Non-trivial handlers may need to *own* state that can be accessed outside the handler:
///
/// ```
/// use h264_reader::nal::{Nal, RefNal, UnitType};
/// use h264_reader::push::{AccumulatedNalHandler, NalAccumulator, NalFragmentHandler, NalInterest};
/// struct MyHandler(Vec<UnitType>);
/// impl AccumulatedNalHandler for MyHandler {
///     fn nal(&mut self, nal: RefNal<'_>) -> NalInterest {
///         self.0.push(nal.header().unwrap().nal_unit_type());
///         NalInterest::Ignore
///     }
/// }
/// let mut acc = NalAccumulator::new(MyHandler(Vec::new()));
/// acc.nal_fragment(&[&b"\x67\x64\x00\x0A\xAC\x72\x84\x44\x26\x84\x00\x00\x03"[..]], false);
/// acc.nal_fragment(&[&b"\x00"[..], &b"\x04\x00\x00\x03\x00\xCA\x3C\x48\x96\x11\x80"[..]], true);
/// acc.nal_fragment(&[&b"\x68"[..]], false);
/// acc.nal_fragment(&[&b"\xE8\x43\x8F\x13\x21\x30"[..]], true);
/// assert_eq!(acc.handler().0, &[
///     UnitType::SeqParameterSet,
///     UnitType::PicParameterSet,
/// ]);
/// ```
pub struct NalAccumulator<H: AccumulatedNalHandler> {
    buf: Vec<u8>,
    nal_handler: H,
    interest: NalInterest,
}
impl<H: AccumulatedNalHandler> NalAccumulator<H> {
    /// Creates a new accumulator which delegates to the given `nal_handler` on every push.
    /// `nal_handler` always sees the NAL from the beginning.
    pub fn new(nal_handler: H) -> Self {
        Self {
            buf: Vec::new(),
            interest: NalInterest::Buffer,
            nal_handler,
        }
    }

    /// Gets a reference to the handler.
    pub fn handler(&self) -> &H {
        &self.nal_handler
    }

    /// Gets a mutable reference to the handler.
    pub fn handler_mut(&mut self) -> &mut H {
        &mut self.nal_handler
    }

    /// Unwraps this `NalAccumulator<h>`, returning the inner handler.
    pub fn into_handler(self) -> H {
        self.nal_handler
    }
}
impl<H: AccumulatedNalHandler> NalFragmentHandler for NalAccumulator<H> {
    /// Calls `nal_handler` with accumulated NAL unless any of the following are true:
    /// *   a previous call on the same NAL returned [`NalInterest::Ignore`].
    /// *   the NAL is totally empty.
    /// *   `bufs` is empty and `end` is false.
    fn nal_fragment(&mut self, bufs: &[&[u8]], end: bool) {
        if self.interest != NalInterest::Ignore {
            let nal = if !self.buf.is_empty() {
                RefNal::new(&self.buf[..], bufs, end)
            } else if bufs.is_empty() {
                return;  // no-op.
            } else {
                RefNal::new(bufs[0], &bufs[1..], end)
            };

            // Call the NAL handler. Avoid copying unless necessary.
            match self.nal_handler.nal(nal) {
                NalInterest::Buffer if !end => {
                    let len = bufs.iter().map(|b| b.len()).sum();
                    self.buf.reserve(len);
                    for b in bufs {
                        self.buf.extend_from_slice(b);
                    }
                },
                NalInterest::Ignore => self.interest = NalInterest::Ignore,
                _ => {},
            }
        }
        if end {
            self.buf.clear();
            self.interest = NalInterest::Buffer;
        }
    }
}
impl<H: AccumulatedNalHandler + std::fmt::Debug> std::fmt::Debug for NalAccumulator<H> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NalAccumulator")
            .field("interest", &self.interest)
            .field("buf", &self.buf)
            .field("header", &self.buf.first().map(|&h| NalHeader::new(h)))
            .field("nal_handler", &self.nal_handler)
            .finish()
    }
}

#[cfg(test)]
mod test {
    use std::io::{BufRead, Read};
    use crate::nal::Nal;

    use super::*;

    #[test]
    fn accumulate() {
        // Try buffering everything.
        let mut nals = Vec::new();
        let handler = |nal: RefNal<'_>| {
            if nal.is_complete() {
                let mut buf = Vec::new();
                nal.reader().read_to_end(&mut buf).unwrap();
                nals.push(buf);
            }
            NalInterest::Buffer
        };
        let mut accumulator = NalAccumulator::new(handler);
        accumulator.nal_fragment(&[], false);
        accumulator.nal_fragment(&[], true);
        accumulator.nal_fragment(&[&[0b0101_0001], &[1]], true);
        accumulator.nal_fragment(&[&[0b0101_0001]], false);
        accumulator.nal_fragment(&[], false);
        accumulator.nal_fragment(&[&[2]], true);
        accumulator.nal_fragment(&[&[0b0101_0001]], false);
        accumulator.nal_fragment(&[], false);
        accumulator.nal_fragment(&[&[3]], false);
        accumulator.nal_fragment(&[], true);
        assert_eq!(nals, &[
            &[0b0101_0001, 1][..],
            &[0b0101_0001, 2][..],
            &[0b0101_0001, 3][..],
        ]);

        // Try buffering nothing and see what's given on the first push.
        nals.clear();
        let handler = |nal: RefNal<'_>| {
            nals.push(nal.reader().fill_buf().unwrap().to_owned());
            NalInterest::Ignore
        };
        let mut accumulator = NalAccumulator::new(handler);
        accumulator.nal_fragment(&[], false);
        accumulator.nal_fragment(&[], true);
        accumulator.nal_fragment(&[&[0b0101_0001, 1]], true);
        accumulator.nal_fragment(&[&[0b0101_0001]], false);
        accumulator.nal_fragment(&[], false);
        accumulator.nal_fragment(&[&[2]], true);
        accumulator.nal_fragment(&[&[0b0101_0001]], false);
        accumulator.nal_fragment(&[], false);
        accumulator.nal_fragment(&[&[3]], false);
        accumulator.nal_fragment(&[], true);
        assert_eq!(nals, &[
            &[0b0101_0001, 1][..],
            &[0b0101_0001][..],
            &[0b0101_0001][..],
        ]);
    }
}
