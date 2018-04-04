pub mod buffering_period;

use annexb::NalReader;
use Context;
use nal::NalHandler;
use nal::NalHeader;
use rbsp::RbspDecoder;

#[derive(Debug, Copy, Clone, PartialEq)]
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
            0   => HeaderType::BufferingPeriod,
            1   => HeaderType::PicTiming,
            2   => HeaderType::PanScanRect,
            3   => HeaderType::FillerPayload,
            4   => HeaderType::UserDataRegisteredItuTT35,
            5   => HeaderType::UserDataUnregistered,
            6   => HeaderType::RecoveryPoint,
            7   => HeaderType::DecRefPicMarkingRepetition,
            8   => HeaderType::SparePic,
            9   => HeaderType::SceneInfo,
            10  => HeaderType::SubSeqInfo,
            11  => HeaderType::SubSeqLayerCharacteristics,
            12  => HeaderType::SubSeqCharacteristics,
            13  => HeaderType::FullFrameFreeze,
            14  => HeaderType::FullFrameFreezeRelease,
            15  => HeaderType::FullFrameSnapshot,
            16  => HeaderType::ProgressiveRefinementSegmentStart,
            17  => HeaderType::ProgressiveRefinementSegmentEnd,
            18  => HeaderType::MotionConstrainedSliceGroupSet,
            19  => HeaderType::FilmGrainCharacteristics,
            20  => HeaderType::DeblockingFilterDisplayPreference,
            21  => HeaderType::StereoVideoInfo,
            22  => HeaderType::PostFilterHint,
            23  => HeaderType::ToneMappingInfo,
            24  => HeaderType::ScalabilityInfo,
            25  => HeaderType::SubPicScalableLayer,
            26  => HeaderType::NonRequiredLayerRep,
            27  => HeaderType::PriorityLayerInfo,
            28  => HeaderType::LayersNotPresent,
            29  => HeaderType::LayerDependencyChange,
            30  => HeaderType::ScalableNesting,
            31  => HeaderType::BaseLayerTemporalHrd,
            32  => HeaderType::QualityLayerIntegrityCheck,
            33  => HeaderType::RedundantPicProperty,
            34  => HeaderType::Tl0DepRepIndex,
            35  => HeaderType::TlSwitchingPoint,
            36  => HeaderType::ParallelDecodingInfo,
            37  => HeaderType::MvcScalableNesting,
            38  => HeaderType::ViewScalabilityInfo,
            39  => HeaderType::MultiviewSceneInfo,
            40  => HeaderType::MultiviewAcquisitionInfo,
            41  => HeaderType::NonRequiredViewComponent,
            42  => HeaderType::ViewDependencyChange,
            43  => HeaderType::OperationPointsNotPresent,
            44  => HeaderType::BaseViewTemporalHrd,
            45  => HeaderType::FramePackingArrangement,
            46  => HeaderType::MultiviewViewPosition,
            47  => HeaderType::DisplayOrientation,
            48  => HeaderType::MvcdScalableNesting,
            49  => HeaderType::MvcdViewScalabilityInfo,
            50  => HeaderType::DepthRepresentationInfo,
            51  => HeaderType::ThreeDimensionalReferenceDisplaysInfo,
            52  => HeaderType::DepthTiming,
            53  => HeaderType::DepthSamplingInfo,
            54  => HeaderType::ConstrainedDepthParameterSetIdentifier,
            56  => HeaderType::GreenMetadata,
            137 => HeaderType::MasteringDisplayColourVolume,
            142 => HeaderType::ColourRemappingInfo,
            147 => HeaderType::AlternativeTransferCharacteristics,
            188 => HeaderType::AlternativeDepthInfo,
            _   => HeaderType::ReservedSeiMessage(id),
        }
    }
}

#[macro_export]
macro_rules! sei_switch {
    ( $( $name:ident => $v:ty ),*, ) => {
        #[allow(non_snake_case)]
        struct SeiSwitch {
            current_type: Option<$crate::nal::sei::HeaderType>,
            $( $name: $crate::nal::sei::SeiBuffer<$v>, )*
        }
        impl Default for SeiSwitch {
            fn default() -> SeiSwitch {
                SeiSwitch {
                    current_type: None,
                    $( $name: $crate::nal::sei::SeiBuffer::new(<$v>::new()), )*
                }
            }
        }
        impl $crate::nal::sei::SeiIncrementalPayloadReader for SeiSwitch {
            fn start(&mut self, ctx: &mut $crate::Context, payload_type: $crate::nal::sei::HeaderType, payload_size: u32) {
                self.current_type = Some(payload_type);
                match payload_type {
                    $(
                    $crate::nal::sei::HeaderType::$name => self.$name.start(ctx, payload_type, payload_size),
                    )*
                    _ => (),
                }
            }

            fn push(&mut self, ctx: &mut $crate::Context, buf: &[u8]) {
                match self.current_type {
                    $(
                    Some($crate::nal::sei::HeaderType::$name) => self.$name.push(ctx, buf),
                    )*
                    Some(_) => (),
                    None => panic!("no previous call to start()"),
                }
            }

            fn end(&mut self, ctx: &mut $crate::Context) {
                match self.current_type {
                    $(
                    Some($crate::nal::sei::HeaderType::$name) => self.$name.end(ctx),
                    )*
                    Some(_) => (),
                    None => panic!("no previous call to start()"),
                }
                self.current_type = None;
            }

            fn reset(&mut self, ctx: &mut $crate::Context) {
                match self.current_type {
                    $(
                    Some($crate::nal::sei::HeaderType::$name) => self.$name.reset(ctx),
                    )*
                    Some(_) => (),
                    None => (),
                }
                self.current_type = None;
            }
        }
    }
}

enum SeiHeaderState {
    Begin,
    PayloadType { payload_type: u32 },
    PayloadSize { payload_type: HeaderType, payload_size: u32 },
    Payload { payload_type: HeaderType, payload_size: u32, consumed_size: u32 },
}

pub trait SeiCompletePayloadReader {
    fn header(&mut self, ctx: &mut Context, payload_type: HeaderType, buf: &[u8]);
}

pub trait SeiIncrementalPayloadReader {
    fn start(&mut self, ctx: &mut Context, payload_type: HeaderType, payload_size: u32);
    fn push(&mut self, ctx: &mut Context, buf: &[u8]);
    fn end(&mut self, ctx: &mut Context);
    fn reset(&mut self, ctx: &mut Context);
}

pub struct SeiBuffer<R: SeiCompletePayloadReader> {
    payload_type: Option<HeaderType>,
    buf: Vec<u8>,
    reader: R,
}
impl<R: SeiCompletePayloadReader> SeiBuffer<R> {
    pub fn new(reader: R) -> SeiBuffer<R> {
        SeiBuffer {
            payload_type: None,
            buf: Vec::new(),
            reader,
        }
    }
}
impl<R: SeiCompletePayloadReader> SeiIncrementalPayloadReader for SeiBuffer<R> {
    fn start(&mut self, ctx: &mut Context, payload_type: HeaderType, payload_size: u32) {
        self.payload_type = Some(payload_type);
    }

    fn push(&mut self, ctx: &mut Context, buf: &[u8]) {
        self.buf.extend_from_slice(buf);
    }

    fn end(&mut self, ctx: &mut Context) {
        self.reader.header(ctx, self.payload_type.unwrap(), &self.buf[..]);
        self.buf.clear();
        self.payload_type = None;
    }

    fn reset(&mut self, ctx: &mut Context) {
        self.buf.clear();
    }
}

pub struct SeiHeaderReader<R: SeiIncrementalPayloadReader> {
    state: SeiHeaderState,
    reader: R,
}
impl<R: SeiIncrementalPayloadReader> SeiHeaderReader<R> {
    pub fn new(reader: R) -> SeiHeaderReader<R> {
        SeiHeaderReader {
            state: SeiHeaderState::Begin,
            reader,
        }
    }
}
impl<R: SeiIncrementalPayloadReader> NalReader for SeiHeaderReader<R> {
    fn start(&mut self, ctx: &mut Context) {
        self.state = SeiHeaderState::Begin;
    }

    fn push(&mut self, ctx: &mut Context, buf: &[u8]) {
        assert!(!buf.is_empty());
        let mut input = &buf[..];
        loop {
            if input.is_empty() {
                break;
            }
            let b = input[0];
            let mut exit = false;
            self.state = match self.state {
                SeiHeaderState::Begin => {
                    match b {
                        0xff => {
                            SeiHeaderState::PayloadType { payload_type: b as u32 }
                        },
                        _ => {
                            SeiHeaderState::PayloadSize { payload_type: HeaderType::from_id(b as u32), payload_size: 0 }
                        }
                    }
                },
                SeiHeaderState::PayloadType { payload_type } => {
                    let new_type = b as u32 + payload_type;
                    match b {
                        0xff => {
                            SeiHeaderState::PayloadType { payload_type: new_type }
                        },
                        _ => {
                            SeiHeaderState::PayloadSize { payload_type: HeaderType::from_id(new_type), payload_size: 0 }
                        }
                    }
                },
                SeiHeaderState::PayloadSize { payload_type, payload_size } => {
                    let new_size = b as u32 + payload_size;
                    match b {
                        0xff => {
                            SeiHeaderState::PayloadSize { payload_type, payload_size: new_size }
                        },
                        _ => {
                            self.reader.start(ctx, payload_type, new_size);
                            SeiHeaderState::Payload { payload_type, payload_size: new_size, consumed_size: 0 }
                        }
                    }
                },
                SeiHeaderState::Payload { payload_type, payload_size, consumed_size } => {
                    let remaining = (payload_size - consumed_size) as usize;
                    if remaining >= input.len() {
                        exit = true;
                        self.reader.push(ctx, input);
                        let consumed_size = consumed_size + input.len() as u32;
                        if consumed_size == payload_size {
                            self.reader.end(ctx);
                            SeiHeaderState::Begin
                        } else {
                            SeiHeaderState::Payload { payload_type, payload_size, consumed_size }
                        }
                    } else {
                        let (head, tail) = input.split_at(remaining);
                        self.reader.push(ctx, head);
                        self.reader.end(ctx);
                        input = tail;
                        SeiHeaderState::Begin
                    }
                },
            };
            if exit { break; }
            if let SeiHeaderState::Begin = self.state {

            } else {
                input = &input[1..];
            }
        }
    }

    fn end(&mut self, ctx: &mut Context) {
        match self.state {
            SeiHeaderState::Begin => (),
            SeiHeaderState::PayloadSize { payload_type: HeaderType::ReservedSeiMessage(0x80), payload_size: 0 } => {
                // TODO: this is a bit of a hack to ignore rbsp_trailing_bits (which will always
                //       be 0b10000000 in an SEI payload since SEI messages are byte-aligned).
            },
            SeiHeaderState::PayloadType { .. } => {
                eprintln!("End of SEI data encountered while reading SEI payloadType");
                self.reader.reset(ctx);
            },
            SeiHeaderState::PayloadSize { .. } => {
                eprintln!("End of SEI data encountered while reading SEI payloadSize");
                self.reader.reset(ctx);
            },
            SeiHeaderState::Payload { payload_type, payload_size, consumed_size } => {
                eprintln!("End of SEI data encountered having read {} bytes of payloadSize={} for header type {:?}", consumed_size, payload_size, payload_type);
                self.reader.reset(ctx);
            },
        }
    }
}

pub struct SeiNalHandler<R: SeiIncrementalPayloadReader> {
    reader: RbspDecoder<SeiHeaderReader<R>>,
}
impl<R: SeiIncrementalPayloadReader> SeiNalHandler<R> {
    pub fn new(r: R) -> SeiNalHandler<R> {
        SeiNalHandler {
            reader: RbspDecoder::new(SeiHeaderReader::new(r)),
        }
    }
}

impl<R: SeiIncrementalPayloadReader> NalHandler for SeiNalHandler<R> {
    fn start(&mut self, ctx: &mut Context, header: &NalHeader) {
        assert_eq!(header.nal_unit_type(), super::UnitType::SEI);
        self.reader.start(ctx);
    }

    fn push(&mut self, ctx: &mut Context, buf: &[u8]) {
        self.reader.push(ctx, buf);
    }

    fn end(&mut self, ctx: &mut Context) {
        self.reader.end(ctx);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::rc::Rc;
    use std::cell::RefCell;

    #[derive(Default)]
    struct State {
        started: u32,
        ended: u32,
        data: Vec<u8>,
    }
    struct MockReader {
        state: Rc<RefCell<State>>
    }
    impl SeiIncrementalPayloadReader for MockReader {
        fn start(&mut self, ctx: &mut Context, payload_type: HeaderType, payload_size: u32) {
            self.state.borrow_mut().started += 1;
        }

        fn push(&mut self, ctx: &mut Context, buf: &[u8]) {
            self.state.borrow_mut().data.extend_from_slice(buf);
        }

        fn end(&mut self, ctx: &mut Context) {
            self.state.borrow_mut().ended += 1;
        }

        fn reset(&mut self, ctx: &mut Context) {
        }
    }

    #[test]
    fn it_works() {
        let data = [
            // header 1
            0x01,  // type
            0x01,  // len
            0x01,  // payload

            // header 2
            0x02,  // type
            0x02,  // len
            0x02, 0x02  // payload
        ];
        let state = Rc::new(RefCell::new(State::default()));
        let mut r = SeiHeaderReader::new(MockReader{ state: state.clone() });
        let mut ctx = &mut Context::default();
        r.start(ctx);
        r.push(ctx, &data[..]);
        r.end(ctx);
        let st = state.borrow();
        assert_eq!(st.started, 2);
        assert_eq!(&st.data[..], [0x01, 0x02, 0x02]);
        assert_eq!(st.ended, 2);
    }
}