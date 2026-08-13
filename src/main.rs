//! `wxqr` — local QR decoder CLI powered by WeChatCV WeChatQRCode CNN.
//!
//! Hand-rolled CLI parser (no clap) to keep the dependency tree minimal.
//! The argument form is:
//!
//! ```text
//! wxqr dec [OPTIONS] <IMAGE>...
//! wxqr --help | --version | --info
//! ```
//!
//! There is **no `enc`** subcommand: WeChatQRCode is decode-only by
//! design. See README for the design rationale.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{anyhow, Context, Result};

mod decode;
mod format;

use decode::{DecodeOptions, Decoded};
use format::{emit_json, emit_tsv, emit_txt};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const PKG_DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

const HELP: &str = "\
wxqr — local QR decoder (WeChatCV CNN)

USAGE:
    wxqr dec [OPTIONS] <IMAGE>...
    wxqr --help | --version | --info

Reads each IMAGE, runs the WeChatCV WeChatQRCode CNN detector + decoder,
and writes one record per detection to stdout. Exit code is 0 if at
least one image produced at least one result, 1 if all images were
scanned but no QR codes were found, 2 on read / decode failure, 64 on
usage error.

This tool only decodes QR codes. It is designed as the fallback when
generic detectors (e.g. zxing) miss damaged / blurred / small /
reflective images. There is no `enc` subcommand — WeChatQRCode is a
decode-only model.

OPTIONS:
    -f, --format <FMT>   Output format: txt (default) | json | tsv
        --no-scale-up    Disable super-resolution pass (faster on clean images)
        --points         Include corner points in JSON / TSV output
    -0, --null           Treat input as NUL-separated path list
        --files-from <P> Read paths from a file ('-' for stdin); one per line
        -q, --quiet       Suppress per-file stderr error logs
    -h, --help           Show this help.
    -V, --version        Show version.

EXAMPLES:
    wxqr dec qr.png                              # one image
    wxqr dec --format json img1.png img2.png     # batch
    wxqr dec --points blurry.jpg                 # include corner coords
    find . -name '*.png' -print0 | \\
        xargs -0 wxqr dec --null --files-from -
";

#[derive(Debug)]
enum Subcmd {
    Dec(DecArgs),
    Help,
    Version,
    Info,
}

#[derive(Debug)]
struct DecArgs {
    format: FormatKind,
    scale_up: bool,
    points: bool,
    null_sep: bool,
    files_from: Option<String>,
    quiet: bool,
    images: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FormatKind {
    Txt,
    Json,
    Tsv,
}

impl FormatKind {
    fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "txt" | "text" | "yml" | "yaml" => Some(Self::Txt),
            "json" => Some(Self::Json),
            "tsv" => Some(Self::Tsv),
            _ => None,
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match parse_args(&args) {
        Ok(Subcmd::Help) => {
            println!("{HELP}");
            ExitCode::SUCCESS
        }
        Ok(Subcmd::Version) => {
            println!("wxqr {VERSION}");
            ExitCode::SUCCESS
        }
        Ok(Subcmd::Info) => {
            println!("wxqr {VERSION}");
            println!("{PKG_DESCRIPTION}");
            ExitCode::SUCCESS
        }
        Ok(Subcmd::Dec(args)) => run_dec(args),
        Err(e) => {
            eprintln!("wxqr: {e}");
            eprintln!();
            eprintln!("Try 'wxqr --help' for usage.");
            ExitCode::from(64)
        }
    }
}

fn parse_args(args: &[String]) -> Result<Subcmd> {
    if args.len() < 2 {
        return Ok(Subcmd::Help);
    }
    match args[1].as_str() {
        "-h" | "--help" => Ok(Subcmd::Help),
        "-V" | "--version" => Ok(Subcmd::Version),
        "--info" => Ok(Subcmd::Info),
        "dec" | "decode" => parse_dec(&args[2..]),
        "enc" | "encode" => Err(anyhow!(
            "wxqr has no 'enc' subcommand — WeChatQRCode is decode-only by design. \
             Use ljh-sh/zxing for encode, or any qrcode / qrencode tool."
        )),
        other => Err(anyhow!("unknown subcommand '{other}'")),
    }
}

fn parse_dec(argv: &[String]) -> Result<Subcmd> {
    let mut args = DecArgs {
        format: FormatKind::Txt,
        scale_up: true,
        points: false,
        null_sep: false,
        files_from: None,
        quiet: false,
        images: Vec::new(),
    };

    let mut i = 0;
    while i < argv.len() {
        let a = &argv[i];
        match a.as_str() {
            "-h" | "--help" => return Ok(Subcmd::Help),
            "-V" | "--version" => return Ok(Subcmd::Version),
            "-f" | "--format" => {
                let v = argv
                    .get(i + 1)
                    .ok_or_else(|| anyhow::anyhow!("--format requires a value"))?;
                args.format = FormatKind::parse(v).ok_or_else(|| {
                    anyhow::anyhow!(
                        "unknown --format '{v}'; expected one of txt|json|tsv|yml"
                    )
                })?;
                i += 2;
            }
            "--no-scale-up" | "--no_scale_up" => {
                args.scale_up = false;
                i += 1;
            }
            "--points" => {
                args.points = true;
                i += 1;
            }
            "-0" | "--null" => {
                args.null_sep = true;
                i += 1;
            }
            "--files-from" => {
                let v = argv
                    .get(i + 1)
                    .ok_or_else(|| anyhow::anyhow!("--files-from requires a value"))?;
                args.files_from = Some(v.clone());
                i += 2;
            }
            "-q" | "--quiet" => {
                args.quiet = true;
                i += 1;
            }
            "--" => {
                args.images.extend(argv[i + 1..].iter().map(PathBuf::from));
                i = argv.len();
            }
            s if s.starts_with('-') => {
                return Err(anyhow::anyhow!("unknown flag '{s}'"));
            }
            _ => {
                args.images.push(PathBuf::from(a));
                i += 1;
            }
        }
    }

    if let Some(src) = args.files_from.take() {
        let content = read_files_from(&src)
            .with_context(|| format!("reading --files-from '{src}'"))?;
        for line in content.split(if args.null_sep { '\0' } else { '\n' }) {
            if line.is_empty() {
                continue;
            }
            args.images.push(PathBuf::from(line));
        }
    }

    if args.images.is_empty() {
        return Err(anyhow::anyhow!(
            "no input images; provide at least one <IMAGE> or use --files-from"
        ));
    }

    Ok(Subcmd::Dec(args))
}

fn read_files_from(src: &str) -> Result<String> {
    if src == "-" {
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s)?;
        return Ok(s);
    }
    Ok(std::fs::read_to_string(src)?)
}

fn run_dec(args: DecArgs) -> ExitCode {
    let opts = DecodeOptions {
        scale_up: args.scale_up,
    };
    let mut stdout = std::io::stdout().lock();
    let mut had_any = false;

    // Initialize the WeChatQRCode detector lazily (one global instance
    // is safe — it's stateless and reentrant for concurrent reads).
    let detector_init = decode::init_detector();
    if let Err(e) = detector_init {
        if !args.quiet {
            eprintln!("wxqr: failed to initialize WeChatQRCode detector: {e}");
            eprintln!("wxqr: hint: ensure the bundled models/ directory exists");
        }
        return ExitCode::from(2);
    }

    for path in &args.images {
        let decoded = decode::decode_path(path, &opts, args.quiet);
        match decoded {
            Ok(results) => {
                if results.is_empty() {
                    continue;
                }
                had_any = true;
                emit(&mut stdout, args.format, path, &results, args.points);
            }
            Err(e) => {
                if !args.quiet {
                    eprintln!("wxqr: {}: {e}", path.display());
                }
                return ExitCode::from(2);
            }
        }
    }

    if had_any {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn emit<W: Write>(
    w: &mut W,
    fmt: FormatKind,
    path: &Path,
    results: &[Decoded],
    with_points: bool,
) {
    match fmt {
        FormatKind::Txt => emit_txt(w, path, results),
        FormatKind::Json => emit_json(w, path, results, with_points),
        FormatKind::Tsv => emit_tsv(w, path, results, with_points),
    }
    let _ = w.flush();
}