use crate::nal::sei::HeaderType;
use crate::nal::sei::SeiMessage;

#[derive(Debug)]
pub enum UserDataUnregisteredError {
    NotEnoughData { expected: usize, actual: usize },
}

/// Parsed `user_data_unregistered()` SEI message (payloadType == 5).
#[derive(Debug, PartialEq, Eq)]
pub struct UserDataUnregistered<'a> {
    buf: &'a [u8],
}

impl<'a> UserDataUnregistered<'a> {
    pub fn read(msg: &SeiMessage<'a>) -> Result<Self, UserDataUnregisteredError> {
        assert_eq!(msg.payload_type, HeaderType::UserDataUnregistered);
        if msg.payload.len() < 16 {
            return Err(UserDataUnregisteredError::NotEnoughData {
                expected: 16,
                actual: msg.payload.len(),
            });
        }
        Ok(UserDataUnregistered { buf: msg.payload })
    }

    /// UUID per ISO/IEC 11578 identifying the user data type.
    pub fn uuid(&self) -> [u8; 16] {
        let mut uuid = [0u8; 16];
        uuid.copy_from_slice(&self.buf[..16]);
        uuid
    }

    /// The `user_data_payload_byte` values following the UUID.
    pub fn payload(&self) -> &'a [u8] {
        &self.buf[16..]
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse() {
        let uuid_bytes: [u8; 16] = [
            0xdc, 0x45, 0xe9, 0xbd, 0xe6, 0xd9, 0x48, 0xb7, 0x96, 0x2c, 0xd8, 0x20, 0xd9, 0x23,
            0xee, 0xef,
        ];
        let user_data = [0x01, 0x02, 0x03];
        let mut payload = Vec::new();
        payload.extend_from_slice(&uuid_bytes);
        payload.extend_from_slice(&user_data);
        let msg = SeiMessage {
            payload_type: HeaderType::UserDataUnregistered,
            payload: &payload,
        };
        let parsed = UserDataUnregistered::read(&msg).unwrap();
        assert_eq!(parsed.uuid(), uuid_bytes);
        assert_eq!(parsed.payload(), &user_data[..]);
    }

    #[test]
    fn parse_no_user_data() {
        let uuid_bytes: [u8; 16] = [
            0xdc, 0x45, 0xe9, 0xbd, 0xe6, 0xd9, 0x48, 0xb7, 0x96, 0x2c, 0xd8, 0x20, 0xd9, 0x23,
            0xee, 0xef,
        ];
        let msg = SeiMessage {
            payload_type: HeaderType::UserDataUnregistered,
            payload: &uuid_bytes,
        };
        let parsed = UserDataUnregistered::read(&msg).unwrap();
        assert_eq!(parsed.uuid(), uuid_bytes);
        assert_eq!(parsed.payload(), &[][..]);
    }

    #[test]
    fn too_short() {
        let msg = SeiMessage {
            payload_type: HeaderType::UserDataUnregistered,
            payload: &[0x01, 0x02, 0x03],
        };
        let err = UserDataUnregistered::read(&msg).unwrap_err();
        match err {
            UserDataUnregisteredError::NotEnoughData { expected, actual } => {
                assert_eq!(expected, 16);
                assert_eq!(actual, 3);
            }
        }
    }
}
