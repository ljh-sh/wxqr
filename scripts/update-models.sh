#!/bin/sh
# Update the bundled WeChatCV WeChatQRCode models from upstream.
#
# Usage:
#   ./scripts/update-models.sh                 # default: latest commit on default branch
#   ./scripts/update-models.sh <ref>           # specific commit / branch / tag
#   ./scripts/update-models.sh 3487ef7         # example: pin to a specific commit
#
# After running:
#   1. Run `cargo build --release` to re-bake include_bytes!
#   2. Run decode tests on the hard-corpus set (./tests/hard-corpus/)
#   3. Update NOTICE.md, ROADMAP.md, and bump the model MD5 table.
#   4. Commit with a release-note entry describing the model change.
#
# This is **deliberately manual** — see ROADMAP.md "Model pinning policy".

set -eu

REF="${1:-HEAD}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT INT TERM HUP

git clone --depth 1 https://github.com/WeChatCV/opencv_3rdparty.git "$TMP"

if [ "$REF" != "HEAD" ]; then
    (cd "$TMP" && git fetch --depth 1 origin "$REF" && git checkout FETCH_HEAD)
fi

# Pin to the wechat_qrcode models: detect.prototxt, detect.caffemodel,
# sr.prototxt, sr.caffemodel. They live at the repo root in upstream.
cp "$TMP/detect.prototxt"    ./models/
cp "$TMP/detect.caffemodel"  ./models/
cp "$TMP/sr.prototxt"        ./models/
cp "$TMP/sr.caffemodel"      ./models/
cp "$TMP/LICENSE"            ./models/

echo "Updated models to ref '$REF'."
echo "MD5 sums (compare to upstream README):"
md5sum ./models/detect.prototxt \
       ./models/detect.caffemodel \
       ./models/sr.prototxt \
       ./models/sr.caffemodel

echo
echo "Next steps:"
echo "  1. cargo build --release"
echo "  2. Run decode tests on ./tests/hard-corpus/"
echo "  3. Update NOTICE.md and ROADMAP.md"
echo "  4. Commit with: 'feat(wxqr): update WeChatQRCode models to <ref>'"