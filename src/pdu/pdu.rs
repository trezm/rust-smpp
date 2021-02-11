use std::convert::TryFrom;
use std::io;
use std::io::Read;
use tokio::io::AsyncWriteExt;

// TODO: search for and replace all PDU type constants

use crate::pdu::formats::{Integer4, WriteStream};
use crate::pdu::operations::bind_transmitter_resp::BIND_TRANSMITTER_RESP;
use crate::pdu::operations::generic_nack::GENERIC_NACK;
use crate::pdu::operations::submit_sm_resp::SUBMIT_SM_RESP;
use crate::pdu::validate_command_length::validate_command_length;
use crate::pdu::{
    check, BindTransmitterPdu, BindTransmitterRespPdu, CheckError,
    CheckOutcome, GenericNackPdu, PduParseError, PduParseErrorBody,
    SubmitSmPdu, SubmitSmRespPdu,
};

// It will be worth considering later whether the reading/writing code
// for the PDUs defined in the pdu::operations module could be generated using
// a derive macro rather than hand-coded as they are now.

#[derive(Debug, PartialEq)]
pub enum PduBody {
    BindTransmitter(BindTransmitterPdu),
    BindTransmitterResp(BindTransmitterRespPdu),
    GenericNack(GenericNackPdu),
    SubmitSm(SubmitSmPdu),
    SubmitSmResp(SubmitSmRespPdu),
}

#[derive(Debug, PartialEq)]
pub struct Pdu {
    pub command_id: Integer4, // TODO: consider removing this since it's implicit in the body type
    pub command_status: Integer4,
    pub sequence_number: Integer4,
    pub body: PduBody,
}

impl Pdu {
    pub fn new(
        command_id: u32,
        command_status: u32,
        sequence_number: u32,
        body: PduBody,
    ) -> Self {
        // TODO: don't allow constructing without this?
        Self {
            command_id: Integer4::new(command_id),
            command_status: Integer4::new(command_status),
            sequence_number: Integer4::new(sequence_number),
            body,
        }
    }

    pub fn new_generic_nack_error(
        command_status: u32,
        sequence_number: u32,
    ) -> Self {
        Self::new(
            GENERIC_NACK,
            command_status,
            sequence_number,
            PduBody::GenericNack(GenericNackPdu::new_error()),
        )
    }

    pub fn new_bind_transmitter_resp(
        sequence_number: u32,
        system_id: &str,
    ) -> Result<Self, PduParseError> {
        Ok(Self::new(
            BIND_TRANSMITTER_RESP,
            0x00000000,
            sequence_number,
            PduBody::BindTransmitterResp(BindTransmitterRespPdu::new(
                system_id,
            )?),
        ))
    }

    pub fn new_bind_transmitter_resp_error(
        command_status: u32,
        sequence_number: u32,
    ) -> Self {
        Self::new(
            BIND_TRANSMITTER_RESP,
            command_status,
            sequence_number,
            PduBody::BindTransmitterResp(BindTransmitterRespPdu::new_error()),
        )
    }

    pub fn new_submit_sm_resp_error(
        command_status: u32,
        sequence_number: u32,
    ) -> Self {
        Pdu::new(
            SUBMIT_SM_RESP,
            command_status,
            sequence_number,
            PduBody::SubmitSmResp(SubmitSmRespPdu::new_error()),
        )
    }

    pub fn parse(bytes: &mut dyn io::BufRead) -> Result<Pdu, PduParseError> {
        let command_length = Integer4::read(bytes)?;
        let mut bytes =
            bytes.take(u64::try_from(command_length.value - 4).unwrap_or(0));

        let command_id = hfld("command_id", &mut bytes, &command_length)?;
        let command_status =
            hfld("command_status", &mut bytes, &command_length).map_err(
                |e| e.into_with_header(Some(command_id.value), None, None),
            )?;
        let sequence_number =
            hfld("sequence_number", &mut bytes, &command_length).map_err(
                |e| {
                    e.into_with_header(
                        Some(command_id.value),
                        Some(command_status.value),
                        None,
                    )
                },
            )?;

        validate_command_length(&command_length).map_err(|e| {
            PduParseError::from(e).into_with_header(
                Some(command_id.value),
                Some(command_status.value),
                Some(sequence_number.value),
            )
        })?;

        let body =
            parse_body(&mut bytes, command_id.value, command_status.value)
                .and_then(|ret| {
                    // There should be no bytes left over
                    let mut buf = [0; 1];
                    if bytes.read(&mut buf)? == 0 {
                        Ok(ret)
                    } else {
                        Err(PduParseError::new(
                            PduParseErrorBody::LengthLongerThanPdu(
                                command_length.value,
                            ),
                        ))
                        // Note: Ideally, the sequence number in the response
                        // would match the one supplied, but currently we don't
                        // include that.  This seems OK since really PDU does
                        // not parse correctly, but it would be even better if
                        // we parsed the header and used that to shape our error
                        // responses.
                    }
                })
                .map_err(|e| {
                    e.into_with_header(
                        Some(command_id.value),
                        Some(command_status.value),
                        Some(sequence_number.value),
                    )
                })?;

        Ok(Pdu {
            command_id,
            command_status,
            sequence_number,
            body,
        })
    }

    pub fn check(
        bytes: &mut dyn io::BufRead,
    ) -> Result<CheckOutcome, CheckError> {
        check::check(bytes)
    }

    pub async fn write(&self, stream: &mut WriteStream) -> io::Result<()> {
        let mut buf = Vec::new();
        self.command_id.write(&mut buf).await?;
        self.command_status.write(&mut buf).await?;
        self.sequence_number.write(&mut buf).await?;
        match &self.body {
            PduBody::BindTransmitter(body) => body.write(&mut buf).await?,
            PduBody::BindTransmitterResp(body) => body.write(&mut buf).await?,
            PduBody::GenericNack(body) => body.write(&mut buf).await?,
            PduBody::SubmitSm(body) => body.write(&mut buf).await?,
            PduBody::SubmitSmResp(body) => body.write(&mut buf).await?,
        }
        let command_length = Integer4::new((buf.len() + 4) as u32);
        command_length.write(stream).await?;
        stream.write(&buf).await?;
        Ok(())
    }
}

pub fn parse_body(
    bytes: &mut dyn io::BufRead,
    command_id: u32,
    command_status: u32,
) -> Result<PduBody, PduParseError> {
    match command_id {
        // TODO: has to be literals here, so only use them here and nearby
        0x00000002 => BindTransmitterPdu::parse(bytes, command_status)
            .map(|p| PduBody::BindTransmitter(p)),
        0x80000002 => BindTransmitterRespPdu::parse(bytes, command_status)
            .map(|p| PduBody::BindTransmitterResp(p)),
        0x00000004 => SubmitSmPdu::parse(bytes, command_status)
            .map(|p| PduBody::SubmitSm(p)),
        0x80000004 => SubmitSmRespPdu::parse(bytes, command_status)
            .map(|p| PduBody::SubmitSmResp(p)),
        _ => Err(PduParseError::new(PduParseErrorBody::UnknownCommandId)),
    }
}

fn hfld(
    field_name: &str,
    mut bytes: &mut dyn io::BufRead,
    command_length: &Integer4,
) -> Result<Integer4, PduParseError> {
    Integer4::read(&mut bytes).map_err(|e| {
        if let Err(len_e) = validate_command_length(command_length) {
            PduParseError::from(len_e)
        } else {
            PduParseError::from(e).into_with_field_name(field_name)
        }
    })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::pdu::operations::bind_transmitter::BIND_TRANSMITTER;
    use crate::pdu::operations::submit_sm::SUBMIT_SM;
    use crate::pdu::operations::submit_sm_resp::SUBMIT_SM_RESP;

    const BIND_TRANSMITTER_RESP_PDU_PLUS_EXTRA: &[u8; 0x1b + 0xa] =
        b"\x00\x00\x00\x1b\x80\x00\x00\x02\x00\x00\x00\x00\x00\x00\x00\x02\
        TestServer\0extrabytes";

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
            b"\x00\x00\x00\x2e\x00\x00\x00\x02\x00\x00\x00\x00\x01\x02\x03\x44\
            mysystem_ID\0pw$xx\0t_p_\0\x34\x13\x50rng\0foobar";

        let mut cursor = Cursor::new(&BIND_TRANSMITTER_PDU_PLUS_EXTRA[..]);
        assert_eq!(
            Pdu::parse(&mut cursor).unwrap(),
            Pdu::new(
                BIND_TRANSMITTER,
                0x00000000,
                0x01020344,
                PduBody::BindTransmitter(
                    BindTransmitterPdu::new(
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
            )
        );
    }

    #[test]
    fn parse_bind_transmitter_with_too_long_system_id() {
        // TODO: wrap lines
        const PDU: &[u8; 0x29] =
            b"\x00\x00\x00\x29\x00\x00\x00\x02\x00\x00\x00\x00\x01\x02\x03\x44\
            ABDEFABCDEFABCDEFA\0\0\0\x34\x13\x50\0";
        let mut cursor = Cursor::new(&PDU[..]);

        let res = Pdu::parse(&mut cursor).unwrap_err();
        assert_eq!(
            res.to_string(),
            "Error parsing PDU \
            (command_id=0x00000002, command_status=0x00000000, \
            sequence_number=0x01020344, field_name=system_id): \
            Octet String is too long.  Max length is 16, including final \
            zero byte.",
        );
    }

    #[test]
    fn parse_bind_transmitter_with_length_ending_within_string() {
        const PDU: &[u8; 0x29] =
            b"\x00\x00\x00\x12\x00\x00\x00\x02\x00\x00\x00\x00\x01\x02\x03\x44\
            ABDEFABCDEFABCDEFA\0\0\0\x34\x13\x50\0";
        let mut cursor = Cursor::new(&PDU[..]);

        let res = Pdu::parse(&mut cursor).unwrap_err();
        assert_eq!(
            res.to_string(),
            "Error parsing PDU \
            (command_id=0x00000002, command_status=0x00000000, \
            sequence_number=0x01020344, field_name=system_id): \
            C-Octet String does not end with the NULL character.",
        );
    }

    #[test]
    fn parse_bind_transmitter_ending_before_all_fields() {
        const PDU: &[u8; 0x13] =
            b"\x00\x00\x00\x13\x00\x00\x00\x02\x00\x00\x00\x00\x01\x02\x03\x44\
            \0\0\0";
        let mut cursor = Cursor::new(&PDU[..]);

        let res = Pdu::parse(&mut cursor).unwrap_err();
        assert_eq!(
            res.to_string(),
            "Error parsing PDU (\
            command_id=0x00000002, command_status=0x00000000, \
            sequence_number=0x01020344, field_name=interface_version): \
            Reached end of PDU length (or end of input) before finding all \
            fields of the PDU.",
        );
    }

    #[test]
    fn parse_bind_transmitter_hitting_eof_before_end_of_length() {
        const PDU: &[u8; 0x0b] =
            b"\x00\x00\x00\x2e\x00\x00\x00\x02\x00\x00\x00";
        let mut cursor = Cursor::new(&PDU[..]);

        let res = Pdu::parse(&mut cursor).unwrap_err();
        assert_eq!(
            res.to_string(),
            "Error parsing PDU (\
            command_id=0x00000002, command_status=UNKNOWN, \
            sequence_number=UNKNOWN, field_name=command_status): \
            Reached end of PDU length (or end of input) before finding all \
            fields of the PDU.",
        );
    }

    #[test]
    fn parse_bind_transmitter_with_short_length() {
        const PDU: &[u8; 4] = b"\x00\x00\x00\x04";
        let mut cursor = Cursor::new(&PDU);

        let res = Pdu::parse(&mut cursor).unwrap_err();
        assert_eq!(
            res.to_string(),
            "Error parsing PDU (\
            command_id=UNKNOWN, command_status=UNKNOWN, \
            sequence_number=UNKNOWN, field_name=UNKNOWN): \
            Length (4) too short.  Min allowed is 8 octets.",
        );
    }

    #[test]
    fn parse_bind_transmitter_with_massive_length() {
        const PDU: &[u8; 16] =
            b"\xff\xff\xff\xff\x00\x00\x00\x02\x00\x00\x00\x00\x00\x00\x00\x00";
        let mut cursor = Cursor::new(&PDU);

        let res = Pdu::parse(&mut cursor).unwrap_err();
        assert_eq!(
            res.to_string(),
            "Error parsing PDU (\
            command_id=0x00000002, command_status=0x00000000, \
            sequence_number=0x00000000, field_name=UNKNOWN): \
            Length (4294967295) too long.  Max allowed is 70000 octets.",
        );
    }

    #[test]
    fn parse_bind_transmitter_containing_nonascii_characters() {
        const PDU: &[u8; 0x2e + 0x6] =
            b"\x00\x00\x00\x2e\x00\x00\x00\x02\x00\x00\x00\x00\x01\x02\x03\x44\
            mys\xf0\x9f\x92\xa9m_ID\0pw$xx\0t_p_\0\x34\x13\x50rng\0foobar";
        let mut cursor = Cursor::new(&PDU);

        let res = Pdu::parse(&mut cursor).unwrap_err();
        assert_eq!(
            res.to_string(),
            "Error parsing PDU (\
            command_id=0x00000002, command_status=0x00000000, \
            sequence_number=0x01020344, field_name=system_id): \
            Octet String is not ASCII (valid up to byte 3).",
        );
    }

    #[test]
    fn parse_bind_transmitter_with_nonzero_status() {
        const PDU: &[u8; 0x2e + 0x6] =
            b"\x00\x00\x00\x2e\x00\x00\x00\x02\x00\x00\x00\x77\x01\x02\x03\x44\
            mysystem_ID\0pw$xx\0t_p_\0\x34\x13\x50rng\0foobar";
        let mut cursor = Cursor::new(&PDU);

        let res = Pdu::parse(&mut cursor).unwrap_err();
        assert_eq!(
            res.to_string(),
            "Error parsing PDU (\
            command_id=0x00000002, command_status=0x00000077, \
            sequence_number=0x01020344, field_name=command_status): \
            command_status must be 0, but was 0x00000077.",
        );
    }

    #[test]
    fn parse_valid_bind_transmitter_resp() {
        let mut cursor = Cursor::new(&BIND_TRANSMITTER_RESP_PDU_PLUS_EXTRA[..]);
        b"\x00\x00\x00\x1b\x80\x00\x00\x02\x00\x00\x00\x00\x00\x00\x00\x02\
        TestServer\0extrabytes";
        assert_eq!(
            Pdu::parse(&mut cursor).unwrap(),
            Pdu::new(
                BIND_TRANSMITTER_RESP,
                0x00000000,
                0x00000002,
                PduBody::BindTransmitterResp(
                    BindTransmitterRespPdu::new("TestServer",).unwrap(),
                )
            )
        );
    }

    #[test]
    fn parse_valid_submit_sm_with_short_message_and_no_tlvs() {
        const PDU: &[u8; 0x3d] = b"\
            \x00\x00\x00\x3d\
            \x00\x00\x00\x04\
            \x00\x00\x00\x00\
            \x00\x00\x00\x03\
            \x00\
            \x00\x00447000123123\x00\
            \x00\x00447111222222\x00\
            \x00\x01\x01\x00\x00\x01\x00\x03\
            \x00\x04hihi";

        let mut cursor = Cursor::new(&PDU[..]);
        assert_eq!(
            Pdu::parse(&mut cursor).unwrap(),
            Pdu::new(
                SUBMIT_SM,
                0x00000000,
                0x00000003,
                PduBody::SubmitSm(
                    SubmitSmPdu::new(
                        "",
                        0x00,
                        0x00,
                        "447000123123",
                        0x00,
                        0x00,
                        "447111222222",
                        0x00,
                        0x01,
                        0x01,
                        "",
                        "",
                        0x01,
                        0x00,
                        0x03,
                        0x00,
                        b"hihi"
                    )
                    .unwrap()
                )
            )
        );
    }

    #[test]
    fn parse_valid_submit_sm_with_empty_short_message_and_no_tlvs() {
        const PDU: &[u8; 0x3e] = b"\
            \x00\x00\x00\x39\
            \x00\x00\x00\x04\
            \x00\x00\x00\x00\
            \x00\x00\x00\x03\
            \x00\
            \x00\x00447000123123\x00\
            \x00\x00447111222222\x00\
            \x00\x01\x01\x00\x00\x01\x00\x03\
            \x00\x00extra";

        let mut cursor = Cursor::new(&PDU[..]);
        assert_eq!(
            Pdu::parse(&mut cursor).unwrap(),
            Pdu::new(
                SUBMIT_SM,
                0x00000000,
                0x00000003,
                PduBody::SubmitSm(
                    SubmitSmPdu::new(
                        "",
                        0x00,
                        0x00,
                        "447000123123",
                        0x00,
                        0x00,
                        "447111222222",
                        0x00,
                        0x01,
                        0x01,
                        "",
                        "",
                        0x01,
                        0x00,
                        0x03,
                        0x00,
                        &[]
                    )
                    .unwrap()
                ),
            )
        );
    }

    #[test]
    fn parse_submit_sm_with_too_long_message_length() {
        const PDU: &[u8; 0x3d] = b"\
            \x00\x00\x00\x3d\
            \x00\x00\x00\x04\
            \x00\x00\x00\x00\
            \x00\x00\x00\x03\
            \x00\
            \x00\x00447000123123\x00\
            \x00\x00447111222222\x00\
            \x00\x01\x01\x00\x00\x01\x00\x03\
            \x00\x08hihi";

        let mut cursor = Cursor::new(&PDU[..]);
        let err = Pdu::parse(&mut cursor).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Error parsing PDU \
            (command_id=0x00000004, command_status=0x00000000, \
            sequence_number=0x00000003, field_name=short_message): \
            IO error creating Octet String: failed to fill whole buffer"
        );
    }

    #[test]
    fn parse_submit_sm_resp_ok_with_message_id() {
        const PDU: &[u8; 0x3a] = b"\
            \x00\x00\x00\x35\
            \x80\x00\x00\x04\
            \x00\x00\x00\x00\
            \x00\x00\x00\x04\
            ea04b3d4-6a18-11eb-a387-c8f7507e3592\x00\
            extra";

        let mut cursor = Cursor::new(&PDU[..]);
        assert_eq!(
            Pdu::parse(&mut cursor).unwrap(),
            Pdu::new(
                SUBMIT_SM_RESP,
                0x00000000,
                0x00000004,
                PduBody::SubmitSmResp(
                    SubmitSmRespPdu::new(
                        "ea04b3d4-6a18-11eb-a387-c8f7507e3592",
                    )
                    .unwrap()
                )
            )
        );
    }

    #[test]
    fn parse_submit_sm_resp_ok_without_message_id_is_an_error() {
        const PDU: &[u8; 0x10] = b"\
            \x00\x00\x00\x10\
            \x80\x00\x00\x04\
            \x00\x00\x00\x00\
            \x00\x00\x00\x04";

        let mut cursor = Cursor::new(&PDU[..]);
        assert_eq!(
            Pdu::parse(&mut cursor).unwrap_err().to_string(),
            "Error parsing PDU (\
            command_id=0x80000004, command_status=0x00000000, \
            sequence_number=0x00000004, field_name=message_id): \
            C-Octet String does not end with the NULL character."
        );
        // Slightly unhelpful error message.  Better would be: submit_sm_resp
        // had command_status of zero but did not include a message_id.
    }

    #[test]
    fn parse_submit_sm_resp_error_without_message_id() {
        const PDU: &[u8; 0x10] = b"\
            \x00\x00\x00\x10\
            \x80\x00\x00\x04\
            \x00\x00\x00\x07\
            \x00\x00\x00\x04";

        let mut cursor = Cursor::new(&PDU[..]);
        assert_eq!(
            Pdu::parse(&mut cursor).unwrap(),
            Pdu::new_submit_sm_resp_error(0x00000007, 0x00000004)
        );
    }

    #[test]
    fn parse_submit_sm_resp_error_with_message_id_is_an_error() {
        const PDU: &[u8; 0x12] = b"\
            \x00\x00\x00\x12\
            \x80\x00\x00\x04\
            \x00\x00\x00\x07\
            \x00\x00\x00\x04\
            a\x00";

        let mut cursor = Cursor::new(&PDU[..]);
        assert_eq!(
            Pdu::parse(&mut cursor).unwrap_err().to_string(),
            "Error parsing PDU (\
            command_id=0x80000004, command_status=0x00000007, \
            sequence_number=0x00000004, field_name=UNKNOWN): \
            PDU body must not be supplied when status is not zero, but \
            command_status is 0x00000007.",
        );
    }

    // Issue#2: submit_sm with message_payload TLV and no short_message
    // Issue#2: submit_sm with message_payload TLV AND short_message is an error
}
