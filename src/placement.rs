//! Popup placement math: put the popup directly below the cursor, or above it
//! when there is not enough room below, clamped to the screen area.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl Rect {
    pub fn new(x: usize, y: usize, width: usize, height: usize) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    pub fn right(&self) -> usize {
        self.x + self.width
    }

    pub fn bottom(&self) -> usize {
        self.y + self.height
    }

    /// Smallest rectangle containing both.
    pub fn union(&self, other: &Rect) -> Rect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Rect::new(
            x,
            y,
            self.right().max(other.right()) - x,
            self.bottom().max(other.bottom()) - y,
        )
    }
}

/// Computes the popup rectangle.
///
/// * `cursor` - absolute screen coordinates `(x, y)` of the origin cursor.
/// * `bounds` - the area floating panes may occupy (the tab's viewport).
/// * `want_width` / `want_height` - desired popup size (both >= 1).
///
/// The popup starts at the cursor column and on the row below the cursor. If
/// it does not fit below, it goes above the cursor row; if it fits neither
/// way, it takes the larger side and is shortened. Horizontally it is shifted
/// left as needed to stay on screen. The returned height may be smaller than
/// requested; the caller should render into whatever it gets.
pub fn place(cursor: (usize, usize), bounds: Rect, want_width: usize, want_height: usize) -> Rect {
    if bounds.width == 0 || bounds.height == 0 {
        return Rect::new(bounds.x, bounds.y, want_width.max(1), want_height.max(1));
    }
    let cx = cursor.0.clamp(bounds.x, bounds.right() - 1);
    let cy = cursor.1.clamp(bounds.y, bounds.bottom() - 1);
    let want_width = want_width.clamp(1, bounds.width);
    let want_height = want_height.max(1);

    let space_below = bounds.bottom() - (cy + 1);
    let space_above = cy - bounds.y;
    let (y, height) = if want_height <= space_below {
        (cy + 1, want_height)
    } else if want_height <= space_above {
        (cy - want_height, want_height)
    } else if space_below >= space_above && space_below > 0 {
        (cy + 1, space_below)
    } else if space_above > 0 {
        (bounds.y, space_above)
    } else {
        // A one-row screen: nothing to do but cover the cursor row.
        (cy, 1)
    };

    let x = if cx + want_width > bounds.right() {
        bounds.right() - want_width
    } else {
        cx
    };
    Rect::new(x, y, want_width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect {
        x: 0,
        y: 1,
        width: 80,
        height: 24,
    }; // rows 1..=24 (tab bar on row 0)

    #[test]
    fn below_cursor_when_room() {
        let r = place((10, 5), SCREEN, 20, 6);
        assert_eq!(r, Rect::new(10, 6, 20, 6));
    }

    #[test]
    fn exactly_fits_below() {
        // cursor on row 18; rows 19..=24 are 6 rows.
        let r = place((0, 18), SCREEN, 10, 6);
        assert_eq!(r, Rect::new(0, 19, 10, 6));
    }

    #[test]
    fn above_cursor_when_no_room_below() {
        let r = place((10, 22), SCREEN, 20, 6);
        assert_eq!(r, Rect::new(10, 16, 20, 6));
        assert_eq!(r.bottom(), 22); // ends right above the cursor row
    }

    #[test]
    fn shrinks_to_larger_side_when_fits_nowhere() {
        let small = Rect::new(0, 0, 80, 10);
        // cursor row 4: 5 rows below (5..=9), 4 above.
        let r = place((0, 4), small, 10, 8);
        assert_eq!(r, Rect::new(0, 5, 10, 5));
        // cursor row 6: 3 below, 6 above.
        let r = place((0, 6), small, 10, 8);
        assert_eq!(r, Rect::new(0, 0, 10, 6));
    }

    #[test]
    fn shifts_left_at_right_edge() {
        let r = place((75, 5), SCREEN, 20, 3);
        assert_eq!(r, Rect::new(60, 6, 20, 3));
    }

    #[test]
    fn width_capped_to_screen() {
        let r = place((5, 5), SCREEN, 200, 3);
        assert_eq!(r, Rect::new(0, 6, 80, 3));
    }

    #[test]
    fn respects_offset_bounds() {
        let b = Rect::new(10, 5, 30, 10); // x 10..40, y 5..15
        let r = place((38, 6), b, 8, 4);
        assert_eq!(r, Rect::new(32, 7, 8, 4));
    }

    #[test]
    fn cursor_outside_bounds_is_clamped() {
        let r = place((100, 0), SCREEN, 10, 3);
        assert_eq!(r, Rect::new(70, 2, 10, 3));
    }

    #[test]
    fn one_row_screen() {
        let b = Rect::new(0, 0, 40, 1);
        assert_eq!(place((3, 0), b, 10, 5), Rect::new(3, 0, 10, 1));
    }

    #[test]
    fn degenerate_bounds() {
        let b = Rect::new(0, 0, 0, 0);
        assert_eq!(place((3, 3), b, 10, 5), Rect::new(0, 0, 10, 5));
    }

    #[test]
    fn union() {
        let a = Rect::new(0, 1, 40, 10);
        let b = Rect::new(40, 1, 40, 23);
        assert_eq!(a.union(&b), Rect::new(0, 1, 80, 23));
    }
}
