# wxqr

> 本地 QR 解码 CLI，由 WeChatCV CNN 检测器驱动。单 binary，零网络，
> 专为硬图设计。

`wxqr` 使用 [`WeChatCV/opencv_3rdparty`](https://github.com/WeChatCV/opencv_3rdparty)
CNN 模型解码图像中的二维码 —— 与 OpenCV `cv::wechat_qrcode::WeChatQRCode`
使用同一个模型。它在通用解码器失效的场景表现出色：模糊、反光、褶皱、
极小、透视畸变的图像。

```
$ wxqr dec photo-of-real-world-qr.jpg
photo-of-real-world-qr.jpg    QR_CODE   https://x-cmd.com
```

**没有 `enc` 子命令。** WeChatQRCode 模型设计上就是 decode-only。
需要 QR 编码请用 [`ljh-sh/zxing`](../zxing)、`x qr enc`、`qrencode`
或其它标准 QR 编码器。

## 为什么

通用 QR 解码器（zxing、OpenCV 自带的 `QRCodeDetector`、`x qr webdec`
使用的 `api.qrserver.com` web 服务）在真实世界图像上经常翻车 —— 商品包装
照片、手机屏幕、投影仪显示。WeChatCV 模型训练于数亿张此类图像，是噪声
输入下当前最强的解码器。

`wxqr` 让 `x qr dec` 在本地有一道兜底，能处理这些场景而不向第三方泄露图片。

## 安装

```sh
# macOS / Linux
curl -fsSL https://raw.githubusercontent.com/ljh-sh/wxqr/main/install.sh | sh

# x-cmd
x eget ljh-sh/wxqr
```

预编译 binary: linux-musl (x86_64 + aarch64), macOS (x86_64 + aarch64)。
Windows deferred 到 v0.2。

每个 release artifact 都用 [cosign][cosign] 签名。四个 WeChatCV 模型文件
(~1 MB 总计) 通过 `include_bytes!` 嵌入 binary；release artifact 不需要
单独下载模型。

[cosign]: https://docs.sigstore.dev/

## 用法

```
wxqr dec [OPTIONS] <IMAGE>...

OPTIONS:
    -f, --format <FMT>   输出格式: txt (默认) | json | tsv
        --no-scale-up    关闭超分预 pass
        --points         JSON/TSV 输出包含角点坐标
                         (当前为空数组 — WeChatQRCode 不暴露坐标)
    -0, --null           输入路径用 NUL 分隔
        --files-from <P> 从文件 (或 stdin '-') 读取路径列表
        -q, --quiet       抑制 per-file stderr 错误日志
    -h, --help           显示帮助
    -V, --version        显示版本
```

### 示例

```sh
# 解码硬图
wxqr dec photo-of-qr.jpg

# 批量 + JSON 输出
wxqr dec --format json img1.jpg img2.png img3.webp

# 角点坐标 (当前为空数组；未来 OpenCV 版本可能暴露)
wxqr dec --format json --points blurry.png

# NUL 分隔文件列表 (来自 find/xargs)
find . -name '*.jpg' -print0 | xargs -0 wxqr dec --null --files-from -
```

### 输出格式

输出 schema 与 `ljh-sh/zxing` 字节兼容:

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

### 退出码

| 码 | 含义 |
|---|---|
| 0 | 至少一个 image 解码出至少一个 QR |
| 1 | 所有 image 都扫过，没找到 QR |
| 2 | 读取失败或解码异常 |
| 64 | usage 错误 |

## 权衡

`wxqr` 比 `zxing` 重:

- 主 binary 5-10 MB (OpenCV DNN runtime)
- Linux 上 ~50 MB bundled `libopencv` 共享库
- ~500 ms 一次性启动成本加载四个模型

回报是:能解码 `zxing` 返回空 (exit 1) 的图像 —— 模糊照片、反光表面、
褶皱纸张、极小印刷、低对比。日常 90% 用 `zxing`；剩下 10% 退化图用
`wxqr` 兜底。`x qr dec` 调度层先试 `zxing`，空结果才 fallback 到 `wxqr`，
成本按需才付。

## 源码构建

```sh
git clone https://github.com/ljh-sh/wxqr
cd wxqr
cargo build --release
./target/release/wxqr --version
```

四个模型文件在 `models/`，编译期嵌入。这些文件在 `.gitignore` 中（出于
礼貌），但 `models/` 目录必须在 build 时存在，内容齐全：

```sh
./scripts/update-models.sh wechat_qrcode-2023-07-23
```

(脚本钉死模型版本；bump 是 deliberate 维护步骤，要走 release notes —
见 ROADMAP。)

跨编译 (`cargo-zigbuild`):

```sh
cargo zigbuild --release --target x86_64-unknown-linux-musl
cargo zigbuild --release --target aarch64-unknown-linux-musl
cargo zigbuild --release --target x86_64-apple-darwin
cargo zigbuild --release --target aarch64-apple-darwin
```

## 架构

```
src/
├── main.rs       — CLI 入口，手写参数解析
├── decode.rs     — 用 opencv::imgcodecs 开图，
│                   OnceLock 构造一次 WeChatQRCode，
│                   调用 detect_and_decode
└── format.rs     — txt / json / tsv 输出，与 zxing schema 字节兼容
```

CLI 故意避开 `clap` / `serde`，保持 binary 小、依赖面可审计。

### 为什么把模型文件嵌入 binary

OpenCV 4.x 的 `WeChatQRCode::new` 要 **文件路径**，不要内存 buffer。
为了把模型文件留在用户文件系统外（避免安装时网络下载），我们:

1. 编译期用 `include_bytes!` 嵌入四个文件。
2. 首次 decode 时写到 `tempfile::TempDir`。
3. 用 temp path 构造 `WeChatQRCode` 检测器。
4. `TempDir` 在进程生命周期内持有；退出时自动清理。

OpenCV 5.x 计划支持 in-memory 模型加载，落地后彻底干掉 temp file。

## 协议

Apache-2.0。见 [LICENSE](LICENSE)。

包含:
- WeChatCV WeChatQRCode 检测器 + 超分模型 (MIT, 见 [NOTICE.md](NOTICE.md))
- OpenCV + opencv-rust (Apache-2.0 / MIT)
- `image` crate (MIT OR Apache-2.0)

## 相关

- [`ljh-sh/zxing`](../zxing) — 兄弟项目。`x qr dec` 调度层先试 zxing
  (快、小、支持 1D)，空结果才 fallback 到 wxqr。
- `x-bash/qr` — 用这两个 binary 的 x-cmd 模块。
- [`mneme/wxqr-design`](https://github.com/ljh-sh/mneme/blob/main/wxqr-design/README.md)
  — 本项目设计 rationale、决策日志、roadmap。