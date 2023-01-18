//! This module contains implementation of BJNP header.

use std::{fmt::Display, num::NonZeroU16};

use memoffset::offset_of;

use crate::serdes::{make_u8_field, FormatError, HasRawRepr, OffsetError};

const MAGIC: &[u8; 4] = b"BJNP";

make_u8_field! {
    #[display("packet type")]
    pub enum PacketType {
        #[display("printer cmd")]
        PrinterCommand = 0x01,
        #[display("scanner cmd")]
        ScannerCommand = 0x02,
        #[display("printer res")]
        PrinterResponse = 0x81,
        #[display("scanner res")]
        ScannerResponse = 0x82,
    }
}

make_u8_field! {
    #[display("payload type")]
    pub enum PayloadType {
        #[display("discover")]
        Discover = 0x01,
        #[display("start scan")]
        StartScan = 0x02,
        #[display("job details")]
        JobDetails = 0x10,
        #[display("close")]
        Close = 0x11,
        #[display("read")]
        Read = 0x20,
        #[display("write")]
        Write = 0x21,
        #[display("get identity")]
        GetId = 0x30,
        #[display("poll")]
        Poll = 0x32,
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Header {
    pub(crate) packet_type: PacketType,
    pub(crate) payload_type: PayloadType,
    pub(crate) error: u8,
    pub(crate) sequence: u16,
    pub(crate) job_id: Option<NonZeroU16>,
    pub(crate) payload_size: u32,
}

#[doc(hidden)]
#[derive(Debug, Clone)]
#[repr(C, packed)]
pub(crate) struct RawHeader {
    magic: [u8; 4],
    packet_type: u8,
    payload_type: u8,
    error: u8,
    unk_1: u8,
    sequence: [u8; 2],
    job_id: [u8; 2],
    len: [u8; 4],
}

impl Display for Header {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad("")?;
        f.write_fmt(format_args!(
            "[{}] [{}] error={:#02x} sequence={}",
            self.packet_type, self.payload_type, self.error, self.sequence
        ))?;
        if let Some(job_id) = self.job_id {
            f.write_fmt(format_args!(" job_id={job_id}"))?;
        }
        f.write_fmt(format_args!(" payload_len={}", self.payload_size))?;
        Ok(())
    }
}

impl HasRawRepr for Header {
    type Repr = RawHeader;
}

impl From<&Header> for RawHeader {
    fn from(header: &Header) -> Self {
        Self {
            magic: MAGIC.to_owned(),
            packet_type: header.packet_type as u8,
            payload_type: header.payload_type as u8,
            error: header.error,
            unk_1: 0,
            sequence: header.sequence.to_be_bytes(),
            job_id: header
                .job_id
                .map(NonZeroU16::get)
                .unwrap_or(0)
                .to_be_bytes(),
            len: header.payload_size.to_be_bytes(),
        }
    }
}

impl TryFrom<&RawHeader> for Header {
    type Error = FormatError;

    fn try_from(raw_header: &RawHeader) -> Result<Self, Self::Error> {
        if &raw_header.magic != MAGIC {
            return Err(FormatError::InvalidSlice {
                span: (0..4),
                message: "magic bytes is not b'BJNP'",
            });
        }

        let packet_type = raw_header.packet_type.try_into()?;
        let payload_type = raw_header
            .payload_type
            .try_into()
            .offset_by(offset_of!(RawHeader, payload_type))?;
        let sequence = u16::from_be_bytes(raw_header.sequence);
        let job_id = NonZeroU16::new(u16::from_be_bytes(raw_header.job_id));
        let len = u32::from_be_bytes(raw_header.len);
        Ok(Self {
            packet_type,
            payload_type,
            error: raw_header.error,
            sequence,
            job_id,
            payload_size: len,
        })
    }
}
