//! SpiderFlayer core: everything the launcher does that isn't UI.
//!
//! Kept free of any GUI dependency so it can be built, tested and reused
//! (for example from a CLI) without a webview.

pub mod auth;
pub mod config;
pub mod instance;
pub mod java;
pub mod launch;
pub mod loaders;
pub mod modpack;
pub mod mods;
pub mod mojang;
pub mod util;
