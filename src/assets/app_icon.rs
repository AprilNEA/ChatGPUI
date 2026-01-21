use strum::IntoStaticStr;

use gpui::{AnyElement, IntoElement, Pixels, SharedString, px};
use gpui_component::{Icon, IconNamed, Sizable, Size};

/// App-specific icons using SVG rendering.
///
/// Each icon variant maps to an SVG file in `assets/icons/ui/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, IntoStaticStr)]
#[strum(serialize_all = "kebab-case")]
pub enum AppIcon {
    // Navigation & Actions
    ArrowRight,
    ArrowLeft,
    ArrowUp,
    ArrowDown,
    ChevronDown,
    ChevronRight,
    ChevronUp,
    ChevronLeft,

    // Common Actions
    Plus,
    Minus,
    Close,
    Check,
    Search,
    Copy,
    Delete,
    Settings,
    Settings2,

    // Panels & Layout
    PanelLeft,
    PanelRight,
    PanelBottom,

    // Content & Files
    File,
    Folder,
    BookOpen,
    Globe,
    ExternalLink,

    // UI States
    Eye,
    EyeOff,
    Star,
    StarOff,
    Loader,
    CircleX,
    CircleCheck,
    Info,
    TriangleAlert,

    // Theme
    Sun,
    Moon,
    Palette,

    // Communication & AI
    Bot,
    Brain,
    User,
    Message,

    // Developer
    SquareTerminal,
    Frame,

    // Custom/App-specific
    Mcp,
}

impl AppIcon {
    /// Create an icon element with the specified size.
    ///
    /// # Size Mapping
    ///
    /// | Size | Icon Pixels |
    /// |------|-------------|
    /// | XSmall | 16px |
    /// | Small | 18px |
    /// | Medium | 20px |
    /// | Large | 22px |
    /// | Size(v) | v |
    pub fn with_size(self, size: impl Into<Size>) -> AppIconElement {
        let size = size.into();
        let px = match size {
            Size::XSmall => px(16.),
            Size::Small => px(18.),
            Size::Medium => px(20.),
            Size::Large => px(22.),
            Size::Size(v) => v,
        };
        AppIconElement { icon: self, px }
    }

    /// Returns the SVG icon path in the assets bundle.
    fn svg_path(self) -> String {
        format!("icons/ui/{}.svg", <&'static str>::from(self))
    }
}

impl IconNamed for AppIcon {
    fn path(self) -> SharedString {
        self.svg_path().into()
    }
}

impl IntoElement for AppIcon {
    type Element = AnyElement;

    fn into_element(self) -> Self::Element {
        self.with_size(px(16.)).into_any_element()
    }
}

// =============================================================================
// Icon Element
// =============================================================================

/// A sized AppIcon element.
#[derive(Clone, Copy)]
pub struct AppIconElement {
    icon: AppIcon,
    px: Pixels,
}

impl IntoElement for AppIconElement {
    type Element = AnyElement;

    fn into_element(self) -> Self::Element {
        Icon::new(self.icon).with_size(self.px).into_any_element()
    }
}

// =============================================================================
// Button Extension
// =============================================================================

/// Extension trait for Button to use AppIcon.
pub trait ButtonAppIconExt {
    /// Set an icon for icon-only buttons.
    fn app_icon(self, icon: AppIcon) -> Self;

    /// Set an icon with a specific size.
    fn app_icon_with_size(self, icon: AppIcon, size: Size) -> Self
    where
        Self: Sized;
}

impl ButtonAppIconExt for gpui_component::button::Button {
    fn app_icon(self, icon: AppIcon) -> Self {
        self.app_icon_with_size(icon, Size::Medium)
    }

    fn app_icon_with_size(self, icon: AppIcon, size: Size) -> Self {
        self.icon(Icon::new(icon)).with_size(size)
    }
}
