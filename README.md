# gta-adf-converter

A fast, dependency-free converter between `.adf` and `.mp3` files.

`.adf` is the audio format used by **Grand Theft Auto Vice City** - the radio
stations in the game's `Audio` directory are stored as `.adf` files. Each one is
an ordinary MP3 whose every byte has been XORed with a single key (`0x22` / `34`).
Because XOR is its own inverse, conversion is symmetric: the *same* operation
goes both ways, and the direction is simply whichever file you pass as input.

This lets you turn the game's radio stations into plain MP3, or turn your own
MP3s into `.adf` files the game can play - with one single command.

## Performance

The transform streams through a 64 KiB buffer and XORs each chunk in place - a
loop the compiler auto-vectorises - over buffered I/O, so throughput is bounded
by disk speed rather than per-byte overhead.

## Build

```sh
cargo build --release
# binary: target/release/gta-adf-converter
```

## Usage

```sh
gta-adf-converter song.mp3 song.adf    # MP3 → ADF
gta-adf-converter song.adf song.mp3    # ADF → MP3
```

Options:

| Flag | Meaning |
| --- | --- |
| `-k, --key <N>` | XOR key as a byte; accepts decimal (`34`), hex (`0x22`), or octal (`0o42`); default `34` |
| `-q, --quiet` | Suppress the progress indicator |
| `-h, --help` | Show help |
| `-V, --version` | Show version |

The progress indicator is only drawn when stderr is an interactive terminal, so
piping output stays clean.

## Test

```sh
cargo test
```

## License

MIT - see [LICENSE](LICENSE).
