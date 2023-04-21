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
use h264_reader::nal::{RefNal, Nal};
use h264_reader::nal::UnitType;
use h264_reader::nal::sei::SeiReader;
use h264_reader::nal::slice::{SliceHeader, SliceHeaderError};
use h264_reader::nal::sps::SeqParameterSet;
use h264_reader::push::NalInterest;
use h264_reader::rbsp::{self, BitReaderError};
use std::io::{BufRead, ErrorKind};
use std::convert::TryFrom;
use h264_reader::annexb::AnnexBReader;
use h264_reader::push::NalFragmentHandler;

/// A NAL handler that does nothing, except maintain counters to limit optimization.
#[derive(Default)]
struct NullNalReader {
    push: u64,
    end: u64,
}
impl NalFragmentHandler for NullNalReader {
    fn nal_fragment(&mut self, _bufs: &[&[u8]], end: bool) {
        self.push += 1;
        if end {
            self.end += 1;
        }
    }
}

fn bench_annexb<'a, H, P>(mut r: AnnexBReader<H>, b: &mut Bencher, pushes: P)
where H: NalFragmentHandler, P: Iterator<Item = &'a [u8]> + Clone {
    b.iter(|| {
        for p in pushes.clone() {
            r.push(p);
        }
        r.reset();
    })
}

fn h264_reader(c: &mut Criterion) {
    let buf = std::fs::read("big_buck_bunny_1080p.h264").expect("reading h264 file failed");
    let mut rbsp_len = 0;
    let mut rbsp_len_nal_handler = |nal: RefNal<'_>| {
        if nal.is_complete() {
            let mut r = nal.rbsp_bytes();
            loop {
                let buf = r.fill_buf().unwrap();
                let len = buf.len();
                if len == 0 {
                    break;
                }
                rbsp_len += u64::try_from(buf.len()).unwrap();
                r.consume(len);
            }
        }
        NalInterest::Buffer
    };
    let mut parsing_ctx = h264_reader::Context::default();
    let mut scratch = Vec::new();
    let mut parsing_nal_handler = |nal: RefNal<'_>| {
        let nal_hdr = nal.header().unwrap();
        match nal_hdr.nal_unit_type() {
            UnitType::SeqParameterSet if nal.is_complete() => {
                let sps = h264_reader::nal::sps::SeqParameterSet::from_bits(nal.rbsp_bits()).unwrap();
                parsing_ctx.put_seq_param_set(sps);
            },
            UnitType::PicParameterSet if nal.is_complete() => {
                let pps = h264_reader::nal::pps::PicParameterSet::from_bits(&parsing_ctx, nal.rbsp_bits()).unwrap();
                parsing_ctx.put_pic_param_set(pps);
            },
            UnitType::SEI if nal.is_complete() => {
                let mut r = SeiReader::from_rbsp_bytes(nal.rbsp_bytes(), &mut scratch);
                while let Some(msg) = r.next().unwrap() {
                    match msg.payload_type {
                        h264_reader::nal::sei::HeaderType::BufferingPeriod => {}, // todo
                        h264_reader::nal::sei::HeaderType::UserDataUnregistered => {}, // todo
                        _ => panic!("unknown SEI payload type {:?}", msg.payload_type),
                    }
                }
            },
            UnitType::SliceLayerWithoutPartitioningIdr
            | UnitType::SliceLayerWithoutPartitioningNonIdr => {
                match SliceHeader::from_bits(&parsing_ctx, &mut nal.rbsp_bits(), nal.header().unwrap()) {
                    Err(SliceHeaderError::RbspError(BitReaderError::ReaderErrorFor(_, e))) => {
                        assert_eq!(e.kind(), ErrorKind::WouldBlock);
                    },
                    Err(e) => panic!("{:?}", e),
                    Ok(_) => return NalInterest::Ignore,
                }
            },
            _ => if nal.is_complete() { panic!("unknown slice type {:?}", nal_hdr) },
        }
        NalInterest::Buffer
    };

    let mut group = c.benchmark_group("parse_annexb");
    group.throughput(Throughput::Bytes(u64::try_from(buf.len()).unwrap()));

    // Benchmark parsing in one big push (as when reading H.264 with a large buffer size),
    // 184-byte pushes (like MPEG-TS), and 1440-byte pushes (~typical for RTP). RTP doesn't
    // use Annex B encoding, but it does use the RBSP decoding and NAL parsing layers, so this
    // is still informative.
    group.bench_function("onepush_null", |b| bench_annexb(
        AnnexBReader::for_fragment_handler(NullNalReader::default()), b, std::iter::once(&buf[..])));
    group.bench_function("chunksize184_null", |b| bench_annexb(
        AnnexBReader::for_fragment_handler(NullNalReader::default()), b, buf.chunks(184)));
    group.bench_function("chunksize1440_null", |b| bench_annexb(
        AnnexBReader::for_fragment_handler(NullNalReader::default()), b, buf.chunks(1440)));
    group.bench_function("onepush_rbsp", |b| bench_annexb(
        AnnexBReader::accumulate(&mut rbsp_len_nal_handler), b, std::iter::once(&buf[..])));
    group.bench_function("chunksize184_rbsp", |b| bench_annexb(
        AnnexBReader::accumulate(&mut rbsp_len_nal_handler), b, buf.chunks(184)));
    group.bench_function("chunksize1440_rbsp", |b| bench_annexb(
        AnnexBReader::accumulate(&mut rbsp_len_nal_handler), b, buf.chunks(1440)));
    group.bench_function("onepush_parse", |b| bench_annexb(
        AnnexBReader::accumulate(&mut parsing_nal_handler), b, std::iter::once(&buf[..])));
    group.bench_function("chunksize184_parse", |b| bench_annexb(
        AnnexBReader::accumulate(&mut parsing_nal_handler), b, buf.chunks(184)));
    group.bench_function("chunksize1440_parse", |b| bench_annexb(
        AnnexBReader::accumulate(&mut parsing_nal_handler), b, buf.chunks(1440)));
}

fn parse_nal(c: &mut Criterion) {
    let sps = hex!(
        "67 64 00 16 AC 1B 1A 80 B0 3D FF FF
        00 28 00 21 6E 0C 0C 0C 80 00 01
        F4 00 00 27 10 74 30 07 D0 00 07
        A1 25 DE 5C 68 60 0F A0 00 0F 42
        4B BC B8 50");
    let rbsp = h264_reader::rbsp::decode_nal(&sps).unwrap();
    let nal = RefNal::new(&sps[..], &[], true);
    let mut group = c.benchmark_group("parse_nal");
    group.bench_function("rbsp_sps", |b| b.iter(|| {
        SeqParameterSet::from_bits(rbsp::BitReader::new(&*rbsp)).unwrap()
    }));
    group.bench_function("nal_sps", |b| b.iter(|| {
        SeqParameterSet::from_bits(nal.rbsp_bits()).unwrap()
    }));
}

criterion_group!(benches, h264_reader, parse_nal);
criterion_main!(benches);
