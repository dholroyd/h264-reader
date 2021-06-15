//! Tests the Annex B parser doesn't crash and has consistent output between a
//! single push call and a pair of push split at each possible byte location.

#![no_main]
use h264_reader::annexb::AnnexBReader;
use hex_slice::AsHex;
use libfuzzer_sys::fuzz_target;
use std::convert::TryFrom;

/// Encodes the stream as (4-byte length prefix, NAL)*, as commonly seen in AVC files.
#[derive(Default)]
struct AvcBuilder {
    cur: Vec<u8>,
    all: Vec<u8>,
}

impl h264_reader::push::NalFragmentHandler for AvcBuilder {
    fn nal_fragment(&mut self, bufs: &[&[u8]], end: bool) {
        assert!(!bufs.is_empty() || (!self.cur.is_empty() || end));
        for buf in bufs {
            assert!(!buf.is_empty());
            self.cur.extend_from_slice(buf);
        }
        if end {
            let len = u32::try_from(self.cur.len()).unwrap();
            self.all.extend_from_slice(&len.to_be_bytes()[..]);
            self.all.extend_from_slice(&self.cur[..]);
            self.cur.clear();
        }
    }
}

fuzz_target!(|data: &[u8]| {
    // Parse in a single push.
    let mut single_push = AnnexBReader::for_fragment_handler(AvcBuilder::default());
    single_push.push(data);
    single_push.reset();
    let single_avc = single_push.into_fragment_handler();

    for i in 0..data.len() {
        // Parse in a split push.
        let mut split_push = AnnexBReader::for_fragment_handler(AvcBuilder::default());
        let (head, tail) = data.split_at(i);
        split_push.push(head);
        split_push.push(&[]); // also ensure empty pushes don't break.
        split_push.push(tail);
        split_push.reset();
        let split_avc = split_push.into_fragment_handler();

        assert!(single_avc.all.as_slice() == split_avc.all.as_slice(),
                "inconsistent output.\n\
                split point: {}\n\
                input:       {:02x}\n\
                single push: {:02x}\n\
                split push:  {:02x}",
                i,
                data.as_hex(),
                single_avc.all.as_hex(),
                split_avc.all.as_hex());
    }
});
