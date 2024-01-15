//! Creates a context from an encoded
//! [`h264_reader::avc::AVCDecoderConfigurationRecord`] and prints it.

use std::convert::TryFrom;

use h264_reader::avcc::AvcDecoderConfigurationRecord;

fn main() {
    let path = {
        let mut args = std::env::args_os();
        if args.len() != 2 {
            eprintln!("Usage: decode_avcc path/to/avcc");
            std::process::exit(1);
        }
        args.nth(1).unwrap()
    };

    let raw = std::fs::read(path).unwrap();
    let record = AvcDecoderConfigurationRecord::try_from(&raw[..]).unwrap();
    let ctx = record.create_context().unwrap();
    println!("{:#?}", &ctx);
}
