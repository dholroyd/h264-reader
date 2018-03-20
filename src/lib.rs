#[cfg(test)]
#[macro_use]
extern crate hex_literal;
extern crate bitreader;
#[cfg(test)]
extern crate hex_slice;

pub mod rbsp;
pub mod annexb;
pub mod nal;

/// Contextual data that needs to be tracked between evaluations of different portions of H264
/// syntax.
pub struct Context {
    seq_param_sets: Vec<Option<nal::sps::SeqParameterSet>>
}
impl Default for Context {
    fn default() -> Self {
        let mut seq_param_sets = vec!();
        for _ in 0..32 { seq_param_sets.push(None); }
        Context {
            seq_param_sets
        }
    }
}
impl Context {
    fn sps_by_id(&self, id: nal::pps::ParamSetId) -> Option<&nal::sps::SeqParameterSet> {
        if id.id() > 31 {
            None
        } else {
            self.seq_param_sets[id.id() as usize].as_ref()
        }
    }
    fn put_seq_param_set(&mut self, sps: nal::sps::SeqParameterSet) {
        let i = sps.seq_parameter_set_id as usize;
        self.seq_param_sets[i] = Some(sps);
    }
}