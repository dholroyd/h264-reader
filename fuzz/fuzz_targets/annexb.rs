//! Tests the Annex B parser doesn't crash and has consistent output between a
//! single push call and a pair of push split at each possible byte location.

#![no_main]
use hex_slice::AsHex;
use h264_reader::Context;
use libfuzzer_sys::fuzz_target;
use std::convert::TryFrom;

/// Encodes the stream as (4-byte length prefix, NAL)*, as commonly seen in AVC files.
#[derive(Default)]
struct AvcBuilder {
    cur: Vec<u8>,
}

impl h264_reader::annexb::NalReader for AvcBuilder {
    type Ctx = Vec<u8>;

    fn push(&mut self, ctx: &mut Context<Self::Ctx>, buf: &[u8], end: bool) {
        assert!(!buf.is_empty() || (!self.cur.is_empty() || end));
        self.cur.extend_from_slice(buf);
        if end {
            let len = u32::try_from(self.cur.len()).unwrap();
            ctx.user_context.extend_from_slice(&len.to_be_bytes()[..]);
            ctx.user_context.extend_from_slice(&self.cur[..]);
            self.cur.clear();
        }
    }
}

fuzz_target!(|data: &[u8]| {
    // Parse in a single push.
    let mut single_push_ctx = h264_reader::Context::new(Vec::new());
    let mut single_push = h264_reader::annexb::AnnexBReader::new(AvcBuilder::default());
    single_push.push(&mut single_push_ctx, data);
    single_push.reset(&mut single_push_ctx);

    for i in 0..data.len() {
        // Parse in a split push.
        let mut split_push_ctx = h264_reader::Context::new(Vec::new());
        let mut split_push = h264_reader::annexb::AnnexBReader::new(AvcBuilder::default());
        let (head, tail) = data.split_at(i);
        split_push.push(&mut split_push_ctx, head);
        split_push.push(&mut split_push_ctx, &[]); // also ensure empty pushes don't break.
        split_push.push(&mut split_push_ctx, tail);
        split_push.reset(&mut split_push_ctx);

        assert!(single_push_ctx.user_context.as_slice() == split_push_ctx.user_context.as_slice(),
                "inconsistent output.\n\
                split point: {}\n\
                input:       {:02x}\n\
                single push: {:02x}\n\
                split push:  {:02x}",
                i,
                data.as_hex(),
                single_push_ctx.user_context.as_hex(),
                split_push_ctx.user_context.as_hex());
    }
});
