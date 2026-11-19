use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Clone, Debug)]
pub enum ItemContent {
    Text {
        display: String,
        search: String,
    },
    Image {
        bytes: Vec<u8>,
    },
}

#[derive(Clone, Debug)]
pub struct ClipboardItem {
    pub row: usize,
    pub pinned: bool,
    pub content: ItemContent,
}

fn copyq_bin() -> &'static str {
    const SYSTEM_COPYQ: &str = "/usr/bin/copyq";
    if Path::new(SYSTEM_COPYQ).is_file() {
        SYSTEM_COPYQ
    } else {
        "copyq"
    }
}

/// Đọc tối đa `limit` mục gần nhất từ CopyQ đang chạy ngầm.
/// Không ghi file tạm ra ổ cứng, không gọi shell, chỉ giao tiếp cục bộ qua tiến trình `copyq`.
pub fn fetch_items(limit: usize) -> Result<Vec<ClipboardItem>, String> {
    let limit = limit.clamp(1, 200);
    let script = format!(
        r#"
var maxItems = {limit};
var total = size();
var count = total < maxItems ? total : maxItems;
var pinnedMime = 'application/x-copyq-item-pinned';
var imgMimes = ['image/png', 'image/jpeg', 'image/webp', 'image/gif', 'image/bmp'];
var maxImgBytes = 12582912;

for (var i = 0; i < count; i++) {{
    var fmts = formats(i);
    var isPin = fmts.indexOf(pinnedMime) !== -1 ? 1 : 0;
    var imgMime = '';
    for (var k = 0; k < imgMimes.length; k++) {{
        if (fmts.indexOf(imgMimes[k]) !== -1) {{
            imgMime = imgMimes[k];
            break;
        }}
    }}
    if (imgMime !== '') {{
        var rawImg = read(imgMime, i);
        if (rawImg && rawImg.length > 0 && rawImg.length <= maxImgBytes) {{
            print('I ' + i + ' ' + isPin + ' ' + str(toBase64(rawImg)) + '\n');
        }} else {{
            print('T ' + i + ' ' + isPin + ' ' + str(toBase64('[Ảnh quá lớn]')) + '\n');
        }}
    }} else {{
        var rawTxt = '';
        if (fmts.indexOf('text/plain') !== -1) {{
            rawTxt = str(read('text/plain', i));
        }} else if (fmts.indexOf('text/uri-list') !== -1) {{
            rawTxt = str(read('text/uri-list', i));
        }} else {{
            rawTxt = str(read(i));
        }}
        if (rawTxt.length > 4096) {{
            rawTxt = rawTxt.substring(0, 4096);
        }}
        print('T ' + i + ' ' + isPin + ' ' + str(toBase64(rawTxt)) + '\n');
    }}
}}
"#
    );

    let output = Command::new(copyq_bin())
        .arg("eval")
        .arg(&script)
        .stdin(Stdio::null())
        .output()
        .map_err(|err| format!("Không gọi được lệnh copyq: {err}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("Cannot connect") || stderr.contains("server") {
            return Err("CopyQ chưa chạy nền. Hãy mở CopyQ trước nhé.".to_string());
        }
        return Err(format!("CopyQ báo lỗi: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_protocol_output(&stdout))
}

fn parse_protocol_output(stdout: &str) -> Vec<ClipboardItem> {
    let mut items = Vec::new();
    for line in stdout.lines() {
        let mut parts = line.splitn(4, ' ');
        let Some(kind) = parts.next() else {
            continue;
        };
        let Some(row_str) = parts.next() else {
            continue;
        };
        let Some(pin_str) = parts.next() else {
            continue;
        };
        let payload_b64 = parts.next().unwrap_or("");

        let Ok(row) = row_str.parse::<usize>() else {
            continue;
        };
        let pinned = pin_str == "1";

        let Ok(decoded) = decode_base64(payload_b64) else {
            continue;
        };

        match kind {
            "I" => {
                if !decoded.is_empty() {
                    items.push(ClipboardItem {
                        row,
                        pinned,
                        content: ItemContent::Image { bytes: decoded },
                    });
                }
            }
            "T" => {
                let raw_text = String::from_utf8_lossy(&decoded);
                let (display, search) = normalize_text(&raw_text);
                if display.is_empty() {
                    continue;
                }
                items.push(ClipboardItem {
                    row,
                    pinned,
                    content: ItemContent::Text { display, search },
                });
            }
            _ => {}
        }
    }
    items
}

/// Chọn mục từ CopyQ và (nếu bật `auto_paste`) tự động dán sau khi popup đóng.
pub fn select_and_paste(row: usize, auto_paste: bool, plain_text: bool) -> Result<(), String> {
    let paste_tail = if auto_paste {
        "sleep(130); paste();"
    } else {
        ""
    };

    let script = if plain_text {
        format!(
            "var r = {row}; var t = str(read('text/plain', r)); if (t.length === 0) {{ t = str(read(r)); }} copy(t); {paste_tail}"
        )
    } else {
        format!("var r = {row}; select(r); {paste_tail}")
    };

    Command::new(copyq_bin())
        .arg("eval")
        .arg(&script)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("Không thể dán mục #{row}: {err}"))?;

    Ok(())
}

/// Bật / tắt trạng thái ghim của một mục trong CopyQ.
pub fn toggle_pin(row: usize) -> Result<(), String> {
    let script = format!(
        r#"
var r = {row};
var pinnedMime = 'application/x-copyq-item-pinned';
var fmts = formats(r);
var isPin = fmts.indexOf(pinnedMime) !== -1;
if (typeof plugins !== 'undefined' && plugins.itempinned) {{
    if (isPin) {{
        plugins.itempinned.unpin(r);
    }} else {{
        plugins.itempinned.pin(r);
    }}
}} else {{
    var it = getItem(r);
    if (isPin) {{
        delete it[pinnedMime];
    }} else {{
        it[pinnedMime] = '';
    }}
    setItem(r, it);
}}
"#
    );

    let status = Command::new(copyq_bin())
        .arg("eval")
        .arg(&script)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| format!("Không thể ghim mục #{row}: {err}"))?;

    if status.success() {
        Ok(())
    } else {
        Err("CopyQ từ chối thay đổi trạng thái ghim.".to_string())
    }
}

/// Xóa một mục khỏi CopyQ (tự bỏ ghim trước nếu mục đang được ghim).
pub fn delete_item(row: usize) -> Result<(), String> {
    let script = format!(
        r#"
var r = {row};
var pinnedMime = 'application/x-copyq-item-pinned';
var fmts = formats(r);
if (fmts.indexOf(pinnedMime) !== -1) {{
    if (typeof plugins !== 'undefined' && plugins.itempinned) {{
        plugins.itempinned.unpin(r);
    }} else {{
        var it = getItem(r);
        delete it[pinnedMime];
        setItem(r, it);
    }}
}}
remove(r);
"#
    );

    let status = Command::new(copyq_bin())
        .arg("eval")
        .arg(&script)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| format!("Không thể xóa mục #{row}: {err}"))?;

    if status.success() {
        Ok(())
    } else {
        Err("CopyQ không xóa được mục này.".to_string())
    }
}

/// Làm sạch văn bản copy từ web/app để hiển thị gọn gàng, đều đặn trên thẻ:
/// - Loại bỏ ký tự lỗi `\u{FFFD}` (), ký tự điều khiển và ký tự ẩn zero-width.
/// - Chuẩn hóa khoảng trắng thừa và bỏ dòng trống thừa.
/// - Trả về `(display_text, lowercase_search_text)`.
pub fn normalize_text(raw: &str) -> (String, String) {
    let mut cleaned_lines: Vec<String> = Vec::new();

    for raw_line in raw.replace("\r\n", "\n").replace('\r', "\n").split('\n') {
        let mut line = String::new();
        let mut prev_space = true;

        for ch in raw_line.chars() {
            if ch == '\u{FFFD}'
                || ch == '\u{200B}'
                || ch == '\u{200C}'
                || ch == '\u{200D}'
                || ch == '\u{FEFF}'
            {
                continue;
            }
            if ch.is_control() && ch != '\t' {
                continue;
            }
            if ch.is_whitespace() || ch == '\u{00A0}' {
                if !prev_space {
                    line.push(' ');
                    prev_space = true;
                }
            } else {
                line.push(ch);
                prev_space = false;
            }
        }

        let trimmed = line.trim_end();
        if !trimmed.is_empty() {
            cleaned_lines.push(trimmed.to_string());
        }
    }

    if cleaned_lines.is_empty() {
        return (String::new(), String::new());
    }

    let search = cleaned_lines.join(" ").to_lowercase();

    // Giữ tối đa 4 dòng đầu và tối đa 300 ký tự cho preview thẻ (Label GTK4 sẽ khóa ở 3 dòng + "...")
    let mut display = String::new();
    let mut char_count = 0usize;
    for (idx, line) in cleaned_lines.iter().take(4).enumerate() {
        if idx > 0 {
            if char_count >= 300 {
                break;
            }
            display.push('\n');
            char_count += 1;
        }
        for ch in line.chars() {
            if char_count >= 300 {
                display.push('…');
                return (display, search);
            }
            display.push(ch);
            char_count += 1;
        }
    }

    (display, search)
}

/// Bộ giải mã Base64 thuần Rust (RFC 4648), không dùng unsafe và không cần crate ngoài.
pub fn decode_base64(input: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buf = [0u8; 4];
    let mut len = 0usize;
    let mut pad = 0usize;

    for b in input.bytes() {
        if b.is_ascii_whitespace() {
            continue;
        }
        if b == b'=' {
            buf[len] = 0;
            len += 1;
            pad += 1;
        } else {
            if pad > 0 {
                return Err("Base64 padding không hợp lệ".to_string());
            }
            let val = match b {
                b'A'..=b'Z' => b - b'A',
                b'a'..=b'z' => b - b'a' + 26,
                b'0'..=b'9' => b - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                _ => return Err("Ký tự Base64 không hợp lệ".to_string()),
            };
            buf[len] = val;
            len += 1;
        }

        if len == 4 {
            out.push((buf[0] << 2) | (buf[1] >> 4));
            if pad < 2 {
                out.push((buf[1] << 4) | (buf[2] >> 2));
            }
            if pad < 1 {
                out.push((buf[2] << 6) | buf[3]);
            }
            len = 0;
            pad = 0;
        }
    }

    if len != 0 {
        return Err("Độ dài Base64 không hợp lệ".to_string());
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_standard_base64() {
        assert_eq!(decode_base64("").unwrap(), b"");
        assert_eq!(decode_base64("SGVsbG8sIFdvcmxkIQ==").unwrap(), b"Hello, World!");
        assert_eq!(
            String::from_utf8(decode_base64("WGluIGNow6BvIFZp4buHdCBOYW0=").unwrap()).unwrap(),
            "Xin chào Việt Nam"
        );
    }

    #[test]
    fn cleans_messy_web_text() {
        let messy = "\n\n  \u{FFFD}\u{FFFD}   Duyệt   file\ttrên   máy chủ  \r\n\r\n\r\n   + Tạo thư mục \u{200B}  \n\n";
        let (display, search) = normalize_text(messy);
        assert_eq!(display, "Duyệt file trên máy chủ\n+ Tạo thư mục");
        assert_eq!(search, "duyệt file trên máy chủ + tạo thư mục");
    }

    #[test]
    fn parses_protocol_lines() {
        let out = "T 0 1 SGVsbG8=\nI 1 0 iVBORw==\n";
        let items = parse_protocol_output(out);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].row, 0);
        assert!(items[0].pinned);
        assert!(matches!(items[0].content, ItemContent::Text { .. }));
        assert_eq!(items[1].row, 1);
        assert!(!items[1].pinned);
        assert!(matches!(items[1].content, ItemContent::Image { .. }));
    }
}
