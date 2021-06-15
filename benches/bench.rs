//! Benchmark on a large video file.
//!
//! Expects a copy of [Big Buck Bunny](https://peach.blender.org/download/):
//! ```text
//! $ curl -OL https://download.blender.org/peach/bigbuckbunny_movies/big_buck_bunny_1080p_h264.mov
//! $ ffmpeg -i big_buck_bunny_1080p_h264.mov -c copy big_buck_bunny_1080p.h264
//! ```

#[macro_use]
extern crate criterion;

use criterion::{Bencher, Criterion, Throughput};
use hex_literal::hex;
use h264_reader::nal::slice::SliceHeaderError;
use h264_reader::nal::sps::SeqParameterSet;
use h264_reader::rbsp::{BitReaderError, RbspDecoder};
use h264_reader::annexb::AnnexBReader;
use h264_reader::annexb::NalReader;
use std::cell::RefCell;
use std::convert::TryFrom;
use std::io::ErrorKind;
use h264_reader::{nal, Context, rbsp};
use h264_reader::nal::{NalHandler, NalHeader};
use h264_reader::nal::sps::SeqParameterSetNalHandler;
use h264_reader::nal::pps::PicParameterSetNalHandler;


struct InProgressSlice {
    header: h264_reader::nal::NalHeader,
    rbsp: Vec<u8>,
}

/// Handles bytes from RbspDecoder, trying on every push to parse a slice header until success.
struct SliceRbspHandler {
    current_slice: Option<InProgressSlice>,
}
impl SliceRbspHandler {
    pub fn new() -> SliceRbspHandler {
        SliceRbspHandler {
            current_slice: None,
        }
    }
}
impl h264_reader::nal::NalHandler for SliceRbspHandler {
    type Ctx = ();

    fn start(&mut self, _ctx: &mut h264_reader::Context<Self::Ctx>, header: h264_reader::nal::NalHeader) {
        let mut buf = Vec::new();
        buf.push(header.into());
        self.current_slice = Some(InProgressSlice {
            header,
            rbsp: buf,
        });
    }

    fn push(&mut self, ctx: &mut h264_reader::Context<Self::Ctx>, buf: &[u8]) {
        if let Some(mut s) = self.current_slice.take() {
            s.rbsp.extend_from_slice(buf);
            let mut r = rbsp::BitReader::new(&s.rbsp[1..]);
            match nal::slice::SliceHeader::read(ctx, &mut r, s.header) {
                Err(SliceHeaderError::RbspError(BitReaderError::ReaderErrorFor(_, e))) if e.kind() == ErrorKind::UnexpectedEof => {
                    // Try again later.
                    self.current_slice = Some(s);
                },
                Err(e) => panic!("{:?}", e),
                Ok(_) =>  {},
            }
        }
    }

    fn end(&mut self, _ctx: &mut h264_reader::Context<Self::Ctx>) {
        assert!(self.current_slice.is_none());
    }
}

/// Handles NAL-encoded bytes, only decoding RBSP until a slice header is successfully parsed.
struct SliceNalHandler(RbspDecoder<SliceRbspHandler>);
impl SliceNalHandler {
    fn new() -> Self {
        SliceNalHandler(RbspDecoder::new(SliceRbspHandler::new()))
    }
}
impl h264_reader::nal::NalHandler for SliceNalHandler {
    type Ctx = ();
    fn start(&mut self, ctx: &mut Context<Self::Ctx>, header: NalHeader) {
        self.0.start(ctx, header);
    }
    fn push(&mut self, ctx: &mut Context<Self::Ctx>, buf: &[u8]) {
        if self.0.handler_ref().current_slice.is_some() {
            self.0.push(ctx, buf);
        }
    }
    fn end(&mut self, ctx: &mut Context<Self::Ctx>) {
        self.0.end(ctx);
    }
}

/// RBSP bytes handler that does nothing, except maintain counters to limit optimization.
#[derive(Default)]
struct NullNalHandler {
    start: u64,
    push: u64,
    end: u64,
}
impl NalHandler for NullNalHandler {
    type Ctx = ();

    fn start(&mut self, _ctx: &mut Context<Self::Ctx>, _header: NalHeader) {
        self.start += 1;
    }

    fn push(&mut self, _ctx: &mut Context<Self::Ctx>, _buf: &[u8]) {
        self.push += 1;
    }

    fn end(&mut self, _ctx: &mut Context<Self::Ctx>) {
        self.end += 1;
    }
}

/// NAL handler that decodes all RBSP bytes.
struct RbspDecodingNalReader {
    decoder: RbspDecoder<NullNalHandler>,
    decoder_started: bool,
}
impl RbspDecodingNalReader {
    fn new() -> Self {
        RbspDecodingNalReader {
            decoder: RbspDecoder::new(NullNalHandler::default()),
            decoder_started: false,
        }
    }
}
impl NalReader for RbspDecodingNalReader {
    type Ctx = ();

    fn push(&mut self, ctx: &mut Context<Self::Ctx>, mut buf: &[u8], end: bool) {
        if !self.decoder_started && !buf.is_empty() {
            let hdr = NalHeader::new(buf[0]).unwrap();
            self.decoder.start(ctx, hdr);
            buf = &buf[1..];
            self.decoder_started = true;
        }
        if self.decoder_started {
            self.decoder.push(ctx, buf);
        }
        if end {
            assert!(self.decoder_started);
            self.decoder.end(ctx);
            self.decoder_started = false;
        }
    }
}

/// A NAL handler that does nothing, except maintain counters to limit optimization.
#[derive(Default)]
struct NullNalReader {
    push: u64,
    end: u64,
}
impl NalReader for NullNalReader {
    type Ctx = ();

    fn push(&mut self, _ctx: &mut Context<Self::Ctx>, _buf: &[u8], end: bool) {
        self.push += 1;
        if end {
            self.end += 1;
        }
    }
}

/// Returns a NAL reader which parses several types of NALs.
fn parse() -> impl NalReader<Ctx = ()> {
    let mut switch = h264_reader::nal::NalSwitch::default();
    let sps_handler = SeqParameterSetNalHandler::default();
    let pps_handler = PicParameterSetNalHandler::default();
    let slice_wout_part_idr_handler = SliceNalHandler::new();
    let slice_wout_part_nonidr_handler = SliceNalHandler::new();
    switch.put_handler(h264_reader::nal::UnitType::SeqParameterSet, Box::new(RefCell::new(sps_handler)));
    switch.put_handler(h264_reader::nal::UnitType::PicParameterSet, Box::new(RefCell::new(pps_handler)));
    switch.put_handler(h264_reader::nal::UnitType::SliceLayerWithoutPartitioningIdr, Box::new(RefCell::new(slice_wout_part_idr_handler)));
    switch.put_handler(h264_reader::nal::UnitType::SliceLayerWithoutPartitioningNonIdr, Box::new(RefCell::new(slice_wout_part_nonidr_handler)));
    switch
}

fn bench_annexb<'a, R, P>(r: R, b: &mut Bencher, pushes: P)
where R: NalReader<Ctx = ()>, P: Iterator<Item = &'a [u8]> + Clone {
    let mut annexb_reader = AnnexBReader::new(r);
    b.iter(|| {
        let mut ctx = Context::default();
        for p in pushes.clone() {
            annexb_reader.push(&mut ctx, p);
        }
        annexb_reader.reset(&mut ctx);
    })
}

fn h264_reader(c: &mut Criterion) {
    let buf = std::fs::read("big_buck_bunny_1080p.h264").expect("reading h264 file failed");
    let mut group = c.benchmark_group("parse_annexb");
    group.throughput(Throughput::Bytes(u64::try_from(buf.len()).unwrap()));

    // Benchmark parsing in one big push (as when reading H.264 with a large buffer size),
    // 184-byte pushes (like MPEG-TS), and 1440-byte pushes (~typical for RTP). RTP doesn't
    // use Annex B encoding, but it does use the RBSP decoding and NAL parsing layers, so this
    // is still informative.
    group.bench_function("onepush_null", |b| bench_annexb(NullNalReader::default(), b, std::iter::once(&buf[..])));
    group.bench_function("chunksize184_null", |b| bench_annexb(NullNalReader::default(), b, buf.chunks(184)));
    group.bench_function("chunksize1440_null", |b| bench_annexb(NullNalReader::default(), b, buf.chunks(1440)));
    group.bench_function("onepush_rbsp", |b| bench_annexb(RbspDecodingNalReader::new(), b, std::iter::once(&buf[..])));
    group.bench_function("chunksize184_rbsp", |b| bench_annexb(RbspDecodingNalReader::new(), b, buf.chunks(184)));
    group.bench_function("chunksize1440_rbsp", |b| bench_annexb(RbspDecodingNalReader::new(), b, buf.chunks(1440)));
    group.bench_function("onepush_parse", |b| bench_annexb(parse(), b, std::iter::once(&buf[..])));
    group.bench_function("chunksize184_parse", |b| bench_annexb(parse(), b, buf.chunks(184)));
    group.bench_function("chunksize1440_parse", |b| bench_annexb(parse(), b, buf.chunks(1440)));
}

fn parse_nal(c: &mut Criterion) {
    let sps = hex!(
        "64 00 16 AC 1B 1A 80 B0 3D FF FF
        00 28 00 21 6E 0C 0C 0C 80 00 01
        F4 00 00 27 10 74 30 07 D0 00 07
        A1 25 DE 5C 68 60 0F A0 00 0F 42
        4B BC B8 50");
    let mut group = c.benchmark_group("parse_nal");
    group.bench_function("sps", |b| b.iter(|| SeqParameterSet::from_bytes(&sps[..]).unwrap()));
}

criterion_group!(benches, h264_reader, parse_nal);
criterion_main!(benches);
