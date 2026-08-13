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
        --points         Include corner points in JSON / TSV output
    -0, --null           Treat input as NUL-separated path list
        --files-from <P> Read paths from a file ('-' for stdin); one per line
        -q, --quiet       Suppress per-file stderr error logs
    -h, --help           Show this help.
    -V, --version        Show version.

EXAMPLES:
    wxqr qr.png                              # one image
    wxqr --format json img1.png img2.png     # batch
    wxqr --points blurry.jpg                 # include corner coords
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
    points: bool,
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
    // directly an image path. Subcommands are exactly the ones that
    // begin with letters (no leading `-` or `/`); image paths almost
    // always start with `./`, `/`, or contain a `.png`/`.jpg`/`.jpeg`.
    // We use the simplest heuristic: if args[1] is a known word
    // ("dec", "decode") treat it as a subcommand; otherwise treat
    // the whole argv tail as direct image args.
    match args[1].as_str() {
        "-h" | "--help" => Ok(Subcmd::Help),
        "-V" | "--version" => Ok(Subcmd::Version),
        "--info" => Ok(Subcmd::Info),
        "enc" | "encode" => Err(anyhow!(
            "wxqr has no 'enc' subcommand — WeChatQRCode is decode-only by design. \
             Use ljh-sh/zxing for encode, or any qrcode / qrencode tool."
        )),
        "dec" | "decode" => parse_dec(&args[2..]).map(Subcmd::Dec),
        _ => parse_dec(&args[1..]).map(Subcmd::Direct),
    }
}

fn parse_dec(argv: &[String]) -> Result<DecArgs> {
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
            "-h" | "--help" => {
                // We can't return Subcmd::Help from parse_dec (wrong
                // return type). The caller handles --help/--version
                // before invoking parse_dec.
                return Err(anyhow!("internal: --help should be handled by parse_args"));
            }
            "-V" | "--version" => {
                return Err(anyhow!(
                    "internal: --version should be handled by parse_args"
                ));
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