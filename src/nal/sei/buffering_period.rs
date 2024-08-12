use super::SeiMessage;
use crate::nal::sei::HeaderType;
use crate::nal::sps;
use crate::rbsp::BitRead;
use crate::rbsp::BitReaderError;
use crate::Context;

#[derive(Debug)]
pub enum BufferingPeriodError {
    ReaderError(BitReaderError),
    UndefinedSeqParamSetId(sps::SeqParamSetId),
    InvalidSeqParamSetId(sps::SeqParamSetIdError),
}
impl From<BitReaderError> for BufferingPeriodError {
    fn from(e: BitReaderError) -> Self {
        BufferingPeriodError::ReaderError(e)
    }
}
impl From<sps::SeqParamSetIdError> for BufferingPeriodError {
    fn from(e: sps::SeqParamSetIdError) -> Self {
        BufferingPeriodError::InvalidSeqParamSetId(e)
    }
}

#[derive(Debug, Eq, PartialEq)]
struct InitialCpbRemoval {
    initial_cpb_removal_delay: u32,
    initial_cpb_removal_delay_offset: u32,
}

fn read_cpb_removal_delay_list<R: BitRead>(
    r: &mut R,
    count: usize,
    length: u32,
) -> Result<Vec<InitialCpbRemoval>, BitReaderError> {
    let mut res = vec![];
    for _ in 0..count {
        res.push(InitialCpbRemoval {
            initial_cpb_removal_delay: r.read(length, "initial_cpb_removal_delay")?,
            initial_cpb_removal_delay_offset: r.read(length, "initial_cpb_removal_delay_offset")?,
        });
    }
    Ok(res)
}

#[derive(Debug, Eq, PartialEq)]
pub struct BufferingPeriod {
    nal_hrd_bp: Option<Vec<InitialCpbRemoval>>,
    vcl_hrd_bp: Option<Vec<InitialCpbRemoval>>,
}
impl BufferingPeriod {
    pub fn read(
        ctx: &Context,
        msg: &SeiMessage<'_>,
    ) -> Result<BufferingPeriod, BufferingPeriodError> {
        assert_eq!(msg.payload_type, HeaderType::BufferingPeriod);
        let mut r = crate::rbsp::BitReader::new(msg.payload);
        let seq_parameter_set_id =
            sps::SeqParamSetId::from_u32(r.read_ue("seq_parameter_set_id")?)?;
        let sps = ctx
            .sps_by_id(seq_parameter_set_id)
            .ok_or_else(|| BufferingPeriodError::UndefinedSeqParamSetId(seq_parameter_set_id))?;
        let vui = sps.vui_parameters.as_ref();
        let mut read = |p: &sps::HrdParameters| {
            read_cpb_removal_delay_list(
                &mut r,
                p.cpb_specs.len(),
                u32::from(p.initial_cpb_removal_delay_length_minus1) + 1,
            )
        };
        let nal_hrd_bp = vui
            .and_then(|v| v.nal_hrd_parameters.as_ref())
            .map(&mut read)
            .transpose()?;
        let vcl_hrd_bp = vui
            .and_then(|v| v.vcl_hrd_parameters.as_ref())
            .map(&mut read)
            .transpose()?;
        r.finish_sei_payload()?;
        Ok(BufferingPeriod {
            nal_hrd_bp,
            vcl_hrd_bp,
        })
    }
}

#[cfg(test)]
mod test {
    use hex_literal::hex;

    use crate::rbsp;

    use super::*;

    #[test]
    fn parse() {
        // https://standards.iso.org/ittf/PubliclyAvailableStandards/ISO_IEC_14496-4_2004_Amd_6_2005_Bitstreams/
        // This example taken from CVSEFDFT3_Sony_E.zip.
        let mut ctx = Context::default();
        let sps_rbsp = hex!(
            "
            4d 60 15 8d 8d 28 58 9d 08 00 00 0f a0 00 07 53
            07 00 00 00 92 7c 00 00 12 4f 80 fb dc 18 00 00
            0f 42 40 00 07 a1 20 7d ee 07 c6 0c 62 60
        "
        );
        ctx.put_seq_param_set(
            sps::SeqParameterSet::from_bits(rbsp::BitReader::new(&sps_rbsp[..])).unwrap(),
        );

        let msg = SeiMessage {
            payload_type: HeaderType::BufferingPeriod,
            payload: &hex!("d7 e4 00 00 57 e4 00 00 40")[..],
        };
        assert_eq!(
            BufferingPeriod::read(&ctx, &msg).unwrap(),
            BufferingPeriod {
                nal_hrd_bp: Some(vec![InitialCpbRemoval {
                    initial_cpb_removal_delay: 45_000,
                    initial_cpb_removal_delay_offset: 0,
                },]),
                vcl_hrd_bp: Some(vec![InitialCpbRemoval {
                    initial_cpb_removal_delay: 45_000,
                    initial_cpb_removal_delay_offset: 0,
                },]),
            }
        );
    }
}
