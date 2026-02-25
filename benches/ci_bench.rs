use h264_reader::annexb::AnnexBReader;
use h264_reader::nal::pps::PicParameterSet;
use h264_reader::nal::sei::buffering_period::BufferingPeriod;
use h264_reader::nal::sei::pic_timing::PicTiming;
use h264_reader::nal::sei::user_data_registered_itu_t_t35::ItuTT35;
use h264_reader::nal::sei::HeaderType;
use h264_reader::nal::slice::SliceHeader;
use h264_reader::nal::sps::SeqParameterSet;
use h264_reader::nal::Nal;
use h264_reader::nal::{sei, RefNal, UnitType};
use h264_reader::push::NalInterest;
use h264_reader::Context;
use iai_callgrind::{library_benchmark, library_benchmark_group, main};
use std::fs::File;
use std::hint::black_box;
use std::io::Read;

fn setup_video(filename: &str) -> Vec<u8> {
    let mut f = File::open(filename).expect("Test file missing.");
    let l = f.metadata().unwrap().len() as usize;
    let size = l.min(10 * 1024 * 1024);
    let mut buf = vec![0; size];
    f.read_exact(&mut buf[..]).unwrap();
    buf
}

#[library_benchmark]
#[bench::read(setup_video("big_buck_bunny_1080p_24fps_h264.h264"))]
fn reader(buf: Vec<u8>) {
    let mut ctx = Context::new();

    let mut reader = AnnexBReader::accumulate(|nal: RefNal<'_>| {
        if !nal.is_complete() {
            return NalInterest::Buffer;
        }

        let nal_header = nal.header().unwrap();
        let nal_unit_type = nal_header.nal_unit_type();

        match nal_unit_type {
            UnitType::SeqParameterSet => {
                let data = SeqParameterSet::from_bits(nal.rbsp_bits()).unwrap();
                ctx.put_seq_param_set(data);
            }
            UnitType::PicParameterSet => {
                let data = PicParameterSet::from_bits(&ctx, nal.rbsp_bits()).unwrap();
                ctx.put_pic_param_set(data);
            }
            UnitType::SliceLayerWithoutPartitioningIdr
            | UnitType::SliceLayerWithoutPartitioningNonIdr => {
                let mut bits = nal.rbsp_bits();
                let (header, _seq_params, _pic_params) =
                    SliceHeader::from_bits(&ctx, &mut bits, nal_header, None).unwrap();
                let _ = black_box(header);
            }
            UnitType::SEI => {
                let mut scratch = vec![];
                let mut reader = sei::SeiReader::from_rbsp_bytes(nal.rbsp_bytes(), &mut scratch);
                loop {
                    match reader.next() {
                        Ok(Some(sei)) => match sei.payload_type {
                            HeaderType::BufferingPeriod => {
                                let bp = BufferingPeriod::read(&ctx, &sei);
                                let _ = black_box(bp);
                            }
                            HeaderType::PicTiming => {
                                let pt =
                                    PicTiming::read(ctx.sps().next().expect("first sps"), &sei);
                                let _ = black_box(pt);
                            }
                            HeaderType::UserDataRegisteredItuTT35 => match ItuTT35::read(&sei) {
                                Ok(ud) => {
                                    let _ = black_box(ud);
                                }
                                Err(e) => {
                                    println!("{:?}", e);
                                }
                            },
                            _ => {}
                        },
                        Ok(None) => break,
                        Err(e) => {
                            println!("{:?}", e);
                        }
                    }
                }
            }
            _ => {
                println!("Unhandled: {:?}", nal_unit_type);
            }
        }
        NalInterest::Ignore
    });

    reader.push(&buf);
}

library_benchmark_group!(
    name = ci;
    benchmarks = reader
);

main!(library_benchmark_groups = ci);
