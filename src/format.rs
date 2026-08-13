//! Output formatters: txt, json, tsv.
//!
//! Output schema is byte-compatible with `ljh-sh/zxing`'s JSON output
//! (`{"format": "QR_CODE", "text": "..."}`) so that an outer
//! dispatcher (`x qr dec`) can fan out to either backend without
//! per-result special-casing.

use std::io::Write;
use std::path::Path;

use crate::decode::Decoded;

/// `txt` — one line per detection:
///   `<file>\tQR_CODE\t<text>`
pub fn emit_txt<W: Write>(w: &mut W, file: &Path, results: &[Decoded]) {
    let f = file.display().to_string();
    for r in results {
        if let Err(e) = writeln!(w, "{f}\tQR_CODE\t{}", r.text) {
            if e.kind() != std::io::ErrorKind::BrokenPipe {
                panic!("stdout write failed: {e}");
            }
        }
    }
}

/// `json` — one entry per file:
/// ```json
/// [{"file": "qr.png", "results": [{"format": "QR_CODE", "text": "https://x-cmd.com"}]}]
/// ```
///
/// With `--points`, each result gains an empty `"points": []` field
/// (WeChatQRCode does not expose corner coordinates; the array is
/// present for schema compatibility with zxing).
pub fn emit_json<W: Write>(w: &mut W, file: &Path, results: &[Decoded], with_points: bool) {
    let f = file.display().to_string();
    let _ = write!(w, "[");
    write_str_obj(w, "file", &f);
    let _ = write!(w, ", \"results\": [");
    for (i, r) in results.iter().enumerate() {
        if i > 0 {
            let _ = write!(w, ", ");
        }
        let _ = write!(w, "{{");
        write_str_obj(w, "format", "QR_CODE");
        let _ = write!(w, ", ");
        write_str_obj(w, "text", &r.text);
        if with_points {
            // WeChatQRCode doesn't expose points; emit empty array for
            // schema compat. Future versions of OpenCV may expose them.
            let _ = write!(w, ", \"points\": []");
        }
        let _ = write!(w, "}}");
    }
    let _ = writeln!(w, "]}}]");
}

/// `tsv` — three columns: `<file>\tQR_CODE\t<text>`.
pub fn emit_tsv<W: Write>(w: &mut W, file: &Path, results: &[Decoded], _with_points: bool) {
    let f = file.display().to_string();
    for r in results {
        if let Err(e) = writeln!(w, "{f}\tQR_CODE\t{}", tsv_escape(&r.text)) {
            if e.kind() != std::io::ErrorKind::BrokenPipe {
                panic!("stdout write failed: {e}");
            }
        }
    }
}

fn tsv_escape(s: &str) -> String {
    s.replace('\t', "\\t")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn write_str_obj<W: Write>(w: &mut W, key: &str, val: &str) {
    let _ = write!(w, "\"{key}\": \"{}\"", json_escape(val));
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}
