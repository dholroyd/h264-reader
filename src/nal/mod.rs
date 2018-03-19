//! Types for reading H264 _Network Abstraction Layer_ Units (NAL Units).
//!
//! The data presented must already be in _RBSP_ form (i.e. have been passed through
//! [`RbspDecoder`](../rbsp/struct.RbspDecoder.html)), where it has been encoded with
//! 'emulation prevention bytes'.

pub mod sps;
pub mod pps;

use std::hash::{Hash, Hasher};
use annexb::NalReader;
use std::cell::RefCell;

#[derive(PartialEq, Debug, Copy, Clone)]
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
    /// The values `13`-`23` are reserved for future use by the H264 spec
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
                13 => UnitType::Reserved(13),
                14 => UnitType::Reserved(14),
                15 => UnitType::Reserved(15),
                16 => UnitType::Reserved(16),
                17 => UnitType::Reserved(17),
                18 => UnitType::Reserved(18),
                19 => UnitType::Reserved(19),
                20 => UnitType::Reserved(20),
                21 => UnitType::Reserved(21),
                22 => UnitType::Reserved(22),
                23 => UnitType::Reserved(23),
                24 => UnitType::Unspecified(24),
                25 => UnitType::Unspecified(25),
                26 => UnitType::Unspecified(26),
                27 => UnitType::Unspecified(27),
                28 => UnitType::Unspecified(28),
                29 => UnitType::Unspecified(29),
                30 => UnitType::Unspecified(30),
                31 => UnitType::Unspecified(31),
                _ => panic!("unexpected {}", id), // shouldn't happen
            };
            Ok(t)
        }
    }

    pub fn id(&self) -> u8 {
        match *self {
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
            UnitType::Reserved(v) => v,
        }
    }
}
impl Hash for UnitType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id().hash(state);
    }
}

#[derive(Debug)]
pub enum UnitTypeError {
    /// if the value was outside the range `0` - `31`.
    ValueOutOfRange(u8)
}

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

    pub fn nal_ref_idc(&self) -> u8 {
        (self.0 & 0b01100000) >> 5
    }

    pub fn nal_unit_type(&self) -> UnitType {
        UnitType::for_id(self.0 & 0b0001_1111).unwrap()
    }
}

enum NalSwitchState {
    Start,
    Handling(UnitType),
    Ignoring,
}
pub struct NalSwitch {
    readers_by_id: Vec<Option<Box<RefCell<NalHandler>>>>,
    state: NalSwitchState,
}
impl NalSwitch {
    pub fn new() -> NalSwitch {
        NalSwitch {
            readers_by_id: Vec::new(),
            state: NalSwitchState::Start,
        }
    }

    pub fn put_handler(&mut self, unit_type: UnitType, handler: Box<RefCell<NalHandler>>) {
        let i = unit_type.id() as usize;
        while i >= self.readers_by_id.len() {
            self.readers_by_id.push(None);
        }
        self.readers_by_id[i] = Some(handler);
    }

    fn get_handler(&self, unit_type: UnitType) -> &Option<Box<RefCell<NalHandler>>> {
        let i = unit_type.id() as usize;
        if i < self.readers_by_id.len() {
            &self.readers_by_id[i]
        } else {
            &None
        }
    }
}
impl NalReader for NalSwitch {
    fn start(&mut self) {
        self.state = NalSwitchState::Start;
    }

    fn push(&mut self, buf: &[u8]) {
        if buf.len() == 0 {
            return;
        }
        match self.state {
            NalSwitchState::Start => {
                let header = NalHeader::new(buf[0]).unwrap();
                self.state = if let &Some(ref handler) = self.get_handler(header.nal_unit_type()) {
                    handler.borrow_mut().start(&header);
                    handler.borrow_mut().push(&buf[1..]);
                    NalSwitchState::Handling(header.nal_unit_type())
                } else {
                    NalSwitchState::Ignoring
                }
            },
            NalSwitchState::Ignoring => (),
            NalSwitchState::Handling(unit_type) => {
                if let &Some(ref handler) = self.get_handler(unit_type) {
                    handler.borrow_mut().push(buf);
                }
            }
        }
    }

    fn end(&mut self) {
        if let NalSwitchState::Handling(unit_type) = self.state {
            if let &Some(ref handler) = self.get_handler(unit_type) {
                handler.borrow_mut().end();
            }
        }
    }
}

pub trait NalHandler {
    fn start(&mut self, header: &NalHeader);
    fn push(&mut self, buf: &[u8]);
    fn end(&mut self);
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn header() {
        let h = NalHeader::new(0b0101_0001).unwrap();
        assert_eq!(0b10, h.nal_ref_idc());
        assert_eq!(UnitType::Reserved(17), h.nal_unit_type());
    }

    struct MockHandler;
    impl NalHandler for MockHandler {
        fn start(&mut self, header: &NalHeader) {
            assert_eq!(header.nal_unit_type(), UnitType::SeqParameterSet);
        }

        fn push(&mut self, buf: &[u8]) {
            let expected = hex!(
               "64 00 0A AC 72 84 44 26 84 00 00
                00 04 00 00 00 CA 3C 48 96 11 80");
            assert_eq!(buf, &expected[..])
        }

        fn end(&mut self) {
        }
    }

    #[test]
    fn switch() {
        let handler = MockHandler;
        let mut s = NalSwitch::new();
        s.put_handler(UnitType::SeqParameterSet, Box::new(RefCell::new(handler)));
        let data = hex!(
           "67 64 00 0A AC 72 84 44 26 84 00 00
            00 04 00 00 00 CA 3C 48 96 11 80");
        s.push(&data[..]);
    }
}