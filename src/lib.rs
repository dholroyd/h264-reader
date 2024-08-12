//! Parser for H264 bitstream syntax.  Not a video decoder.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

use std::fmt::Debug;

pub use bitstream_io;

pub mod annexb;
pub mod avcc;
pub mod nal;
pub mod push;
pub mod rbsp;

/// Contextual data that needs to be tracked between evaluations of different portions of H264
/// syntax.
#[derive(Default, Debug)]
pub struct Context {
    seq_param_sets: ParamSetMap<nal::sps::SeqParameterSet>,
    pic_param_sets: ParamSetMap<nal::pps::PicParameterSet>,
}
impl Context {
    #[inline]
    pub fn new() -> Self {
        Default::default()
    }
    #[inline]
    pub fn sps_by_id(&self, id: nal::sps::SeqParamSetId) -> Option<&nal::sps::SeqParameterSet> {
        self.seq_param_sets.get(usize::from(id.id()))
    }
    #[inline]
    pub fn sps(&self) -> impl Iterator<Item = &nal::sps::SeqParameterSet> {
        self.seq_param_sets.iter()
    }
    #[inline]
    pub fn put_seq_param_set(&mut self, sps: nal::sps::SeqParameterSet) {
        let i = usize::from(sps.seq_parameter_set_id.id());
        self.seq_param_sets.put(i, sps);
    }
    #[inline]
    pub fn pps_by_id(&self, id: nal::pps::PicParamSetId) -> Option<&nal::pps::PicParameterSet> {
        self.pic_param_sets.get(usize::from(id.id()))
    }
    #[inline]
    pub fn pps(&self) -> impl Iterator<Item = &nal::pps::PicParameterSet> {
        self.pic_param_sets.iter()
    }
    #[inline]
    pub fn put_pic_param_set(&mut self, pps: nal::pps::PicParameterSet) {
        let i = usize::from(pps.pic_parameter_set_id.id());
        self.pic_param_sets.put(i, pps);
    }
}

/// A map for very small indexes; SPS/PPS IDs must be in `[0, 32)`, and typically only 0 is used.
struct ParamSetMap<T>(Vec<Option<T>>);
impl<T> Default for ParamSetMap<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}
impl<T> ParamSetMap<T> {
    fn get(&self, index: usize) -> Option<&T> {
        self.0.get(index).map(Option::as_ref).flatten()
    }
    fn put(&mut self, index: usize, t: T) {
        if self.0.len() <= index {
            self.0.resize_with(index + 1, || None);
        }
        self.0[index] = Some(t);
    }
    fn iter(&self) -> impl Iterator<Item = &T> {
        self.0.iter().filter_map(Option::as_ref)
    }
}
impl<T: Debug> Debug for ParamSetMap<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map()
            .entries(
                self.0
                    .iter()
                    .enumerate()
                    .filter_map(|(i, p)| p.as_ref().map(|p| (i, p))),
            )
            .finish()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn map() {
        let mut s = super::ParamSetMap::default();
        assert!(s.iter().copied().collect::<Vec<_>>().is_empty());
        s.put(0, 0);
        assert_eq!(s.iter().copied().collect::<Vec<_>>(), &[0]);
        s.put(2, 2);
        assert_eq!(s.iter().copied().collect::<Vec<_>>(), &[0, 2]);
        s.put(1, 1);
        assert_eq!(s.iter().copied().collect::<Vec<_>>(), &[0, 1, 2]);
    }
}
