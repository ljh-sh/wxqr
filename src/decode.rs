//! Decode logic using the WeChatCV WeChatQRCode detector + super-resolution
//! model bundled via `include_bytes!`.
//!
//! The detector is initialized once via `OnceLock` because loading the
//! four model files + spinning up the OpenCV DNN graph is ~500 ms of
//! fixed cost that we don't want to pay per image.

use std::io::Write;
use std::path::Path;
use std::sync::OnceLock;

use anyhow::{anyhow, Context, Result};
use opencv::core::Mat;
use opencv::prelude::*;
use opencv::wechat_qrcode::WeChatQRCode;

/// The four model files are embedded at compile time. This works around
/// the OpenCV 4.x constraint that `WeChatQRCode::new` requires file paths,
/// not in-memory blobs. (OpenCV 5.x will lift this — we upgrade then.)
const DETECT_PROTOTXT: &[u8] =
    include_bytes!("../models/detect.prototxt");
const DETECT_MODEL: &[u8] =
    include_bytes!("../models/detect.caffemodel");
const SR_PROTOTXT: &[u8] =
    include_bytes!("../models/sr.prototxt");
const SR_MODEL: &[u8] =
    include_bytes!("../models/sr.caffemodel");

/// License text bundled next to the models so attribution stays intact
/// even after `include_bytes!` pulls them in.
const MODEL_LICENSE: &[u8] = include_bytes!("../models/LICENSE");

#[derive(Debug, Clone, Copy)]
pub struct DecodeOptions {
    /// When true (default), the super-resolution pre-pass is enabled.
    /// Disable via `--no-scale-up` for clean images where it only adds
    /// latency.
    pub scale_up: bool,
}

#[derive(Debug, Clone)]
pub struct Decoded {
    pub text: String,
    pub points: Vec<(f32, f32)>,
}

/// Lazily-initialized detector handle. Holds:
///   - the `WeChatQRCode` instance (which owns the loaded DNN graph)
///   - a `tempfile::TempDir` keeping the model files alive for the
///     lifetime of the process
struct Detector {
    qr: WeChatQRCode,
    _tempdir: tempfile::TempDir,
    _scale_up: bool,
}

static DETECTOR: OnceLock<Result<Detector, String>> = OnceLock::new();

/// Initialize the global detector. Idempotent — safe to call multiple
/// times; only the first call does real work.
pub fn init_detector() -> Result<()> {
    let r = DETECTOR.get_or_init(|| match build_detector(true) {
        Ok(d) => Ok(d),
        Err(e) => Err(format!("{e:#}")),
    });
    match r {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow!(e.clone())),
    }
}

fn build_detector(scale_up: bool) -> Result<Detector> {
    let tempdir = tempfile::Builder::new()
        .prefix("wxqr-models-")
        .tempdir()
        .context("creating temp dir for WeChatQRCode models")?;

    let detect_prototxt = tempdir.path().join("detect.prototxt");
    let detect_model = tempdir.path().join("detect.caffemodel");
    let sr_prototxt = tempdir.path().join("sr.prototxt");
    let sr_model = tempdir.path().join("sr.caffemodel");
    let license_path = tempdir.path().join("LICENSE");

    write_file(&detect_prototxt, DETECT_PROTOTXT)?;
    write_file(&detect_model, DETECT_MODEL)?;
    write_file(&sr_prototxt, SR_PROTOTXT)?;
    write_file(&sr_model, SR_MODEL)?;
    write_file(&license_path, MODEL_LICENSE)?;

    let detect_prototxt_str = detect_prototxt
        .to_str()
        .ok_or_else(|| anyhow!("model path is not valid UTF-8"))?;
    let detect_model_str = detect_model
        .to_str()
        .ok_or_else(|| anyhow!("model path is not valid UTF-8"))?;
    let sr_prototxt_str = sr_prototxt
        .to_str()
        .ok_or_else(|| anyhow!("model path is not valid UTF-8"))?;
    let sr_model_str = sr_model
        .to_str()
        .ok_or_else(|| anyhow!("model path is not valid UTF-8"))?;

    let qr = WeChatQRCode::new(
        detect_prototxt_str,
        detect_model_str,
        sr_prototxt_str,
        sr_model_str,
    )
    .context("constructing WeChatQRCode detector (models corrupt or wrong version?)")?;

    Ok(Detector {
        qr,
        _tempdir: tempdir,
        _scale_up: scale_up,
    })
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut f = std::fs::File::create(path)
        .with_context(|| format!("creating {}", path.display()))?;
    f.write_all(bytes)
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

fn get_detector() -> Result<&'static WeChatQRCode> {
    let r = DETECTOR.get_or_init(|| match build_detector(true) {
        Ok(d) => Ok(d),
        Err(e) => Err(format!("{e:#}")),
    });
    match r {
        Ok(d) => Ok(&d.qr),
        Err(e) => Err(anyhow!(e.clone())),
    }
}

pub fn decode_path(path: &Path, _opts: &DecodeOptions, quiet: bool) -> Result<Vec<Decoded>> {
    if !path.exists() {
        return Err(anyhow!("file not found"));
    }
    let path_str = path
        .to_str()
        .ok_or_else(|| anyhow!("path is not valid UTF-8"))?;

    let img = opencv::imgcodecs::imread(path_str, opencv::imgcodecs::IMREAD_COLOR)
        .with_context(|| format!("reading image '{}'", path.display()))?;
    if img.empty() {
        return Err(anyhow!(
            "image '{}' could not be decoded (empty / corrupt / unsupported format)",
            path.display()
        ));
    }

    let detector = get_detector()?;
    decode_with(detector, &img, quiet)
}

fn decode_with(detector: &WeChatQRCode, img: &Mat, _quiet: bool) -> Result<Vec<Decoded>> {
    // WeChatQRCode::detect_and_decode returns
    //   (Vec<String>, bool /* found */)
    // The boolean indicates whether any QR was found; the Vec contains
    // the decoded strings (one per found code).
    let (texts, found) = detector
        .detect_and_decode(img)
        .context("WeChatQRCode::detect_and_decode failed")?;

    if !found {
        return Ok(Vec::new());
    }

    // WeChatQRCode does not return corner points (the underlying
    // detector does, but the high-level wrapper discards them). For
    // schema compat with zxing's output we emit empty points when
    // --points is not given, and an empty array when --points is given.
    Ok(texts
        .into_iter()
        .map(|text| Decoded {
            text,
            points: Vec::new(),
        })
        .collect())
}