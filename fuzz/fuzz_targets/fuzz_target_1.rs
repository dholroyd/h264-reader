#![no_main]
use libfuzzer_sys::fuzz_target;
use h264_reader::rbsp::RbspDecoder;
use h264_reader::annexb::AnnexBReader;
use std::cell::RefCell;
use std::io::Read;
use h264_reader::{nal, Context, rbsp};
use h264_reader::nal::{NalHandler, NalHeader};
use h264_reader::nal::sps::SeqParameterSetNalHandler;
use h264_reader::nal::pps::PicParameterSetNalHandler;

#[derive(Default)]
struct NalCapture {
    buf: Vec<u8>,
}
impl NalHandler for NalCapture {
    type Ctx = ();

    fn start(&mut self, ctx: &mut h264_reader::Context<Self::Ctx>, header: NalHeader) {
        self.buf.clear();
    }

    fn push(&mut self, ctx: &mut h264_reader::Context<Self::Ctx>, buf: &[u8]) {
        self.buf.extend_from_slice(buf);
    }

    fn end(&mut self, ctx: &mut h264_reader::Context<Self::Ctx>) {
    }
}

struct PicTimingFuzz;
impl nal::sei::pic_timing::PicTimingHandler for PicTimingFuzz {
    type Ctx = ();

    fn handle(&mut self, ctx: &mut h264_reader::Context<Self::Ctx>, pic_timing: nal::sei::pic_timing::PicTiming) {
        println!("  {:?}", pic_timing);
    }
}

h264_reader::sei_switch!{
    SeiSwitch<()> {
        //BufferingPeriod: h264_reader::nal::sei::buffering_period::BufferingPeriodPayloadReader
        //    => h264_reader::nal::sei::buffering_period::BufferingPeriodPayloadReader::new(),
        //UserDataRegisteredItuTT35: h264_reader::nal::sei::user_data_registered_itu_t_t35::UserDataRegisteredItuTT35Reader<TT35Switch>
        //    => h264_reader::nal::sei::user_data_registered_itu_t_t35::UserDataRegisteredItuTT35Reader::new(TT35Switch::default()),
        PicTiming: h264_reader::nal::sei::pic_timing::PicTimingReader<PicTimingFuzz>
            => h264_reader::nal::sei::pic_timing::PicTimingReader::new(PicTimingFuzz),
    }
}
struct FuzzSeiPayoadReader {
    switch: SeiSwitch,
}
impl h264_reader::nal::sei::SeiIncrementalPayloadReader for FuzzSeiPayoadReader {
    type Ctx = ();

    fn start(&mut self, ctx: &mut h264_reader::Context<Self::Ctx>, payload_type: h264_reader::nal::sei::HeaderType, payload_size: u32) {
        //println!("  SEI: {:?} size={}", payload_type, payload_size);
        self.switch.start(ctx, payload_type, payload_size)
    }

    fn push(&mut self, ctx: &mut h264_reader::Context<Self::Ctx>, buf: &[u8]) {
        self.switch.push(ctx, buf)
    }

    fn end(&mut self, ctx: &mut h264_reader::Context<Self::Ctx>) {
        self.switch.end(ctx)
    }

    fn reset(&mut self, ctx: &mut h264_reader::Context<Self::Ctx>) {
        self.switch.reset(ctx)
    }
}

struct InProgressSlice {
    header: h264_reader::nal::NalHeader,
    buf: Vec<u8>,
}
struct SliceFuzz {
    current_slice: Option<InProgressSlice>,
}
impl SliceFuzz {
    pub fn new() -> SliceFuzz {
        SliceFuzz {
            current_slice: None,
        }
    }
}
impl h264_reader::nal::NalHandler for SliceFuzz {
    type Ctx = ();

    fn start(&mut self, ctx: &mut h264_reader::Context<Self::Ctx>, header: h264_reader::nal::NalHeader) {
        let mut buf = Vec::new();
        buf.push(header.into());
        self.current_slice = Some(InProgressSlice {
            header,
            buf,
        });
    }

    fn push(&mut self, ctx: &mut h264_reader::Context<Self::Ctx>, buf: &[u8]) {
        self.current_slice
            .as_mut()
            .unwrap()
            .buf
            .extend_from_slice(buf);
    }

    fn end(&mut self, ctx: &mut h264_reader::Context<Self::Ctx>) {
        let current_slice = self.current_slice.take().unwrap();
        let capture = NalCapture::default();
        let mut decode = RbspDecoder::new(capture);
        decode.start(ctx, current_slice.header);
        decode.push(ctx, &current_slice.buf[..]);
        decode.end(ctx);
        let capture = decode.into_handler();
        let mut r = rbsp::BitReader::new(&capture.buf[1..]);
        match nal::slice::SliceHeader::read(ctx, &mut r, current_slice.header) {
            Ok((header, sps, pps)) => {
                println!("{:#?}", header);
            },
            Err(e) => println!("slice_header() error: SliceHeaderError::{:?}", e),
        }
    }
}
fuzz_target!(|data: &[u8]| {
    let mut switch = h264_reader::nal::NalSwitch::default();
    let sei_handler = h264_reader::nal::sei::SeiNalHandler::new(FuzzSeiPayoadReader { switch: SeiSwitch::default() });
    let sps_handler = SeqParameterSetNalHandler::default();
    let pps_handler = PicParameterSetNalHandler::default();
    let slice_wout_part_idr_handler = SliceFuzz::new();
    let slice_wout_part_nonidr_handler = SliceFuzz::new();
    switch.put_handler(h264_reader::nal::UnitType::SEI, Box::new(RefCell::new(sei_handler)));
    switch.put_handler(h264_reader::nal::UnitType::SeqParameterSet, Box::new(RefCell::new(sps_handler)));
    switch.put_handler(h264_reader::nal::UnitType::PicParameterSet, Box::new(RefCell::new(pps_handler)));
    switch.put_handler(h264_reader::nal::UnitType::SliceLayerWithoutPartitioningIdr, Box::new(RefCell::new(slice_wout_part_idr_handler)));
    switch.put_handler(h264_reader::nal::UnitType::SliceLayerWithoutPartitioningNonIdr, Box::new(RefCell::new(slice_wout_part_nonidr_handler)));

    let mut ctx = Context::default();
    let mut annexb_reader = AnnexBReader::new(switch);
    annexb_reader.push(&mut ctx, data);
    annexb_reader.reset(&mut ctx);
    ctx.sps().for_each(|sps| { let _ = sps.pixel_dimensions(); });
});
