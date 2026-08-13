# Roadmap

## Done

### v0.1.0 (in progress, 2026-08-14)

- `wxqr dec [OPTIONS] <IMAGE>...` subcommand
- `opencv = "0.100"` with minimal feature set:
  `core`, `imgcodecs`, `imgproc`, `dnn`, `wechat_qrcode`
- Hand-rolled CLI parser (no clap)
- Hand-rolled JSON / TSV writers (no serde) — byte-compatible with
  `ljh-sh/zxing` schema
- WeChatCV WeChatQRCode model files bundled via `include_bytes!`
  (4 files, ~1 MB, pinned to `wechat_qrcode-2023-07-23` tag commit
  hash 3487ef7)
- Models extracted to `tempfile::TempDir` on first decode call (one
  shared `OnceLock<Detector>` keeps them alive for process lifetime)
- Super-resolution pre-pass `--no-scale-up` to disable on clean images
- Output formats: `txt` / `json` / `tsv`
- `--points` (schema-compatible empty points array)
- `--null` / `--files-from <PATH|->` for chardet-style batch
- `--quiet` for script-friendly use
- Exit codes: 0 / 1 / 2 / 64
- NOTICE.md with WeChatCV MIT attribution
- README.md (English) + README.cn.md (中文)
- Apache-2.0 license

## Next

### v0.2.0 — Windows + perf

- **Windows matrix** (x86_64 + aarch64, MSYS2). — Major CI work
  because OpenCV's contrib build chain on Windows MSYS2 is the
  trickiest of the four target families.
- **Criterion bench** on the hard fixture set (8-12 images covering
  blur / reflection / wrinkles / tiny / perspective).
- **`--max-image-dim <PX>`** to downscale oversized inputs before
  decode (saves 50-200 ms on 4K camera photos).
- **`wxqr dec --single`** for explicit single-result mode (vs.
  default multi-result).
- Document the opencv-rust contrib build cache strategy in CI:
  the `.cargo/registry/src/.../opencv-*/3rdparty` cache hit is the
  single biggest CI speedup.

### v0.3.0 — OpenCV 5.x upgrade

- Wait for OpenCV 5.x stable release.
- Switch `WeChatQRCode::new` to the in-memory API (drops the
  tempfile dance entirely, saves ~50 ms per process startup).
- Drop `tempfile` + `dirs` dependencies if no longer needed.

### Deferred

- **`enc` subcommand** — structurally doesn't exist. WeChatQRCode
  is decode-only. For encoding, use `ljh-sh/zxing enc` (v0.2),
  `x qr enc`, or `qrencode`.
- **1D barcode support** — out of scope. The WeChatCV model was
  trained on QR codes only. 1D codes are handled by zxing.
- **Camera capture** — out of scope for a CLI; needs a separate
  GUI tool.

## Model pinning policy

Models are pinned to a specific `WeChatCV/opencv_3rdparty` commit
(currently `3487ef7` — the commit just after the LICENSE was added
in July 2023). Upgrading requires:

1. `./scripts/update-models.sh <new-tag-or-ref>`
2. Manual decode regression test on a known hard-corpus set
3. NOTICE.md + ROADMAP.md bump + release notes entry

This is **not** auto-updated — a model change is a deliberate
maintenance decision because every version shift re-introduces the
risk of decode regressions on previously working inputs.

## Compatibility with `x qr`

`wxqr` is designed to pair with `ljh-sh/zxing`. The output schema
is intentionally byte-compatible:

```
zxing  →  --format json  →  [{"file": "...", "results": [{"format": "QR_CODE", "text": "..."}]}]
wxqr   →  --format json  →  [{"file": "...", "results": [{"format": "QR_CODE", "text": "..."}]}]
```

So an outer dispatcher can fan out to either binary without
per-result special-casing:

```sh
x qr dec <img>
  → x zxing dec <img>           # default fast path
  → x wxqr dec <img>           # fallback on empty result
```

The `format` field is the constant string `"QR_CODE"` for all
wxqr results (it never decodes 1D barcodes). zxing's `format`
field carries the actual barcode format name.

## Compatibility with `x qr dec` (future)

This is the dispatcher that lives in `x-bash/qr`. It is **not** a
v0.1 deliverable of `wxqr` itself — `wxqr` is a standalone CLI that
any tool can call. The dispatcher is a separate piece of glue that
lives in the `x-bash/qr` repo.