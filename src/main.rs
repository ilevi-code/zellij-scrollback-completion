//! Zellij glue: events, permissions, pane I/O, positioning and rendering.
//!
//! All decisions are delegated to the pure modules in the library crate; this
//! file only talks to Zellij. On non-wasm targets the plugin entry points are
//! not registered (there is no Zellij host to link against), but the code is
//! still type-checked so `cargo check`/`clippy` cover it.
#![cfg_attr(not(target_family = "wasm"), allow(dead_code))]

use std::collections::BTreeMap;

use zellij_scrollback_completion::{
    candidates, completion,
    config::Config,
    keys::{Bare, KeyPress},
    menu::{Menu, Outcome},
    placement::{self, Rect},
    render, scrollback,
};
use zellij_tile::prelude::*;

#[derive(Default)]
struct State {
    config: Config,
    permissions_granted: bool,
    /// A trigger arrived before permissions were granted.
    pending_trigger: bool,
    own_plugin_id: u32,
    /// From `ModeUpdate`; needed to forward keys with `send_key_to_session_pane`.
    session_name: Option<String>,
    tabs: Vec<TabInfo>,
    manifest: Option<PaneManifest>,
    session: Option<Session>,
}

/// One open popup.
struct Session {
    origin: PaneId,
    /// Absolute screen coordinates where the menu is anchored: the start of
    /// the word being completed, on the cursor row.
    anchor: (usize, usize),
    /// The area the popup may occupy.
    bounds: Rect,
    menu: Menu,
    /// Last geometry sent to Zellij.
    rect: Option<Rect>,
}

#[cfg(target_family = "wasm")]
register_plugin!(State);

#[cfg(not(target_family = "wasm"))]
fn main() {
    eprintln!("zellij-scrollback-completion is a Zellij plugin; build it for wasm32-wasip1.");
}

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        let (config, warnings) = Config::from_map(&configuration);
        for warning in warnings {
            eprintln!("zellij-scrollback-completion: {warning}");
        }
        self.config = config;
        self.own_plugin_id = get_plugin_ids().plugin_id;
        request_permission(&[
            PermissionType::ReadPaneContents,
            PermissionType::WriteToStdin,
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
        ]);
        subscribe(&[
            EventType::Key,
            EventType::PermissionRequestResult,
            EventType::PaneUpdate,
            EventType::TabUpdate,
            EventType::ModeUpdate,
        ]);
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        if pipe_message.source != PipeSource::Keybind {
            return false;
        }
        if !self.permissions_granted {
            // Calling permission-gated commands now would get no reply.
            self.pending_trigger = true;
            return false;
        }
        self.trigger();
        true
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::PermissionRequestResult(PermissionStatus::Granted) => {
                self.permissions_granted = true;
                if std::mem::take(&mut self.pending_trigger) {
                    self.trigger();
                } else if self.session.is_none() {
                    hide_self();
                }
                true
            }
            Event::PermissionRequestResult(PermissionStatus::Denied) => {
                eprintln!(
                    "zellij-scrollback-completion: permissions denied; the plugin cannot work"
                );
                hide_self();
                false
            }
            Event::ModeUpdate(mode) => {
                self.session_name = mode.session_name;
                false
            }
            Event::TabUpdate(tabs) => {
                self.tabs = tabs;
                false
            }
            Event::PaneUpdate(manifest) => {
                self.manifest = Some(manifest);
                false
            }
            Event::Key(key) => self.handle_key(key),
            _ => false,
        }
    }

    fn render(&mut self, rows: usize, cols: usize) {
        match &self.session {
            Some(session) => {
                print!(
                    "{}",
                    render::render_lines(&session.menu, rows, cols).join("\n")
                );
            }
            None if !self.permissions_granted => {
                print!("scrollback-completion: grant permissions, then press the keybind again");
            }
            None => {}
        }
    }
}

impl State {
    fn own_pane_id(&self) -> PaneId {
        PaneId::Plugin(self.own_plugin_id)
    }

    /// Records the origin pane, harvests its scrollback and shows the popup.
    fn trigger(&mut self) {
        let Some((tab, origin)) = self.focused_pane() else {
            eprintln!("zellij-scrollback-completion: could not determine the focused pane");
            return;
        };
        if origin == self.own_pane_id() {
            // We are focused ourselves (e.g. the user just answered the
            // permission prompt); there is no origin pane to complete into.
            self.close();
            return;
        }
        let info = get_pane_info(origin).or_else(|| self.pane_info_from_manifest(origin));
        let contents = get_pane_scrollback(origin, true).unwrap_or_else(|e| {
            eprintln!("zellij-scrollback-completion: failed to read scrollback: {e}");
            PaneContents::default()
        });

        let content_columns = info.as_ref().map_or(0, |i| i.pane_content_columns);
        let rows = scrollback::ordered_rows(
            &contents.lines_above_viewport,
            &contents.viewport,
            &contents.lines_below_viewport,
        );
        let words = candidates::from_rows(rows, content_columns, &self.config);

        let cursor = placement::content_cursor(
            contents.cursor,
            info.as_ref().and_then(|i| i.cursor_coordinates_in_pane),
            info.as_ref().map_or((0, 0), |i| {
                (
                    i.pane_content_x.saturating_sub(i.pane_x),
                    i.pane_content_y.saturating_sub(i.pane_y),
                )
            }),
        );
        let word = cursor
            .map(|c| completion::cursor_word(&contents.viewport, c, content_columns, &self.config))
            .unwrap_or_default();
        let anchor = menu_anchor(info.as_ref(), cursor, word.start_col);
        let bounds = self.screen_bounds(tab, info.as_ref());
        self.session = Some(Session {
            origin,
            anchor,
            bounds,
            menu: Menu::with_prefix(words, self.config.height, &word.text),
            rect: None,
        });
        show_self(true);
        self.reposition();
    }

    /// The menu keeps focus (focusing a tiled pane would hide the floating
    /// layer), so keys the menu doesn't consume are forwarded to the origin
    /// pane: typing continues in the shell while the menu follows along.
    ///
    /// Forwarding goes through `send_key_to_session_pane` rather than raw
    /// bytes so Zellij encodes the key for the pane's input mode (application
    /// cursor keys, kitty keyboard protocol).
    fn handle_key(&mut self, key: KeyWithModifier) -> bool {
        let Some(session) = self.session.as_mut() else {
            return false;
        };
        let config = &self.config;
        let outcome = session
            .menu
            .handle_key(&map_key(&key), |c| config.is_separator(c));
        let origin = session.origin;
        let session_name = self.session_name.as_deref();
        let forward = || match session_name {
            Some(name) => send_key_to_session_pane(name, origin, key.clone()),
            None => {
                eprintln!("zellij-scrollback-completion: session name unknown, dropping {key:?}")
            }
        };
        match outcome {
            Outcome::Redraw => {
                self.reposition();
                true
            }
            Outcome::ForwardAndRedraw => {
                forward();
                self.reposition();
                true
            }
            Outcome::Ignore => false,
            Outcome::Insert(word) => {
                let text = completion::replacement(session.menu.filter(), &word);
                write_chars_to_pane_id(&text, origin);
                self.close();
                false
            }
            Outcome::Close => {
                self.close();
                false
            }
            Outcome::CloseAndForward => {
                forward();
                self.close();
                false
            }
        }
    }

    /// Hides the popup and returns focus to the origin pane.
    fn close(&mut self) {
        hide_self();
        if let Some(session) = self.session.take() {
            focus_pane_with_id(session.origin, false, false);
        }
    }

    /// Resizes/moves the popup to fit the current menu, if that changed.
    fn reposition(&mut self) {
        let own = self.own_pane_id();
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let (width, height) = render::desired_size(&session.menu);
        let rect = placement::place(session.anchor, session.bounds, width, height);
        session.menu.set_available_list_rows(rect.height);
        if session.rect == Some(rect) {
            return;
        }
        session.rect = Some(rect);
        let coordinates = FloatingPaneCoordinates::new(
            Some(rect.x.to_string()),
            Some(rect.y.to_string()),
            Some(rect.width.to_string()),
            Some(rect.height.to_string()),
            None,
            Some(true), // borderless: no frame, title or counter
        );
        if let Some(coordinates) = coordinates {
            change_floating_panes_coordinates(vec![(own, coordinates)]);
        }
    }

    /// The focused pane (and its tab), preferring the host query and falling
    /// back to the last PaneUpdate/TabUpdate.
    fn focused_pane(&self) -> Option<(Option<usize>, PaneId)> {
        match get_focused_pane_info() {
            Ok((tab, pane)) => Some((Some(tab), pane)),
            Err(e) => {
                eprintln!("zellij-scrollback-completion: get_focused_pane_info failed: {e}");
                self.focused_pane_from_manifest()
            }
        }
    }

    fn active_tab(&self) -> Option<&TabInfo> {
        self.tabs.iter().find(|t| t.active)
    }

    fn focused_pane_from_manifest(&self) -> Option<(Option<usize>, PaneId)> {
        let tab = self.active_tab()?;
        let panes = self.manifest.as_ref()?.panes.get(&tab.position)?;
        let pick = |floating: bool| {
            panes.iter().find(|p| {
                p.is_focused && !p.is_suppressed && !p.is_plugin && p.is_floating == floating
            })
        };
        let pane = if tab.are_floating_panes_visible {
            pick(true).or_else(|| pick(false))
        } else {
            pick(false)
        }?;
        Some((Some(tab.position), PaneId::Terminal(pane.id)))
    }

    fn pane_info_from_manifest(&self, pane_id: PaneId) -> Option<PaneInfo> {
        let (id, is_plugin) = match pane_id {
            PaneId::Terminal(id) => (id, false),
            PaneId::Plugin(id) => (id, true),
        };
        self.manifest
            .as_ref()?
            .panes
            .values()
            .flatten()
            .find(|p| p.id == id && p.is_plugin == is_plugin)
            .cloned()
    }

    /// The area floating panes can occupy: the union of the tab's visible,
    /// selectable tiled panes (this excludes the tab bar / status bar). Falls
    /// back to the origin pane itself.
    fn screen_bounds(&self, tab: Option<usize>, origin: Option<&PaneInfo>) -> Rect {
        let tab = tab.or_else(|| self.active_tab().map(|t| t.position));
        let tiled = tab
            .and_then(|t| self.manifest.as_ref()?.panes.get(&t))
            .into_iter()
            .flatten()
            .filter(|p| !p.is_floating && !p.is_suppressed && p.is_selectable)
            .map(pane_rect)
            .reduce(|a, b| a.union(&b));
        tiled.or_else(|| origin.map(pane_rect)).unwrap_or_default()
    }
}

fn pane_rect(p: &PaneInfo) -> Rect {
    Rect::new(p.pane_x, p.pane_y, p.pane_columns, p.pane_rows)
}

/// Absolute screen position the menu hangs from: the start of the word on
/// the cursor row. Without a known cursor, the start of the last content row.
fn menu_anchor(
    info: Option<&PaneInfo>,
    content_cursor: Option<(usize, usize)>,
    word_start_col: usize,
) -> (usize, usize) {
    let (x, y, rows) = info.map_or((0, 0, 1), |i| {
        (i.pane_content_x, i.pane_content_y, i.pane_content_rows)
    });
    match content_cursor {
        Some((_, row)) => (x + word_start_col, y + row),
        None => (x, y + rows.saturating_sub(1)),
    }
}

fn map_key(key: &KeyWithModifier) -> KeyPress {
    let mods = &key.key_modifiers;
    let bare = match key.bare_key {
        // Super chords aren't text; Zellij may forward them as CSI-u.
        BareKey::Char(_) if mods.contains(&KeyModifier::Super) => Bare::Other,
        BareKey::Char(c) => Bare::Char(c),
        BareKey::Enter => Bare::Enter,
        BareKey::Tab => Bare::Tab,
        BareKey::Backspace => Bare::Backspace,
        BareKey::Esc => Bare::Esc,
        BareKey::Up => Bare::Up,
        BareKey::Down => Bare::Down,
        _ => Bare::Other,
    };
    KeyPress {
        bare,
        ctrl: mods.contains(&KeyModifier::Ctrl),
        alt: mods.contains(&KeyModifier::Alt),
        shift: mods.contains(&KeyModifier::Shift),
    }
}
