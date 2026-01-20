// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

//! Scroll state management for virtual lists.
//!
//! Inspired by Zed's scroll management pattern where scroll state is
//! independent of content, allowing for:
//! - Preserving scroll position when switching conversations
//! - Automatic scroll-to-bottom behavior during streaming
//! - Smooth scroll following as new content arrives

use gpui::*;
use gpui_component::VirtualListScrollHandle;

/// Threshold for determining if the view is "near bottom"
const AUTO_SCROLL_THRESHOLD: Pixels = px(24.);
/// Epsilon for detecting scroll position changes
const SCROLL_CHANGE_EPSILON: f32 = 1.0;

/// Manages scroll state independently of content.
///
/// With the per-conversation Entity architecture (following Zed's pattern),
/// each MessageList Entity has its own ScrollManager. Scroll state is preserved
/// automatically because the Entity itself is cached.
pub struct ScrollManager {
    /// The underlying scroll handle for the virtual list
    scroll_handle: VirtualListScrollHandle,
    /// Whether to scroll to bottom on next render
    pending_scroll_to_bottom: bool,
    /// Whether the view should keep following the bottom
    stick_to_bottom: bool,
    /// Last known scroll offset for change detection
    last_scroll_offset: Pixels,
    /// Last known max offset for change detection
    last_max_offset: Pixels,
}

impl ScrollManager {
    pub fn new() -> Self {
        Self {
            scroll_handle: VirtualListScrollHandle::new(),
            pending_scroll_to_bottom: false,
            stick_to_bottom: true,
            last_scroll_offset: Pixels::ZERO,
            last_max_offset: Pixels::ZERO,
        }
    }

    /// Get a reference to the scroll handle for binding to virtual list
    pub fn handle(&self) -> &VirtualListScrollHandle {
        &self.scroll_handle
    }

    /// Check if pending scroll to bottom
    #[allow(dead_code)]
    pub fn is_pending_scroll_to_bottom(&self) -> bool {
        self.pending_scroll_to_bottom
    }

    /// Check if currently sticking to bottom
    #[allow(dead_code)]
    pub fn is_stick_to_bottom(&self) -> bool {
        self.stick_to_bottom
    }

    /// Request scroll to bottom on next render
    #[allow(dead_code)]
    pub fn scroll_to_bottom(&mut self) {
        self.pending_scroll_to_bottom = true;
        self.stick_to_bottom = true;
    }

    /// Request scroll to bottom if currently near bottom or sticking
    pub fn scroll_to_bottom_if_following(&mut self) {
        if self.stick_to_bottom || self.was_near_bottom() {
            self.pending_scroll_to_bottom = true;
        }
    }

    /// Reset scroll tracking (e.g., when starting a new chat)
    pub fn reset(&mut self) {
        self.last_scroll_offset = Pixels::ZERO;
        self.last_max_offset = Pixels::ZERO;
        self.stick_to_bottom = true;
        self.pending_scroll_to_bottom = true;
    }

    /// Check if the view is currently near the bottom
    pub fn is_near_bottom(&self) -> bool {
        let max_offset = self.scroll_handle.max_offset().height;
        if max_offset <= Pixels::ZERO {
            return true;
        }
        let offset = self.scroll_handle.offset().y;
        (offset + max_offset).abs() <= AUTO_SCROLL_THRESHOLD
    }

    /// Check if the view was near bottom based on last recorded state
    pub fn was_near_bottom(&self) -> bool {
        let max_offset = self.last_max_offset;
        if max_offset <= Pixels::ZERO {
            return true;
        }
        let offset = self.last_scroll_offset;
        (offset + max_offset).abs() <= AUTO_SCROLL_THRESHOLD
    }

    /// Update scroll follow state based on user interaction and content changes.
    ///
    /// Call this during render to update internal state.
    pub fn update_scroll_follow(&mut self, auto_scroll_enabled: bool) {
        let offset = self.scroll_handle.offset().y;
        let max_offset = self.scroll_handle.max_offset().height;
        let offset_delta = f32::from(offset) - f32::from(self.last_scroll_offset);
        let max_delta = (f32::from(max_offset) - f32::from(self.last_max_offset)).abs();
        let content_size_changed = max_delta > SCROLL_CHANGE_EPSILON;
        let user_scrolled_up = offset_delta > SCROLL_CHANGE_EPSILON && !content_size_changed;
        let user_scrolled_down = offset_delta < -SCROLL_CHANGE_EPSILON && !content_size_changed;

        if !auto_scroll_enabled {
            self.stick_to_bottom = false;
        } else if self.pending_scroll_to_bottom || (content_size_changed && self.was_near_bottom())
        {
            self.stick_to_bottom = true;
        } else if self.stick_to_bottom {
            if user_scrolled_up {
                self.stick_to_bottom = false;
            }
        } else if user_scrolled_down && self.is_near_bottom() {
            self.stick_to_bottom = true;
        }

        self.last_scroll_offset = offset;
        self.last_max_offset = max_offset;
    }

    /// Apply pending scroll operations. Call this during render.
    ///
    /// Returns true if scroll was applied.
    pub fn apply_pending_scroll(&mut self, auto_scroll_enabled: bool) -> bool {
        let should_scroll =
            auto_scroll_enabled && (self.stick_to_bottom || self.pending_scroll_to_bottom);

        if should_scroll {
            let max_offset = self.scroll_handle.max_offset().height;
            let current_x = self.scroll_handle.offset().x;
            let target_y = if max_offset > Pixels::ZERO {
                -max_offset
            } else {
                Pixels::ZERO
            };
            self.scroll_handle.set_offset(point(current_x, target_y));
        }

        self.pending_scroll_to_bottom = false;
        should_scroll
    }

    /// Get the scroll handle bounds (useful for width calculations)
    pub fn bounds(&self) -> Bounds<Pixels> {
        self.scroll_handle.bounds()
    }

    /// Get the current scroll offset
    #[allow(dead_code)]
    pub fn offset(&self) -> Point<Pixels> {
        self.scroll_handle.offset()
    }

    /// Get the max scroll offset
    #[allow(dead_code)]
    pub fn max_offset(&self) -> Size<Pixels> {
        self.scroll_handle.max_offset()
    }
}

impl Default for ScrollManager {
    fn default() -> Self {
        Self::new()
    }
}
