//! Cursor navigation between LSP diagnostics on `Editor`.
//!
//! `jump_to_next_error` and `jump_to_previous_error` walk the active
//! buffer's diagnostic overlays, find the nearest one in each direction,
//! and emit a MoveCursor event. Status messages report the diagnostic at
//! the new cursor position.

use fresh_i18n::t;

use crate::model::event::Event;

use super::Editor;

#[derive(Clone, Copy)]
enum Direction {
    Forward,
    Backward,
}

fn find_target_position(
    positions: &[usize],
    cursor_pos: usize,
    direction: Direction,
) -> Option<usize> {
    match direction {
        Direction::Forward => positions
            .iter()
            .find(|&&pos| pos > cursor_pos)
            .or_else(|| positions.first()) // Wrap around
            .copied(),
        Direction::Backward => positions
            .iter()
            .rev()
            .find(|&&pos| pos < cursor_pos)
            .or_else(|| positions.last()) // Wrap around
            .copied(),
    }
}

impl Editor {
    /// Jump to next error/diagnostic
    pub(super) fn jump_to_next_error(&mut self) {
        self.jump_to_diagnostic(Direction::Forward);
    }

    /// Jump to previous error/diagnostic
    pub(super) fn jump_to_previous_error(&mut self) {
        self.jump_to_diagnostic(Direction::Backward);
    }

    fn jump_to_diagnostic(&mut self, direction: Direction) {
        let cursor_pos = self.active_cursors().primary().position;
        let positions = self.sorted_diagnostic_positions();

        if positions.is_empty() {
            self.set_status_message(t!("diagnostics.none").to_string());
            return;
        }

        if let Some(new_pos) = find_target_position(&positions, cursor_pos, direction) {
            self.move_cursor_to_diagnostic(new_pos);
        }
    }

    fn sorted_diagnostic_positions(&mut self) -> Vec<usize> {
        let diagnostic_ns = self.active_window().lsp_diagnostic_namespace.clone();
        let state = self.active_state_mut();

        // Get all diagnostic overlay positions
        let mut positions: Vec<usize> = state
            .overlays
            .all()
            .iter()
            .filter_map(|overlay| {
                // Only consider LSP diagnostics (those in the diagnostic namespace)
                if overlay.namespace.as_ref() == Some(&diagnostic_ns) {
                    Some(overlay.range(&state.marker_list).start)
                } else {
                    None
                }
            })
            .collect();

        // Sort positions
        positions.sort_unstable();
        positions.dedup();
        positions
    }

    fn move_cursor_to_diagnostic(&mut self, new_pos: usize) {
        let diagnostic_ns = self.active_window().lsp_diagnostic_namespace.clone();
        let cursor_id = self.active_cursors().primary_id();
        let cursor = *self.active_cursors().primary();

        let event = Event::MoveCursor {
            cursor_id,
            old_position: cursor.position,
            new_position: new_pos,
            old_anchor: cursor.anchor,
            new_anchor: None,
            old_sticky_column: cursor.sticky_column,
            new_sticky_column: None,
        };
        self.active_event_log_mut().append(event.clone());
        self.apply_event_to_active_buffer(&event);
        // Diagnostics can be on any line; the viewport must scroll so the
        // user actually sees the error after pressing F8 (#1689).
        self.active_window_mut()
            .ensure_active_cursor_visible_for_navigation(true);

        // Show diagnostic message in status bar
        let state = self.active_state();
        if let Some(msg) = state.overlays.all().iter().find_map(|overlay| {
            let range = overlay.range(&state.marker_list);
            if range.start == new_pos && overlay.namespace.as_ref() == Some(&diagnostic_ns) {
                overlay.message.clone()
            } else {
                None
            }
        }) {
            self.set_status_message(msg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_finds_next_position() {
        assert_eq!(find_target_position(&[5, 10, 20], 10, Direction::Forward), Some(20));
    }

    #[test]
    fn forward_wraps_to_first() {
        assert_eq!(find_target_position(&[5, 10, 20], 20, Direction::Forward), Some(5));
    }

    #[test]
    fn backward_finds_previous_position() {
        assert_eq!(find_target_position(&[5, 10, 20], 10, Direction::Backward), Some(5));
    }

    #[test]
    fn backward_wraps_to_last() {
        assert_eq!(find_target_position(&[5, 10, 20], 5, Direction::Backward), Some(20));
    }

    #[test]
    fn empty_list_returns_none() {
        assert_eq!(find_target_position(&[], 0, Direction::Forward), None);
        assert_eq!(find_target_position(&[], 0, Direction::Backward), None);
    }
}