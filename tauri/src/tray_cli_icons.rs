//! Embedded PNG icons for tray menu sections tied to coding CLIs / tools.
//!
//! - `opencode.png`: from the app's OpenCode mark (`web/assets/opencode.svg`). In **dark** tray mode, RGB is inverted like other black glyphs.
//! - `claude.png` / `codex.png`: rasterized from [Simple Icons](https://simpleicons.org/) (CC0)
//!   — Anthropic and OpenAI marks; use is subject to each vendor's trademark guidelines.
//!   In **dark** tray mode (see [`TrayMenuIcons`]), RGB is inverted so black glyphs stay visible on dark menus.
//! - `openclaw.png`: app-original simple mark (no third-party logo).
//! - `omo.png`: Oh My OpenCode project mark; **dark** tray mode applies the same RGB inversion (brand colors will shift — preview and adjust assets if needed).
//! - `menu_spacer.png`: fully transparent 128×128 — same canvas as CLI marks so native menus reserve an identical icon column (macOS / Windows / GTK).
//! - `menu_radio_on.png` / `menu_radio_off.png`: ◉ / ◎ for the **root** tray list; same dark-mode inversion as Claude/Codex when needed.
//! - Popup submenus: selected rows reuse `menu_radio_on.png` (often with a transparent left half); unselected rows use `menu_spacer.png`.

use std::sync::OnceLock;

use tauri::image::Image;

macro_rules! cached_tray_png {
    ($name:ident, $relative_path:literal) => {
        pub fn $name() -> Option<Image<'static>> {
            static CACHE: OnceLock<Option<Image<'static>>> = OnceLock::new();
            CACHE
                .get_or_init(|| {
                    Image::from_bytes(include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/",
                        $relative_path
                    )))
                    .ok()
                })
                .clone()
        }
    };
}

cached_tray_png!(opencode, "icons/tray/cli/opencode.png");
cached_tray_png!(claude, "icons/tray/cli/claude.png");
cached_tray_png!(codex, "icons/tray/cli/codex.png");
cached_tray_png!(openclaw, "icons/tray/cli/openclaw.png");
cached_tray_png!(omo, "icons/tray/cli/omo.png");
cached_tray_png!(menu_spacer, "icons/tray/cli/menu_spacer.png");
cached_tray_png!(menu_radio_on, "icons/tray/cli/menu_radio_on.png");
cached_tray_png!(menu_radio_off, "icons/tray/cli/menu_radio_off.png");

const CLAUDE_PNG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/icons/tray/cli/claude.png"
));
const CODEX_PNG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/icons/tray/cli/codex.png"
));
const MENU_RADIO_ON_PNG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/icons/tray/cli/menu_radio_on.png"
));
const MENU_RADIO_OFF_PNG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/icons/tray/cli/menu_radio_off.png"
));
const OPENCODE_PNG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/icons/tray/cli/opencode.png"
));
const OMO_PNG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/icons/tray/cli/omo.png"
));

/// Matches app Settings theme: `dark` / `light` / `system` (OS detect via `dark-light`).
pub fn resolve_tray_menu_dark_mode(theme_setting: &str) -> bool {
    match theme_setting {
        "dark" => true,
        "light" => false,
        _ => matches!(dark_light::detect(), dark_light::Mode::Dark),
    }
}

fn png_inverted(bytes: &[u8]) -> Option<Image<'static>> {
    let img = Image::from_bytes(bytes).ok()?;
    let mut rgba = img.rgba().to_vec();
    for px in rgba.chunks_exact_mut(4) {
        if px[3] == 0 {
            continue;
        }
        px[0] = 255 - px[0];
        px[1] = 255 - px[1];
        px[2] = 255 - px[2];
    }
    Some(Image::new_owned(rgba, img.width(), img.height()))
}

#[inline]
fn themed_cached(
    dark: bool,
    bytes: &[u8],
    light: fn() -> Option<Image<'static>>,
) -> Option<Image<'static>> {
    if dark {
        png_inverted(bytes)
    } else {
        light()
    }
}

/// Theme-aware icons for the tray menu (rebuilt each [`crate::tray::refresh_tray_menus`] pass).
pub struct TrayMenuIcons {
    dark: bool,
}

impl TrayMenuIcons {
    pub fn new(theme_setting: &str) -> Self {
        Self {
            dark: resolve_tray_menu_dark_mode(theme_setting),
        }
    }

    #[inline]
    pub fn menu_spacer(&self) -> Option<Image<'static>> {
        menu_spacer()
    }

    pub fn claude(&self) -> Option<Image<'static>> {
        themed_cached(self.dark, CLAUDE_PNG, claude)
    }

    pub fn codex(&self) -> Option<Image<'static>> {
        themed_cached(self.dark, CODEX_PNG, codex)
    }

    pub fn opencode(&self) -> Option<Image<'static>> {
        themed_cached(self.dark, OPENCODE_PNG, opencode)
    }

    pub fn omo(&self) -> Option<Image<'static>> {
        themed_cached(self.dark, OMO_PNG, omo)
    }

    pub fn menu_radio_on(&self) -> Option<Image<'static>> {
        themed_cached(self.dark, MENU_RADIO_ON_PNG, menu_radio_on)
    }

    pub fn menu_radio_off(&self) -> Option<Image<'static>> {
        themed_cached(self.dark, MENU_RADIO_OFF_PNG, menu_radio_off)
    }

    #[inline]
    pub fn menu_radio_icon(&self, selected: bool) -> Option<Image<'static>> {
        if selected {
            self.menu_radio_on()
        } else {
            self.menu_radio_off()
        }
    }

    /// Popup submenu: ◉ when selected (same asset as root on), spacer when not.
    #[inline]
    pub fn menu_radio_icon_submenu(&self, selected: bool) -> Option<Image<'static>> {
        if selected {
            self.menu_radio_on()
        } else {
            menu_spacer()
        }
    }
}
