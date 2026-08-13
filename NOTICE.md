# Third-Party Notices

This project bundles four model files from
[`WeChatCV/opencv_3rdparty`](https://github.com/WeChatCV/opencv_3rdparty),
used under the MIT License. The full license text is reproduced below
and is also shipped at `models/LICENSE`.

```
MIT License

Copyright (c) 2017-present, WeChat CV, Tencent Inc.

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Bundled files

| File | MD5 | Size | Purpose |
|------|-----|------|---------|
| `models/detect.prototxt` | `6fb4976b32695f9f5c6305c19f12537d` | 42 KB | Detector CNN architecture |
| `models/detect.caffemodel` | `238e2b2d6f3c18d6c3a30de0c31e23cf` | 943 KB | Detector CNN weights |
| `models/sr.prototxt` | `69db99927a70df953b471daaba03fbef` | 6 KB | Super-resolution CNN architecture |
| `models/sr.caffemodel` | `cbfcd60361a73beb8c583eea7e8e6664` | 23 KB | Super-resolution CNN weights |

These four files are embedded into the `wxqr` binary at compile time via
`include_bytes!`. They are extracted to a temporary directory on first
decode call and remain on disk for the lifetime of the process.

Upstream source: <https://github.com/WeChatCV/opencv_3rdparty>

For model upgrade scripts and pinned-version policy, see
[`mneme/wxqr-design/README.md`](https://github.com/ljh-sh/mneme/blob/main/wxqr-design/README.md).

## Other third-party components

- **OpenCV** (bundled via `opencv-rust` build) — Apache-2.0.
  <https://opencv.org/>
- **`opencv-rust`** crate — MIT. <https://github.com/twistedfall/opencv-rust>
- **`image`** crate — MIT OR Apache-2.0. <https://github.com/image-rs/image>