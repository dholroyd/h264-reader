pub mod buffering_period;
pub mod pic_timing;
pub mod user_data_registered_itu_t_t35;

use crate::rbsp::BitReaderError;
use std::convert::TryFrom;
use std::fmt::{Debug, Formatter};
use std::io::BufRead;
use hex_slice::AsHex;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum HeaderType {
    BufferingPeriod,
    PicTiming,
    PanScanRect,
    FillerPayload,
    UserDataRegisteredItuTT35,
    UserDataUnregistered,
    RecoveryPoint,
    DecRefPicMarkingRepetition,
    SparePic,
    SceneInfo,
    SubSeqInfo,
    SubSeqLayerCharacteristics,
    SubSeqCharacteristics,
    FullFrameFreeze,
    FullFrameFreezeRelease,
    FullFrameSnapshot,
    ProgressiveRefinementSegmentStart,
    ProgressiveRefinementSegmentEnd,
    MotionConstrainedSliceGroupSet,
    FilmGrainCharacteristics,
    DeblockingFilterDisplayPreference,
    StereoVideoInfo,
    PostFilterHint,
    ToneMappingInfo,
    ScalabilityInfo,
    SubPicScalableLayer,
    NonRequiredLayerRep,
    PriorityLayerInfo,
    LayersNotPresent,
    LayerDependencyChange,
    ScalableNesting,
    BaseLayerTemporalHrd,
    QualityLayerIntegrityCheck,
    RedundantPicProperty,
    Tl0DepRepIndex,
    TlSwitchingPoint,
    ParallelDecodingInfo,
    MvcScalableNesting,
    ViewScalabilityInfo,
    MultiviewSceneInfo,
    MultiviewAcquisitionInfo,
    NonRequiredViewComponent,
    ViewDependencyChange,
    OperationPointsNotPresent,
    BaseViewTemporalHrd,
    FramePackingArrangement,
    MultiviewViewPosition,
    DisplayOrientation,
    MvcdScalableNesting,
    MvcdViewScalabilityInfo,
    DepthRepresentationInfo,
    ThreeDimensionalReferenceDisplaysInfo,
    DepthTiming,
    DepthSamplingInfo,
    ConstrainedDepthParameterSetIdentifier,
    GreenMetadata,
    MasteringDisplayColourVolume,
    ColourRemappingInfo,
    AlternativeTransferCharacteristics,
    AlternativeDepthInfo,
    ReservedSeiMessage(u32),
}
impl HeaderType {
    fn from_id(id: u32) -> HeaderType {
        match id {
            0 => HeaderType::BufferingPeriod,
            1 => HeaderType::PicTiming,
            2 => HeaderType::PanScanRect,
            3 => HeaderType::FillerPayload,
            4 => HeaderType::UserDataRegisteredItuTT35,
            5 => HeaderType::UserDataUnregistered,
            6 => HeaderType::RecoveryPoint,
            7 => HeaderType::DecRefPicMarkingRepetition,
            8 => HeaderType::SparePic,
            9 => HeaderType::SceneInfo,
            10 => HeaderType::SubSeqInfo,
            11 => HeaderType::SubSeqLayerCharacteristics,
            12 => HeaderType::SubSeqCharacteristics,
            13 => HeaderType::FullFrameFreeze,
            14 => HeaderType::FullFrameFreezeRelease,
            15 => HeaderType::FullFrameSnapshot,
            16 => HeaderType::ProgressiveRefinementSegmentStart,
            17 => HeaderType::ProgressiveRefinementSegmentEnd,
            18 => HeaderType::MotionConstrainedSliceGroupSet,
            19 => HeaderType::FilmGrainCharacteristics,
            20 => HeaderType::DeblockingFilterDisplayPreference,
            21 => HeaderType::StereoVideoInfo,
            22 => HeaderType::PostFilterHint,
            23 => HeaderType::ToneMappingInfo,
            24 => HeaderType::ScalabilityInfo,
            25 => HeaderType::SubPicScalableLayer,
            26 => HeaderType::NonRequiredLayerRep,
            27 => HeaderType::PriorityLayerInfo,
            28 => HeaderType::LayersNotPresent,
            29 => HeaderType::LayerDependencyChange,
            30 => HeaderType::ScalableNesting,
            31 => HeaderType::BaseLayerTemporalHrd,
            32 => HeaderType::QualityLayerIntegrityCheck,
            33 => HeaderType::RedundantPicProperty,
            34 => HeaderType::Tl0DepRepIndex,
            35 => HeaderType::TlSwitchingPoint,
            36 => HeaderType::ParallelDecodingInfo,
            37 => HeaderType::MvcScalableNesting,
            38 => HeaderType::ViewScalabilityInfo,
            39 => HeaderType::MultiviewSceneInfo,
            40 => HeaderType::MultiviewAcquisitionInfo,
            41 => HeaderType::NonRequiredViewComponent,
            42 => HeaderType::ViewDependencyChange,
            43 => HeaderType::OperationPointsNotPresent,
            44 => HeaderType::BaseViewTemporalHrd,
            45 => HeaderType::FramePackingArrangement,
            46 => HeaderType::MultiviewViewPosition,
            47 => HeaderType::DisplayOrientation,
            48 => HeaderType::MvcdScalableNesting,
            49 => HeaderType::MvcdViewScalabilityInfo,
            50 => HeaderType::DepthRepresentationInfo,
            51 => HeaderType::ThreeDimensionalReferenceDisplaysInfo,
            52 => HeaderType::DepthTiming,
            53 => HeaderType::DepthSamplingInfo,
            54 => HeaderType::ConstrainedDepthParameterSetIdentifier,
            56 => HeaderType::GreenMetadata,
            137 => HeaderType::MasteringDisplayColourVolume,
            142 => HeaderType::ColourRemappingInfo,
            147 => HeaderType::AlternativeTransferCharacteristics,
            188 => HeaderType::AlternativeDepthInfo,
            _ => HeaderType::ReservedSeiMessage(id),
        }
    }
}

/// Reader of messages in an SEI NAL.
pub struct SeiReader<'a, R: BufRead + Clone> {
    reader: R,
    scratch: &'a mut Vec<u8>,
    payloads_seen: usize,
    done: bool,
}

impl<'a, R: BufRead + Clone> SeiReader<'a, R> {
    pub fn from_rbsp_bytes(reader: R, scratch: &'a mut Vec<u8>) -> Self {
        Self {
            reader,
            scratch,
            payloads_seen: 0,
            done: false,
        }
    }

    /// Returns the next payload.
    ///
    /// This is unfortunately not compatible with `std::iter::Iterator` because
    /// of lifetime constraints.
    pub fn next(&mut self) -> Result<Option<SeiMessage<'_>>, BitReaderError> {
        if self.done {
            return Ok(None);
        }

        // Fused iterator: once this returns `None` or `Err`, don't try to parse
        // again and return a strange result. (Set done preemptively then clear
        // it on success, rather than adjust each failure path.)
        self.done = true;
        let payload_type = read_u32(&mut self.reader, "payload_type")?;

        // If this is not the first payload, the byte we just read may actually
        // be a rbsp_trailing_bits (which is always byte-aligned). Check for EOF.
        if payload_type == 0x80 && self.payloads_seen > 0 {
            let buf = self
                .reader
                .fill_buf()
                .map_err(|e| BitReaderError::ReaderErrorFor("payload_type", e))?;
            if buf.is_empty() {
                return Ok(None);
            }
        }
        let payload_type = HeaderType::from_id(payload_type);
        let payload_len = usize::try_from(read_u32(&mut self.reader, "payload_len")?).unwrap();

        // Read into scratch. We could instead directly use reader's buffer if
        // the next chunk is long enough, or pass along a BufRead that uses
        // something like std::io::Take, but it's probably not worth the
        // complexity.
        self.scratch.resize(payload_len, 0);
        self.reader
            .read_exact(&mut self.scratch)
            .map_err(|e| BitReaderError::ReaderErrorFor("payload", e))?;

        self.payloads_seen += 1;
        self.done = false;
        Ok(Some(SeiMessage {
            payload_type,
            payload: &self.scratch[..],
        }))
    }
}

#[derive(PartialEq, Eq)]
pub struct SeiMessage<'a> {
    pub payload_type: HeaderType,
    pub payload: &'a [u8],
}

impl<'a> Debug for SeiMessage<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SeiMessage")
            .field("payload_type", &self.payload_type)
            .field("payload", &format!("{:02x}", self.payload.plain_hex(false)))
            .finish()
    }
}

/// Reads a u32 in the special `sei_message` format used for payload type and size.
fn read_u32<R: BufRead>(reader: &mut R, name: &'static str) -> Result<u32, BitReaderError> {
    let mut acc = 0u32;
    loop {
        let mut buf = [0];
        reader
            .read_exact(&mut buf[..])
            .map_err(|e| BitReaderError::ReaderErrorFor(name, e))?;
        let byte = buf[0];
        acc = acc.checked_add(u32::from(byte)).ok_or_else(|| {
            BitReaderError::ReaderErrorFor(
                name,
                std::io::Error::new(std::io::ErrorKind::InvalidData, "overflowed u32"),
            )
        })?;
        if byte != 0xFF {
            return Ok(acc);
        }
    }
}

#[cfg(test)]
mod test {
    use crate::nal::{Nal, RefNal};

    use super::*;

    #[test]
    fn it_works() {
        let data = [
            0x06, // SEI
            // header 1
            0x01, // type
            0x01, // len
            0x01, // payload
            // header 2
            0x02, // type
            0x02, // len
            0x02, 0x02, // payload
            0x80, // rbsp_trailing_bits
        ];
        let nal = RefNal::new(&data[..], &[], true);
        let mut scratch = Vec::new();
        let mut r = SeiReader::from_rbsp_bytes(nal.rbsp_bytes(), &mut scratch);
        let m1 = r.next().unwrap().unwrap();
        assert_eq!(m1.payload_type, HeaderType::PicTiming);
        assert_eq!(m1.payload, &[0x01]);
        let m2 = r.next().unwrap().unwrap();
        assert_eq!(m2.payload_type, HeaderType::PanScanRect);
        assert_eq!(m2.payload, &[0x02, 0x02]);
        assert_eq!(r.next().unwrap(), None);
        assert_eq!(r.next().unwrap(), None);
    }
}
