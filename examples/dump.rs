use h264_reader::annexb::AnnexBReader;
use h264_reader::nal::pps::PicParameterSet;
use h264_reader::nal::sei::buffering_period::BufferingPeriod;
use h264_reader::nal::sei::pic_timing::PicTiming;
use h264_reader::nal::sei::user_data_registered_itu_t_t35::ItuTT35;
use h264_reader::nal::sei::HeaderType;
use h264_reader::nal::slice::SliceHeader;
use h264_reader::nal::sps::SeqParameterSet;
use h264_reader::nal::{sei, Nal, RefNal, UnitType};
use h264_reader::push::NalInterest;
use h264_reader::Context;
use hex_slice::AsHex;
use std::io::Read;

fn main() {
    let path = {
        let mut args = std::env::args_os();
        if args.len() != 2 {
            eprintln!("Usage: dump-param-sets path/to/data.h264");
            std::process::exit(1);
        }
        args.nth(1).unwrap()
    };

    let mut file = std::fs::File::open(path).expect("open");

    // Create a context to keep track of SPS and PPS NALs that we receive. It *needs* to be
    // persistent through all parsing
    let mut ctx = Context::new();

    // Then we prepare an AnnexBReader to handle the parsed data
    let mut reader = AnnexBReader::accumulate(|nal: RefNal<'_>| {
        // We only ever want to parse complete NALs.
        // You can filter for the specific types of NALs you're
        // interested in and NalInterest::Ignore the rest here.
        //
        // If a NAL is incomplete, trying to read its data will result in a WouldBlock.
        if !nal.is_complete() {
            return NalInterest::Buffer;
        }

        // Parse the NAL header, so we know what the NAL type is
        let nal_header = nal.header().unwrap();
        let nal_unit_type = nal_header.nal_unit_type();

        // Decode the NAL types that we're interested in
        match nal_unit_type {
            UnitType::SeqParameterSet => {
                hex_dump(&nal);
                let data = SeqParameterSet::from_bits(nal.rbsp_bits()).unwrap();
                println!("{:#?}", data);
                // Don't forget to tell stream_context that we have a new SPS.
                // If you want to handle it separately, you can clone the struct before passing along,
                // But if you only care about it when a slice calls for it, you don't have to handle it here.
                ctx.put_seq_param_set(data);
            }
            UnitType::PicParameterSet => {
                hex_dump(&nal);
                // Same as when parsing an SPS, except it borrows the stream context so it can pick out
                // the SPS that this PPS references
                let data = PicParameterSet::from_bits(&ctx, nal.rbsp_bits()).unwrap();
                println!("{:#?}", data);
                // Same as with an SPS, tell the context that we've found a PPS
                ctx.put_pic_param_set(data);
            }
            UnitType::SliceLayerWithoutPartitioningIdr
            | UnitType::SliceLayerWithoutPartitioningNonIdr => {
                let mut bits = nal.rbsp_bits();
                // We can parse the slice header, and it will give us:
                let (
                    header,      // The header of the slice
                    _seq_params, // A borrow of the SPS...
                    _pic_params, // ...and PPS activated by the header
                ) = SliceHeader::from_bits(
                    &ctx,
                    &mut bits, // takes a mutable borrow so the body parser can continue from where this ended
                    nal_header,
                )
                .unwrap();
                println!("{:#?}", header);
            }
            UnitType::SEI => {
                let mut scratch = vec![];
                let mut reader = sei::SeiReader::from_rbsp_bytes(nal.rbsp_bytes(), &mut scratch);
                loop {
                    match reader.next() {
                        Ok(Some(sei)) => {
                            match sei.payload_type {
                                HeaderType::BufferingPeriod => {
                                    let bp = BufferingPeriod::read(&ctx, &sei);
                                    println!("{:#?}", bp);
                                }
                                HeaderType::PicTiming => {
                                    let pt =
                                        PicTiming::read(ctx.sps().next().expect("first sps"), &sei);
                                    println!("{:#?}", pt);
                                }
                                HeaderType::UserDataRegisteredItuTT35 => {
                                    match ItuTT35::read(&sei) {
                                        Ok(ud) => {
                                            match ud.0 {
                                                ItuTT35::UnitedStates => {
                                                    // TODO: check for ATSC provider code, look
                                                    //       at caption data etc
                                                    println!("{:#?}", ud);
                                                }
                                                _ => {
                                                    println!("{:#?}", ud);
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            println!("{:?}", e);
                                        }
                                    }
                                }
                                _ => {
                                    println!("{:#?}", sei);
                                }
                            }
                        }
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

    // Push data. Doesn't have to be aligned in any way. You can push multiple times for a single
    // NAL, or send an entire file in at once.
    let mut buf = vec![0; 2 * 1024 * 1024];
    loop {
        match file.read(&mut buf[..]).expect("read") {
            0 => break,
            n => reader.push(&buf[0..n]),
        }
    }

    // If we're sure that the entire current NAL has been pushed, then we can call this to signal
    // that the parser should immediately stop waiting for a new NAL marker.
    reader.reset();
}

fn hex_dump(nal: &RefNal) {
    let mut nal_rbsp_bytes = vec![];
    nal.rbsp_bytes()
        .read_to_end(&mut nal_rbsp_bytes)
        .expect("read NAL");
    println!(
        "{:?}: {:02x}",
        nal.header().unwrap().nal_unit_type(),
        &nal_rbsp_bytes[..].plain_hex(false)
    );
}
