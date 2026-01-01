//! Command implementations.

mod build_version;
mod changed;
mod common;
mod compare;
mod current;
mod dev;
mod dioxus;
mod latest;
mod next;
mod post_bump_hook;
mod pre_bump_hook;
mod rust_toolchain;
mod tag;

// Re-export all command argument structs
pub use build_version::{
    BuildVersionArgs,
    build_version,
};
pub use changed::{
    ChangedArgs,
    changed,
};
pub use compare::{
    CompareArgs,
    compare,
};
pub use current::{
    CurrentArgs,
    current,
};
pub use dev::{
    DevArgs,
    dev,
};
pub use dioxus::{
    DioxusArgs,
    dioxus,
};
pub use latest::{
    LatestArgs,
    latest,
};
pub use next::NextArgs;
// Re-export all command functions
pub use next::next;
pub use post_bump_hook::{
    PostBumpHookArgs,
    post_bump_hook,
};
pub use pre_bump_hook::{
    PreBumpHookArgs,
    pre_bump_hook,
};
pub use rust_toolchain::{
    RustToolchainArgs,
    rust_toolchain,
};
pub use tag::{
    TagArgs,
    tag,
};
