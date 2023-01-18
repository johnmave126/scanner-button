//! This module contains structs related to the command of a poll request.

use std::{convert::Infallible, fmt::Display, mem::transmute, slice};

use memoffset::span_of;
use time::{
    format_description::FormatItem, macros::format_description, parsing::Parsed, PrimitiveDateTime,
};

use crate::serdes::{
    deserialized_into, make_wider_field, Deserialize, FormatError, HasRawRepr, OffsetError,
    ParseError, Serialize,
};

make_wider_field! {
    #[display("poll type")]
    #[repr(u16)]
    pub enum PollType {
        #[display("empty")]
        Empty = 0x00,
        #[display("host only")]
        HostOnly = 0x01,
        #[display("full")]
        Full = 0x02,
        #[display("reset")]
        Reset = 0x05,
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(align(2))]
pub struct Host([u8; Host::MAX_HOST_LENGTH]);

impl Host {
    const MAX_HOST_LENGTH: usize = 64;

    pub fn new<T: AsRef<str>>(host: T) -> Self {
        // alignment = 2
        let mut u16_buffer: [u16; Self::MAX_HOST_LENGTH / 2] = [0; Self::MAX_HOST_LENGTH / 2];
        let mut overflowing = false;
        let mut cur_len = 0;
        // holding 4 previous character lengths
        // since each character can only take 1 or 2 u16, each length takes at most 2
        // bits
        let mut prev_len: u8 = 0;
        for c in host.as_ref().chars() {
            let cur_start = cur_len;
            cur_len += c.len_utf16();
            // pack current character length in `prev_len`
            prev_len = (prev_len << 2) | (c.len_utf16() as u8);
            if cur_len > u16_buffer.len() {
                overflowing = true;
                break;
            } else {
                // NOPANIC: cur_len <= u16_buf.len()
                c.encode_utf16(&mut u16_buffer[cur_start..]);
            }
        }

        if overflowing {
            // backing until we can fit in "..."
            // 1. prev_len must contain exactly 4 lengths since overflow is happening
            // 2. prev_len contains exactly 3 characters in range, so guaranteed to fit
            // "..."
            while cur_len > u16_buffer.len() - 3 {
                cur_len -= (prev_len & 0b0000_0011) as usize;
                prev_len >>= 2;
            }
            u16_buffer[cur_len..cur_len + 3].fill('.' as u16);
            u16_buffer[cur_len + 3..].fill(0);
        }

        // it is always big endian on the wire
        for c in u16_buffer.iter_mut() {
            *c = c.to_be();
        }

        // SAFETY: u16_buffer has alignment 2, same as host
        let u8_buffer = unsafe { transmute(u16_buffer) };
        Self(u8_buffer)
    }

    pub fn into_buf(self) -> [u16; Self::MAX_HOST_LENGTH / 2] {
        // SAFETY: alignment of self is 2, same as u16
        let mut u16_buffer: [u16; Self::MAX_HOST_LENGTH / 2] = unsafe { transmute(self.0) };

        // it is always big endian on the wire
        for c in u16_buffer.iter_mut() {
            *c = u16::from_be(*c);
        }

        u16_buffer
    }
}

impl Display for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut u16_buffer: [u16; Self::MAX_HOST_LENGTH / 2] = [0; Self::MAX_HOST_LENGTH / 2];
        // SAFETY: alignment requirement of u8 < u16, size_of::<u8>() * 2 ==
        // size_of::<u16>()
        let u8_buffer: &mut [u8] = unsafe {
            slice::from_raw_parts_mut(u16_buffer.as_mut_ptr().cast(), u16_buffer.len() * 2)
        };
        u8_buffer.copy_from_slice(&self.0);

        // it is always big endian on the wire
        for c in u16_buffer.iter_mut() {
            *c = u16::from_be(*c);
        }

        // Host could contain invalid codepoint, so we use lossy decoding to display it
        String::from_utf16_lossy(&u16_buffer).fmt(f)
    }
}

#[derive(Debug, Clone)]
pub struct Command(InnerCommand);

#[derive(Debug, Clone)]
enum InnerCommand {
    Empty(EmptyCommand),
    HostOnly(HostOnlyCommand),
    Full(FullCommand),
    Reset(ResetCommand),
}

impl Command {
    pub fn poll_type(&self) -> PollType {
        use InnerCommand::*;
        match &self.0 {
            Empty(_) => PollType::Empty,
            HostOnly(_) => PollType::HostOnly,
            Full(_) => PollType::Full,
            Reset(_) => PollType::Reset,
        }
    }

    pub fn session_id(&self) -> Option<u32> {
        use InnerCommand::*;
        match &self.0 {
            Full(command) => Some(command.session_id),
            Reset(command) => Some(command.session_id),
            _ => None,
        }
    }

    pub fn host(&self) -> Option<&Host> {
        use InnerCommand::*;
        match &self.0 {
            Empty(_) => None,
            HostOnly(command) => Some(&command.host),
            Full(command) => Some(&command.host),
            Reset(command) => Some(&command.host),
        }
    }

    pub fn action_id(&self) -> Option<u32> {
        use InnerCommand::*;
        match &self.0 {
            Reset(command) => Some(command.action_id),
            _ => None,
        }
    }

    pub fn datetime(&self) -> Option<&PrimitiveDateTime> {
        use InnerCommand::*;
        match &self.0 {
            Full(command) => Some(&command.datetime),
            _ => None,
        }
    }
}

impl Serialize for Command {
    fn serialize<W>(&self, writer: &mut W) -> Result<(), std::io::Error>
    where
        W: std::io::Write,
    {
        use InnerCommand::*;
        writer.write_all(&(self.poll_type() as u16).to_be_bytes())?;
        match &self.0 {
            Empty(command) => command.serialize(writer),
            HostOnly(command) => command.serialize(writer),
            Full(command) => command.serialize(writer),
            Reset(command) => command.serialize(writer),
        }
    }

    fn size(&self) -> usize {
        use InnerCommand::*;
        2 + match &self.0 {
            Empty(command) => command.size(),
            HostOnly(command) => command.size(),
            Full(command) => command.size(),
            Reset(command) => command.size(),
        }
    }
}

impl Deserialize for Command {
    fn deserialize(buffer: &[u8]) -> Result<(Self, usize), ParseError> {
        use PollType::*;

        let poll_type = buffer.get(0..2).ok_or(ParseError::UnexpectedEnd {
            expected: 2,
            actual: buffer.len(),
        })?;
        // NOPANIC: poll_type == &[u8; 2]
        let poll_type = u16::from_be_bytes(poll_type.try_into().unwrap());
        let poll_type: PollType = poll_type.try_into()?;
        let buffer = &buffer[2..];

        let deserialize_result = match poll_type {
            Empty => EmptyCommand::deserialize(buffer).map(deserialized_into),
            HostOnly => HostOnlyCommand::deserialize(buffer).map(deserialized_into),
            Full => FullCommand::deserialize(buffer).map(deserialized_into),
            Reset => ResetCommand::deserialize(buffer).map(deserialized_into),
        };

        deserialize_result
            .map(|(cmd, size)| (cmd, size + 2))
            .map_err(|e| e.offset_by(2))
    }
}

impl Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use InnerCommand::*;
        f.pad("")?;
        match &self.0 {
            Empty(_) => f.write_fmt(format_args!("empty")),
            HostOnly(cmd) => f.write_fmt(format_args!("host-only: {}", cmd)),
            Full(cmd) => f.write_fmt(format_args!("full: {}", cmd)),
            Reset(cmd) => f.write_fmt(format_args!("reset: {}", cmd)),
        }
    }
}

#[derive(Debug, Clone)]
struct EmptyCommand;

#[derive(Debug, Clone)]
#[repr(C, packed)]
struct RawEmptyCommand {
    empty: [u8; 78],
}

impl HasRawRepr for EmptyCommand {
    type Repr = RawEmptyCommand;
}

impl From<&EmptyCommand> for RawEmptyCommand {
    #[inline(always)]
    fn from(_: &EmptyCommand) -> Self {
        Self { empty: [0; 78] }
    }
}

impl TryFrom<&RawEmptyCommand> for EmptyCommand {
    type Error = Infallible;

    #[inline(always)]
    fn try_from(_: &RawEmptyCommand) -> Result<Self, Self::Error> {
        Ok(EmptyCommand)
    }
}

impl Display for EmptyCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad("")?;
        f.write_str("empty")
    }
}

impl From<EmptyCommand> for Command {
    fn from(value: EmptyCommand) -> Self {
        Self(InnerCommand::Empty(value))
    }
}

#[doc(hidden)]
#[derive(Debug, Clone)]
struct HostOnlyCommand {
    host: Host,
}

#[derive(Debug, Clone)]
#[repr(C, packed)]
struct RawHostOnlyCommand {
    pad_1: [u8; 6],
    host: [u8; Host::MAX_HOST_LENGTH],
    unk_1: [u8; 4],
}

impl HasRawRepr for HostOnlyCommand {
    type Repr = RawHostOnlyCommand;
}

impl From<&HostOnlyCommand> for RawHostOnlyCommand {
    fn from(command: &HostOnlyCommand) -> Self {
        Self {
            pad_1: [0; 6],
            host: command.host.0,
            unk_1: [0; 4],
        }
    }
}

impl TryFrom<&RawHostOnlyCommand> for HostOnlyCommand {
    type Error = Infallible;

    fn try_from(raw_command: &RawHostOnlyCommand) -> Result<Self, Self::Error> {
        // We don't check validity of host string, downstream use could be lossy
        Ok(Self {
            host: Host(raw_command.host),
        })
    }
}

impl Display for HostOnlyCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("host={}", self.host))
    }
}

impl From<HostOnlyCommand> for Command {
    fn from(value: HostOnlyCommand) -> Self {
        Self(InnerCommand::HostOnly(value))
    }
}

#[derive(Debug, Clone)]
struct FullCommand {
    session_id: u32,
    host: Host,
    datetime: PrimitiveDateTime,
}

#[derive(Debug, Clone)]
#[repr(C, packed)]
struct RawFullCommand {
    pad_1: [u8; 2],
    session_id: [u8; 4],
    host: [u8; Host::MAX_HOST_LENGTH],
    unk_1: [u8; 4], // 00 00 00 14
    unk_2: [u8; 20],
    unk_3: [u8; 4], // 00 00 00 10
    datetime: [u8; 14],
    pad_2: [u8; 2],
}

impl HasRawRepr for FullCommand {
    type Repr = RawFullCommand;
}

impl RawFullCommand {
    const DATETIME_FORMAT: &'static [FormatItem<'static>] =
        format_description!("[year][month][day][hour][minute][second]");
}

impl From<&FullCommand> for RawFullCommand {
    fn from(command: &FullCommand) -> Self {
        let mut datetime = [0; 14];
        command
            .datetime
            .format_into(
                &mut datetime.as_mut_slice(),
                RawFullCommand::DATETIME_FORMAT,
            )
            .unwrap();

        Self {
            pad_1: [0; 2],
            session_id: command.session_id.to_be_bytes(),
            host: command.host.0,
            unk_1: [0x00, 0x00, 0x00, 0x14],
            unk_2: [0; 20],
            unk_3: [0x00, 0x00, 0x00, 0x10],
            datetime,
            pad_2: [0; 2],
        }
    }
}

impl TryFrom<&RawFullCommand> for FullCommand {
    type Error = FormatError;

    fn try_from(raw_command: &RawFullCommand) -> Result<Self, Self::Error> {
        let mut parser = Parsed::new();
        parser
            .parse_items(&raw_command.datetime, RawFullCommand::DATETIME_FORMAT)
            .map_err(|_| FormatError::InvalidSlice {
                span: span_of!(RawFullCommand, datetime),
                message: "invalid datetime string",
            })?;
        // if `parse_items` succeeds, it is sufficient to construct `PrimitiveDateTime`
        let datetime = parser.try_into().unwrap();
        Ok(Self {
            session_id: u32::from_be_bytes(raw_command.session_id),
            host: Host(raw_command.host),
            datetime,
        })
    }
}

impl Display for FullCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "session_id={} host={} datetime={}",
            self.session_id, self.host, self.datetime
        ))
    }
}

impl From<FullCommand> for Command {
    fn from(value: FullCommand) -> Self {
        Self(InnerCommand::Full(value))
    }
}

#[derive(Debug, Clone)]
struct ResetCommand {
    session_id: u32,
    host: Host,
    action_id: u32,
}

#[derive(Debug, Clone)]
#[repr(C, packed)]
struct RawResetCommand {
    pad_1: [u8; 2],
    session_id: [u8; 4],
    host: [u8; Host::MAX_HOST_LENGTH],
    unk_1: [u8; 4], // 00 00 00 14
    action_id: [u8; 4],
    unk_2: [u8; 20],
}

impl HasRawRepr for ResetCommand {
    type Repr = RawResetCommand;
}

impl From<&ResetCommand> for RawResetCommand {
    fn from(command: &ResetCommand) -> Self {
        Self {
            pad_1: [0; 2],
            session_id: command.session_id.to_be_bytes(),
            host: command.host.0,
            unk_1: [0x00, 0x00, 0x00, 0x14],
            action_id: command.action_id.to_be_bytes(),
            unk_2: [0; 20],
        }
    }
}

impl TryFrom<&RawResetCommand> for ResetCommand {
    type Error = Infallible;

    fn try_from(raw_command: &RawResetCommand) -> Result<Self, Self::Error> {
        Ok(Self {
            session_id: u32::from_be_bytes(raw_command.session_id),
            host: Host(raw_command.host),
            action_id: u32::from_be_bytes(raw_command.action_id),
        })
    }
}

impl Display for ResetCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "session_id={} host={} action_id={}",
            self.session_id, self.host, self.action_id
        ))
    }
}

impl From<ResetCommand> for Command {
    fn from(value: ResetCommand) -> Self {
        Self(InnerCommand::Reset(value))
    }
}

#[derive(Debug, Clone)]
pub struct CommandBuilder {
    poll_type: PollType,
    session_id: Option<u32>,
    host: Option<Host>,
    action_id: Option<u32>,
    datetime: Option<PrimitiveDateTime>,
}

impl CommandBuilder {
    pub fn new(poll_type: PollType) -> Self {
        Self {
            poll_type,
            session_id: None,
            host: None,
            action_id: None,
            datetime: None,
        }
    }

    pub fn poll_type(&mut self, poll_type: PollType) -> &mut Self {
        self.poll_type = poll_type;
        self
    }

    pub fn session_id(&mut self, session_id: u32) -> &mut Self {
        self.session_id = Some(session_id);
        self
    }

    pub fn host(&mut self, host: Host) -> &mut Self {
        self.host = Some(host);
        self
    }

    pub fn action_id(&mut self, action_id: u32) -> &mut Self {
        self.action_id = Some(action_id);
        self
    }

    pub fn datetime(&mut self, datetime: PrimitiveDateTime) -> &mut Self {
        self.datetime = Some(datetime);
        self
    }

    pub fn build(&self) -> Option<Command> {
        use PollType::*;
        Some(match self.poll_type {
            Empty => EmptyCommand.into(),
            HostOnly => HostOnlyCommand { host: self.host? }.into(),
            Full => FullCommand {
                session_id: self.session_id?,
                host: self.host?,
                datetime: self.datetime?,
            }
            .into(),
            Reset => ResetCommand {
                session_id: self.session_id?,
                host: self.host?,
                action_id: self.action_id?,
            }
            .into(),
        })
    }
}
