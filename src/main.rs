//! `gta-adf-converter` - command-line front end for the ADF ⇄ MP3 transform.
//!
//! The transform is a single-byte XOR, which is its own inverse, so one
//! operation converts in both directions - the direction is simply whichever
//! file you pass as input:
//!
//!     gta-adf-converter <input.mp3> <output.adf>   # MP3 → ADF
//!     gta-adf-converter <input.adf> <output.mp3>   # ADF → MP3

use std::fs::File;
use std::io::{self, BufReader, BufWriter, IsTerminal, Write};
use std::process::ExitCode;

use gta_adf_converter::{xor_copy, DEFAULT_KEY};

const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Program name for help/usage/errors, taken from the binary target name so it
/// never drifts from `Cargo.toml`.
const PROG: &str = env!("CARGO_BIN_NAME");

const HELP: &str = concat!(
    env!("CARGO_BIN_NAME"), " - convert between MP3 and ADF files\n",
    "\n",
    "An ADF file is an MP3 with every byte XORed by a single key. The transform\n",
    "is symmetric, so the same conversion runs both ways depending on the input.\n",
    "\n",
    "USAGE:\n",
    "    ", env!("CARGO_BIN_NAME"), " <INPUT> <OUTPUT> [OPTIONS]\n",
    "\n",
    "OPTIONS:\n",
    "    -k, --key <N>   XOR key as a 0-255 byte (default: 34)\n",
    "    -q, --quiet     Suppress the progress indicator\n",
    "    -h, --help      Print this help text\n",
    "    -V, --version   Print version information\n",
    "\n",
    "EXAMPLES:\n",
    "    ", env!("CARGO_BIN_NAME"), " song.mp3 song.adf    # MP3 → ADF\n",
    "    ", env!("CARGO_BIN_NAME"), " song.adf song.mp3    # ADF → MP3\n",
);

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{PROG}: {err}");
            ExitCode::FAILURE
        }
    }
}

/// What the user asked us to do, fully parsed and validated.
struct Job {
    input: String,
    output: String,
    key: u8,
    quiet: bool,
}

fn run() -> Result<(), String> {
    let job = match parse_args()? {
        Parsed::Job(job) => job,
        Parsed::Exit => return Ok(()), // --help / --version already printed
    };

    if job.input == job.output {
        return Err(format!(
            "input and output are the same file ({}); refusing to overwrite",
            job.input
        ));
    }

    let in_file =
        File::open(&job.input).map_err(|e| format!("cannot open {} for reading: {e}", job.input))?;
    let total_len = in_file.metadata().map(|m| m.len()).unwrap_or(0);

    let out_file = File::create(&job.output)
        .map_err(|e| format!("cannot open {} for writing: {e}", job.output))?;

    let reader = BufReader::new(in_file);
    let writer = BufWriter::new(out_file);

    let mut progress = Progress::new(total_len, job.quiet);
    let total = xor_copy(reader, writer, job.key, |done| progress.update(done))
        .map_err(|e| format!("{}: {e}", job.input))?;
    progress.finish();

    if !job.quiet {
        let bytes = if total == 1 { "byte" } else { "bytes" };
        if total < 1024 {
            eprintln!("Processed {total} {bytes}");
        } else {
            eprintln!("Processed {total} {bytes} ({})", human_bytes(total));
        }
    }

    Ok(())
}

/// Render a byte count as a compact binary size, e.g. `1.5 MiB`. Only used for
/// the summary line once the count is known to be at least 1 KiB.
fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    format!("{size:.1} {}", UNITS[unit])
}

enum Parsed {
    Job(Job),
    Exit,
}

fn parse_args() -> Result<Parsed, String> {
    let mut key = DEFAULT_KEY;
    let mut quiet = false;
    let mut positionals: Vec<String> = Vec::new();

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{HELP}");
                return Ok(Parsed::Exit);
            }
            "-V" | "--version" => {
                println!("{PROG} {VERSION}");
                return Ok(Parsed::Exit);
            }
            "-q" | "--quiet" => quiet = true,
            "-k" | "--key" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "--key requires a value (0-255)".to_string())?;
                key = parse_key(&raw)?;
            }
            other if other.starts_with('-') && other.len() > 1 => {
                return Err(format!("unknown option: {other}\n\n{HELP}"));
            }
            other => positionals.push(other.to_string()),
        }
    }

    let [input, output] = match positionals.as_slice() {
        [a, b] => [a.clone(), b.clone()],
        [] | [_] => return Err(format!("expected <INPUT> and <OUTPUT> paths\n\n{HELP}")),
        _ => return Err("too many file arguments".to_string()),
    };

    Ok(Parsed::Job(Job {
        input,
        output,
        key,
        quiet,
    }))
}

fn parse_key(raw: &str) -> Result<u8, String> {
    // Accept decimal (34), hex (0x22) or octal (0o42) byte literals.
    let parsed = if let Some(hex) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
        u8::from_str_radix(hex, 16)
    } else if let Some(oct) = raw.strip_prefix("0o").or_else(|| raw.strip_prefix("0O")) {
        u8::from_str_radix(oct, 8)
    } else {
        raw.parse::<u8>()
    };
    parsed.map_err(|_| format!("invalid key {raw:?}: expected a byte in 0-255"))
}

struct Progress {
    total: u64,
    enabled: bool,
    last_pct: u8,
}

impl Progress {
    fn new(total: u64, quiet: bool) -> Self {
        let enabled = !quiet && total > 0 && io::stderr().is_terminal();
        Progress {
            total,
            enabled,
            last_pct: u8::MAX, // force the first draw
        }
    }

    fn update(&mut self, done: u64) {
        if !self.enabled {
            return;
        }
        let pct = (done.saturating_mul(100) / self.total) as u8;
        if pct != self.last_pct {
            self.last_pct = pct;
            let mut err = io::stderr().lock();
            let _ = write!(err, "\rConverting... {pct}%");
            let _ = err.flush();
        }
    }

    fn finish(&self) {
        if self.enabled {
            let mut err = io::stderr().lock();
            let _ = writeln!(err, "\r\x1b[KDone");
        }
    }
}
