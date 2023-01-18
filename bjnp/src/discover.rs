//! This module contains structs related to the response of a discover command

use std::{
    fmt::Display,
    mem::size_of,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    slice,
};

use memoffset::offset_of;

use crate::serdes::{
    Deserialize, FormatError, OffsetError, ParseError, Serialize, SizedDeserialize,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, packed)]
pub struct Eui48([u8; 6]);

impl Serialize for Eui48 {
    #[inline(always)]
    fn serialize<W>(&self, writer: &mut W) -> Result<(), std::io::Error>
    where
        W: std::io::Write,
    {
        writer.write_all(&self.0)
    }

    #[inline(always)]
    fn size(&self) -> usize {
        size_of::<Eui48>()
    }
}

impl SizedDeserialize for Eui48 {
    const SIZE: usize = size_of::<Self>();

    #[inline(always)]
    unsafe fn deserialize_exact(buffer: &[u8]) -> Result<Self, FormatError> {
        Ok(Self(buffer[..Self::SIZE].try_into().unwrap()))
    }
}

impl Display for Eui48 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        ))
    }
}

impl From<Eui48> for [u8; 6] {
    #[inline(always)]
    fn from(value: Eui48) -> Self {
        value.0
    }
}

impl From<[u8; 6]> for Eui48 {
    #[inline(always)]
    fn from(value: [u8; 6]) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, packed)]
pub struct Eui64([u8; 8]);

impl Serialize for Eui64 {
    #[inline(always)]
    fn serialize<W>(&self, writer: &mut W) -> Result<(), std::io::Error>
    where
        W: std::io::Write,
    {
        writer.write_all(&self.0)
    }

    #[inline(always)]
    fn size(&self) -> usize {
        size_of::<Eui64>()
    }
}

impl SizedDeserialize for Eui64 {
    const SIZE: usize = size_of::<Self>();

    #[inline(always)]
    unsafe fn deserialize_exact(buffer: &[u8]) -> Result<Self, FormatError> {
        Ok(Self(buffer[..Self::SIZE].try_into().unwrap()))
    }
}

impl Display for Eui64 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5], self.0[6], self.0[7],
        ))
    }
}

impl From<Eui64> for [u8; 8] {
    #[inline(always)]
    fn from(value: Eui64) -> Self {
        value.0
    }
}

impl From<[u8; 8]> for Eui64 {
    #[inline(always)]
    fn from(value: [u8; 8]) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacAddr {
    Eui48(Eui48),
    Eui64(Eui64),
}

impl Display for MacAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MacAddr::Eui48(addr) => addr.fmt(f),
            MacAddr::Eui64(addr) => addr.fmt(f),
        }
    }
}

impl From<Eui48> for MacAddr {
    #[inline(always)]
    fn from(value: Eui48) -> Self {
        Self::Eui48(value)
    }
}

impl From<Eui64> for MacAddr {
    #[inline(always)]
    fn from(value: Eui64) -> Self {
        Self::Eui64(value)
    }
}

impl Serialize for MacAddr {
    fn serialize<W>(&self, writer: &mut W) -> Result<(), std::io::Error>
    where
        W: std::io::Write,
    {
        use MacAddr::*;
        match self {
            Eui48(addr) => addr.serialize(writer),
            Eui64(addr) => addr.serialize(writer),
        }
    }

    fn size(&self) -> usize {
        use MacAddr::*;
        match self {
            Eui48(addr) => addr.size(),
            Eui64(addr) => addr.size(),
        }
    }
}

impl Serialize for Ipv4Addr {
    fn serialize<W>(&self, writer: &mut W) -> Result<(), std::io::Error>
    where
        W: std::io::Write,
    {
        writer.write_all(&self.octets())
    }

    #[inline(always)]
    fn size(&self) -> usize {
        size_of::<Self>()
    }
}

impl SizedDeserialize for Ipv4Addr {
    const SIZE: usize = size_of::<Self>();

    unsafe fn deserialize_exact(buffer: &[u8]) -> Result<Self, FormatError> {
        Ok(<Self as From<[u8; 4]>>::from(
            buffer[..Self::SIZE].try_into().unwrap(),
        ))
    }
}

impl Serialize for Ipv6Addr {
    fn serialize<W>(&self, writer: &mut W) -> Result<(), std::io::Error>
    where
        W: std::io::Write,
    {
        writer.write_all(&self.octets())
    }

    #[inline(always)]
    fn size(&self) -> usize {
        size_of::<Self>()
    }
}

impl SizedDeserialize for Ipv6Addr {
    const SIZE: usize = size_of::<Self>();

    unsafe fn deserialize_exact(buffer: &[u8]) -> Result<Self, FormatError> {
        Ok(<Self as From<[u8; 16]>>::from(
            buffer[..Self::SIZE].try_into().unwrap(),
        ))
    }
}

impl Serialize for IpAddr {
    fn serialize<W>(&self, writer: &mut W) -> Result<(), std::io::Error>
    where
        W: std::io::Write,
    {
        use IpAddr::*;
        match self {
            V4(addr) => addr.serialize(writer),
            V6(addr) => addr.serialize(writer),
        }
    }

    fn size(&self) -> usize {
        use IpAddr::*;
        match self {
            V4(addr) => addr.size(),
            V6(addr) => addr.size(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Response {
    mac_addr: MacAddr,
    ip_addr: IpAddr,
}

impl Response {
    pub fn new(mac_addr: MacAddr, ip_addr: IpAddr) -> Self {
        Self { mac_addr, ip_addr }
    }

    #[inline(always)]
    pub fn mac_addr(&self) -> &MacAddr {
        &self.mac_addr
    }

    #[inline(always)]
    pub fn ip_addr(&self) -> &IpAddr {
        &self.ip_addr
    }
}

impl Serialize for Response {
    fn serialize<W>(&self, writer: &mut W) -> Result<(), std::io::Error>
    where
        W: std::io::Write,
    {
        let raw_header = RawResponseHeader {
            unk_1: [0x00, 0x01, 0x08, 0x00],
            mac_len: self.mac_addr.size() as u8,
            ip_len: self.ip_addr.size() as u8,
        };
        raw_header.serialize(writer)?;
        self.mac_addr.serialize(writer)?;
        self.ip_addr.serialize(writer)?;
        Ok(())
    }

    fn size(&self) -> usize {
        size_of::<RawResponseHeader>() + self.mac_addr.size() + self.ip_addr.size()
    }
}

impl Deserialize for Response {
    fn deserialize(buffer: &[u8]) -> Result<(Response, usize), ParseError> {
        let (raw_header, mut size): (&RawResponseHeader, _) = Deserialize::deserialize(buffer)?;
        let buffer = &buffer[size..];
        let (mac_addr, next) = match raw_header.mac_len {
            6 => Eui48::deserialize(buffer)
                .map_err(|e| e.offset_by(size))
                .map(|(addr, size)| (addr.into(), size))?,
            8 => Eui64::deserialize(buffer)
                .map_err(|e| e.offset_by(size))
                .map(|(addr, size)| (addr.into(), size))?,
            b => {
                return Err(FormatError::InvalidByte {
                    byte: b,
                    offset: offset_of!(RawResponseHeader, mac_len),
                    message: "invalid MAC address size, can only be 6 or 8",
                }
                .into());
            }
        };
        size += next;
        let buffer = &buffer[next..];

        let (ip_addr, next) = match raw_header.ip_len {
            4 => Ipv4Addr::deserialize(buffer)
                .map_err(|e| e.offset_by(size))
                .map(|(addr, size)| (addr.into(), size))?,
            16 => Ipv6Addr::deserialize(buffer)
                .map_err(|e| e.offset_by(size))
                .map(|(addr, size)| (addr.into(), size))?,
            b => {
                return Err(FormatError::InvalidByte {
                    byte: b,
                    offset: offset_of!(RawResponseHeader, ip_len),
                    message: "invalid IP address size, can only be 4 or 16",
                }
                .into());
            }
        };
        size += next;
        Ok((Self { mac_addr, ip_addr }, size))
    }
}

impl Display for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "mac={mac} ip={ip}",
            mac = self.mac_addr,
            ip = self.ip_addr
        ))
    }
}

#[derive(Debug, Clone)]
#[repr(C, packed)]
struct RawResponseHeader {
    unk_1: [u8; 4], // 00 01 08 00
    mac_len: u8,
    ip_len: u8,
}

impl Serialize for RawResponseHeader {
    fn serialize<W>(&self, writer: &mut W) -> Result<(), std::io::Error>
    where
        W: std::io::Write,
    {
        // SAFETY: raw_u8 in scope and not escaping
        let raw_u8 =
            unsafe { slice::from_raw_parts((self as *const Self) as *const u8, size_of::<Self>()) };
        writer.write_all(raw_u8)
    }

    #[inline(always)]
    fn size(&self) -> usize {
        size_of::<RawResponseHeader>()
    }
}

impl SizedDeserialize for &RawResponseHeader {
    const SIZE: usize = size_of::<RawResponseHeader>();

    unsafe fn deserialize_exact(buffer: &[u8]) -> Result<Self, FormatError> {
        Ok(&*(buffer.as_ptr() as *const RawResponseHeader))
    }
}
