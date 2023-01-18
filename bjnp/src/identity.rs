//! This module contains structs related to the response of a get identity
//! command

use std::{
    collections::{hash_map, HashMap},
    fmt::Display,
    str,
};

use crate::serdes::{Deserialize, FormatError, OffsetError, ParseError, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response(HashMap<String, String>);

impl Response {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    pub fn iter(&self) -> hash_map::Iter<String, String> {
        self.0.iter()
    }

    fn as_str_len(&self) -> usize {
        self.0
            .iter()
            .map(|(key, value)| key.len() + value.len() + 2)
            .sum()
    }
}

impl Display for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad("")?;
        for (key, value) in self.0.iter() {
            f.write_fmt(format_args!("{}:{};", key, value))?;
        }
        Ok(())
    }
}

impl Deserialize for Response {
    fn deserialize(buffer: &[u8]) -> Result<(Self, usize), ParseError> {
        use FormatError::*;
        use ParseError::*;

        let identity_len = buffer.get(..2).ok_or_else(|| UnexpectedEnd {
            expected: 2,
            actual: buffer.len(),
        })?;
        // NOPANIC: identity_len == &[u8; 2]
        let mut identity_len = (u16::from_be_bytes(identity_len.try_into().unwrap())) as usize;
        let buffer = &buffer[2..];

        if identity_len < 2 {
            return Err(InvalidSlice {
                span: (0..2),
                message: "invalid length of identity, should always be >=2",
            }
            .into());
        }
        identity_len -= 2;

        let identity = buffer.get(..identity_len).ok_or_else(|| {
            UnexpectedEnd {
                expected: identity_len,
                actual: buffer.len(),
            }
            .offset_by(2)
        })?;
        let identity = str::from_utf8(identity).map_err(|e| match e.error_len() {
            Some(len) if len > 1 => InvalidSlice {
                span: (e.valid_up_to()..e.valid_up_to() + len),
                message: "invalid UTF-8 bytes",
            }
            .into(),
            Some(_) => InvalidByte {
                byte: buffer[e.valid_up_to() - 1],
                offset: e.valid_up_to() - 1,
                message: "invalid UTF-8 byte",
            }
            .into(),
            None => UnexpectedEnd {
                expected: identity_len + 1,
                actual: identity_len,
            },
        })?;

        let identity_map = identity
            .split_terminator(';')
            .filter_map(|item| item.split_once(':'))
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect();

        Ok((Self(identity_map), identity_len + 2))
    }
}

impl Serialize for Response {
    fn serialize<W>(&self, writer: &mut W) -> Result<(), std::io::Error>
    where
        W: std::io::Write,
    {
        let u16_size: u16 = self.as_str_len().try_into().map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "length of identity exceeds maximum limit (u16::MAX)",
            )
        })?;
        writer.write_all(&u16_size.to_be_bytes())?;
        writer.write_fmt(format_args!("{}", self))
    }

    fn size(&self) -> usize {
        2 + self.as_str_len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize() {
        // MFG:Canon;MDL:Dummy;CLS:IMAGE;
        let response = Response::deserialize(&[
            0x00, 0x20, 0x4d, 0x46, 0x47, 0x3a, 0x43, 0x61, 0x6e, 0x6f, 0x6e, 0x3b, 0x4d, 0x44,
            0x4c, 0x3a, 0x44, 0x75, 0x6d, 0x6d, 0x79, 0x3b, 0x43, 0x4c, 0x53, 0x3a, 0x49, 0x4d,
            0x41, 0x47, 0x45, 0x3b,
        ])
        .unwrap()
        .0;
        assert_eq!(response.get("MFG"), Some("Canon"));
        assert_eq!(response.get("MDL"), Some("Dummy"));
        assert_eq!(response.get("CLS"), Some("IMAGE"));
    }
}
