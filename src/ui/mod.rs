//! OpenWave UI kit. Everything here is token-driven (see `tools/tailwind.css`), keyboard
//! operable, has a visible `:focus-visible` ring and exposes proper ARIA.
//!
//! Import what you need with `use crate::ui::*;` (or from the submodule paths below).
//!
//! | Component | Module | One-liner |
//! |---|---|---|
//! | `Icon` | `icon` | `Icon { name: "play", size: 20 }` (Lucide; drop SVGs in `assets/icons`) |
//! | `Button` | `button` | `Button { variant: ButtonVariant::Primary, icon: "plus", onclick, "Ekle" }` |
//! | `IconButton` | `button` | `IconButton { icon: "heart", label: "Beğen", shortcut: "Ctrl+L", pressed: Some(liked) }` |
//! | `Tooltip` | `tooltip` | `Tooltip { label: "Sıra", shortcut: "Q", { child } }` |
//! | `Kbd`, `Shortcut` | `kbd` | `Shortcut { keys: "Ctrl+K" }` |
//! | `Dialog` | `dialog` | `Dialog { open, on_close, title, footer: Some(rsx!{..}), {body} }` |
//! | `DropdownMenu`, `ContextMenu`, `MenuItem`, `MenuSeparator` | `menu` | see module docs |
//! | `Tabs`, `TabPanel`, `SegmentedControl`, `TabItem` | `tabs` | `Tabs { items, value, on_change, aria_label }` |
//! | `Slider` | `slider` | `Slider { value, max, step, big_step, label, on_input, on_commit }` |
//! | `Switch`, `SwitchRow` | `switch` | `SwitchRow { title, description, checked, on_change }` |
//! | `ToastHost`, `toast()` | `toast` | `toast("Kaydedildi")`, `toast_error("...")` |
//! | `TextInput`, `SearchInput`, `Select` | `input` | `SearchInput { value, oninput, aria_label }` |
//! | `Skeleton`, `Spinner`, `ProgressBar`, `EmptyState`, `Banner` | `feedback` | `EmptyState { icon, title, text, {actions} }` |
//! | `Cover`, `MediaCard` | `media` | `MediaCard { title, subtitle, cover, play_label, on_open, on_play }` |
//! | `GradientPlayButton`, `HeroBanner`, `CoverThumb` | `hero` | `HeroBanner { title, meta, cover, actions: Some(rsx!{..}) }` |
//!
//! Theme state lives in `theme` (`THEME`, `ACCENT`, `LOW_POWER`); the JS bridge in `js`.

pub mod button;
pub mod cls;
pub mod dialog;
pub mod emblem;
pub mod feedback;
pub mod hero;
pub mod icon;
pub mod input;
pub mod js;
pub mod kbd;
pub mod media;
pub mod menu;
pub mod morph;
pub mod slider;
pub mod switch;
pub mod tabs;
pub mod theme;
pub mod toast;
pub mod tooltip;

pub use button::{Button, ButtonSize, ButtonVariant, IconButton, IconButtonSize};
pub use cls::{cx, next_id};
pub use dialog::{Dialog, DialogSize};
pub use emblem::Emblem;
pub use feedback::{Banner, BannerKind, EmptyState, ProgressBar, Skeleton, Spinner};
pub use hero::{CoverThumb, GradientPlayButton, PlaySize};
pub use icon::Icon;
pub use input::{SearchInput, Select, SelectOption, TextInput};
pub use kbd::Kbd;
pub use media::{Cover, CoverShape, MediaCard};
pub use menu::{ContextMenu, DropdownMenu, MenuAlign, MenuItem, MenuSeparator};
pub use slider::Slider;
pub use switch::Switch;
pub use tabs::{SegmentedControl, TabItem, TabPanel, Tabs};
pub use theme::{Accent, Theme, ACCENT, LOW_POWER, THEME};
pub use toast::{toast, toast_error, toast_success, ToastHost};
pub use tooltip::{Tooltip, TooltipAlign, TooltipSide};
