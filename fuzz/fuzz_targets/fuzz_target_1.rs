#![no_main]
use libfuzzer_sys::fuzz_target;
use h264_reader::annexb::AnnexBReader;
use h264_reader::Context;
use h264_reader::nal::{Nal, RefNal, UnitType, pps, slice, sei, sps, sps_extension, subset_sps};
use h264_reader::push::NalInterest;

fuzz_target!(|data: &[u8]| {
    let mut ctx = Context::default();
    let mut scratch = Vec::new();
    let mut annexb_reader = AnnexBReader::accumulate(|nal: RefNal<'_>| {
        if !nal.is_complete() {
            return NalInterest::Buffer;
        }
        let hdr = match nal.header() {
            Ok(h) => h,
            Err(_) => return NalInterest::Buffer,
        };
        match hdr.nal_unit_type() {
            UnitType::SeqParameterSet => {
                if let Ok(sps) = sps::SeqParameterSet::from_bits(nal.rbsp_bits()) {
                    ctx.put_seq_param_set(sps);
                }
            },
            UnitType::PicParameterSet => {
                if let Ok(pps) = pps::PicParameterSet::from_bits(&ctx, nal.rbsp_bits()) {
                    ctx.put_pic_param_set(pps);
                }
            },
            UnitType::SEI => {
                let mut r = sei::SeiReader::from_rbsp_bytes(nal.rbsp_bytes(), &mut scratch);
                while let Ok(Some(msg)) = r.next() {
                    match msg.payload_type {
                        sei::HeaderType::PicTiming => {
                            let sps = match ctx.sps().next() {
                                Some(s) => s,
                                None => continue,
                            };
                            let _ = sei::pic_timing::PicTiming::read(sps, &msg);
                        },
                        _ => {},
                    }
                }
            },
            UnitType::SliceLayerWithoutPartitioningIdr | UnitType::SliceLayerWithoutPartitioningNonIdr => {
                let _ = slice::SliceHeader::from_bits(&ctx, &mut nal.rbsp_bits(), hdr);
            },
            UnitType::SeqParameterSetExtension => {
                let _ = sps_extension::SeqParameterSetExtension::from_bits(nal.rbsp_bits());
            },
            UnitType::SubsetSeqParameterSet => {
                if let Ok(subset) = subset_sps::SubsetSps::from_bits(nal.rbsp_bits()) {
                    ctx.put_subset_seq_param_set(subset);
                }
            },
            _ => {},
        }
        NalInterest::Buffer
    });
    annexb_reader.push(data);
    annexb_reader.reset();
    ctx.sps().for_each(|sps| { let _ = sps.pixel_dimensions(); });
});
