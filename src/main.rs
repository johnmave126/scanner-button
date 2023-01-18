mod channel;
mod poll;
mod scan;
mod utils;

use std::{
    cmp,
    ffi::OsString,
    io,
    net::{SocketAddr, ToSocketAddrs},
};

use bjnp::Host;
use clap::{Args, Parser, Subcommand};
use gethostname::gethostname;

#[derive(Parser)]
#[command(author, version)]
#[command(propagate_version = true)]
/// A utility program for Canon multi-function printer, used for detecting
/// presence of printer(s) or listening for scan button press(es)
struct Cli {
    /// Initial max_waiting in seconds for an awaiting response
    #[arg(
        global = true,
        long,
        value_name = "SECS",
        default_value_t = 5,
        value_parser = clap::value_parser!(u64).range(1..),
        display_order = 3
    )]
    max_waiting: u64,

    /// Verbosity of messages (use `-v`, `-vv`, `-vvv`... to increase verbosity)
    #[arg(
        global = true,
        short, long,
        action = clap::ArgAction::Count,
        display_order = 999
    )]
    verbose: u8,

    /// Disable logging
    #[arg(global = true, short, long, display_order = 999)]
    quiet: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Listens on a scanner for scan button press and execute a command
    Listen(Listen),
    /// Scans for Canon multi-function printers in the LAN
    Scan,
}

static COMMAND_LONG_HELP: &str = "\
Command to execute when scan button is pressed

The configuration reported by the printer is passed to the executed command by environment variables:
  SCANNER_COLOR_MODE = COLOR | MONO
  SCANNER_PAGE       = A4 | LETTER | 10x15 | 13x18 | AUTO
  SCANNER_FORMAT     = JPEG | TIFF | PDF | KOMPAKT_PDF
  SCANNER_DPI        = 75 | 150 | 300 | 600
  SCANNER_SOURCE     = FLATBED | FEEDER
  SCANNER_ADF_TYPE   = SIMPLEX | DUPLEX
  SCANNER_ADF_ORIENT = PORTRAIT | LANDSCAPE\
";
#[derive(Args)]
struct Listen {
    /// The address of the scanner
    #[arg(
        short,
        long,
        value_name = "ADDR",
        value_parser = parse_addr,
        display_order = 1
    )]
    scanner: SocketAddr,

    /// Name of the host to be displayed on the scanner
    #[arg(long, default_value_os_t = gethostname(), display_order = 2)]
    hostname: OsString,

    /// Exponential factor of backing off for retrying connection
    #[arg(
        long,
        value_name = "FACTOR",
        default_value_t = 2.0,
        value_parser = parse_factor,
        display_order = 4
    )]
    backoff_factor: f32,

    /// Maximum max_waiting in seconds of backing off for retrying connection
    #[arg(
        long,
        value_name = "SECS",
        default_value_t = 1800,
        value_parser = clap::value_parser!(u64).range(1..),
        display_order = 5
    )]
    backoff_maximum: u64,

    /// Command to execute when scan button is pressed
    #[arg(long_help = COMMAND_LONG_HELP)]
    command: OsString,

    /// Arguments to the command if any
    args: Vec<OsString>,
}

fn parse_addr(s: &str) -> Result<SocketAddr, io::Error> {
    let mut addrs = s.to_socket_addrs()?;
    // NOPANIC: if the former call succeeds, there is at least one address
    // (actually exactly one according to the current implementation)
    Ok(addrs
        .next()
        .expect("successful `to_socket_addrs()` call should produces exactly one address"))
}

fn parse_factor(s: &str) -> Result<f32, String> {
    let factor: f32 = s.parse().map_err(|_| format!("`{s}` is not a number"))?;
    if factor.is_finite() && matches!(factor.partial_cmp(&1.0f32), Some(cmp::Ordering::Greater)) {
        Ok(factor)
    } else {
        Err(format!("`{s}` is not in range (1.0..+inf)"))
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    stderrlog::new()
        .modules([module_path!(), "bjnp"])
        .quiet(cli.quiet)
        .verbosity(cli.verbose as usize + 1)
        .init()
        .unwrap();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .thread_name("main")
        .thread_stack_size(8 * 1024 * 1024)
        .build()
        .unwrap();

    match cli.command {
        Commands::Listen(args) => {
            let config = poll::ListenConfig {
                scanner_addr: args.scanner,
                hostname: Host::new(args.hostname.to_string_lossy()),
                initial_max_waiting: cli.max_waiting,
                backoff_factor: args.backoff_factor,
                backoff_maximum: args.backoff_maximum,
                command: (args.command, args.args),
            };
            rt.block_on(poll::listen(config))
        }
        Commands::Scan => rt.block_on(scan::scan(cli.max_waiting)),
    }
}
