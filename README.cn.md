# wxqr

> 本地 QR 解码 CLI，由 WeChatCV CNN 检测器驱动。单 binary，零网络，
> 专为硬图设计。

`wxqr` 使用 [`WeChatCV/opencv_3rdparty`](https://github.com/WeChatCV/opencv_3rdparty)
CNN 模型解码图像中的二维码 —— 与 OpenCV `cv::wechat_qrcode::WeChatQRCode`
使用同一个模型。它在通用解码器失效的场景表现出色：模糊、反光、褶皱、
极小、透视畸变的图像。

```
$ wxqr photo-of-real-world-qr.jpg
photo-of-real-world-qr.jpg    QR_CODE   https://x-cmd.com
```

`decode` 是默认子命令（仓库路径已经暗示了 decode）—— `wxqr <img>` 直接工作，
不用打 `dec`。显式形式 `wxqr decode <img>` 也接受。

**没有 `enc` 子命令。** WeChatQRCode 模型设计上就是 decode-only。
需要 QR 编码请用 [`ljh-sh/zxing`](../zxing)、`x qr enc`、`qrencode`
或其它标准 QR 编码器。

## 为什么

通用 QR 解码器（`ljh-sh/zxing`、OpenCV 自带的 `QRCodeDetector`、
`api.qrserver.com` web 服务）在真实世界图像上经常翻车 —— 商品包装照片、
手机屏幕、投影仪显示。WeChatCV 模型训练于数亿张此类图像，是噪声输入下
当前最强的解码器。

`wxqr` 让 `x qr dec` 在本地有一道兜底，能处理这些场景而不向第三方泄露图片。

## 为什么和 `ljh-sh/zxing` 分开

我们考虑过把 `wxqr` 合并进 `ljh-sh/zxing`（一个 CLI 同时含两个解码器）。
最终选择分开是 deliberate 的：

- **构建成本** —— 加 WeChatCV 模型层会让 `zxing` 的 binary 从 1.7 MB
  膨胀到 ~10 MB，并拉入 OpenCV 作为运行时依赖。大多数调用方不需要
  CNN 解码，不该为此付费。
- **冷启动成本** —— CNN 模型加载每次进程 ~500 ms。紧循环里每张图
  调 `wxqr` 很浪费；`zxing` 才是易例的默认选择。
- **可选依赖** —— `wxqr` 只在易例解码器失败时有用。`x qr dec` 调度器可以
  先调 `zxing`，需要时再 fallback 到 `wxqr`（见 [x-cmd/x-cmd#467](https://github.com/x-cmd/x-cmd/issues/467)）。

如果想要一个工具通吃两者，用上面的调度器。`wxqr` 作为独立 binary 是
硬图批处理场景的正确选择（例如批量处理 QR 贴纸照片）。

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
wxqr [OPTIONS] <IMAGE>...

OPTIONS:
    -f, --format <FMT>   输出格式: txt (默认) | json | tsv
        --no-scale-up    关闭超分预 pass（清洁图加速）
    -0, --null           输入路径用 NUL 分隔
        --files-from <P> 从文件 (或 stdin '-') 读取路径列表
        -q, --quiet       抑制 per-file stderr 错误日志
    -h, --help           显示帮助
    -V, --version        显示版本
```

JSON 输出里 `points` 字段总是出现（永远是 `[]` —— `WeChatQRCode` 高级
wrapper 不暴露 corner coordinates；这个字段保留是为了和 `ljh-sh/zxing`
字节兼容）。

### 示例

```sh
# 解码硬图
wxqr photo-of-qr.jpg

# 批量 + JSON 输出
wxqr --format json img1.jpg img2.png img3.webp

# NUL 分隔文件列表 (来自 find/xargs)
find . -name '*.jpg' -print0 | xargs -0 wxqr --null --files-from -

# 干净图跳过超分 pass
wxqr --no-scale-up clean-photo.jpg
```

### 输出格式

**txt**:
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

### 退出码

| 码 | 含义 |
|---|---|
| 0 | 至少一个 image 解码出至少一个 QR |
| 1 | 所有 image 都扫过，没找到 QR |
| 2 | 读取失败或解码异常 |
| 64 | usage 错误 |

## 权衡

`wxqr` 比 `ljh-sh/zxing` 重:

- 主 binary ~10 MB (OpenCV DNN runtime)
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
./scripts/update-models.sh
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

macOS 本地测试需要 `brew install opencv@4`（opencv@4 有 26 个 transitive
依赖，opencv-rust build pipeline 通过 pkg-config 全部拉入）。CI 在 Linux
用 `apt install libopencv-dev` 打包一切；CI 是规范的全量验证路径。

## 架构

```
src/
├── main.rs       — CLI 入口（薄壳，调 wxqr::run()）
├── lib.rs        — pub mod cli / decode / format
├── cli.rs        — 手写参数解析 + 分发
├── decode.rs     — OnceLock 构造 WeChatQRCode 检测器
│                   （Mutex<WeChatQRCode> 包在 OnceLock 里）；
│                   include_bytes! 的 4 个模型首次解码时
│                   写到 tempfile::TempDir
└── format.rs     — txt / json / tsv 全手写
                    (不依赖 serde / json crate)
```

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

## 相关项目

- [`ljh-sh/zxing`](../zxing) — 兄弟项目，是 QR / 1D 条码解码
  易例 90% 的 **快路径**。先用 `zxing`，它返回 exit 1 时 fallback
  到 `wxqr`。调度器在 `x-bash/qr`（跟踪于
  [x-cmd/x-cmd#467](https://github.com/x-cmd/x-cmd/issues/467)）。
- [`mneme/wxqr-design`](https://github.com/ljh-sh/mneme/blob/main/wxqr-design/README.md)
  — 本项目设计 rationale、决策日志、roadmap。

### wxqr vs zxing 对比

| | `ljh-sh/zxing` | `ljh-sh/wxqr` (本项目) |
|---|---|---|
| 算法 | ZXing (rxing) — 纯 Rust | WeChatCV WeChatQRCode — CNN (OpenCV) |
| 子命令 | `dec`（强制） | `decode` 默认（`wxqr <img>` 直接工作） |
| 格式覆盖 | QR + 1D (EAN/UPC/Code 128/…) | 仅 QR |
| 模型 | 无 —— 算法自身 | ~1 MB WeChatCV Caffe 模型内嵌 |
| 原生依赖 | 无 | OpenCV + bundled .dylib/.so |
| Binary 大小 | ~1.7 MB (linux-musl) | ~10 MB + ~50 MB bundled libopencv |
| 冷启动开销 | 无 | ~500 ms（模型加载） |
| 典型解码速度 | ~30 ms / 张 | ~30-300 ms / 张（CNN） |
| 最擅长场景 | 清洁 / 标准条码 | 模糊、反光、褶皱、极小、低对比度 |

两个后端输出 **字节兼容的 JSON**，调度层可以无差别扇出：

```sh
x qr dec <img>   # zxing 先试，wxqr 兜底 —— 见 x-cmd/x-cmd#467
```