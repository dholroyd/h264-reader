use super::SeiCompletePayloadReader;
use Context;
use nal::sei::HeaderType;
use nal::pps;
use rbsp::RbspBitReader;
use bitreader;

#[derive(Debug)]
enum BufferingPeriodError {
    ReaderError(bitreader::BitReaderError),
    UndefinedSeqParamSetId(pps::ParamSetId),
    InvalidSeqParamSetId(pps::ParamSetIdError),
}
impl From<bitreader::BitReaderError> for BufferingPeriodError {
    fn from(e: bitreader::BitReaderError) -> Self {
        BufferingPeriodError::ReaderError(e)
    }
}
impl From<pps::ParamSetIdError> for BufferingPeriodError {
    fn from(e: pps::ParamSetIdError) -> Self {
        BufferingPeriodError::InvalidSeqParamSetId(e)
    }
}

#[derive(Debug)]
struct InitialCpbRemoval {
    initial_cpb_removal_delay: u32,
    initial_cpb_removal_delay_offset: u32,
}

fn read_cpb_removal_delay_list(r: &mut RbspBitReader, count: usize, length: u8) -> Result<Vec<InitialCpbRemoval>,bitreader::BitReaderError> {
    let mut res = vec!();
    for _ in 0..count {
        res.push(InitialCpbRemoval {
            initial_cpb_removal_delay: r.read_u32(length)?,
            initial_cpb_removal_delay_offset: r.read_u32(length)?,
        });
    }
    Ok(res)
}

#[derive(Debug)]
struct BufferingPeriod {
    nal_hrd_bp: Option<Vec<InitialCpbRemoval>>,
    vcl_hrd_bp: Option<Vec<InitialCpbRemoval>>,
}
impl BufferingPeriod {
    fn read(ctx: &Context, buf: &[u8]) -> Result<BufferingPeriod,BufferingPeriodError> {
        let mut r = RbspBitReader::new(buf);
        let seq_parameter_set_id = pps::ParamSetId::from_u32(r.read_ue()?)?;
        match ctx.sps_by_id(seq_parameter_set_id) {
            None => Err(BufferingPeriodError::UndefinedSeqParamSetId(seq_parameter_set_id)),
            Some(sps) => {
                let vui = sps.vui_parameters.as_ref();
                let nal_hrd_bp = if let Some((cpb_removal_delay_length_minus1, nal_cpb_cnt)) = vui
                    .and_then(|vui_params| vui_params.nal_hrd_parameters.as_ref() )
                    .and_then(|nal_hrd_params| Some((nal_hrd_params.cpb_removal_delay_length_minus1, nal_hrd_params.cpb_specs.len())) )
                {
                    Some(read_cpb_removal_delay_list(&mut r, nal_cpb_cnt, cpb_removal_delay_length_minus1+1)?)
                } else {
                    None
                };
                let vcl_hrd_bp = if let Some((cpb_removal_delay_length_minus1, vcl_cpb_cnt)) = vui
                    .and_then(|vui_params| vui_params.vcl_hrd_parameters.as_ref() )
                    .and_then(|vcl_hrd_params| Some((vcl_hrd_params.cpb_removal_delay_length_minus1, vcl_hrd_params.cpb_specs.len())) )
                {
                    Some(read_cpb_removal_delay_list(&mut r, vcl_cpb_cnt, cpb_removal_delay_length_minus1+1)?)
                } else {
                    None
                };

                Ok(BufferingPeriod {
                    nal_hrd_bp,
                    vcl_hrd_bp,
                })
            }
        }
    }
}
pub struct BufferingPeriodPayloadReader {

}
impl BufferingPeriodPayloadReader {
    pub fn new() -> BufferingPeriodPayloadReader {
        BufferingPeriodPayloadReader {
        }
    }
}
impl SeiCompletePayloadReader for BufferingPeriodPayloadReader {
    fn header(&mut self, ctx: &mut Context, payload_type: HeaderType, buf: &[u8]) {
        assert_eq!(payload_type, HeaderType::BufferingPeriod);
        match BufferingPeriod::read(ctx, buf) {
            Err(e) => eprintln!("Failure reading buffering_period: {:?}", e),
            Ok(buffering_period) => {
println!("buffering_period {:#?}", buffering_period);
            }
        }
    }
}