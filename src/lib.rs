//! Parser for H264 bitstream syntax.  Not a video decoder.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

pub mod rbsp;
pub mod annexb;
pub mod nal;
pub mod avcc;

/// Contextual data that needs to be tracked between evaluations of different portions of H264
/// syntax.
pub struct Context<Ctx> {
    seq_param_sets: Vec<Option<nal::sps::SeqParameterSet>>,
    pic_param_sets: Vec<Option<nal::pps::PicParameterSet>>,
    pub user_context: Ctx,
}
impl Default for Context<()> {
    fn default() -> Self {
        Self::new(())
    }
}
impl<Ctx> Context<Ctx> {
    pub fn new(user_context: Ctx) -> Self {
        let mut seq_param_sets = vec!();
        for _ in 0..32 { seq_param_sets.push(None); }
        let mut pic_param_sets = vec!();
        for _ in 0..32 { pic_param_sets.push(None); }
        Context {
            seq_param_sets,
            pic_param_sets,
            user_context,
        }
    }
}
impl<Ctx> Context<Ctx> {
    pub fn sps_by_id(&self, id: nal::pps::ParamSetId) -> Option<&nal::sps::SeqParameterSet> {
        if id.id() > 31 {
            None
        } else {
            self.seq_param_sets[id.id() as usize].as_ref()
        }
    }
    pub fn put_seq_param_set(&mut self, sps: nal::sps::SeqParameterSet) {
        let i = sps.seq_parameter_set_id.id() as usize;
        self.seq_param_sets[i] = Some(sps);
    }
    pub fn pps_by_id(&self, id: nal::pps::ParamSetId) -> Option<&nal::pps::PicParameterSet> {
        if id.id() > 31 {
            None
        } else {
            self.pic_param_sets[id.id() as usize].as_ref()
        }
    }
    pub fn put_pic_param_set(&mut self, pps: nal::pps::PicParameterSet) {
        let i = pps.pic_parameter_set_id.id() as usize;
        self.pic_param_sets[i] = Some(pps);
    }
}