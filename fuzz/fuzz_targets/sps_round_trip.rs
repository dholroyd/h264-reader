//! Fuzz test: decode(encode(decode(sps_rbsp))) == decode(sps_rbsp).
//!
//! For any SPS RBSP that successfully parses, re-encoding and re-parsing must
//! produce a structurally identical SeqParameterSet.
//!
//! We do not require the stricter encode(decode(sps_rbsp)) == sps_rbsp, as there
//! are allowed deviations:
//!
//! * the `rbsp_trailing_zero_bits` can contain arbitrarily many zero bytes.
//! * currently `sps::ScalingList` stores computed values rather than the
//!   `delta_scale` in the bitstream. There are multiple `delta_scale` sequences
//!   that can produce the same computed values. Arguably this means we should
//!   store `delta_scale` instead to match the spirit of a "bitstream-oriented
//!   representation" described at <https://github.com/dholroyd/h264-reader/pull/90#discussion_r1929611947>.
//!   But for now we allow the deviation.

#![no_main]
use h264_reader::nal::sps::SeqParameterSet;
use h264_reader::nal::WritableNal;
use h264_reader::rbsp::{BitReader, BitWriter};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Try to parse the input as SPS RBSP (no NAL header byte).
    let sps = match SeqParameterSet::from_bits(BitReader::new(data)) {
        Ok(s) => s,
        Err(_) => return,
    };

    // Re-encode to RBSP (plain Vec<u8>, no emulation prevention).
    let mut encoded = Vec::new();
    let mut bw = BitWriter::new(&mut encoded);
    sps.write_bits(&mut bw)
        .expect("write_bits should not fail for a valid SPS");

    // Re-parse the encoded output and compare structurally.
    let sps2 = SeqParameterSet::from_bits(BitReader::new(&encoded[..]))
        .expect("re-encoded SPS must parse successfully");

    assert_eq!(
        sps, sps2,
        "decode(encode(decode(input))) mismatch on {sps:#?}"
    );
});
