//! Parses the keybind snippet from README.md with Zellij 0.46's own config
//! parser, so the documented KDL is known to be valid.

use std::collections::BTreeMap;
use zellij_utils::data::{InputMode, KeyWithModifier};
use zellij_utils::input::actions::Action;
use zellij_utils::input::config::Config;

const WASM_URL: &str = "file:~/.config/zellij/plugins/zellij-scrollback-completion.wasm";

fn readme_kdl_snippet() -> String {
    let readme = include_str!("../README.md");
    let start = readme.find("```kdl\n").expect("kdl block in README") + "```kdl\n".len();
    let end = start + readme[start..].find("```").expect("end of kdl block");
    readme[start..end].to_string()
}

fn alt_semicolon_action(kdl: &str) -> Action {
    let config = Config::from_kdl(kdl, None).expect("config parses");
    let key: KeyWithModifier = "Alt ;".parse().unwrap();
    let actions = config
        .keybinds
        .get_actions_for_key_in_mode(&InputMode::Normal, &key)
        .expect("Alt ; bound in normal mode");
    // shared_except "locked": not bound in locked mode.
    assert!(config
        .keybinds
        .get_actions_for_key_in_mode(&InputMode::Locked, &key)
        .is_none());
    assert_eq!(actions.len(), 1);
    actions[0].clone()
}

#[test]
fn readme_snippet_parses_to_floating_message_plugin() {
    match alt_semicolon_action(&readme_kdl_snippet()) {
        Action::KeybindPipe {
            plugin,
            floating,
            configuration,
            ..
        } => {
            assert_eq!(plugin.as_deref(), Some(WASM_URL));
            assert_eq!(floating, Some(true));
            // `floating` is a reserved word, so it is not passed to the plugin.
            assert_eq!(configuration, None);
        }
        other => panic!("unexpected action {other:?}"),
    }
}

#[test]
fn readme_snippet_with_options_passes_them_as_strings() {
    // Uncomment the option lines, drop the prose comment.
    let kdl: String = readme_kdl_snippet()
        .lines()
        .filter(|l| !l.trim_start().starts_with("// Optional"))
        .map(|l| l.replacen("// ", "", 1) + "\n")
        .collect();
    match alt_semicolon_action(&kdl) {
        Action::KeybindPipe { configuration, .. } => {
            let expected: BTreeMap<String, String> = [
                ("separators", "()[]{}<>\"',;"),
                ("trim_trailing", ".:"),
                ("min_length", "3"),
                ("max_lines", "unlimited"),
                ("height", "10"),
                ("rejoin_wrapped", "true"),
            ]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
            let configuration = configuration.expect("configuration passed");
            assert_eq!(configuration, expected);
            let (parsed, warnings) =
                zellij_scrollback_completion::config::Config::from_map(&configuration);
            assert!(warnings.is_empty(), "{warnings:?}");
            assert_eq!(
                parsed,
                zellij_scrollback_completion::config::Config::default()
            );
        }
        other => panic!("unexpected action {other:?}"),
    }
}
