#[macro_use]
extern crate criterion;
extern crate h264_reader;

use criterion::Criterion;
use std::fs::File;
use criterion::Benchmark;
use criterion::Throughput;
use std::io::Read;
use h264_reader::annexb::AnnexBReader;
use h264_reader::annexb::NalReader;
use h264_reader::Context;

#[derive(Default)]
struct NullNalReader {
    start: u64,
    push: u64,
    end: u64,
}
impl NalReader for NullNalReader {
    fn start(&mut self, ctx: &mut Context) {
        self.start += 1;
    }
    fn push(&mut self, ctx: &mut Context, buf: &[u8]) {
        self.push += 1;
    }
    fn end(&mut self, ctx: &mut Context) {
        self.end += 1;
    }
}

fn h264_reader(c: &mut Criterion) {
    let mut f = File::open("big_buck_bunny_1080p_24fps_h264.h264").expect("file not found");
    let size = f.metadata().unwrap().len() as usize;
    let mut buf = vec![0; size];
    f.read(&mut buf[..]).unwrap();
    let mut ctx = Context::default();
    let nal_reader = NullNalReader::default();
    let mut annexb_reader = AnnexBReader::new(nal_reader);
    c.bench("parse", Benchmark::new("parse", move |b| {
        b.iter(|| {
            annexb_reader.start(&mut ctx);
            annexb_reader.push(&mut ctx, &buf[..]);
            annexb_reader.end_units(&mut ctx);
        } );
    }).throughput(Throughput::Bytes(size as u32)));
}

criterion_group!(benches, h264_reader);
criterion_main!(benches);
