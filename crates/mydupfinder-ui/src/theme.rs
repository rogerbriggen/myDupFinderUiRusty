//! Light and dark colour palettes (§7). Follows `window.appearance()`.

use dupreport_core::Category;
use gpui::{Hsla, Window, WindowAppearance, rgb, rgba};

/// All colours used by the views.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// Window background.
    pub bg: Hsla,
    /// Default text.
    pub fg: Hsla,
    /// Secondary text (meta line, placeholder).
    pub muted_fg: Hsla,
    /// Toolbar / table header background.
    pub surface: Hsla,
    /// Borders and the row separators (`#eee` in light mode).
    pub border: Hsla,
    /// Row hover background.
    pub row_hover: Hsla,
    /// Error banner background.
    pub error_bg: Hsla,
    /// Error banner text.
    pub error_fg: Hsla,
    /// Splitter colour.
    pub splitter: Hsla,
    /// Splitter colour while hovered or dragged.
    pub splitter_hover: Hsla,
    /// Selected row background in the tree.
    pub row_selected: Hsla,
    /// Fill of an enabled filter chip (§7.3 `on` state).
    pub chip_on_bg: Hsla,
    /// Border of an enabled filter chip.
    pub chip_on_border: Hsla,
    /// Border of a disabled (off) filter chip.
    pub chip_off_border: Hsla,
    /// Identical-status dots: green, yellow, red.
    #[expect(dead_code, reason = "used by the status dots (step 11)")]
    pub status: [Hsla; 3],
    /// Category colours, indexed by `Category as usize`.
    category: [Hsla; 6],
}

impl Palette {
    /// Palette matching the window's current appearance.
    #[must_use]
    pub fn of(window: &Window) -> Self {
        Self::for_appearance(window.appearance())
    }

    /// Palette for an explicit appearance.
    #[must_use]
    pub fn for_appearance(appearance: WindowAppearance) -> Self {
        match appearance {
            WindowAppearance::Light | WindowAppearance::VibrantLight => Self::light(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::dark(),
        }
    }

    /// Colour of a category's text.
    #[must_use]
    pub fn category(&self, category: Category) -> Hsla {
        self.category[category as usize]
    }

    fn light() -> Self {
        Self {
            bg: rgb(0xffffff).into(),
            fg: rgb(0x1f2328).into(),
            muted_fg: rgb(0x6e7781).into(),
            surface: rgb(0xf6f8fa).into(),
            border: rgb(0xeeeeee).into(),
            row_hover: rgba(0x4080ff14).into(),
            error_bg: rgb(0xfdecea).into(),
            error_fg: rgb(0xb71c1c).into(),
            splitter: rgb(0xcccccc).into(),
            splitter_hover: rgb(0x6796e1).into(),
            row_selected: rgba(0x4080ff2e).into(),
            chip_on_bg: rgb(0xe8f0ff).into(),
            chip_on_border: rgb(0x6796e1).into(),
            chip_off_border: rgb(0xcccccc).into(),
            status: [
                rgb(0x2ea043).into(),
                rgb(0xd4a017).into(),
                rgb(0xcf222e).into(),
            ],
            category: [
                rgb(0x1565c0).into(), // Duplicate
                rgb(0x6a1b9a).into(), // Moved
                rgb(0x2e7d32).into(), // Unique
                rgb(0xc62828).into(), // Missing
                rgb(0xef6c00).into(), // New
                rgb(0xad1457).into(), // Changed
            ],
        }
    }

    fn dark() -> Self {
        Self {
            bg: rgb(0x1e1e1e).into(),
            fg: rgb(0xe6e6e6).into(),
            muted_fg: rgb(0x9aa0a6).into(),
            surface: rgb(0x2a2a2a).into(),
            border: rgb(0x383838).into(),
            row_hover: rgba(0x4080ff1f).into(),
            error_bg: rgb(0x4a1f1c).into(),
            error_fg: rgb(0xffb4ab).into(),
            splitter: rgb(0x444444).into(),
            splitter_hover: rgb(0x6796e1).into(),
            row_selected: rgba(0x4080ff47).into(),
            chip_on_bg: rgb(0x23324d).into(),
            chip_on_border: rgb(0x6796e1).into(),
            chip_off_border: rgb(0x555555).into(),
            status: [
                rgb(0x3fb950).into(),
                rgb(0xe3b341).into(),
                rgb(0xf85149).into(),
            ],
            category: [
                rgb(0x64b5f6).into(), // Duplicate
                rgb(0xce93d8).into(), // Moved
                rgb(0x81c784).into(), // Unique
                rgb(0xef9a9a).into(), // Missing
                rgb(0xffb74d).into(), // New
                rgb(0xf48fb1).into(), // Changed
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appearance_selects_palette() {
        let light = Palette::for_appearance(WindowAppearance::Light);
        let dark = Palette::for_appearance(WindowAppearance::Dark);
        assert_ne!(light.bg, dark.bg);
        assert_eq!(
            Palette::for_appearance(WindowAppearance::VibrantDark).bg,
            dark.bg
        );
    }

    #[test]
    fn every_category_has_a_colour() {
        let p = Palette::for_appearance(WindowAppearance::Light);
        for c in Category::ALL {
            assert_eq!(p.category(c), p.category[c as usize]);
        }
        assert_eq!(p.category(Category::Duplicate), Hsla::from(rgb(0x1565c0)));
    }
}
