//! Benchmark on a large video file.
//!
//! Expects a copy of [Big Buck Bunny](https://peach.blender.org/download/):
//! ```text
//! $ curl --OL https://download.blender.org/peach/bigbuckbunny_movies/big_buck_bunny_1080p_h264.mov
//! $ ffmpeg -i big_buck_bunny_1080p_h264.mov -c copy big_buck_bunny_1080p.h264
//! ```

#[macro_use]
extern crate criterion;
extern crate h264_reader;

use criterion::Criterion;
use h264_reader::nal::sps::SeqParameterSet;
use std::fs::File;
use criterion::Throughput;
use std::convert::TryFrom;
use std::io::Read;
use hex_literal::hex;
use h264_reader::annexb::AnnexBReader;
use h264_reader::annexb::NalReader;
use h264_reader::Context;
use h264_reader::rbsp::RbspDecoder;
use h264_reader::nal::NalHandler;
use h264_reader::nal::NalHeader;

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

struct NullRbspNalReader {
    decoder: RbspDecoder<NullNalHandler>,
    decoder_started: bool,
}
impl NalReader for NullRbspNalReader {
    type Ctx = ();

    fn start(&mut self, _ctx: &mut Context<Self::Ctx>) {
        assert!(!self.decoder_started);
    }
    fn push(&mut self, ctx: &mut Context<Self::Ctx>, mut buf: &[u8]) {
        if !self.decoder_started && !buf.is_empty() {
            let hdr = NalHeader::new(buf[0]).unwrap();
            self.decoder.start(ctx, hdr);
            buf = &buf[1..];
            self.decoder_started = true;
        }
        if self.decoder_started {
            self.decoder.push(ctx, buf);
        }
    }
    fn end(&mut self, ctx: &mut Context<Self::Ctx>) {
        assert!(self.decoder_started);
        self.decoder.end(ctx);
        self.decoder_started = false;
    }
}

struct NullNalReader {
    start: u64,
    push: u64,
    end: u64,
}
impl NalReader for NullNalReader {
    type Ctx = ();

    fn start(&mut self, _ctx: &mut Context<Self::Ctx>) {
        self.start += 1;
    }
    fn push(&mut self, _ctx: &mut Context<Self::Ctx>, _buf: &[u8]) {
        self.push += 1;
    }
    fn end(&mut self, _ctx: &mut Context<Self::Ctx>) {
        self.end += 1;
    }
}

fn h264_reader(c: &mut Criterion) {
    let mut f = File::open("big_buck_bunny_1080p.h264").expect("file not found");
    let len = f.metadata().unwrap().len();
    let mut buf = vec![0; usize::try_from(len).unwrap()];
    f.read(&mut buf[..]).unwrap();
    let mut ctx = Context::default();
    let nal_handler = NullNalHandler {
        start: 0,
        push: 0,
        end: 0,
    };
    let rbsp_nal_reader = NullRbspNalReader {
        decoder: RbspDecoder::new(nal_handler),
        decoder_started: false,
    };
    let nal_reader = NullNalReader {
        start: 0,
        push: 0,
        end: 0,
    };
    let mut annexb_rbsp_reader = AnnexBReader::new(rbsp_nal_reader);
    let mut annexb_reader = AnnexBReader::new(nal_reader);

    let mut group = c.benchmark_group("parse_annexb");
    group.throughput(Throughput::Bytes(len));
    group.bench_function("annexb_only", |b| {
        b.iter(|| {
            annexb_reader.start(&mut ctx);
            annexb_reader.push(&mut ctx, &buf[..]);
            annexb_reader.end_units(&mut ctx);
        })
    });
    group.bench_function("annexb_rbsp", |b| {
        b.iter(|| {
            annexb_rbsp_reader.start(&mut ctx);
            annexb_rbsp_reader.push(&mut ctx, &buf[..]);
            annexb_rbsp_reader.end_units(&mut ctx);
        })
    });
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
