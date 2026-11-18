//! Pure logic for the `zellij-scrollback-completion` plugin.
//!
//! Nothing in this library depends on `zellij-tile`; everything here is
//! unit-tested on the host target with plain `cargo test`. The Zellij glue
//! lives in `src/main.rs`.

pub mod candidates;
pub mod completion;
pub mod config;
pub mod menu;
pub mod placement;
pub mod ranking;
pub mod render;
pub mod scrollback;
