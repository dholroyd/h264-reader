//! Types for reading H264 _Network Abstraction Layer_ Units (NAL Units).
//!
//! The data presented must already be in _RBSP_ form (i.e. have been passed through
//! [`RbspDecoder`](../rbsp/struct.RbspDecoder.html)), where it has been encoded with
//! 'emulation prevention bytes'.

pub mod sps;
pub mod pps;
pub mod sei;
pub mod slice;

use crate::annexb::NalReader;
use std::cell::RefCell;
use crate::Context;
use std::fmt;
use log::*;

#[derive(PartialEq, Hash, Debug, Copy, Clone)]
pub enum UnitType {
    /// The values `0` and `24`-`31` are unspecified in the H264 spec
    Unspecified(u8),
    SliceLayerWithoutPartitioningNonIdr,
    SliceDataPartitionALayer,
    SliceDataPartitionBLayer,
    SliceDataPartitionCLayer,
    SliceLayerWithoutPartitioningIdr,
    /// Supplemental enhancement information
    SEI,
    SeqParameterSet,
    PicParameterSet,
    AccessUnitDelimiter,
    EndOfSeq,
    EndOfStream,
    FillerData,
    SeqParameterSetExtension,
    PrefixNALUnit,
    SubsetSeqParameterSet,
    DepthParameterSet,
    SliceLayerWithoutPartitioningAux,
    SliceExtension,
    SliceExtensionViewComponent,
    /// The values `17`, `18`, `22` and `23` are reserved for future use by the H264 spec
    Reserved(u8),
}
impl UnitType {
    pub fn for_id(id: u8) -> Result<UnitType, UnitTypeError> {
        if id > 31 {
            Err(UnitTypeError::ValueOutOfRange(id))
        } else {
            let t = match id {
                0  => UnitType::Unspecified(0),
                1  => UnitType::SliceLayerWithoutPartitioningNonIdr,
                2  => UnitType::SliceDataPartitionALayer,
                3  => UnitType::SliceDataPartitionBLayer,
                4  => UnitType::SliceDataPartitionCLayer,
                5  => UnitType::SliceLayerWithoutPartitioningIdr,
                6  => UnitType::SEI,
                7  => UnitType::SeqParameterSet,
                8  => UnitType::PicParameterSet,
                9  => UnitType::AccessUnitDelimiter,
                10 => UnitType::EndOfSeq,
                11 => UnitType::EndOfStream,
                12 => UnitType::FillerData,
                13 => UnitType::SeqParameterSetExtension,
                14 => UnitType::PrefixNALUnit,
                15 => UnitType::SubsetSeqParameterSet,
                16 => UnitType::DepthParameterSet,
                17..=18 => UnitType::Reserved(id),
                19 => UnitType::SliceLayerWithoutPartitioningAux,
                20 => UnitType::SliceExtension,
                21 => UnitType::SliceExtensionViewComponent,
                22..=23 => UnitType::Reserved(id),
                24..=31 => UnitType::Unspecified(id),
                _ => panic!("unexpected {}", id), // shouldn't happen
            };
            Ok(t)
        }
    }

    pub fn id(self) -> u8 {
        match self {
            UnitType::Unspecified(v) => v,
            UnitType::SliceLayerWithoutPartitioningNonIdr => 1,
            UnitType::SliceDataPartitionALayer => 2,
            UnitType::SliceDataPartitionBLayer => 3,
            UnitType::SliceDataPartitionCLayer => 4,
            UnitType::SliceLayerWithoutPartitioningIdr => 5,
            UnitType::SEI => 6,
            UnitType::SeqParameterSet => 7,
            UnitType::PicParameterSet => 8,
            UnitType::AccessUnitDelimiter => 9,
            UnitType::EndOfSeq => 10,
            UnitType::EndOfStream => 11,
            UnitType::FillerData => 12,
            UnitType::SeqParameterSetExtension => 13,
            UnitType::PrefixNALUnit => 14,
            UnitType::SubsetSeqParameterSet => 15,
            UnitType::DepthParameterSet => 16,
            UnitType::SliceLayerWithoutPartitioningAux => 19,
            UnitType::SliceExtension => 20,
            UnitType::SliceExtensionViewComponent => 21,
            UnitType::Reserved(v) => v,
        }
    }
}

#[derive(Debug)]
pub enum UnitTypeError {
    /// if the value was outside the range `0` - `31`.
    ValueOutOfRange(u8)
}

#[derive(Copy,Clone)]
pub struct NalHeader ( u8 );

#[derive(Debug)]
pub enum NalHeaderError {
    ForbiddenZeroBit,
}
impl NalHeader {
    pub fn new(header_value: u8) -> Result<NalHeader, NalHeaderError> {
        if header_value & 0b1000_0000 != 0 {
            Err(NalHeaderError::ForbiddenZeroBit)
        } else {
            Ok(NalHeader(header_value))
        }
    }

    pub fn nal_ref_idc(self) -> u8 {
        (self.0 & 0b0110_0000) >> 5
    }

    pub fn nal_unit_type(self) -> UnitType {
        UnitType::for_id(self.0 & 0b0001_1111).unwrap()
    }
}
impl From<NalHeader> for u8 {
    fn from(v: NalHeader) -> Self {
        v.0
    }
}

impl fmt::Debug for NalHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(),fmt::Error> {
        f.debug_struct("NalHeader")
            .field("nal_ref_idc", &self.nal_ref_idc())
            .field("nal_unit_type", &self.nal_unit_type())
            .finish()
    }
}

#[derive(Debug)]
enum NalSwitchState {
    Start,
    Handling(UnitType),
    Ignoring,
}
// TODO: generate enum at compile time rather than Vec<Box<>>
pub struct NalSwitch<Ctx> {
    readers_by_id: Vec<Option<Box<RefCell<dyn NalHandler<Ctx=Ctx>>>>>,
    state: NalSwitchState,
}
impl<Ctx> Default for NalSwitch<Ctx> {
    fn default() -> Self {
        NalSwitch {
            readers_by_id: Vec::new(),
            state: NalSwitchState::Start,
        }
    }
}
impl<Ctx> NalSwitch<Ctx> {
    pub fn put_handler(&mut self, unit_type: UnitType, handler: Box<RefCell<dyn NalHandler<Ctx=Ctx>>>) {
        let i = unit_type.id() as usize;
        while i >= self.readers_by_id.len() {
            self.readers_by_id.push(None);
        }
        self.readers_by_id[i] = Some(handler);
    }

    fn get_handler(&self, unit_type: UnitType) -> &Option<Box<RefCell<dyn NalHandler<Ctx=Ctx>>>> {
        let i = unit_type.id() as usize;
        if i < self.readers_by_id.len() {
            &self.readers_by_id[i]
        } else {
            &None
        }
    }
}
impl<Ctx> NalReader for NalSwitch<Ctx> {
    type Ctx = Ctx;

    fn start(&mut self, _ctx: &mut Context<Ctx>) {
        self.state = NalSwitchState::Start;
    }

    fn push(&mut self, ctx: &mut Context<Ctx>, buf: &[u8]) {
        if buf.is_empty() {
            return;
        }
        match self.state {
            NalSwitchState::Start => {
                self.state = match NalHeader::new(buf[0]) {
                    Ok(header) => {
                        if let Some(ref handler) = self.get_handler(header.nal_unit_type()) {
                            handler.borrow_mut().start(ctx, header);
                            handler.borrow_mut().push(ctx, &buf[1..]);
                            NalSwitchState::Handling(header.nal_unit_type())
                        } else {
                            NalSwitchState::Ignoring
                        }
                    },
                    Err(e) => {
                        // TODO: proper error propagation
                        error!("Bad NAL header: {:?}", e);
                        NalSwitchState::Ignoring
                    }
                };
            },
            NalSwitchState::Ignoring => (),
            NalSwitchState::Handling(unit_type) => {
                if let Some(ref handler) = self.get_handler(unit_type) {
                    handler.borrow_mut().push(ctx, buf);
                }
            }
        }
    }

    fn end(&mut self, ctx: &mut Context<Ctx>) {
        if let NalSwitchState::Handling(unit_type) = self.state {
            if let Some(ref handler) = self.get_handler(unit_type) {
                handler.borrow_mut().end(ctx);
            }
        }
        self.state = NalSwitchState::Ignoring
    }
}

// TODO: rename to 'RbspHandler' or something, to indicate it's only for post-emulation-prevention-bytes data
pub trait NalHandler {
    type Ctx;

    fn start(&mut self, ctx: &mut Context<Self::Ctx>, header: NalHeader);
    fn push(&mut self, ctx: &mut Context<Self::Ctx>, buf: &[u8]);
    fn end(&mut self, ctx: &mut Context<Self::Ctx>);
}

#[cfg(test)]
mod test {
    use super::*;
    use hex_literal::*;

    #[test]
    fn header() {
        let h = NalHeader::new(0b0101_0001).unwrap();
        assert_eq!(0b10, h.nal_ref_idc());
        assert_eq!(UnitType::Reserved(17), h.nal_unit_type());
    }

    struct MockHandler;
    impl NalHandler for MockHandler {
        type Ctx = ();

        fn start(&mut self, _ctx: &mut Context<Self::Ctx>, header: NalHeader) {
            assert_eq!(header.nal_unit_type(), UnitType::SeqParameterSet);
        }

        fn push(&mut self, _ctx: &mut Context<Self::Ctx>, buf: &[u8]) {
            let expected = hex!(
               "64 00 0A AC 72 84 44 26 84 00 00
                00 04 00 00 00 CA 3C 48 96 11 80");
            assert_eq!(buf, &expected[..])
        }

        fn end(&mut self, _ctx: &mut Context<Self::Ctx>) {
        }
    }

    #[test]
    fn switch() {
        let handler = MockHandler;
        let mut s = NalSwitch::default();
        s.put_handler(UnitType::SeqParameterSet, Box::new(RefCell::new(handler)));
        let data = hex!(
           "67 64 00 0A AC 72 84 44 26 84 00 00
            00 04 00 00 00 CA 3C 48 96 11 80");
        let mut ctx = Context::default();
        s.push(&mut ctx, &data[..]);
    }
}