//! `wxqr` library — exposes the decode + format modules and the
//! CLI entry point so the integration tests in `tests/` can exercise
//! the formatters without spinning up the OpenCV + WeChatQRCode
//! pipeline. The CLI binary in `src/main.rs` is a thin shim that
//! calls `wxqr::run()`.

pub mod cli;
pub mod decode;
pub mod format;

pub use cli::run;
pub use decode::{DecodeOptions, Decoded};
pub use format::{emit_json, emit_tsv, emit_txt};
