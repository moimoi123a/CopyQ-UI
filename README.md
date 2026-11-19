# CopyQ-UI

Giao diện popup hiện đại, gọn gàng kiểu `Win + V` (Windows 11) dành cho **CopyQ** trên **Linux Mint Cinnamon (X11)**, viết bằng **Rust + GTK4**.

## Cam kết bảo mật & quyền riêng tư (Security by Design)
Bạn có thể tự kiểm tra toàn bộ mã nguồn (chỉ gồm 4 file `.rs` nhỏ gọn):
- **`#![forbid(unsafe_code)]`**: Tuyệt đối không dùng code `unsafe` trong toàn bộ dự án.
- **Chỉ 1 thư viện duy nhất (`gtk4`)**: Không kéo theo hàng trăm thư viện lạ, không có thư viện mạng (`reqwest`, `hyper`, `tokio`...).
- **100% Offline**: Không mở kết nối mạng, không nghe cổng (port), không gửi dữ liệu hay telemetry ra ngoài.
- **Không chạy lệnh qua Shell (`sh -c`)**: Gọi trực tiếp `/usr/bin/copyq` với tham số cố định và chỉ số dòng dạng số nguyên (`usize`), triệt tiêu hoàn toàn nguy cơ command injection.
- **Không ghi dữ liệu clipboard ra ổ cứng**: Toàn bộ văn bản và hình ảnh đọc từ CopyQ chỉ nằm trên RAM khi popup mở, và được giải phóng ngay khi popup đóng (**0 MB RAM lúc không dùng**).
- **Quyền file riêng tư (`0600` / `0700`)**: Chỉ lưu duy nhất 2 file tùy chọn giao diện của bạn trong `~/.config/copyq-ui/`.
- **Build minh bạch trên Cloud**: GitHub Actions chỉ dùng action chính chủ (`actions/checkout@v4`, `actions/upload-artifact@v4`) và gói Ubuntu gốc (`libgtk-4-dev`).

---

## Tính năng chính
- **Bố cục sạch sẽ:** Logo CopyQ gốc góc trái, thanh tìm kiếm vừa khít ở giữa, nút **Sáng / Tối** (mặt trời / mặt trăng) góc phải tự lưu trạng thái.
- **Cố định khung cửa sổ:** Chuyển tab *Tất cả / Đã ghim / Ảnh* hoặc lọc ít mục không bị giật/thu nhỏ cửa sổ.
- **Làm sạch văn bản copy từ web:** Chỉ đọc `text/plain`, lọc sạch mảng màu nền HTML, dòng trống thừa và ký tự lỗi ``; hỗ trợ dán nhanh chữ thuần bằng **`Shift + Enter`** hoặc trong menu `⋯`.
- **Hiển thị ảnh rõ nét:** Khung ảnh nền trơn (không kẻ caro), giữ nguyên tỉ lệ ảnh; bấm **`⋯ → Phóng to`** hoặc phím **`Space`** để xem ảnh lớn trước khi dán.
- **Nút hành động xếp dọc (trên–dưới):** Trên là nút **`⋯`** (bấm mới hiện *Delete*), dưới là nút **Ghim**.
- **Tự tùy biến CSS không cần build lại:** Chỉnh trực tiếp file `~/.config/copyq-ui/style.css`.

---

## Cách build trên Cloud (GitHub Actions — không cần cài Rust trên máy)

1. Tạo một repository mới trên GitHub (ví dụ `CopyQ-UI`) và đẩy thư mục này lên (hoặc upload file lên repo trên web GitHub).
2. Vào tab **Actions** trên GitHub → chọn workflow **Build CopyQ-UI (Ubuntu 24.04 / Linux Mint 22.x)** → chờ khoảng 2 phút build xong.
3. Tải file artifact **`copyq-ui-linux-x86_64`** về máy và giải nén lấy file `copyq-ui`.

*(Hoặc nếu bạn build trên VPS Ubuntu 24.04: `sudo apt install libgtk-4-dev pkg-config` rồi chạy `cargo build --release`, lấy file tại `target/release/copyq-ui`).*

---

## Cách cài đặt & gắn vào phím `Win + V` trên Linux Mint Cinnamon

1. Chép file `copyq-ui` đã tải về vào `~/.local/bin/` và cấp quyền chạy:
   ```bash
   mkdir -p ~/.local/bin
   cp copyq-ui ~/.local/bin/copyq-ui
   chmod +x ~/.local/bin/copyq-ui
   ```
2. Đảm bảo **CopyQ** vẫn đang chạy ngầm ở khay hệ thống như bình thường.
3. Mở **System Settings → Keyboard → Shortcuts → Custom Shortcuts**:
   - Đổi lệnh của phím **`Super + V` (`Win + V`)** từ `copyq toggle` thành:
     ```
     /home/<tên-user-của-bạn>/.local/bin/copyq-ui
     ```
   *(Ví dụ: `/home/user/.local/bin/copyq-ui`)*

---

## Phím tắt trong cửa sổ popup
| Phím | Chức năng |
| :--- | :--- |
| `↑` / `↓` | Di chuyển chọn thẻ |
| `Enter` hoặc Click | Dán mục đang chọn & tự đóng |
| `Shift + Enter` | Dán dạng **chữ thuần** (bỏ sạch format web) |
| `Space` (hoặc `Ctrl + Space`) | Phóng to / thu nhỏ ảnh đang chọn |
| `Ctrl + F` | Tập trung vào ô tìm kiếm |
| `Esc` | Đóng xem ảnh → xóa từ khóa tìm → đóng popup |
| `Win + V` (lần 2) | Đóng popup ngay (toggle) |

---

## Tùy chỉnh giao diện (`~/.config/copyq-ui/`)
Lần đầu chạy, app sẽ tự tạo 2 file trong `~/.config/copyq-ui/`:
- **`config.conf`**: chỉnh `theme`, `max_items`, `width`, `height`, `auto_paste`, `plain_text_paste`, `image_fit` (`contain` hoặc `cover`), `close_on_unfocus`.
- **`style.css`**: chỉnh màu sắc, độ bo góc, cỡ chữ tùy ý — lưu xong bấm `Win + V` là ăn ngay, không cần build lại.
