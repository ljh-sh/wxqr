# wxqr

> Local QR decoder CLI powered by the WeChatCV CNN detector. Single
> binary, zero network, designed for the hard images.

`wxqr` decodes QR codes from images using the
[`WeChatCV/opencv_3rdparty`](https://github.com/WeChatCV/opencv_3rdparty)
CNN — the same model that backs OpenCV's `cv::wechat_qrcode::WeChatQRCode`.
It excels at the cases where generic decoders fall short: blurry,
reflective, wrinkled, tiny, or perspective-distorted images.

```
$ wxqr photo-of-real-world-qr.jpg
photo-of-real-world-qr.jpg    QR_CODE   https://x-cmd.com
```

`decode` is the implicit subcommand (the repo path already implies
decode) — `wxqr <img>` works directly without typing it. The
explicit form `wxqr decode <img>` is also accepted.

**There is no `enc` subcommand.** WeChatQRCode is decode-only by design.
For QR encoding, use [`ljh-sh/zxing`](../zxing), `x qr enc`, `qrencode`,
or any standard QR encoder.

## Why

Generic QR decoders (`ljh-sh/zxing`, OpenCV's built-in
`QRCodeDetector`, `api.qrserver.com`) routinely fail on real-world
images — photos of product packaging, phone screens, overhead
projector displays. The WeChatCV model was trained on hundreds of
millions of such images and is the state-of-the-art decoder for noisy
inputs.

`wxqr` exists to give the `x qr dec` flow an on-device fallback that
handles these cases without leaking the image to a third party.

## Why a separate project from `ljh-sh/zxing`

We considered merging `wxqr` into `ljh-sh/zxing` (one CLI that bundles
both decoders). The split is deliberate:

- **Build cost** — adding the WeChatCV model layer would balloon
  `zxing`'s binary from 1.7 MB to ~10 MB and pull in OpenCV as a
  runtime dependency. Most callers don't need CNN-based decoding and
  shouldn't pay for it.
- **Cold-start cost** — the CNN model load takes ~500 ms per process.
  Calling `wxqr` per image in a tight loop is wasteful; `zxing` is
  the right default for the easy case.
- **Optional dependency** — `wxqr` is only useful when the easy
  decoder fails. An `x qr dec` dispatcher can call `zxing` first
  and fall back to `wxqr` only when needed (see
  [x-cmd/x-cmd#467](https://github.com/x-cmd/x-cmd/issues/467)).

If you want one tool that does both, use the dispatcher above. `wxqr`
as a standalone binary is the right choice for hard-image-specific
workflows (e.g. processing a batch of photos of QR stickers).

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
wxqr [OPTIONS] <IMAGE>...

OPTIONS:
    -f, --format <FMT>   Output format: txt (default) | json | tsv
        --no-scale-up    Disable super-resolution pre-pass
                         (faster on clean images)
    -0, --null           Treat input as NUL-separated path list
        --files-from <P> Read paths from a file ('-' for stdin)
        -q, --quiet       Suppress per-file stderr error logs
    -h, --help           Show this help.
    -V, --version        Show version.
```

The `points` field is always emitted in JSON (always `[]` — the
high-level `WeChatQRCode` wrapper does not expose corner coordinates;
the field is preserved for byte-compatibility with `ljh-sh/zxing`).

### Examples

```sh
# Decode a hard image
wxqr photo-of-qr.jpg

# Batch decode with JSON output
wxqr --format json img1.jpg img2.png img3.webp

# NUL-separated file list from find/xargs
find . -name '*.jpg' -print0 | xargs -0 wxqr --null --files-from -

# Skip the super-resolution pass on clean images
wxqr --no-scale-up clean-photo.jpg
```

### Output formats

**txt** (default):
```
photo.jpg   QR_CODE   https://x-cmd.com
```

**json**:
```json
[{"file": "photo.jpg", "results": [{"format": "QR_CODE", "text": "https://x-cmd.com", "points": []}]}]
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

`wxqr` is heavier than `ljh-sh/zxing`:

- ~10 MB main binary (OpenCV DNN runtime)
- ~50 MB bundled `libopencv` shared library on Linux
- ~500 ms one-time startup cost to load the four models

In return, it decodes images where `zxing` returns exit 1 (no result):
blurry photos, reflective surfaces, wrinkled paper, tiny print, low
contrast. Use `zxing` for the easy 90% of cases; use `wxqr` as a
fallback for the remaining 10%.

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
./scripts/update-models.sh
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

Local runtime testing on macOS requires `brew install opencv@4`
(opencv@4 has 26 transitive deps that the opencv-rust build
pipeline pulls in via pkg-config). CI on Linux uses `apt install
libopencv-dev` which bundles everything; CI is the canonical full
verification path.

## Architecture

```
src/
├── main.rs       — CLI binary (thin shim that calls wxqr::run())
├── lib.rs        — pub mod cli / decode / format; public API
├── cli.rs        — Hand-rolled arg parser + dispatcher
├── decode.rs     — Builds the WeChatQRCode detector once
│                   (Mutex<WeChatQRCode> in a OnceLock<Result>);
│                   extracts the 4 model files from include_bytes!
│                   to a tempfile::TempDir on first decode call
└── format.rs     — txt / json / tsv emitters, all hand-written
                    (no serde / json crate dependency)
```

### Why we extract models to a temp dir at runtime

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
  (MIT, see [NOTICE.md](NOTICE.md))
- OpenCV + opencv-rust (Apache-2.0 / MIT)
- `image` crate (MIT OR Apache-2.0)

## Related projects

- [`ljh-sh/zxing`](../zxing) — sibling project that is the **fast
  path** for the easy 90% of QR / 1D barcode decoding. Use `zxing`
  first, fall back to `wxqr` when `zxing` returns exit 1. The
  dispatcher lives in `x-bash/qr` (tracked in
  [x-cmd/x-cmd#467](https://github.com/x-cmd/x-cmd/issues/467)).
- [`mneme/wxqr-design`](https://github.com/ljh-sh/mneme/blob/main/wxqr-design/README.md)
  — design rationale, decision log, and roadmap for this project.

### wxqr vs zxing at a glance

| | `ljh-sh/zxing` | `ljh-sh/wxqr` (this) |
|---|---|---|
| Algorithm | ZXing (rxing) — pure Rust | WeChatCV WeChatQRCode — CNN (OpenCV) |
| Subcommand | `dec` (mandatory) | `decode` is implicit (`wxqr <img>` works directly) |
| Format coverage | QR + 1D (EAN/UPC/Code 128/…) | QR only |
| Models | none — algorithm only | ~1 MB WeChatCV Caffe models bundled |
| Native deps | none | OpenCV + bundled .dylib/.so |
| Binary size | ~1.7 MB (linux-musl) | ~10 MB + ~50 MB bundled libopencv |
| Cold-start cost | none | ~500 ms (model load) |
| Typical decode | ~30 ms / image | ~30-300 ms / image (CNN) |
| Best at | clean / standard barcodes | blurry, reflective, wrinkled, tiny, low-contrast |

Both backends emit **byte-compatible JSON**, so a dispatcher can fan
out without per-result special-casing:

```sh
x qr dec <img>   # zxing first, wxqr fallback — see x-cmd/x-cmd#467
```