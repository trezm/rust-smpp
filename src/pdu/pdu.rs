use std::convert::TryFrom;
use std::io;
use std::io::Read;

use crate::pdu::formats::{Integer4, WriteStream};
use crate::pdu::validate_command_length::validate_command_length;
use crate::pdu::{
    check, BindTransmitterPdu, BindTransmitterRespPdu, CheckError,
    CheckOutcome, GenericNackPdu, PduParseError, PduParseErrorKind,
};

#[derive(Debug, PartialEq)]
pub enum Pdu {
    BindTransmitter(BindTransmitterPdu),
    BindTransmitterResp(BindTransmitterRespPdu),
    GenericNack(GenericNackPdu),
}

impl Pdu {
    pub fn parse(bytes: &mut dyn io::BufRead) -> Result<Pdu, PduParseError> {
        let command_length = Integer4::read(bytes)?;
        validate_command_length(&command_length)?;

        let mut bytes =
            bytes.take(u64::try_from(command_length.value - 4).unwrap_or(0));
        let command_id = Integer4::read(&mut bytes)?;

        match command_id.value {
            0x00000002 => BindTransmitterPdu::parse(&mut bytes)
                .map(|p| Pdu::BindTransmitter(p)),
            0x80000002 => BindTransmitterRespPdu::parse(&mut bytes)
                .map(|p| Pdu::BindTransmitterResp(p)),
            _ => Err(PduParseError {
                kind: PduParseErrorKind::UnknownCommandId,
                message: format!("Unknown command id: {}", command_id.value),
                command_id: Some(command_id.value),
                io_errorkind: None,
            }),
        }
        .map_err(|mut e| {
            e.command_id = Some(command_id.value);
            e
        }).and_then(|ret| {
            // There should be no bytes left over
            let mut buf = [0; 1];
            if bytes.read(&mut buf)? == 0 {
                Ok(ret)
            } else {
                Err(PduParseError {
                    kind: PduParseErrorKind::LengthLongerThanPdu,
                    message: format!(
                        "Finished parsing PDU with command_id: {} but its length suggested it was longer.",
                        command_id.value
                    ),
                    command_id: Some(command_id.value),
                    io_errorkind: None,
                })
                // Note: Ideally, the sequence number in the response would
                // match the one supplied, but currently we don't include that.
                // This seems OK since really PDU does not parse correctly, but
                // it would be even better if we parsed the header and used that
                // to shape our error responses.
            }
        }
        )
    }

    pub fn check(
        bytes: &mut dyn io::BufRead,
    ) -> Result<CheckOutcome, CheckError> {
        check::check(bytes)
    }

    pub async fn write(&self, stream: &mut WriteStream) -> io::Result<()> {
        match self {
            Pdu::BindTransmitter(pdu) => pdu.write(stream).await,
            Pdu::BindTransmitterResp(pdu) => pdu.write(stream).await,
            Pdu::GenericNack(pdu) => pdu.write(stream).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use ascii::AsciiString;
    use std::io::Cursor;

    use super::*;

    const BIND_TRANSMITTER_RESP_PDU_PLUS_EXTRA: &[u8; 0x1b + 0xa] =
        b"\x00\x00\x00\x1b\x80\x00\x00\x02\x00\x00\x00\x00\x00\x00\x00\x02TestServer\0extrabytes";

    #[test]
    fn check_is_ok_if_more_bytes() {
        // Most tests for check are in the check module.  Here we do enough
        // to confirm that we are calling through it that from Pdu::check.
        let mut cursor = Cursor::new(&BIND_TRANSMITTER_RESP_PDU_PLUS_EXTRA[..]);
        assert_eq!(Pdu::check(&mut cursor).unwrap(), CheckOutcome::Ready);
    }

    #[test]
    fn check_is_incomplete_if_fewer_bytes() {
        let mut cursor =
            Cursor::new(&BIND_TRANSMITTER_RESP_PDU_PLUS_EXTRA[..0x1a]);
        assert_eq!(Pdu::check(&mut cursor).unwrap(), CheckOutcome::Incomplete);
    }

    #[test]
    fn parse_valid_bind_transmitter() {
        const BIND_TRANSMITTER_PDU_PLUS_EXTRA: &[u8; 0x2e + 0x6] =
            b"\x00\x00\x00\x2e\x00\x00\x00\x02\x00\x00\x00\x00\x01\x02\x03\x44mysystem_ID\0pw$xx\0t_p_\0\x34\x13\x50rng\0foobar";

        let mut cursor = Cursor::new(&BIND_TRANSMITTER_PDU_PLUS_EXTRA[..]);
        assert_eq!(
            Pdu::parse(&mut cursor).unwrap(),
            Pdu::BindTransmitter(
                BindTransmitterPdu::new(
                    0x01020344,
                    "mysystem_ID",
                    "pw$xx",
                    "t_p_",
                    0x34,
                    0x13,
                    0x50,
                    "rng"
                )
                .unwrap()
            )
        );
    }

    #[test]
    fn parse_bind_transmitter_with_too_long_system_id() {
        const PDU: &[u8; 0x29] =
            b"\x00\x00\x00\x29\x00\x00\x00\x02\x00\x00\x00\x00\x01\x02\x03\x44ABDEFABCDEFABCDEFA\0\0\0\x34\x13\x50\0";
        let mut cursor = Cursor::new(&PDU[..]);

        let res = Pdu::parse(&mut cursor).unwrap_err();
        assert_eq!(
            res,
            PduParseError::new(
                PduParseErrorKind::COctetStringTooLong,
                "String value for system_id is too long.  Max length is 16, including final zero byte.",
                Some(0x00000002),
                None,
            )
        );
    }

    #[test]
    fn parse_bind_transmitter_with_length_ending_within_string() {
        const PDU: &[u8; 0x29] =
            b"\x00\x00\x00\x12\x00\x00\x00\x02\x00\x00\x00\x00\x01\x02\x03\x44ABDEFABCDEFABCDEFA\0\0\0\x34\x13\x50\0";
        let mut cursor = Cursor::new(&PDU[..]);

        let res = Pdu::parse(&mut cursor).unwrap_err();
        assert_eq!(
            res,
            PduParseError::new(
                PduParseErrorKind::COctetStringDoesNotEndWithZeroByte,
                "String value for system_id did not end with a zero byte.",
                Some(0x00000002),
                None,
            )
        );
    }

    #[test]
    fn parse_bind_transmitter_ending_before_all_fields() {
        const PDU: &[u8; 0x13] =
            b"\x00\x00\x00\x13\x00\x00\x00\x02\x00\x00\x00\x00\x01\x02\x03\x44\0\0\0";
        let mut cursor = Cursor::new(&PDU[..]);

        let res = Pdu::parse(&mut cursor).unwrap_err();
        assert_eq!(
            res,
            PduParseError::new(
                PduParseErrorKind::NotEnoughBytes,
                "Reached end of PDU length (or end of input) before finding all fields of the PDU.",
                Some(0x00000002),
                Some(io::ErrorKind::UnexpectedEof)
            )
        );
    }

    #[test]
    fn parse_bind_transmitter_hitting_eof_before_end_of_length() {
        const PDU: &[u8; 0x0b] =
            b"\x00\x00\x00\x2e\x00\x00\x00\x02\x00\x00\x00";
        let mut cursor = Cursor::new(&PDU[..]);

        let res = Pdu::parse(&mut cursor).unwrap_err();
        assert_eq!(
            res,
            PduParseError::new(
                PduParseErrorKind::NotEnoughBytes,
                "Reached end of PDU length (or end of input) before finding all fields of the PDU.",
                Some(0x00000002),
                Some(io::ErrorKind::UnexpectedEof)
            )
        );
    }

    #[test]
    fn parse_bind_transmitter_with_short_length() {
        const PDU: &[u8; 4] = b"\x00\x00\x00\x04";
        let mut cursor = Cursor::new(&PDU);

        let res = Pdu::parse(&mut cursor).unwrap_err();
        assert_eq!(
            res,
            PduParseError::new(
                PduParseErrorKind::LengthTooShort,
                "PDU too short!  Length: 4, min allowed: 8.",
                None,
                None
            )
        );
    }

    #[test]
    fn parse_bind_transmitter_with_massive_length() {
        const PDU: &[u8; 16] =
            b"\xff\xff\xff\xff\x00\x00\x00\x02\x00\x00\x00\x00\x00\x00\x00\x00";
        let mut cursor = Cursor::new(&PDU);

        let res = Pdu::parse(&mut cursor).unwrap_err();
        assert_eq!(
            res,
            PduParseError::new(
                PduParseErrorKind::LengthTooLong,
                "PDU too long!  Length: 4294967295, max allowed: 70000.",
                None,
                None
            )
        );
    }

    #[test]
    fn parse_bind_transmitter_containing_nonascii_characters() {
        const PDU: &[u8; 0x2e + 0x6] =
            b"\x00\x00\x00\x2e\x00\x00\x00\x02\x00\x00\x00\x00\x01\x02\x03\x44mys\xf0\x9f\x92\xa9m_ID\0pw$xx\0t_p_\0\x34\x13\x50rng\0foobar";
        let mut cursor = Cursor::new(&PDU);

        let res = Pdu::parse(&mut cursor).unwrap_err();
        assert_eq!(
            res,
            PduParseError::new(
                PduParseErrorKind::COctetStringIsNotAscii,
                "String value of system_id is not ASCII (valid up to byte 3).",
                Some(0x00000002),
                None,
            )
        );
    }

    #[test]
    fn parse_bind_transmitter_with_nonzero_status() {
        const PDU: &[u8; 0x2e + 0x6] =
            b"\x00\x00\x00\x2e\x00\x00\x00\x02\x00\x00\x00\x77\x01\x02\x03\x44mysystem_ID\0pw$xx\0t_p_\0\x34\x13\x50rng\0foobar";
        let mut cursor = Cursor::new(&PDU);

        let res = Pdu::parse(&mut cursor).unwrap_err();
        assert_eq!(
            res,
            PduParseError::new(
                PduParseErrorKind::StatusIsNotZero,
                "command_status must be 0, but was 119",
                Some(0x00000002),
                None,
            )
        );
    }

    #[test]
    fn parse_valid_bind_transmitter_resp() {
        let mut cursor = Cursor::new(&BIND_TRANSMITTER_RESP_PDU_PLUS_EXTRA[..]);
        assert_eq!(
            Pdu::parse(&mut cursor).unwrap(),
            Pdu::BindTransmitterResp(BindTransmitterRespPdu::new(
                0x00000002,
                &AsciiString::from_ascii("TestServer").unwrap(),
            ),)
        );
    }
}
