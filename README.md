# wxqr

> Local QR decoder CLI powered by the WeChatCV CNN detector. Single
> binary, zero network, designed for the hard images.

`wxqr` decodes QR codes from images using the
[`WeChatCV/opencv_3rdparty`](https://github.com/WeChatCV/opencv_3rdparty)
CNN — the same model that backs OpenCV's `cv::wechat_qrcode::WeChatQRCode`.
It excels at the cases where generic decoders fall short: blurry,
reflective, wrinkled, tiny, or perspective-distorted images.

```
$ wxqr dec photo-of-real-world-qr.jpg
photo-of-real-world-qr.jpg    QR_CODE   https://x-cmd.com
```

**There is no `enc` subcommand.** WeChatQRCode is decode-only by design.
For QR encoding, use [`ljh-sh/zxing`](../zxing), `x qr enc`, `qrencode`,
or any standard QR encoder.

## Why

Generic QR decoders (zxing, OpenCV's built-in `QRCodeDetector`, the
`api.qrserver.com` web service that `x qr webdec` uses today) routinely
fail on real-world images — photos of product packaging, phone screens,
overhead projector displays. The WeChatCV model was trained on
hundreds of millions of such images and is the state-of-the-art
decoder for noisy inputs.

`wxqr` exists to give the `x qr dec` flow an on-device fallback that
handles these cases without leaking the image to a third party.

## Install

```sh
# macOS / Linux
curl -fsSL https://raw.githubusercontent.com/ljh-sh/wxqr/main/install.sh | sh

# via x-cmd
x eget ljh-sh/wxqr
```

Pre-built binaries: linux-musl (x86_64 + aarch64), macOS
(x86_64 + aarch64). Windows deferred to v0.2.

Every release artifact is signed with [cosign][cosign]. The four
WeChatCV model files (~1 MB total) are embedded in the binary via
`include_bytes!`; the release artifact has no separate model download.

[cosign]: https://docs.sigstore.dev/

## Usage

```
wxqr dec [OPTIONS] <IMAGE>...

OPTIONS:
    -f, --format <FMT>   Output format: txt (default) | json | tsv
        --no-scale-up    Disable super-resolution pre-pass
        --points         Include corner points in JSON / TSV output
                         (currently empty array — WeChatQRCode
                         does not expose coordinates)
    -0, --null           Treat input as NUL-separated path list
        --files-from <P> Read paths from a file ('-' for stdin)
        -q, --quiet       Suppress per-file stderr error logs
    -h, --help           Show this help.
    -V, --version        Show version.
```

### Examples

```sh
# Decode a hard image
wxqr dec photo-of-qr.jpg

# Batch decode with JSON output
wxqr dec --format json img1.jpg img2.png img3.webp

# Find the corner points (currently empty array; future OpenCV
# versions may expose them)
wxqr dec --format json --points blurry.png

# NUL-separated file list from find/xargs
find . -name '*.jpg' -print0 | xargs -0 wxqr dec --null --files-from -
```

### Output format

The output schema is byte-compatible with `ljh-sh/zxing`:

**txt**:
```
photo.jpg   QR_CODE   https://x-cmd.com
```

**json**:
```json
[{"file": "photo.jpg", "results": [{"format": "QR_CODE", "text": "https://x-cmd.com"}]}]
```

**tsv**:
```
photo.jpg   QR_CODE   https://x-cmd.com
```

### Exit codes

| code | meaning |
|---|---|
| 0 | at least one image decoded at least one QR |
| 1 | all images scanned, no QR codes found |
| 2 | read failure or decode exception |
| 64 | usage error |

## Trade-offs

`wxqr` is heavier than `zxing`:

- 5-10 MB main binary (OpenCV DNN runtime)
- ~50 MB bundled `libopencv` shared library (Linux)
- ~500 ms one-time startup cost to load the four models

In return, it decodes images where `zxing` returns exit 1 (no result):
blurry photos, reflective surfaces, wrinkled paper, tiny print, low
contrast. Use `zxing` for the easy 90% of cases; use `wxqr` as a
fallback for the remaining 10%.

The `x qr dec` dispatcher picks `zxing` first and only falls back to
`wxqr` on empty results, so the cost is paid only when needed.

## Build from source

```sh
git clone https://github.com/ljh-sh/wxqr
cd wxqr
cargo build --release
./target/release/wxqr --version
```

The four model files live in `models/` and are embedded at compile
time. They are gitignored as a courtesy, but a `models/` directory
with the four files is required at build time. To populate them:

```sh
./scripts/update-models.sh wechat_qrcode-2023-07-23
```

(That script pins the model version; bumping it is a deliberate
maintenance step that goes through release notes — see ROADMAP.)

Cross-compile (via `cargo-zigbuild`):

```sh
cargo zigbuild --release --target x86_64-unknown-linux-musl
cargo zigbuild --release --target aarch64-unknown-linux-musl
cargo zigbuild --release --target x86_64-apple-darwin
cargo zigbuild --release --target aarch64-apple-darwin
```

## Architecture

```
src/
├── main.rs       — CLI entry point, hand-rolled arg parser
├── decode.rs     — opens images via opencv::imgcodecs,
│                   constructs WeChatQRCode once (OnceLock),
│                   calls detect_and_decode
└── format.rs     — txt / json / tsv emitters, byte-compatible
                    with ljh-sh/zxing's schema
```

The CLI deliberately avoids `clap` / `serde` to keep the binary small
and the dependency surface auditable.

### Why we ship the model files inside the binary

OpenCV 4.x's `WeChatQRCode::new` takes **file paths**, not memory
buffers. To keep the model files out of the user's filesystem (and
avoid requiring a network download at install time), we:

1. Embed the four files at compile time via `include_bytes!`.
2. On first decode call, write them to a `tempfile::TempDir`.
3. Construct the `WeChatQRCode` detector from those temp paths.
4. The `TempDir` is held for the process lifetime; on exit it cleans
   up.

OpenCV 5.x plans to support in-memory model loading. When that lands
we drop the temp file dance entirely.

## License

Apache-2.0. See [LICENSE](LICENSE).

Includes:
- The WeChatCV WeChatQRCode detector + super-resolution models
  (MIT, see [NOTICE.md](NOTICE.md)).
- OpenCV + opencv-rust (Apache-2.0 / MIT).
- `image` crate (MIT OR Apache-2.0).

## Related

- [`ljh-sh/zxing`](../zxing) — sibling project. The `x qr dec`
  dispatcher tries zxing first (fast, small, 1D support) and falls
  back to wxqr only on empty results.
- `x-bash/qr` — the x-cmd module that will use both binaries.
- [`mneme/wxqr-design`](https://github.com/ljh-sh/mneme/blob/main/wxqr-design/README.md)
  — design rationale, decision log, and roadmap for this project.