Scanner Button
==============

A command line utility for my Canon MX922 Multi-Function Printer. This program supports two commands:
- `scan`: scans the network to discover Canon Scanners.
- `listen`: listens to a specific scanner and executes a specified external command when a scan button is pressed. Scanner configuration is passed to the external command via environment variables.

# Why
The big question is why I don't use `scanimage`. It turns out that `scanimage --button-controlled` does not distinguish between color and monochrome (`GRAY` button stops the program). Also from its help, the scan job won't respect the settings on the scanner. In my use case, `scanimage` is used in the downstream script though.

Another big question is why I don't simply interface to `libsane`, which supports much more scanners. Well because I don't want to write C/C++ and I have no plan buying many more scanners in the foreseeable future.

# Usage

## Scan
```
Scans for Canon multi-function printers in the LAN

Usage: scanner-button.exe scan [OPTIONS]

Options:
      --max-waiting <SECS>  Initial max_waiting in seconds for an awaiting response [default: 5]
  -h, --help                Print help information
  -q, --quiet               Disable logging
  -v, --verbose...          Verbosity of messages (use `-v`, `-vv`, `-vvv`... to increase verbosity)
  -V, --version             Print version information
```

## Listen
```
Listens on a scanner for scan button press and execute a command

Usage: scanner-button.exe listen [OPTIONS] --scanner <ADDR> <COMMAND> [ARGS]...

Arguments:
  <COMMAND>
          Command to execute when scan button is pressed

          The configuration reported by the printer is passed to the executed command by environment variables:
            SCANNER_COLOR_MODE = COLOR | MONO
            SCANNER_PAGE       = A4 | LETTER | 10x15 | 13x18 | AUTO
            SCANNER_FORMAT     = JPEG | TIFF | PDF | KOMPAKT_PDF
            SCANNER_DPI        = 75 | 150 | 300 | 600
            SCANNER_SOURCE     = FLATBED | FEEDER
            SCANNER_ADF_TYPE   = SIMPLEX | DUPLEX
            SCANNER_ADF_ORIENT = PORTRAIT | LANDSCAPE

  [ARGS]...
          Arguments to the command if any

Options:
  -s, --scanner <ADDR>
          The address of the scanner

      --hostname <HOSTNAME>
          Name of the host to be displayed on the scanner

          [default: Youmu-Desktop]

      --max-waiting <SECS>
          Initial max_waiting in seconds for an awaiting response

          [default: 5]

      --backoff-factor <FACTOR>
          Exponential factor of backing off for retrying connection

          [default: 2]

      --backoff-maximum <SECS>
          Maximum max_waiting in seconds of backing off for retrying connection

          [default: 1800]

  -h, --help
          Print help information (use `-h` for a summary)

  -q, --quiet
          Disable logging

  -v, --verbose...
          Verbosity of messages (use `-v`, `-vv`, `-vvv`... to increase verbosity)

  -V, --version
          Print version information
```

# Attributions
See [ATTRIBUTION.md](ATTRIBUTION.md).

# Known Bugs
- Stack overflow when running in debug mode on Windows.

# License
Licensed under [GNU General Public License, version 2](https://www.gnu.org/licenses/old-licenses/gpl-2.0.en.html).