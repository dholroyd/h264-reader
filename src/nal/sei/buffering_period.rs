use super::SeiCompletePayloadReader;
use std::marker;
use crate::nal::{sps, pps};
use crate::rbsp::BitRead;
use crate::Context;
use crate::nal::sei::HeaderType;
use crate::rbsp::BitReaderError;
use log::*;

#[derive(Debug)]
enum BufferingPeriodError {
    ReaderError(BitReaderError),
    UndefinedSeqParamSetId(pps::ParamSetId),
    InvalidSeqParamSetId(pps::ParamSetIdError),
}
impl From<BitReaderError> for BufferingPeriodError {
    fn from(e: BitReaderError) -> Self {
        BufferingPeriodError::ReaderError(e)
    }
}
impl From<pps::ParamSetIdError> for BufferingPeriodError {
    fn from(e: pps::ParamSetIdError) -> Self {
        BufferingPeriodError::InvalidSeqParamSetId(e)
    }
}

#[derive(Debug, Eq, PartialEq)]
struct InitialCpbRemoval {
    initial_cpb_removal_delay: u32,
    initial_cpb_removal_delay_offset: u32,
}

fn read_cpb_removal_delay_list<R: BitRead>(r: &mut R, count: usize, length: u32) -> Result<Vec<InitialCpbRemoval>,BitReaderError> {
    let mut res = vec!();
    for _ in 0..count {
        res.push(InitialCpbRemoval {
            initial_cpb_removal_delay: r.read_u32(length, "initial_cpb_removal_delay")?,
            initial_cpb_removal_delay_offset: r.read_u32(length, "initial_cpb_removal_delay_offset")?,
        });
    }
    Ok(res)
}

#[derive(Debug, Eq, PartialEq)]
struct BufferingPeriod {
    nal_hrd_bp: Option<Vec<InitialCpbRemoval>>,
    vcl_hrd_bp: Option<Vec<InitialCpbRemoval>>,
}
impl BufferingPeriod {
    fn read<Ctx>(ctx: &Context<Ctx>, buf: &[u8]) -> Result<BufferingPeriod,BufferingPeriodError> {
        let mut r = crate::rbsp::BitReader::new(buf);
        let seq_parameter_set_id = pps::ParamSetId::from_u32(r.read_ue("seq_parameter_set_id")?)?;
        let sps = ctx.sps_by_id(seq_parameter_set_id)
            .ok_or_else(|| BufferingPeriodError::UndefinedSeqParamSetId(seq_parameter_set_id))?;
        let vui = sps.vui_parameters.as_ref();
        let mut read = |p: &sps::HrdParameters| read_cpb_removal_delay_list(
            &mut r,
            p.cpb_specs.len(),
            u32::from(p.initial_cpb_removal_delay_length_minus1) + 1,
        );
        let nal_hrd_bp = vui.and_then(|v| v.nal_hrd_parameters.as_ref()).map(&mut read).transpose()?;
        let vcl_hrd_bp = vui.and_then(|v| v.vcl_hrd_parameters.as_ref()).map(&mut read).transpose()?;
        Ok(BufferingPeriod {
            nal_hrd_bp,
            vcl_hrd_bp,
        })
    }
}
pub struct BufferingPeriodPayloadReader<Ctx> {
    phantom: marker::PhantomData<Ctx>,
}
impl<Ctx> Default for BufferingPeriodPayloadReader<Ctx> {
    fn default() -> Self {
        BufferingPeriodPayloadReader {
            phantom: marker::PhantomData
        }
    }
}
impl<Ctx> SeiCompletePayloadReader for BufferingPeriodPayloadReader<Ctx> {
    type Ctx = Ctx;

    fn header(&mut self, ctx: &mut Context<Ctx>, payload_type: HeaderType, buf: &[u8]) {
        assert_eq!(payload_type, HeaderType::BufferingPeriod);
        match BufferingPeriod::read(ctx, buf) {
            Err(e) => error!("Failure reading buffering_period: {:?}", e),
            Ok(buffering_period) => {
                info!("TODO: expose buffering_period {:#?}", buffering_period);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use hex_literal::hex;

    use super::*;

    #[test]
    fn parse() {
        // https://standards.iso.org/ittf/PubliclyAvailableStandards/ISO_IEC_14496-4_2004_Amd_6_2005_Bitstreams/
        // This example taken from CVSEFDFT3_Sony_E.zip.
        let mut ctx = Context::default();
        let sps_rbsp = hex!("
            4d 60 15 8d 8d 28 58 9d 08 00 00 0f a0 00 07 53
            07 00 00 00 92 7c 00 00 12 4f 80 fb dc 18 00 00
            0f 42 40 00 07 a1 20 7d ee 07 c6 0c 62 60
        ");
        ctx.put_seq_param_set(sps::SeqParameterSet::from_bytes(&sps_rbsp[..]).unwrap());

        let payload = &hex!("d7 e4 00 00 57 e4 00 00 40")[..];
        assert_eq!(BufferingPeriod::read(&ctx, payload).unwrap(), BufferingPeriod {
            nal_hrd_bp: Some(vec![
                InitialCpbRemoval {
                    initial_cpb_removal_delay: 45_000,
                    initial_cpb_removal_delay_offset: 0,
                },
            ]),
            vcl_hrd_bp: Some(vec![
                InitialCpbRemoval {
                    initial_cpb_removal_delay: 45_000,
                    initial_cpb_removal_delay_offset: 0,
                },
            ]),
        });
    }
}
