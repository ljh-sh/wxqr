//! CLI argument parsing + dispatch for `wxqr`.
//!
//! The repo path (`ljh-sh/wxqr`) already implies the subcommand is
//! decode — there is no `enc` ever (WeChatQRCode is decode-only).
//! So `wxqr <image>` works directly; `dec` is accepted as a deprecated
//! alias for back-compat with the early design drafts.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{anyhow, Context, Result};

use crate::decode::{Decoded, DecodeOptions};
use crate::format::{emit_json, emit_tsv, emit_txt};

pub const HELP: &str = "\
wxqr — local QR decoder (WeChatCV CNN)

USAGE:
    wxqr [OPTIONS] <IMAGE>...
    wxqr [decode] [OPTIONS] <IMAGE>...
    wxqr --help | --version | --info

Reads each IMAGE, runs the WeChatCV WeChatQRCode CNN detector + decoder,
and writes one record per detection to stdout. Exit code is 0 if at
least one image produced at least one result, 1 if all images were
scanned but no QR codes were found, 2 on read / decode failure, 64 on
usage error.

There is no `enc` subcommand — WeChatQRCode is a decode-only model.

OPTIONS:
    -f, --format <FMT>   Output format: txt (default) | json | tsv
        --no-scale-up    Disable super-resolution pass (faster on clean images)
    -0, --null           Treat input as NUL-separated path list
        --files-from <P> Read paths from a file ('-' for stdin); one per line
        -q, --quiet       Suppress per-file stderr error logs
    -h, --help           Show this help.
    -V, --version        Show version.

EXAMPLES:
    wxqr qr.png                              # one image
    wxqr --format json img1.png img2.png     # batch
    find . -name '*.png' -print0 | \\
        xargs -0 wxqr --null --files-from -
";

#[derive(Debug)]
enum Subcmd {
    /// Direct call: `wxqr <image>...` (the canonical form).
    Direct(DecArgs),
    /// Deprecated `dec` subcommand alias (still accepted for back-compat).
    Dec(DecArgs),
    Help,
    Version,
    Info,
}

#[derive(Debug)]
struct DecArgs {
    format: FormatKind,
    scale_up: bool,
    null_sep: bool,
    files_from: Option<String>,
    quiet: bool,
    images: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatKind {
    Txt,
    Json,
    Tsv,
}

impl FormatKind {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "txt" | "text" | "yml" | "yaml" => Some(Self::Txt),
            "json" => Some(Self::Json),
            "tsv" => Some(Self::Tsv),
            _ => None,
        }
    }
}

pub fn run() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match parse_args(&args) {
        Ok(Subcmd::Help) => {
            println!("{HELP}");
            ExitCode::SUCCESS
        }
        Ok(Subcmd::Version) => {
            println!("wxqr {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Ok(Subcmd::Info) => {
            println!("wxqr {}", env!("CARGO_PKG_VERSION"));
            println!("{}", env!("CARGO_PKG_DESCRIPTION"));
            ExitCode::SUCCESS
        }
        Ok(Subcmd::Direct(args)) | Ok(Subcmd::Dec(args)) => run_dec(args),
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
    // The first non-flag argument is either a subcommand (`dec`) or
    // directly an image path. Subcommands are exactly the words that
    // begin with letters and are not paths; we use the simplest
    // heuristic: if args[1] is a known word ("dec", "decode") treat
    // it as a subcommand; otherwise treat the whole argv tail as
    // direct image args.
    match args[1].as_str() {
        "-h" | "--help" => Ok(Subcmd::Help),
        "-V" | "--version" => Ok(Subcmd::Version),
        "--info" => Ok(Subcmd::Info),
        "dec" | "decode" => parse_dec(&args[2..]).map(Subcmd::Dec),
        _ => parse_dec(&args[1..]).map(Subcmd::Direct),
    }
}

fn parse_dec(argv: &[String]) -> Result<DecArgs> {
    let mut args = DecArgs {
        format: FormatKind::Txt,
        scale_up: true,
        null_sep: false,
        files_from: None,
        quiet: false,
        images: Vec::new(),
    };

    let mut i = 0;
    while i < argv.len() {
        let a = &argv[i];
        match a.as_str() {
            "-h" | "--help" => {
                println!("{HELP}");
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("wxqr {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "-f" | "--format" => {
                let v = argv
                    .get(i + 1)
                    .ok_or_else(|| anyhow!("--format requires a value"))?;
                args.format = FormatKind::parse(v).ok_or_else(|| {
                    anyhow!("unknown --format '{v}'; expected one of txt|json|tsv|yml")
                })?;
                i += 2;
            }
            "--no-scale-up" | "--no_scale_up" => {
                args.scale_up = false;
                i += 1;
            }
            "-0" | "--null" => {
                args.null_sep = true;
                i += 1;
            }
            "--files-from" => {
                let v = argv
                    .get(i + 1)
                    .ok_or_else(|| anyhow!("--files-from requires a value"))?;
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
                return Err(anyhow!("unknown flag '{s}'"));
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
        return Err(anyhow!(
            "no input images; provide at least one <IMAGE> or use --files-from"
        ));
    }

    Ok(args)
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

    if let Err(e) = crate::decode::init_detector() {
        if !args.quiet {
            eprintln!("wxqr: failed to initialize WeChatQRCode detector: {e}");
            eprintln!("wxqr: hint: ensure the bundled models/ directory exists");
        }
        return ExitCode::from(2);
    }

    for path in &args.images {
        let decoded = crate::decode::decode_path(path, &opts, args.quiet);
        match decoded {
            Ok(results) => {
                if results.is_empty() {
                    continue;
                }
                had_any = true;
                // `points` is always emitted in the output. WeChatQRCode
                // does not expose corner points in its high-level wrapper,
                // so the array is always empty here — but the schema stays
                // byte-compatible with zxing's output regardless.
                emit(&mut stdout, args.format, path, &results);
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

fn emit<W: Write>(w: &mut W, fmt: FormatKind, path: &Path, results: &[Decoded]) {
    match fmt {
        FormatKind::Txt => emit_txt(w, path, results),
        FormatKind::Json => emit_json(w, path, results),
        FormatKind::Tsv => emit_tsv(w, path, results),
    }
    let _ = w.flush();
}