use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::PathBuf;

pub const DEFAULT_CSS: &str = include_str!("../assets/style.css");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    Dark,
    Light,
}

impl Theme {
    pub fn as_str(self) -> &'static str {
        match self {
            Theme::Dark => "dark",
            Theme::Light => "light",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "light" => Theme::Light,
            _ => Theme::Dark,
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            Theme::Dark => Theme::Light,
            Theme::Light => Theme::Dark,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageFit {
    Contain,
    Cover,
}

impl ImageFit {
    pub fn as_str(self) -> &'static str {
        match self {
            ImageFit::Contain => "contain",
            ImageFit::Cover => "cover",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "cover" => ImageFit::Cover,
            _ => ImageFit::Contain,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppConfig {
    pub theme: Theme,
    pub max_items: usize,
    pub width: i32,
    pub height: i32,
    pub auto_paste: bool,
    pub plain_text_paste: bool,
    pub image_fit: ImageFit,
    pub close_on_unfocus: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            max_items: 35,
            width: 372,
            height: 480,
            auto_paste: true,
            plain_text_paste: false,
            image_fit: ImageFit::Contain,
            close_on_unfocus: true,
        }
    }
}

impl AppConfig {
    pub fn config_dir() -> Option<PathBuf> {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            if !xdg.is_empty() {
                return Some(PathBuf::from(xdg).join("copyq-ui"));
            }
        }
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config").join("copyq-ui"))
    }

    pub fn load_or_create() -> Self {
        let Some(dir) = Self::config_dir() else {
            return Self::default();
        };
        let path = dir.join("config.conf");
        match fs::read_to_string(&path) {
            Ok(content) => Self::parse(&content),
            Err(_) => {
                let cfg = Self::default();
                let _ = cfg.save();
                cfg
            }
        }
    }

    pub fn parse(content: &str) -> Self {
        let mut cfg = Self::default();
        for raw_line in content.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, val)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let val = val.trim();
            match key {
                "theme" => cfg.theme = Theme::from_str(val),
                "max_items" => {
                    if let Ok(n) = val.parse::<usize>() {
                        cfg.max_items = n.clamp(5, 200);
                    }
                }
                "width" => {
                    if let Ok(n) = val.parse::<i32>() {
                        cfg.width = n.clamp(300, 720);
                    }
                }
                "height" => {
                    if let Ok(n) = val.parse::<i32>() {
                        cfg.height = n.clamp(340, 900);
                    }
                }
                "auto_paste" => cfg.auto_paste = parse_bool(val, cfg.auto_paste),
                "plain_text_paste" => cfg.plain_text_paste = parse_bool(val, cfg.plain_text_paste),
                "image_fit" => cfg.image_fit = ImageFit::from_str(val),
                "close_on_unfocus" => cfg.close_on_unfocus = parse_bool(val, cfg.close_on_unfocus),
                _ => {}
            }
        }
        cfg
    }

    pub fn serialize(&self) -> String {
        format!(
            "# Cấu hình CopyQ-UI (chỉ lưu tùy chọn giao diện cục bộ)\n\
             theme = {}\n\
             max_items = {}\n\
             width = {}\n\
             height = {}\n\
             auto_paste = {}\n\
             plain_text_paste = {}\n\
             image_fit = {}\n\
             close_on_unfocus = {}\n",
            self.theme.as_str(),
            self.max_items,
            self.width,
            self.height,
            self.auto_paste,
            self.plain_text_paste,
            self.image_fit.as_str(),
            self.close_on_unfocus,
        )
    }

    pub fn save(&self) -> std::io::Result<()> {
        let Some(dir) = Self::config_dir() else {
            return Ok(());
        };
        ensure_private_dir(&dir)?;
        let path = dir.join("config.conf");
        write_private_file(&path, self.serialize().as_bytes())
    }

    pub fn load_or_create_css() -> String {
        let Some(dir) = Self::config_dir() else {
            return DEFAULT_CSS.to_string();
        };
        let css_path = dir.join("style.css");
        if let Ok(existing) = fs::read_to_string(&css_path) {
            if !existing.trim().is_empty() {
                return existing;
            }
        }
        if ensure_private_dir(&dir).is_ok() {
            let _ = write_private_file(&css_path, DEFAULT_CSS.as_bytes());
        }
        DEFAULT_CSS.to_string()
    }
}

fn parse_bool(val: &str, default: bool) -> bool {
    match val.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => true,
        "false" | "0" | "no" | "off" => false,
        _ => default,
    }
}

fn ensure_private_dir(dir: &PathBuf) -> std::io::Result<()> {
    if !dir.exists() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
    }
    Ok(())
}

fn write_private_file(path: &PathBuf, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_roundtrip_config() {
        let input = "\
            # comment\n\
            theme = light\n\
            max_items = 50\n\
            width = 400\n\
            height = 520\n\
            auto_paste = false\n\
            plain_text_paste = true\n\
            image_fit = cover\n\
            close_on_unfocus = false\n\
        ";
        let cfg = AppConfig::parse(input);
        assert_eq!(cfg.theme, Theme::Light);
        assert_eq!(cfg.max_items, 50);
        assert_eq!(cfg.width, 400);
        assert_eq!(cfg.height, 520);
        assert!(!cfg.auto_paste);
        assert!(cfg.plain_text_paste);
        assert_eq!(cfg.image_fit, ImageFit::Cover);
        assert!(!cfg.close_on_unfocus);

        let serialized = cfg.serialize();
        assert_eq!(AppConfig::parse(&serialized), cfg);
    }

    #[test]
    fn clamps_extreme_values() {
        let cfg = AppConfig::parse("max_items = 99999\nwidth = 10\nheight = 9999\n");
        assert_eq!(cfg.max_items, 200);
        assert_eq!(cfg.width, 300);
        assert_eq!(cfg.height, 900);
    }
}
