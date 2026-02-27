//! Roundtrip fuzz test: decode(ByteWriter(rbsp)) == rbsp for arbitrary RBSP bytes.

#![no_main]
use h264_reader::rbsp::{decode_nal, ByteWriter};
use libfuzzer_sys::fuzz_target;
use std::io::Write as _;

fuzz_target!(|data: &[u8]| {
    // Single write: encode then decode must recover the original RBSP.
    let mut nal = vec![0x01u8]; // NAL header byte
    ByteWriter::new(&mut nal).write_all(data).unwrap();
    let decoded = decode_nal(&nal).unwrap();
    assert_eq!(&*decoded, data, "decode(ByteWriter(rbsp)) != rbsp");

    // Also exercise split writes at sampled boundaries to verify that
    // zero_count state is preserved across separate write() calls.
    // Limit to 16 split points so runtime stays O(n) not O(n^2).
    let step = (data.len() / 16).max(1);
    for split in (0..=data.len()).step_by(step) {
        let (head, tail) = data.split_at(split);
        let mut buf = vec![0x01u8]; // NAL header byte
        let mut w = ByteWriter::new(&mut buf);
        w.write_all(head).unwrap();
        w.write_all(tail).unwrap();
        drop(w);
        let decoded2 = decode_nal(&buf).unwrap();
        assert_eq!(&*decoded2, data, "split-write decode mismatch at split={split}");
    }
});
