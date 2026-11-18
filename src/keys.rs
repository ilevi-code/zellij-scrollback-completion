//! A host-independent key press, and its encoding back into the bytes a
//! terminal would have sent, so keys the menu doesn't consume can be
//! forwarded to the origin pane.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bare {
    Char(char),
    Enter,
    Tab,
    Backspace,
    Esc,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    Delete,
    Insert,
    PageUp,
    PageDown,
    F(u8),
    /// Anything we can't represent; never forwarded.
    Unknown,
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

    /// The bytes a terminal (xterm conventions) sends for this key, or `None`
    /// if it can't be encoded.
    pub fn encode(&self) -> Option<Vec<u8>> {
        // xterm modifier parameter: 1 + shift + 2*alt + 4*ctrl.
        let modifier = 1 + self.shift as u8 + 2 * self.alt as u8 + 4 * self.ctrl as u8;
        let csi_letter = |letter: char| -> Vec<u8> {
            if modifier == 1 {
                format!("\x1b[{letter}").into_bytes()
            } else {
                format!("\x1b[1;{modifier}{letter}").into_bytes()
            }
        };
        let csi_tilde = |n: u8| -> Vec<u8> {
            if modifier == 1 {
                format!("\x1b[{n}~").into_bytes()
            } else {
                format!("\x1b[{n};{modifier}~").into_bytes()
            }
        };
        let alt_prefixed = |mut bytes: Vec<u8>| -> Vec<u8> {
            if self.alt {
                bytes.insert(0, 0x1b);
            }
            bytes
        };
        let bytes = match self.bare {
            Bare::Char(c) => {
                if self.ctrl {
                    alt_prefixed(vec![ctrl_byte(c)?])
                } else {
                    let c = if self.shift {
                        c.to_uppercase().next().unwrap_or(c)
                    } else {
                        c
                    };
                    alt_prefixed(c.to_string().into_bytes())
                }
            }
            Bare::Enter => alt_prefixed(b"\r".to_vec()),
            Bare::Tab if self.shift => b"\x1b[Z".to_vec(),
            Bare::Tab => alt_prefixed(b"\t".to_vec()),
            Bare::Backspace if self.ctrl => alt_prefixed(vec![0x08]),
            Bare::Backspace => alt_prefixed(vec![0x7f]),
            Bare::Esc => alt_prefixed(vec![0x1b]),
            Bare::Up => csi_letter('A'),
            Bare::Down => csi_letter('B'),
            Bare::Right => csi_letter('C'),
            Bare::Left => csi_letter('D'),
            Bare::Home => csi_letter('H'),
            Bare::End => csi_letter('F'),
            Bare::Insert => csi_tilde(2),
            Bare::Delete => csi_tilde(3),
            Bare::PageUp => csi_tilde(5),
            Bare::PageDown => csi_tilde(6),
            Bare::F(n @ 1..=4) => {
                let letter = (b'P' + n - 1) as char;
                if modifier == 1 {
                    format!("\x1bO{letter}").into_bytes()
                } else {
                    format!("\x1b[1;{modifier}{letter}").into_bytes()
                }
            }
            Bare::F(n) => {
                let code = match n {
                    5 => 15,
                    6 => 17,
                    7 => 18,
                    8 => 19,
                    9 => 20,
                    10 => 21,
                    11 => 23,
                    12 => 24,
                    _ => return None,
                };
                csi_tilde(code)
            }
            Bare::Unknown => return None,
        };
        Some(bytes)
    }
}

/// The control byte for Ctrl+`c`.
fn ctrl_byte(c: char) -> Option<u8> {
    let c = c.to_ascii_lowercase();
    match c {
        'a'..='z' => Some(c as u8 - b'a' + 1),
        ' ' | '@' | '2' => Some(0),
        '[' | '3' => Some(0x1b),
        '\\' | '4' => Some(0x1c),
        ']' | '5' => Some(0x1d),
        '^' | '6' => Some(0x1e),
        '_' | '/' | '7' | '-' => Some(0x1f),
        '?' | '8' => Some(0x7f),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enc(k: KeyPress) -> Vec<u8> {
        k.encode().unwrap()
    }

    #[test]
    fn printable_chars() {
        assert_eq!(enc(KeyPress::plain(Bare::Char('a'))), b"a");
        assert_eq!(enc(KeyPress::shift(Bare::Char('a'))), b"A");
        assert_eq!(enc(KeyPress::plain(Bare::Char('é'))), "é".as_bytes());
    }

    #[test]
    fn ctrl_and_alt() {
        assert_eq!(enc(KeyPress::ctrl('a')), vec![1]);
        assert_eq!(enc(KeyPress::ctrl('W')), vec![0x17]);
        assert_eq!(enc(KeyPress::ctrl(' ')), vec![0]);
        let alt_b = KeyPress {
            alt: true,
            ..KeyPress::plain(Bare::Char('b'))
        };
        assert_eq!(enc(alt_b), b"\x1bb");
        assert_eq!(KeyPress::ctrl('é').encode(), None);
    }

    #[test]
    fn special_keys() {
        assert_eq!(enc(KeyPress::plain(Bare::Enter)), b"\r");
        assert_eq!(enc(KeyPress::plain(Bare::Tab)), b"\t");
        assert_eq!(enc(KeyPress::shift(Bare::Tab)), b"\x1b[Z");
        assert_eq!(enc(KeyPress::plain(Bare::Backspace)), vec![0x7f]);
        assert_eq!(enc(KeyPress::plain(Bare::Esc)), vec![0x1b]);
        assert_eq!(enc(KeyPress::plain(Bare::Left)), b"\x1b[D");
        assert_eq!(enc(KeyPress::plain(Bare::Home)), b"\x1b[H");
        assert_eq!(enc(KeyPress::plain(Bare::Delete)), b"\x1b[3~");
        assert_eq!(enc(KeyPress::plain(Bare::F(1))), b"\x1bOP");
        assert_eq!(enc(KeyPress::plain(Bare::F(5))), b"\x1b[15~");
        assert_eq!(KeyPress::plain(Bare::Unknown).encode(), None);
    }

    #[test]
    fn modified_cursor_keys() {
        let ctrl_left = KeyPress {
            ctrl: true,
            ..KeyPress::plain(Bare::Left)
        };
        assert_eq!(enc(ctrl_left), b"\x1b[1;5D");
        let shift_del = KeyPress::shift(Bare::Delete);
        assert_eq!(enc(shift_del), b"\x1b[3;2~");
    }

    #[test]
    fn typed_char_only_for_plain_printables() {
        assert_eq!(KeyPress::plain(Bare::Char('x')).typed_char(), Some('x'));
        assert_eq!(KeyPress::shift(Bare::Char('x')).typed_char(), Some('X'));
        assert_eq!(KeyPress::ctrl('x').typed_char(), None);
        assert_eq!(KeyPress::plain(Bare::Enter).typed_char(), None);
    }
}
