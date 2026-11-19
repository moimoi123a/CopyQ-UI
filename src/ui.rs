use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use gtk4 as gtk;
use gtk::prelude::*;
use gtk::{
    gdk, glib, pango, Align, Application, ApplicationWindow, Button, ContentFit, CssProvider,
    EventControllerKey, IconTheme, Image, Label, ListBox, ListBoxRow, MenuButton, Orientation,
    Overflow, Overlay, Picture, PolicyType, Popover, PositionType, PropagationPhase,
    ScrolledWindow, SearchEntry, SelectionMode,
};

use crate::config::{AppConfig, ImageFit, Theme};
use crate::copyq::{self, ClipboardItem, ItemContent};

#[derive(Clone, Copy, PartialEq, Eq)]
enum FilterTab {
    All,
    Pinned,
    Images,
}

enum UiItemKind {
    Text { display: String, search: String },
    Image { texture: Option<gdk::Texture> },
}

struct UiItem {
    row: usize,
    pinned: bool,
    kind: UiItemKind,
}

struct AppState {
    config: AppConfig,
    items: Vec<UiItem>,
    visible_indices: Vec<usize>,
    filter: FilterTab,
    query: String,
    error_message: Option<String>,
    zoomed_item_index: Option<usize>,
}

#[derive(Clone)]
struct UiHandles {
    window: ApplicationWindow,
    list_box: ListBox,
    scrolled: ScrolledWindow,
    search_entry: SearchEntry,
    theme_btn: Button,
    tab_all: Button,
    tab_pinned: Button,
    tab_images: Button,
    zoom_overlay: gtk::Box,
    zoom_picture: Picture,
    state: Rc<RefCell<AppState>>,
    popover_open_count: Rc<Cell<u32>>,
}

pub fn build_ui(app: &Application) {
    // Nếu popup đang mở mà người dùng bấm Win+V lần nữa -> đóng luôn (cơ chế toggle chuẩn)
    let existing = app.windows();
    if !existing.is_empty() {
        for win in existing {
            win.close();
        }
        return;
    }

    let config = AppConfig::load_or_create();
    load_stylesheet();

    let window = ApplicationWindow::builder()
        .application(app)
        .title("CopyQ-UI")
        .decorated(false)
        .resizable(false)
        .default_width(config.width)
        .default_height(config.height)
        .build();
    window.add_css_class("popup-window");

    let popup_container = gtk::Box::new(Orientation::Vertical, 0);
    popup_container.add_css_class("popup-container");
    popup_container.set_size_request(config.width, config.height);
    popup_container.set_overflow(Overflow::Hidden);

    // ===== 1. HEADER: Logo CopyQ (trái) | Tìm kiếm (giữa vừa khít) | Nút Sáng/Tối (phải) =====
    let header_bar = gtk::Box::new(Orientation::Horizontal, 8);
    header_bar.add_css_class("header-bar");

    let logo = create_copyq_logo();
    logo.add_css_class("copyq-logo");
    logo.set_tooltip_text(Some("CopyQ"));

    let search_entry = SearchEntry::new();
    search_entry.add_css_class("search-input");
    search_entry.set_placeholder_text(Some("Tìm kiếm..."));
    search_entry.set_hexpand(true);
    search_entry.set_search_delay(60);

    let theme_btn = Button::new();
    theme_btn.add_css_class("theme-toggle-btn");
    theme_btn.set_focusable(false);

    header_bar.append(&logo);
    header_bar.append(&search_entry);
    header_bar.append(&theme_btn);

    // ===== 2. THANH LỌC (Tất cả / Đã ghim / Ảnh) =====
    let filter_bar = gtk::Box::new(Orientation::Horizontal, 4);
    filter_bar.add_css_class("filter-bar");

    let tab_all = make_filter_button("Tất cả");
    let tab_pinned = make_filter_button("Đã ghim");
    let tab_images = make_filter_button("Ảnh");

    filter_bar.append(&tab_all);
    filter_bar.append(&tab_pinned);
    filter_bar.append(&tab_images);

    // ===== 3. DANH SÁCH THẺ CỐ ĐỊNH KHUNG CHIỀU CAO =====
    let list_box = ListBox::new();
    list_box.add_css_class("clip-list");
    list_box.set_selection_mode(SelectionMode::Single);
    list_box.set_activate_on_single_click(true);

    let scrolled = ScrolledWindow::new();
    scrolled.add_css_class("clip-scroll");
    scrolled.set_hscrollbar_policy(PolicyType::Never);
    scrolled.set_vscrollbar_policy(PolicyType::Automatic);
    scrolled.set_vexpand(true);
    scrolled.set_hexpand(true);
    scrolled.set_propagate_natural_height(false);
    scrolled.set_propagate_natural_width(false);
    scrolled.set_child(Some(&list_box));

    // ===== 4. THANH GỢI Ý PHÍM TẮT =====
    let footer_bar = gtk::Box::new(Orientation::Horizontal, 8);
    footer_bar.add_css_class("footer-bar");

    let hint_left = Label::new(Some("↑↓ Chọn · Enter Dán · Shift+Enter Chữ thuần"));
    hint_left.set_xalign(0.0);
    hint_left.set_hexpand(true);

    let hint_right = Label::new(Some("Esc Đóng"));
    hint_right.set_xalign(1.0);

    footer_bar.append(&hint_left);
    footer_bar.append(&hint_right);

    let main_box = gtk::Box::new(Orientation::Vertical, 0);
    main_box.set_vexpand(true);
    main_box.set_hexpand(true);
    main_box.append(&header_bar);
    main_box.append(&filter_bar);
    main_box.append(&scrolled);
    main_box.append(&footer_bar);

    // ===== 5. LỚP PHỦ PHÓNG TO ẢNH (ZOOM OVERLAY) =====
    let (zoom_overlay, zoom_picture, zoom_close_btn, zoom_paste_btn) = build_zoom_overlay();

    let root_overlay = Overlay::new();
    root_overlay.set_vexpand(true);
    root_overlay.set_hexpand(true);
    root_overlay.set_child(Some(&main_box));
    root_overlay.add_overlay(&zoom_overlay);

    popup_container.append(&root_overlay);
    window.set_child(Some(&popup_container));

    let state = Rc::new(RefCell::new(AppState {
        config,
        items: Vec::new(),
        visible_indices: Vec::new(),
        filter: FilterTab::All,
        query: String::new(),
        error_message: None,
        zoomed_item_index: None,
    }));

    let handles = UiHandles {
        window: window.clone(),
        list_box: list_box.clone(),
        scrolled,
        search_entry: search_entry.clone(),
        theme_btn: theme_btn.clone(),
        tab_all: tab_all.clone(),
        tab_pinned: tab_pinned.clone(),
        tab_images: tab_images.clone(),
        zoom_overlay,
        zoom_picture,
        state,
        popover_open_count: Rc::new(Cell::new(0)),
    };

    handles.apply_theme();
    handles.update_filter_tabs();
    handles.reload_from_copyq();

    // Nút đổi Sáng / Tối
    {
        let ui = handles.clone();
        theme_btn.connect_clicked(move |_| {
            {
                let mut st = ui.state.borrow_mut();
                st.config.theme = st.config.theme.toggle();
                let _ = st.config.save();
            }
            ui.apply_theme();
        });
    }

    // Các tab lọc
    connect_filter_tab(&tab_all, FilterTab::All, handles.clone());
    connect_filter_tab(&tab_pinned, FilterTab::Pinned, handles.clone());
    connect_filter_tab(&tab_images, FilterTab::Images, handles.clone());

    // Tìm kiếm trực tiếp
    {
        let ui = handles.clone();
        search_entry.connect_search_changed(move |entry| {
            ui.state.borrow_mut().query = entry.text().to_string();
            ui.rebuild_list(0);
        });
    }

    // Click thẻ để dán ngay
    {
        let ui = handles.clone();
        list_box.connect_row_activated(move |_, row| {
            let idx = row.index();
            if idx < 0 {
                return;
            }
            let item_idx = {
                let st = ui.state.borrow();
                st.visible_indices.get(idx as usize).copied()
            };
            if let Some(item_idx) = item_idx {
                ui.paste_item(item_idx, false);
            }
        });
    }

    // Nút đóng / dán trên Zoom Overlay
    {
        let ui = handles.clone();
        zoom_close_btn.connect_clicked(move |_| ui.close_zoom());
    }
    {
        let ui = handles.clone();
        zoom_paste_btn.connect_clicked(move |_| {
            let zoomed = ui.state.borrow().zoomed_item_index;
            if let Some(item_idx) = zoomed {
                ui.paste_item(item_idx, false);
            }
        });
    }

    // Điều khiển bàn phím toàn cửa sổ (Capture phase)
    setup_keyboard_controller(&window, handles.clone());

    // Tự đóng khi click ra ngoài (có cơ chế bảo vệ để không đóng nhầm khi mở popover ⋯)
    setup_unfocus_autoclose(&window, handles.clone());

    window.present();
    search_entry.grab_focus();
}

impl UiHandles {
    fn apply_theme(&self) {
        let theme = self.state.borrow().config.theme;
        match theme {
            Theme::Dark => {
                self.window.remove_css_class("theme-light");
                self.window.add_css_class("theme-dark");
                self.theme_btn.set_icon_name("weather-clear-symbolic");
                self.theme_btn.set_tooltip_text(Some("Chuyển sang giao diện Sáng"));
            }
            Theme::Light => {
                self.window.remove_css_class("theme-dark");
                self.window.add_css_class("theme-light");
                self.theme_btn.set_icon_name("weather-clear-night-symbolic");
                self.theme_btn.set_tooltip_text(Some("Chuyển sang giao diện Tối"));
            }
        }
    }

    fn update_filter_tabs(&self) {
        let active = self.state.borrow().filter;
        set_tab_active(&self.tab_all, active == FilterTab::All);
        set_tab_active(&self.tab_pinned, active == FilterTab::Pinned);
        set_tab_active(&self.tab_images, active == FilterTab::Images);
    }

    fn reload_from_copyq(&self) {
        let max_items = self.state.borrow().config.max_items;
        match copyq::fetch_items(max_items) {
            Ok(raw_items) => {
                let ui_items: Vec<UiItem> = raw_items.into_iter().map(convert_item).collect();
                let mut st = self.state.borrow_mut();
                st.items = ui_items;
                st.error_message = None;
            }
            Err(err) => {
                let mut st = self.state.borrow_mut();
                st.items.clear();
                st.error_message = Some(err);
            }
        }
        self.rebuild_list(0);
    }

    fn rebuild_list(&self, preferred_visible_index: usize) {
        self.popover_open_count.set(0);

        let (visible, empty_text, image_fit) = {
            let mut st = self.state.borrow_mut();
            let query_lower = st.query.trim().to_lowercase();
            let filter = st.filter;

            let mut indices: Vec<usize> = st
                .items
                .iter()
                .enumerate()
                .filter(|(_, item)| {
                    if filter == FilterTab::Pinned && !item.pinned {
                        return false;
                    }
                    if filter == FilterTab::Images && !matches!(item.kind, UiItemKind::Image { .. })
                    {
                        return false;
                    }
                    if !query_lower.is_empty() {
                        match &item.kind {
                            UiItemKind::Text { search, .. } => search.contains(&query_lower),
                            UiItemKind::Image { .. } => {
                                query_lower == "ảnh" || query_lower == "anh" || query_lower == "image"
                            }
                        }
                    } else {
                        true
                    }
                })
                .map(|(idx, _)| idx)
                .collect();

            // Đưa các mục đã ghim lên đầu, giữ nguyên thứ tự mới nhất trong từng nhóm
            indices.sort_by_key(|&idx| !st.items[idx].pinned);
            st.visible_indices = indices.clone();

            let empty_msg = if let Some(err) = &st.error_message {
                err.clone()
            } else if !query_lower.is_empty() {
                "Không tìm thấy mục nào khớp".to_string()
            } else if filter == FilterTab::Pinned {
                "Chưa có mục nào được ghim".to_string()
            } else if filter == FilterTab::Images {
                "Chưa có ảnh nào trong lịch sử".to_string()
            } else {
                "Lịch sử CopyQ đang trống".to_string()
            };

            (indices, empty_msg, st.config.image_fit)
        };

        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }

        if visible.is_empty() {
            let row = ListBoxRow::new();
            row.set_selectable(false);
            row.set_activatable(false);

            let label = Label::new(Some(&empty_text));
            label.add_css_class("empty-label");
            label.set_wrap(true);
            label.set_justify(gtk::Justification::Center);
            label.set_halign(Align::Center);
            row.set_child(Some(&label));
            self.list_box.append(&row);
            return;
        }

        for &item_idx in &visible {
            let row_widget = self.build_card_row(item_idx, image_fit);
            self.list_box.append(&row_widget);
        }

        let target = preferred_visible_index.min(visible.len().saturating_sub(1));
        if let Some(row) = self.list_box.row_at_index(target as i32) {
            self.list_box.select_row(Some(&row));
            self.scroll_row_into_view(&row);
        }
    }

    fn build_card_row(&self, item_idx: usize, image_fit: ImageFit) -> ListBoxRow {
        let st = self.state.borrow();
        let item = &st.items[item_idx];
        let copyq_row = item.row;
        let is_pinned = item.pinned;
        let is_image = matches!(item.kind, UiItemKind::Image { .. });

        let list_row = ListBoxRow::new();
        list_row.add_css_class("clip-row");
        list_row.set_selectable(true);
        list_row.set_activatable(true);

        let card_box = gtk::Box::new(Orientation::Horizontal, 6);
        if is_image {
            card_box.add_css_class("clip-card-image");
        } else {
            card_box.add_css_class("clip-card");
            card_box.set_size_request(-1, 60);
        }

        match &item.kind {
            UiItemKind::Text { display, .. } => {
                let label = Label::new(Some(display));
                label.add_css_class("clip-text");
                label.set_use_markup(false);
                label.set_wrap(true);
                label.set_wrap_mode(pango::WrapMode::WordChar);
                label.set_lines(3);
                label.set_ellipsize(pango::EllipsizeMode::End);
                label.set_xalign(0.0);
                label.set_yalign(0.5);
                label.set_hexpand(true);
                card_box.append(&label);
            }
            UiItemKind::Image { texture } => {
                // Dùng Overlay + sizer chiều cao cố định 116px để ảnh mọi kích cỡ luôn hiển thị sắc nét và không làm vỡ khung thẻ
                let sizer = gtk::Box::new(Orientation::Horizontal, 0);
                sizer.add_css_class("img-frame");
                sizer.set_size_request(-1, 116);
                sizer.set_hexpand(true);

                let img_overlay = Overlay::new();
                img_overlay.add_css_class("img-frame");
                img_overlay.set_overflow(Overflow::Hidden);
                img_overlay.set_hexpand(true);
                img_overlay.set_child(Some(&sizer));

                if let Some(tex) = texture {
                    let pic = Picture::new();
                    pic.set_paintable(Some(tex));
                    pic.set_can_shrink(true);
                    pic.set_can_target(false);
                    pic.set_hexpand(true);
                    pic.set_vexpand(true);
                    pic.set_content_fit(match image_fit {
                        ImageFit::Contain => ContentFit::Contain,
                        ImageFit::Cover => ContentFit::Cover,
                    });
                    img_overlay.add_overlay(&pic);
                } else {
                    let fallback = Label::new(Some("[Không hiển thị được ảnh]"));
                    fallback.add_css_class("empty-label");
                    fallback.set_halign(Align::Center);
                    fallback.set_valign(Align::Center);
                    img_overlay.add_overlay(&fallback);
                }

                card_box.append(&img_overlay);
            }
        }

        // Cột nút xếp dọc trên-dưới (trên: ⋯ mở menu Delete, dưới: Ghim)
        let actions_col = gtk::Box::new(Orientation::Vertical, 4);
        actions_col.set_valign(Align::Fill);

        // Nút ⋯ (MenuButton + Popover)
        let popover = Popover::new();
        popover.add_css_class("card-popover");
        popover.set_has_arrow(false);
        popover.set_position(PositionType::Left);

        {
            let count = self.popover_open_count.clone();
            popover.connect_show(move |_| {
                count.set(count.get().saturating_add(1));
            });
        }
        {
            let count = self.popover_open_count.clone();
            popover.connect_closed(move |_| {
                count.set(count.get().saturating_sub(1));
            });
        }

        let pop_box = gtk::Box::new(Orientation::Vertical, 2);

        if is_image {
            let zoom_btn = make_popover_action("zoom-in-symbolic", "Phóng to", false);
            let ui = self.clone();
            let pop = popover.clone();
            zoom_btn.connect_clicked(move |_| {
                pop.popdown();
                ui.open_zoom(item_idx);
            });
            pop_box.append(&zoom_btn);
        } else {
            let plain_btn = make_popover_action("edit-paste-symbolic", "Dán chữ thuần", false);
            let ui = self.clone();
            let pop = popover.clone();
            plain_btn.connect_clicked(move |_| {
                pop.popdown();
                let ui_idle = ui.clone();
                glib::idle_add_local_once(move || {
                    ui_idle.paste_item(item_idx, true);
                });
            });
            pop_box.append(&plain_btn);
        }

        let delete_btn = make_popover_action("user-trash-symbolic", "Delete", true);
        {
            let ui = self.clone();
            let pop = popover.clone();
            delete_btn.connect_clicked(move |_| {
                pop.popdown();
                let ui_idle = ui.clone();
                glib::idle_add_local_once(move || {
                    if copyq::delete_item(copyq_row).is_ok() {
                        ui_idle.reload_from_copyq();
                    }
                });
            });
        }
        pop_box.append(&delete_btn);
        popover.set_child(Some(&pop_box));

        let menu_btn = MenuButton::new();
        menu_btn.add_css_class("card-menu-btn");
        menu_btn.set_icon_name("view-more-horizontal-symbolic");
        menu_btn.set_has_frame(false);
        menu_btn.set_always_show_arrow(false);
        menu_btn.set_tooltip_text(Some("Tùy chọn"));
        menu_btn.set_valign(Align::Start);
        menu_btn.set_focusable(false);
        menu_btn.set_popover(Some(&popover));

        // Spacer đẩy nút Ghim xuống góc dưới thẻ
        let spacer = gtk::Box::new(Orientation::Vertical, 0);
        spacer.set_vexpand(true);

        // Nút Ghim (nằm dưới nút ⋯)
        let pin_btn = Button::from_icon_name("view-pin-symbolic");
        pin_btn.add_css_class("card-icon-btn");
        if is_pinned {
            pin_btn.add_css_class("is-pinned");
        }
        pin_btn.set_has_frame(false);
        pin_btn.set_valign(Align::End);
        pin_btn.set_focusable(false);
        pin_btn.set_tooltip_text(Some(if is_pinned { "Bỏ ghim" } else { "Ghim lên đầu" }));
        {
            let ui = self.clone();
            pin_btn.connect_clicked(move |_| {
                let ui_idle = ui.clone();
                glib::idle_add_local_once(move || {
                    if copyq::toggle_pin(copyq_row).is_ok() {
                        ui_idle.reload_from_copyq();
                    }
                });
            });
        }

        actions_col.append(&menu_btn);
        actions_col.append(&spacer);
        actions_col.append(&pin_btn);

        card_box.append(&actions_col);
        list_row.set_child(Some(&card_box));
        list_row
    }

    fn selected_item_index(&self) -> Option<usize> {
        let row = self.list_box.selected_row()?;
        let vis_idx = usize::try_from(row.index()).ok()?;
        self.state.borrow().visible_indices.get(vis_idx).copied()
    }

    fn move_selection(&self, delta: isize) {
        let count = self.state.borrow().visible_indices.len();
        if count == 0 {
            return;
        }
        let current = self
            .list_box
            .selected_row()
            .and_then(|r| usize::try_from(r.index()).ok())
            .unwrap_or(0);
        let next = ((current as isize + delta).rem_euclid(count as isize)) as usize;
        if let Some(row) = self.list_box.row_at_index(next as i32) {
            self.list_box.select_row(Some(&row));
            self.scroll_row_into_view(&row);
        }
    }

    fn scroll_row_into_view(&self, row: &ListBoxRow) {
        let Some(bounds) = row.compute_bounds(&self.list_box) else {
            return;
        };
        let adj = self.scrolled.vadjustment();
        let top = f64::from(bounds.y());
        let bottom = top + f64::from(bounds.height());
        let view_top = adj.value();
        let view_bottom = view_top + adj.page_size();

        if top < view_top {
            adj.set_value(top);
        } else if bottom > view_bottom {
            adj.set_value((bottom - adj.page_size()).max(0.0));
        }
    }

    fn open_zoom(&self, item_idx: usize) {
        let texture = {
            let st = self.state.borrow();
            match st.items.get(item_idx) {
                Some(UiItem {
                    kind: UiItemKind::Image { texture: Some(tex) },
                    ..
                }) => Some(tex.clone()),
                _ => None,
            }
        };

        if let Some(tex) = texture {
            self.zoom_picture.set_paintable(Some(&tex));
            self.state.borrow_mut().zoomed_item_index = Some(item_idx);
            self.zoom_overlay.set_visible(true);
        }
    }

    fn close_zoom(&self) {
        self.state.borrow_mut().zoomed_item_index = None;
        self.zoom_overlay.set_visible(false);
        self.zoom_picture.set_paintable(None::<&gdk::Paintable>);
    }

    fn paste_item(&self, item_idx: usize, force_plain_text: bool) {
        let (copyq_row, auto_paste, plain_text) = {
            let st = self.state.borrow();
            let Some(item) = st.items.get(item_idx) else {
                return;
            };
            let is_text = matches!(item.kind, UiItemKind::Text { .. });
            let use_plain = is_text && (force_plain_text || st.config.plain_text_paste);
            (item.row, st.config.auto_paste, use_plain)
        };

        let _ = copyq::select_and_paste(copyq_row, auto_paste, plain_text);
        self.window.set_visible(false);
        self.window.close();
    }
}

fn convert_item(item: ClipboardItem) -> UiItem {
    let kind = match item.content {
        ItemContent::Text { display, search } => UiItemKind::Text { display, search },
        ItemContent::Image { bytes } => {
            let gbytes = glib::Bytes::from_owned(bytes);
            let texture = gdk::Texture::from_bytes(&gbytes).ok();
            UiItemKind::Image { texture }
        }
    };
    UiItem {
        row: item.row,
        pinned: item.pinned,
        kind,
    }
}

fn create_copyq_logo() -> Image {
    if let Some(display) = gdk::Display::default() {
        let theme = IconTheme::for_display(&display);
        if theme.has_icon("copyq") {
            let img = Image::from_icon_name("copyq");
            img.set_pixel_size(24);
            return img;
        }
    }

    const COPYQ_SVG: &str = "/usr/share/icons/hicolor/scalable/apps/copyq.svg";
    let img = if Path::new(COPYQ_SVG).is_file() {
        Image::from_file(COPYQ_SVG)
    } else {
        Image::from_icon_name("edit-paste-symbolic")
    };
    img.set_pixel_size(24);
    img
}

fn make_filter_button(label: &str) -> Button {
    let btn = Button::with_label(label);
    btn.add_css_class("filter-tab");
    btn.set_has_frame(false);
    btn.set_focusable(false);
    btn
}

fn set_tab_active(btn: &Button, active: bool) {
    if active {
        btn.add_css_class("is-active");
    } else {
        btn.remove_css_class("is-active");
    }
}

fn connect_filter_tab(btn: &Button, tab: FilterTab, ui: UiHandles) {
    btn.connect_clicked(move |_| {
        {
            let mut st = ui.state.borrow_mut();
            if st.filter == tab {
                return;
            }
            st.filter = tab;
        }
        ui.update_filter_tabs();
        ui.rebuild_list(0);
    });
}

fn make_popover_action(icon_name: &str, label_text: &str, danger: bool) -> Button {
    let content = gtk::Box::new(Orientation::Horizontal, 8);
    let icon = Image::from_icon_name(icon_name);
    icon.set_pixel_size(14);
    let label = Label::new(Some(label_text));
    label.set_xalign(0.0);
    content.append(&icon);
    content.append(&label);

    let btn = Button::new();
    btn.add_css_class("popover-action-btn");
    if danger {
        btn.add_css_class("danger-btn");
    }
    btn.set_has_frame(false);
    btn.set_child(Some(&content));
    btn
}

fn build_zoom_overlay() -> (gtk::Box, Picture, Button, Button) {
    let overlay_box = gtk::Box::new(Orientation::Vertical, 10);
    overlay_box.add_css_class("zoom-overlay");
    overlay_box.set_vexpand(true);
    overlay_box.set_hexpand(true);
    overlay_box.set_visible(false);

    let top_bar = gtk::Box::new(Orientation::Horizontal, 8);
    let title = Label::new(Some("Xem trước ảnh"));
    title.add_css_class("zoom-title");
    title.set_xalign(0.0);
    title.set_hexpand(true);

    let close_btn = Button::with_label("Đóng (Esc)");
    close_btn.add_css_class("zoom-close-btn");
    close_btn.set_has_frame(false);

    top_bar.append(&title);
    top_bar.append(&close_btn);

    let sizer = gtk::Box::new(Orientation::Vertical, 0);
    sizer.set_vexpand(true);
    sizer.set_hexpand(true);

    let pic = Picture::new();
    pic.set_can_shrink(true);
    pic.set_content_fit(ContentFit::Contain);
    pic.set_hexpand(true);
    pic.set_vexpand(true);

    let pic_holder = Overlay::new();
    pic_holder.add_css_class("zoom-frame");
    pic_holder.set_overflow(Overflow::Hidden);
    pic_holder.set_vexpand(true);
    pic_holder.set_hexpand(true);
    pic_holder.set_child(Some(&sizer));
    pic_holder.add_overlay(&pic);

    let paste_btn = Button::with_label("Dán ảnh này (Enter)");
    paste_btn.add_css_class("zoom-paste-btn");
    paste_btn.set_has_frame(false);

    overlay_box.append(&top_bar);
    overlay_box.append(&pic_holder);
    overlay_box.append(&paste_btn);

    (overlay_box, pic, close_btn, paste_btn)
}

fn setup_keyboard_controller(window: &ApplicationWindow, ui: UiHandles) {
    let key_ctrl = EventControllerKey::new();
    key_ctrl.set_propagation_phase(PropagationPhase::Capture);

    key_ctrl.connect_key_pressed(move |_, key, _keycode, modifiers| {
        let zoomed = ui.state.borrow().zoomed_item_index;
        if let Some(item_idx) = zoomed {
            match key {
                gdk::Key::Escape | gdk::Key::space => {
                    ui.close_zoom();
                    return glib::Propagation::Stop;
                }
                gdk::Key::Return | gdk::Key::KP_Enter => {
                    ui.paste_item(item_idx, false);
                    return glib::Propagation::Stop;
                }
                _ => return glib::Propagation::Stop,
            }
        }

        if modifiers.contains(gdk::ModifierType::CONTROL_MASK)
            && matches!(key, gdk::Key::f | gdk::Key::F)
        {
            ui.search_entry.grab_focus();
            return glib::Propagation::Stop;
        }

        match key {
            gdk::Key::Escape => {
                if ui.popover_open_count.get() > 0 {
                    return glib::Propagation::Proceed;
                }
                if !ui.search_entry.text().is_empty() {
                    ui.search_entry.set_text("");
                    ui.search_entry.grab_focus();
                } else {
                    ui.window.close();
                }
                glib::Propagation::Stop
            }
            gdk::Key::Down => {
                ui.move_selection(1);
                glib::Propagation::Stop
            }
            gdk::Key::Up => {
                ui.move_selection(-1);
                glib::Propagation::Stop
            }
            gdk::Key::Return | gdk::Key::KP_Enter => {
                if let Some(item_idx) = ui.selected_item_index() {
                    let force_plain = modifiers.contains(gdk::ModifierType::SHIFT_MASK);
                    ui.paste_item(item_idx, force_plain);
                }
                glib::Propagation::Stop
            }
            gdk::Key::space => {
                let ctrl = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
                let search_empty = ui.search_entry.text().is_empty();
                if ctrl || search_empty {
                    if let Some(item_idx) = ui.selected_item_index() {
                        let is_img = matches!(
                            ui.state.borrow().items.get(item_idx),
                            Some(UiItem {
                                kind: UiItemKind::Image { .. },
                                ..
                            })
                        );
                        if is_img {
                            ui.open_zoom(item_idx);
                            return glib::Propagation::Stop;
                        }
                    }
                }
                glib::Propagation::Proceed
            }
            _ => glib::Propagation::Proceed,
        }
    });

    window.add_controller(key_ctrl);
}

fn setup_unfocus_autoclose(window: &ApplicationWindow, ui: UiHandles) {
    let close_on_unfocus = ui.state.borrow().config.close_on_unfocus;
    if !close_on_unfocus {
        return;
    }

    let has_been_active = Rc::new(Cell::new(false));
    window.connect_is_active_notify(move |win| {
        if win.is_active() {
            has_been_active.set(true);
            return;
        }
        if !has_been_active.get() {
            return;
        }
        let win_weak = win.downgrade();
        let pop_count = ui.popover_open_count.clone();
        glib::timeout_add_local_once(Duration::from_millis(90), move || {
            if let Some(w) = win_weak.upgrade() {
                if !w.is_active() && pop_count.get() == 0 {
                    w.close();
                }
            }
        });
    });
}

fn load_stylesheet() {
    let css = AppConfig::load_or_create_css();
    let provider = CssProvider::new();
    provider.load_from_string(&css);

    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
