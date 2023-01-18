//! This module contains implementation of a generic BJNP packet.

use std::{fmt::Display, num::NonZeroU16};

pub use crate::header::{PacketType, PayloadType};
use crate::{
    header::Header,
    serdes::{Deserialize, ParseError, Serialize},
    write_nested,
};

#[derive(Debug, Clone)]
pub struct Packet<T> {
    header: Header,
    payload: T,
}

impl<T> Packet<T> {
    #[inline(always)]
    pub fn packet_type(&self) -> PacketType {
        self.header.packet_type
    }

    #[inline(always)]
    pub fn payload_type(&self) -> PayloadType {
        self.header.payload_type
    }

    #[inline(always)]
    pub fn error(&self) -> u8 {
        self.header.error
    }

    #[inline(always)]
    pub fn sequence(&self) -> u16 {
        self.header.sequence
    }

    #[inline(always)]
    pub fn job_id(&self) -> Option<NonZeroU16> {
        self.header.job_id
    }

    #[inline(always)]
    pub fn payload_size(&self) -> u32 {
        self.header.payload_size
    }

    #[inline(always)]
    pub fn payload_ref(&self) -> &T {
        &self.payload
    }

    #[inline(always)]
    pub fn payload(self) -> T {
        self.payload
    }
}

impl<T> Serialize for Packet<T>
where
    T: Serialize,
{
    fn serialize<W>(&self, writer: &mut W) -> Result<(), std::io::Error>
    where
        W: std::io::Write,
    {
        self.header.serialize(writer)?;
        self.payload.serialize(writer)?;
        Ok(())
    }

    fn size(&self) -> usize {
        self.header.size() + self.payload.size()
    }
}

impl<T> Display for Packet<T>
where
    T: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad("")?;
        f.write_fmt(format_args!("{}", self.header))?;
        write_nested!(f, self.payload)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct PacketBuilder {
    packet_type: PacketType,
    payload_type: PayloadType,
    error: Option<u8>,
    sequence: Option<u16>,
    job_id: Option<NonZeroU16>,
}

impl PacketBuilder {
    pub fn new(packet_type: PacketType, payload_type: PayloadType) -> Self {
        Self {
            packet_type,
            payload_type,
            error: None,
            sequence: None,
            job_id: None,
        }
    }

    #[inline(always)]
    pub fn packet_type(&mut self, packet_type: PacketType) -> &mut Self {
        self.packet_type = packet_type;
        self
    }

    #[inline(always)]
    pub fn payload_type(&mut self, payload_type: PayloadType) -> &mut Self {
        self.payload_type = payload_type;
        self
    }

    #[inline(always)]
    pub fn error(&mut self, error: u8) -> &mut Self {
        self.error = Some(error);
        self
    }

    #[inline(always)]
    pub fn sequence(&mut self, sequence: u16) -> &mut Self {
        self.sequence = Some(sequence);
        self
    }

    #[inline(always)]
    pub fn job_id(&mut self, job_id: NonZeroU16) -> &mut Self {
        self.job_id = Some(job_id);
        self
    }

    pub fn build<T: Serialize>(&self, payload: T) -> Packet<T> {
        let header = Header {
            packet_type: self.packet_type,
            payload_type: self.payload_type,
            error: self.error.unwrap_or(0),
            sequence: self.sequence.unwrap_or(0),
            job_id: self.job_id,
            payload_size: payload.size() as u32,
        };
        Packet { header, payload }
    }
}

#[derive(Debug, Clone)]
pub struct PacketHeaderOnly<'buf> {
    header: Header,
    payload: &'buf [u8],
}

impl<'buf> PacketHeaderOnly<'buf> {
    pub fn parse(buffer: &'buf [u8]) -> Result<Self, ParseError> {
        let (header, offset) = Header::deserialize(buffer)?;
        let payload_size = header.payload_size as usize;
        let payload =
            buffer
                .get(offset..offset + payload_size)
                .ok_or(ParseError::UnexpectedEnd {
                    expected: offset + payload_size,
                    actual: buffer.len(),
                })?;
        Ok(Self { header, payload })
    }

    #[inline(always)]
    pub fn packet_type(&self) -> PacketType {
        self.header.packet_type
    }

    #[inline(always)]
    pub fn payload_type(&self) -> PayloadType {
        self.header.payload_type
    }

    #[inline(always)]
    pub fn error(&self) -> u8 {
        self.header.error
    }

    #[inline(always)]
    pub fn sequence(&self) -> u16 {
        self.header.sequence
    }

    #[inline(always)]
    pub fn job_id(&self) -> Option<NonZeroU16> {
        self.header.job_id
    }

    #[inline(always)]
    pub fn payload_size(&self) -> u32 {
        self.header.payload_size
    }
}

impl<'buf> Display for PacketHeaderOnly<'buf> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.header.fmt(f)
    }
}

impl<'buf, T> TryFrom<PacketHeaderOnly<'buf>> for Packet<T>
where
    T: Deserialize,
{
    type Error = ParseError;

    fn try_from(packet: PacketHeaderOnly<'buf>) -> Result<Self, Self::Error> {
        let (payload, _) = T::deserialize(packet.payload)?;
        Ok(Self {
            header: packet.header,
            payload,
        })
    }
}
