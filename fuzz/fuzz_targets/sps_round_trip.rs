//! Fuzz test: encode(decode(sps_rbsp)) == sps_rbsp.
//!
//! For any SPS RBSP that successfully parses, re-encoding must
//! produce identical bytes, other than removing trailing zero bytes.

#![no_main]
use h264_reader::nal::sps::SeqParameterSet;
use h264_reader::nal::WritableNal;
use h264_reader::rbsp::{BitReader, BitWriter};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Only check the minimal case with no extra RBSP trailing zero bytes.
    // The encoder is not expected to preserve these.
    if data.last() == Some(&0) {
        return;
    }

    // Try to parse the input as SPS RBSP (no NAL header byte or emulation-prevention-three bytes).
    let sps = match SeqParameterSet::from_bits(BitReader::new(data)) {
        Ok(s) => s,
        Err(_) => return,
    };

    // Re-encode.
    let mut encoded = Vec::new();
    let mut bw = BitWriter::new(&mut encoded);
    sps.write_bits(&mut bw)
        .expect("write_bits should not fail for a valid SPS");

    assert_eq!(data, encoded, "data != encode(decode(data)) on {sps:#?}");
});
