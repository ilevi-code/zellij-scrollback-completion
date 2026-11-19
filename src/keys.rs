//! A host-independent view of a key press: just enough for the menu to
//! decide whether it consumes a key or lets it through to the shell.
//! Forwarding itself re-sends Zellij's original key, so no encoding lives here.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bare {
    Char(char),
    Enter,
    Tab,
    Backspace,
    Esc,
    Up,
    Down,
    /// Any key the menu has no special meaning for.
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyPress {
    pub bare: Bare,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl KeyPress {
    pub fn plain(bare: Bare) -> KeyPress {
        KeyPress {
            bare,
            ctrl: false,
            alt: false,
            shift: false,
        }
    }

    pub fn ctrl(c: char) -> KeyPress {
        KeyPress {
            ctrl: true,
            ..KeyPress::plain(Bare::Char(c))
        }
    }

    pub fn shift(bare: Bare) -> KeyPress {
        KeyPress {
            shift: true,
            ..KeyPress::plain(bare)
        }
    }

    pub fn no_mods(&self) -> bool {
        !self.ctrl && !self.alt && !self.shift
    }

    /// The character this key types into the filter, if it's a plain
    /// (optionally shifted) printable character.
    pub fn typed_char(&self) -> Option<char> {
        match self.bare {
            Bare::Char(c) if !self.ctrl && !self.alt && !c.is_control() => Some(if self.shift {
                c.to_uppercase().next().unwrap_or(c)
            } else {
                c
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_char_only_for_plain_printables() {
        assert_eq!(KeyPress::plain(Bare::Char('x')).typed_char(), Some('x'));
        assert_eq!(KeyPress::shift(Bare::Char('x')).typed_char(), Some('X'));
        assert_eq!(KeyPress::plain(Bare::Char('X')).typed_char(), Some('X'));
        assert_eq!(KeyPress::ctrl('x').typed_char(), None);
        assert_eq!(KeyPress::plain(Bare::Char('\u{1b}')).typed_char(), None);
        assert_eq!(KeyPress::plain(Bare::Enter).typed_char(), None);
    }
}
