use std::{cmp, ffi::OsString, net::SocketAddr, process::Command};

use anyhow::{anyhow, Context};
use bjnp::{
    discover,
    poll::{self, Interrupt},
    serdes::Empty,
    Host, PayloadType,
};
use log::{debug, info, trace, warn};
use time::{OffsetDateTime, PrimitiveDateTime};
use tokio::time::{sleep, timeout, Duration};

use crate::{channel::Channel, utils::ignore_err};

#[derive(Debug)]
enum State {
    Init,
    Poll,
    Backoff(Duration),
}

#[derive(Debug)]
pub struct ListenConfig {
    pub scanner_addr: SocketAddr,
    pub hostname: Host,
    pub initial_max_waiting: u64,
    pub backoff_factor: f32,
    pub backoff_maximum: u64,
    pub command: (OsString, Vec<OsString>),
}

struct Listener {
    channel: Channel,
    state: State,
    session_id: u32,
    config: ListenConfig,
}

impl Listener {
    async fn new(config: ListenConfig) -> anyhow::Result<Self> {
        let channel = Channel::new(config.scanner_addr).await?;

        Ok(Self {
            channel,
            state: State::Init,
            session_id: 0,
            config,
        })
    }

    async fn next(&mut self) -> anyhow::Result<State> {
        match &self.state {
            State::Init => {
                trace!("initialize listener");

                self.try_init(Duration::from_secs(self.config.initial_max_waiting))
                    .await?;

                Ok(State::Poll)
            }
            State::Poll => {
                trace!("polling listener");

                let max_waiting = Duration::from_secs(self.config.initial_max_waiting);
                let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
                let now = PrimitiveDateTime::new(now.date(), now.time());

                let command = poll::CommandBuilder::new(poll::PollType::Full)
                    .host(self.config.hostname)
                    .session_id(self.session_id)
                    .datetime(now)
                    .build()
                    .unwrap();
                timeout(max_waiting, self.channel.send(PayloadType::Poll, command))
                    .await?
                    .context("timeout when sending poll command")?;
                let resp: poll::Response = timeout(max_waiting, self.channel.recv())
                    .await?
                    .context("timeout awaiting poll response")?;

                if let Some(session_id) = resp.session_id() {
                    self.session_id = session_id;
                }

                if resp.status() == 0x8000 {
                    if let Some(interrupt) = resp.interrupt() {
                        info!("received scanner job: {interrupt}");
                        ignore_err(self.launch(interrupt));
                    }

                    // cancel job
                    let command = poll::CommandBuilder::new(poll::PollType::Reset)
                        .host(self.config.hostname)
                        .session_id(self.session_id)
                        .action_id(resp.action_id().unwrap_or(0))
                        .build()
                        .unwrap();

                    timeout(max_waiting, self.channel.send(PayloadType::Poll, command))
                        .await?
                        .context("timeout when sending poll command")?;

                    let _: poll::Response = timeout(max_waiting, self.channel.recv())
                        .await?
                        .context("timeout awaiting poll response")?;
                }

                // 1 seconds between polling
                sleep(Duration::from_secs(1)).await;
                Ok(State::Poll)
            }
            State::Backoff(dur) => {
                trace!("backing off listener");

                // try again
                self.try_init(*dur).await?;

                Ok(State::Poll)
            }
        }
    }

    async fn try_init(&mut self, max_waiting: Duration) -> anyhow::Result<()> {
        self.channel.reset_sequence();

        // Detect scanner online
        timeout(max_waiting, self.channel.send(PayloadType::Discover, Empty))
            .await?
            .context("timeout when sending discover command")?;
        let _: discover::Response = timeout(max_waiting, self.channel.recv())
            .await?
            .context("timeout awaiting disover response")?;

        // Send initial poll
        let command = poll::CommandBuilder::new(poll::PollType::HostOnly)
            .host(self.config.hostname)
            .build()
            .unwrap();
        timeout(max_waiting, self.channel.send(PayloadType::Poll, command))
            .await?
            .context("timeout when sending poll command")?;
        let resp: poll::Response = timeout(max_waiting, self.channel.recv())
            .await?
            .context("timeout awaiting poll response")?;

        self.session_id = resp
            .session_id()
            .ok_or_else(|| anyhow!("unexpected interrupt during first poll"))?;

        Ok(())
    }

    fn transit_err(&mut self) {
        match &self.state {
            State::Init => {
                trace!("transit to Backoff");
                self.state = State::Backoff(Duration::from_secs(self.config.initial_max_waiting));
            }
            State::Poll => {
                trace!("transit to Init");
                self.state = State::Init;
            }
            State::Backoff(dur) => {
                trace!("transit to longer Backoff");

                let new_dur = cmp::min(
                    self.config.backoff_maximum,
                    (dur.as_secs() as f32 * self.config.backoff_factor) as u64,
                );
                self.state = State::Backoff(Duration::from_secs(new_dur));
            }
        }
    }

    fn launch(&self, interrupt: &Interrupt) -> anyhow::Result<()> {
        trace!("launch external program");

        let color_mode = match interrupt.color_mode() {
            poll::ColorMode::Color => "COLOR",
            poll::ColorMode::Mono => "MONO",
        };
        let size = match interrupt.size() {
            poll::Size::A4 => "A4",
            poll::Size::Letter => "LETTER",
            poll::Size::_10x15 => "10x15",
            poll::Size::_13x18 => "13x18",
            poll::Size::Auto => "AUTO",
        };
        let format = match interrupt.format() {
            poll::Format::Jpeg => "JPEG",
            poll::Format::Tiff => "TIFF",
            poll::Format::Pdf => "PDF",
            poll::Format::KompaktPdf => "KOMPAKT_PDF",
        };
        let dpi = match interrupt.dpi() {
            poll::DPI::_75 => "75",
            poll::DPI::_150 => "150",
            poll::DPI::_300 => "300",
            poll::DPI::_600 => "600",
        };
        let source = match interrupt.source() {
            poll::Source::Flatbed => "FLATBED",
            poll::Source::AutoDocumentFeeder => "FEEDER",
        };
        let feeder_type = match interrupt.feeder_type() {
            Some(poll::FeederType::Simplex) => "SIMPLEX",
            Some(poll::FeederType::Duplex) => "DUPLEX",
            None => "",
        };
        let feeder_orientation = match interrupt.feeder_orientation() {
            Some(poll::FeederOrientation::Portrait) => "PORTRAIT",
            Some(poll::FeederOrientation::Landscape) => "LANDSCAPE",
            None => "",
        };

        let (cmd, args) = &self.config.command;

        Command::new(cmd)
            .args(args)
            .env("SCANNER_COLOR_MODE", color_mode)
            .env("SCANNER_PAGE", size)
            .env("SCANNER_FORMAT", format)
            .env("SCANNER_DPI", dpi)
            .env("SCANNER_SOURCE", source)
            .env("SCANNER_ADF_TYPE", feeder_type)
            .env("SCANNER_ADF_ORIENT", feeder_orientation)
            .spawn()
            .with_context(|| format!("failed to launch executable `{}`", cmd.to_string_lossy()))?;

        Ok(())
    }
}

pub async fn listen(config: ListenConfig) -> anyhow::Result<()> {
    debug!("loaded listening config {config:?}");

    let mut listener = Listener::new(config).await?;

    loop {
        match listener.next().await {
            Ok(new_state) => listener.state = new_state,
            Err(e) => {
                warn!("{e}");
                listener.transit_err();
            }
        }
    }
}
