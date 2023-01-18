//! This module contains structs related to the response of a poll request

use std::fmt::Display;

use crate::{
    serdes::{make_u8_field, FormatError, HasRawRepr},
    write_nested,
};

make_u8_field! {
    #[display("color mode")]
    pub enum ColorMode {
        #[display("color")]
        Color = 0x01,
        #[display("mono")]
        Mono = 0x02,
    }
}

make_u8_field! {
    #[display("page size")]
    pub enum Size {
        #[display("A4")]
        A4 = 0x01,
        #[display("Letter")]
        Letter = 0x02,
        #[display("10x15")]
        _10x15 = 0x08,
        #[display("13x18")]
        _13x18 = 0x09,
        #[display("Auto")]
        Auto = 0x0b,
    }
}

make_u8_field! {
    #[display("format")]
    pub enum Format {
        #[display("JPEG")]
        Jpeg = 0x01,
        #[display("TIFF")]
        Tiff = 0x02,
        #[display("PDF")]
        Pdf = 0x03,
        #[display("Kompakt-PDF")]
        KompaktPdf = 0x04,
    }
}

make_u8_field! {
    #[display("DPI")]
    pub enum DPI {
        #[display("75")]
        _75 = 0x01,
        #[display("150")]
        _150 = 0x02,
        #[display("300")]
        _300 = 0x03,
        #[display("600")]
        _600 = 0x04,
    }
}

make_u8_field! {
    #[display("source")]
    pub enum Source {
        #[display("flatbed")]
        Flatbed = 0x01,
        #[display("feeder")]
        AutoDocumentFeeder = 0x02,
    }
}

make_u8_field! {
    #[display("feeder type")]
    pub enum FeederType {
        #[display("simplex")]
        Simplex = 0x01,
        #[display("duplex")]
        Duplex = 0x02,
    }
}

make_u8_field! {
    #[display("feeder orientation")]
    pub enum FeederOrientation {
        #[display("portrait")]
        Portrait = 0x01,
        #[display("landscape")]
        Landscape = 0x02,
    }
}

impl DPI {
    pub fn dpi_value(&self) -> u32 {
        match self {
            DPI::_75 => 75,
            DPI::_150 => 150,
            DPI::_300 => 300,
            DPI::_600 => 600,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Interrupt {
    color_mode: ColorMode,
    size: Size,
    format: Format,
    dpi: DPI,
    source: Source,
    feeder_type: Option<FeederType>,
    feeder_orientation: Option<FeederOrientation>,
}

/// Interrupt layout for MX920
#[doc(hidden)]
#[derive(Debug, Clone)]
#[repr(C, packed)]
pub struct RawInterrupt {
    unk_1: [u8; 7],
    color_mode: u8,  // pos 7
    source: u8,      // pos 8
    feeder_type: u8, // pos 9
    size: u8,        // pos 10
    format: u8,      // pos 11
    dpi: u8,         // pos 12
    unk_4: [u8; 3],
    feeder_orientation: u8, // pos 16
    unk_5: [u8; 3],
}

impl Interrupt {
    #[inline(always)]
    pub fn color_mode(&self) -> ColorMode {
        self.color_mode
    }

    #[inline(always)]
    pub fn size(&self) -> Size {
        self.size
    }

    #[inline(always)]
    pub fn format(&self) -> Format {
        self.format
    }

    #[inline(always)]
    pub fn dpi(&self) -> DPI {
        self.dpi
    }

    #[inline(always)]
    pub fn source(&self) -> Source {
        self.source
    }

    #[inline(always)]
    pub fn feeder_type(&self) -> Option<FeederType> {
        self.feeder_type
    }

    #[inline(always)]
    pub fn feeder_orientation(&self) -> Option<FeederOrientation> {
        self.feeder_orientation
    }
}

impl HasRawRepr for Interrupt {
    type Repr = RawInterrupt;
}

impl From<&Interrupt> for RawInterrupt {
    fn from(interrupt: &Interrupt) -> Self {
        Self {
            unk_1: [0; 7],
            color_mode: interrupt.color_mode as u8,
            source: interrupt.source as u8,
            feeder_type: interrupt.feeder_type.map(|v| v as u8).unwrap_or(0),
            size: interrupt.size as u8,
            format: interrupt.format as u8,
            dpi: interrupt.dpi as u8,
            unk_4: [0; 3],
            feeder_orientation: interrupt.feeder_orientation.map(|v| v as u8).unwrap_or(0),
            unk_5: [0; 3],
        }
    }
}

impl TryFrom<&RawInterrupt> for Interrupt {
    type Error = FormatError;

    fn try_from(raw_interrupt: &RawInterrupt) -> Result<Self, Self::Error> {
        let feeder_type = if raw_interrupt.feeder_type != 0 {
            Some(raw_interrupt.feeder_type.try_into()?)
        } else {
            None
        };

        let feeder_orientation = if raw_interrupt.feeder_orientation != 0 {
            Some(raw_interrupt.feeder_orientation.try_into()?)
        } else {
            None
        };

        Ok(Self {
            color_mode: raw_interrupt.color_mode.try_into()?,
            source: raw_interrupt.source.try_into()?,
            feeder_type,
            size: raw_interrupt.size.try_into()?,
            format: raw_interrupt.format.try_into()?,
            dpi: raw_interrupt.dpi.try_into()?,
            feeder_orientation,
        })
    }
}

impl Display for Interrupt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad("")?;
        f.write_fmt(format_args!(
            "interrupt: color_mode={} size={} source={} format={} dpi={}",
            self.color_mode, self.size, self.source, self.format, self.dpi
        ))?;
        if let Some(feeder_type) = self.feeder_type.as_ref() {
            f.write_fmt(format_args!(" feeder_type={feeder_type}"))?;
        }
        if let Some(feeder_orientation) = self.feeder_orientation.as_ref() {
            f.write_fmt(format_args!(" feeder_orientation={feeder_orientation}"))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Response {
    status: u32,
    session_id: Option<u32>,
    action_id: Option<u32>,
    interrupt: Option<Interrupt>,
}

#[doc(hidden)]
#[derive(Debug, Clone)]
#[repr(C, packed)]
pub struct RawResponse {
    status: [u8; 4],
    session_id: [u8; 4],
    unk_1: [u8; 4], // 00 00 00 14
    action_id: [u8; 4],
    interrupt: RawInterrupt,
}

impl Response {
    pub fn status(&self) -> u32 {
        self.status
    }

    pub fn session_id(&self) -> Option<u32> {
        self.session_id
    }

    pub fn action_id(&self) -> Option<u32> {
        self.action_id
    }

    pub fn interrupt(&self) -> Option<&Interrupt> {
        self.interrupt.as_ref()
    }
}

impl HasRawRepr for Response {
    type Repr = RawResponse;
}

impl TryFrom<&RawResponse> for Response {
    type Error = FormatError;

    fn try_from(raw_response: &RawResponse) -> Result<Self, Self::Error> {
        let status = u32::from_be_bytes(raw_response.status);
        if status & 0x00008000 != 0 {
            // interrupted
            let action_id = u32::from_be_bytes(raw_response.action_id);
            let interrupt = (&raw_response.interrupt).try_into()?;
            Ok(Self {
                status,
                session_id: None,
                action_id: Some(action_id),
                interrupt: Some(interrupt),
            })
        } else {
            let session_id = u32::from_be_bytes(raw_response.session_id);
            Ok(Self {
                status,
                session_id: Some(session_id),
                action_id: None,
                interrupt: None,
            })
        }
    }
}

impl Display for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad("")?;
        f.write_fmt(format_args!("status={:#08x}", self.status))?;
        if let Some(session_id) = self.session_id.as_ref() {
            f.write_fmt(format_args!(" session_id={session_id}"))?;
        }
        if let Some(action_id) = self.action_id.as_ref() {
            f.write_fmt(format_args!(" action_id={action_id}"))?;
        }
        if let Some(interrupt) = self.interrupt.as_ref() {
            write_nested!(f, interrupt)?;
        }
        Ok(())
    }
}
