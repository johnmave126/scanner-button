//! This module contains traits and blanket implementation for the purpose of
//! serialization and deserialization.
//!
//! # Fixed Sized Auto Implementation
//! A typical usage is to define a struct for business usage, and another struct
//! for raw layout on the wire. In such case, this module will automatically
//! implement serialization and deserialization if corresponding [`From`] and
//! [`TryFrom`] are also implemented.
//!
//! See [`HasRawRepr`] for an example.

use std::{
    convert::{Infallible, TryInto},
    fmt::Display,
    io::{self, Write},
    mem::size_of,
    ops::Range,
    slice,
};

use thiserror::Error;

/// Error pertaining only the format
#[derive(Error, Debug, Clone)]
pub enum FormatError {
    #[error("invalid byte `{byte:#02x}` at offset {offset}: {message}")]
    InvalidByte {
        byte: u8,
        offset: usize,
        message: &'static str,
    },
    #[error("invalid byte slice at offset ({}..{}): {message}", .span.start, .span.end)]
    InvalidSlice {
        span: Range<usize>,
        message: &'static str,
    },
}

/// Error pertaining format and mismatching size
#[derive(Error, Debug, Clone)]
pub enum ParseError {
    #[error("invalid packet format")]
    InvalidFormat(#[from] FormatError),
    #[error("unexpected end of packet, expect size >= {expected}, found size {actual}")]
    UnexpectedEnd { expected: usize, actual: usize },
}

impl From<Infallible> for FormatError {
    fn from(x: Infallible) -> FormatError {
        match x {}
    }
}

impl From<Infallible> for ParseError {
    fn from(x: Infallible) -> ParseError {
        match x {}
    }
}

#[doc(hidden)]
pub trait OffsetError {
    fn offset_by(self, offset: usize) -> Self;
}

#[doc(hidden)]
impl OffsetError for FormatError {
    fn offset_by(mut self, by: usize) -> Self {
        use FormatError::*;
        match &mut self {
            InvalidByte { offset, .. } => {
                *offset += by;
            }
            InvalidSlice { span, .. } => {
                span.end += by;
                span.start += by;
            }
        }
        self
    }
}

#[doc(hidden)]
impl OffsetError for ParseError {
    fn offset_by(self, offset: usize) -> Self {
        use ParseError::*;
        match self {
            InvalidFormat(err) => InvalidFormat(err.offset_by(offset)),
            UnexpectedEnd { expected, actual } => UnexpectedEnd {
                expected: expected + offset,
                actual,
            },
        }
    }
}

#[doc(hidden)]
impl<T, U> OffsetError for Result<U, T>
where
    T: OffsetError,
{
    fn offset_by(self, offset: usize) -> Self {
        self.map_err(|err| err.offset_by(offset))
    }
}

/// Link a struct to its raw representation, allowing auto implementation of
/// [`Serialize`] and [`Deserialize`].
///
///
/// # Example
/// ```
/// use bjnp::serdes::{Deserialize, FormatError, HasRawRepr, Serialize};
/// # use bjnp::serdes::{ParseError};
///
/// // some payload struct
/// #[derive(Debug, PartialEq, Eq)]
/// struct Payload {
///     some_number: u32,
///     other_number: u16,
///     more_number: u32,
/// }
///
/// // corresponding raw representation
/// #[repr(C, packed)]
/// struct PayloadRaw {
///     // magic beginning, must be 0x01020304
///     magic: [u8; 4],
///     // u32, big endian
///     some_number: [u8; 4],
///     // u16, little endian
///     other_number: [u8; 2],
///     // zero padding
///     padding: [u8; 2],
///     // u32, little   endian
///     more_number: [u8; 4],
/// }
///
/// // declare the relationship between 2 types
/// impl HasRawRepr for Payload {
///     type Repr = PayloadRaw;
/// }
///
/// impl From<&Payload> for PayloadRaw {
///     fn from(payload: &Payload) -> Self {
///         Self {
///             magic: [0x01, 0x02, 0x03, 0x04],
///             some_number: payload.some_number.to_be_bytes(),
///             other_number: payload.other_number.to_le_bytes(),
///             padding: [0; 2],
///             more_number: payload.more_number.to_le_bytes(),
///         }
///     }
/// }
///
/// impl TryFrom<&PayloadRaw> for Payload {
///     type Error = FormatError;
///
///     fn try_from(raw_payload: &PayloadRaw) -> Result<Self, Self::Error> {
///         use FormatError::*;
///
///         if &raw_payload.magic == &[0x01, 0x02, 0x03, 0x04] {
///             Ok(Self {
///                 some_number: u32::from_be_bytes(raw_payload.some_number),
///                 other_number: u16::from_le_bytes(raw_payload.other_number),
///                 more_number: u32::from_le_bytes(raw_payload.more_number),
///             })
///         } else {
///             Err(InvalidSlice {
///                 span: (0..4),
///                 message: "magic bytes not `0x01020304`",
///             })
///         }
///     }
/// }
///
/// # fn main() -> Result<(), ParseError> {
/// let payload = Payload {
///     some_number: 0x05060708,
///     other_number: 0x0304,
///     more_number: 0x05060708,
/// };
///
/// // serialize
/// let serialized = payload.serialize_to_vec();
/// assert_eq!(
///     &serialized,
///     &[
///         0x01, 0x02, 0x03, 0x04, // magic
///         0x05, 0x06, 0x07, 0x08, // 0x05060708 in big endian
///         0x04, 0x03, // 0x0304 in little endian
///         0x00, 0x00, // padding
///         0x08, 0x07, 0x06, 0x05 // 0x05060708 in little endian
///     ]
/// );
///
/// // deserialize
/// let payload_2 = Payload::deserialize(&serialized)?;
/// assert_eq!(payload, payload_2.0);
/// # Ok(())
/// # }
/// ```
pub trait HasRawRepr {
    /// Raw representation of `Self` type.
    type Repr: Sized;
}

pub trait Serialize {
    fn serialize<W>(&self, writer: &mut W) -> Result<(), io::Error>
    where
        W: Write;

    fn size(&self) -> usize;

    fn serialize_to_vec(&self) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(self.size());
        // NOPANIC: write to an allocated vector should never fail
        self.serialize(&mut buffer).unwrap();
        buffer
    }
}

impl<T> Serialize for T
where
    T: HasRawRepr,
    T::Repr: for<'a> From<&'a T>,
{
    fn serialize<W>(&self, writer: &mut W) -> Result<(), io::Error>
    where
        W: Write,
    {
        let raw_repr = T::Repr::from(self);
        // SAFETY: raw_u8 and raw_repr in scope and not escaping
        let raw_u8 = unsafe {
            slice::from_raw_parts(
                ((&raw_repr) as *const T::Repr) as *const u8,
                size_of::<T::Repr>(),
            )
        };
        writer.write_all(raw_u8)?;
        Ok(())
    }

    #[inline(always)]
    fn size(&self) -> usize {
        size_of::<T::Repr>()
    }
}

pub trait Deserialize: Sized {
    fn deserialize(buffer: &[u8]) -> Result<(Self, usize), ParseError>;
}

pub(crate) fn deserialized_into<T, U: From<T>>((obj, size): (T, usize)) -> (U, usize) {
    (obj.into(), size)
}

pub trait SizedDeserialize: Sized {
    const SIZE: usize;
    unsafe fn deserialize_exact(buffer: &[u8]) -> Result<Self, FormatError>;
}

impl<T> Deserialize for T
where
    T: SizedDeserialize,
{
    #[inline]
    fn deserialize(buffer: &[u8]) -> Result<(T, usize), ParseError> {
        if buffer.len() < T::SIZE {
            Err(ParseError::UnexpectedEnd {
                expected: T::SIZE,
                actual: buffer.len(),
            })
        } else {
            unsafe { Ok((T::deserialize_exact(&buffer)?, T::SIZE)) }
        }
    }
}

impl<T, E> SizedDeserialize for T
where
    E: Into<FormatError>,
    T: HasRawRepr + for<'a> TryFrom<&'a T::Repr, Error = E>,
{
    const SIZE: usize = size_of::<T::Repr>();
    unsafe fn deserialize_exact(buffer: &[u8]) -> Result<Self, FormatError> {
        let raw_repr = &*(buffer.as_ptr() as *const T::Repr);
        raw_repr.try_into().map_err(Into::into)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Empty;

impl Display for Empty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<empty>")
    }
}

impl Serialize for Empty {
    #[inline(always)]
    fn serialize<W>(&self, _writer: &mut W) -> Result<(), io::Error>
    where
        W: Write,
    {
        Ok(())
    }

    #[inline(always)]
    fn size(&self) -> usize {
        0
    }
}

impl Deserialize for Empty {
    #[inline(always)]
    fn deserialize(_buffer: &[u8]) -> Result<(Self, usize), ParseError> {
        Ok((Empty, 0))
    }
}

macro_rules! make_u8_field {
    (
        $(#[doc = $field_docs: expr])?
        #[display($field_name: expr)]
        $(#[$field_attr: meta])*
        $visibility: vis enum $field: ident {
            $(
                $(#[doc = $variant_docs: expr])?
                #[display($variant_name: expr)]
                $(#[$variant_attr: meta])*
                $variant: ident = $value: literal,
            )+
        }
    ) => {
        $(#[doc = $field_docs])?
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[repr(u8)]
        $(#[$field_attr])*
        $visibility enum $field {
            $(
                $(#[doc = $variant_docs])?
                $(#[$variant_attr])*
                $variant = $value,
            )+
        }

        impl TryFrom<u8> for $field {
            type Error = crate::serdes::FormatError;

            fn try_from(value: u8) -> Result<Self, Self::Error> {
                use $field::*;
                match value {
                    $($value => Ok($variant), )+
                    _ => Err(crate::serdes::FormatError::InvalidByte {
                        byte: value,
                        offset: 0,
                        message: concat!("unknown ", $field_name)
                    })
                }
            }
        }

        impl ::std::fmt::Display for $field {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                use $field::*;
                match self {
                    $($variant => f.write_str($variant_name), )+
                }
            }
        }
    };
}
pub(crate) use make_u8_field;

macro_rules! make_wider_field {
    (
        $(#[doc = $field_docs: expr])?
        #[display($field_name: expr)]
        #[repr($type_name: ty)]
        $(#[$field_attr: meta])*
        $visibility: vis enum $field: ident {
            $(
                $(#[doc = $variant_docs: expr])?
                #[display($variant_name: expr)]
                $(#[$variant_attr: meta])*
                $variant: ident = $value: literal,
            )+
        }
    ) => {
        $(#[doc = $field_docs])?
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[repr($type_name)]
        $(#[$field_attr])*
        $visibility enum $field {
            $(
                $(#[doc =$variant_docs])?
                $(#[$variant_attr])*
                $variant = $value,
            )+
        }

        impl TryFrom<$type_name> for $field {
            type Error = crate::serdes::FormatError;

            fn try_from(value: $type_name) -> Result<Self, Self::Error> {
                use $field::*;
                match value {
                    $($value => Ok($variant), )+
                    _ => Err(crate::serdes::FormatError::InvalidSlice {
                        span: (0..::std::mem::size_of::<$type_name>()),
                        message: concat!("unknown ", $field_name)
                    })
                }
            }
        }

        impl ::std::fmt::Display for $field {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                use $field::*;
                match self {
                    $($variant => f.write_str($variant_name), )+
                }
            }
        }
    };
}
pub(crate) use make_wider_field;
