//! Integration tests for the format module + CLI argument parsing.
//!
//! These tests don't load images (which would require the OpenCV
//! runtime + bundled WeChatQRCode models) — they cover the pieces
//! that don't depend on the OpenCV decode path: the txt/json/tsv
//! output formatters and the format-string parsing.

use std::path::Path;

use wxqr::Decoded;

#[test]
fn txt_emits_one_line_per_detection() {
    let mut buf = Vec::new();
    let results = vec![Decoded {
        text: "https://x-cmd.com".to_string(),
        points: vec![],
    }];
    wxqr::emit_txt(&mut buf, Path::new("qr.png"), &results);
    let s = String::from_utf8(buf).unwrap();
    assert_eq!(s, "qr.png\tQR_CODE\thttps://x-cmd.com\n");
}

#[test]
fn json_emits_valid_shape_with_empty_points_always() {
    // WeChatQRCode does not expose corner points at the high-level
    // wrapper, so the array is always empty even when --points would
    // have been set. The field is present for schema compatibility
    // with ljh-sh/zxing.
    let mut buf = Vec::new();
    let results = vec![Decoded {
        text: "https://x-cmd.com".to_string(),
        points: vec![],
    }];
    wxqr::emit_json(&mut buf, Path::new("qr.png"), &results);
    let s = String::from_utf8(buf).unwrap();
    assert_eq!(
        s,
        "[{\"file\": \"qr.png\", \"results\": [{\"format\": \"QR_CODE\", \"text\": \"https://x-cmd.com\", \"points\": []}]}]\n"
    );
}

#[test]
fn tsv_escapes_tabs_and_newlines_in_text_field() {
    let mut buf = Vec::new();
    let results = vec![Decoded {
        text: "col1\tcol2\tcol3".to_string(),
        points: vec![],
    }];
    wxqr::emit_tsv(&mut buf, Path::new("qr.png"), &results);
    let s = String::from_utf8(buf).unwrap();

    let mut cols = s.split('\t');
    assert_eq!(cols.next(), Some("qr.png"));
    assert_eq!(cols.next(), Some("QR_CODE"));
    let text_field = cols.next().unwrap_or("").trim_end_matches('\n');
    assert_eq!(
        text_field, "col1\\tcol2\\tcol3",
        "text field should escape internal tabs"
    );
    assert!(!text_field.contains('\t'));
}