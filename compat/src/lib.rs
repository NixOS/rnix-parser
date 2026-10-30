//! Compares rnix's parser with Nix's, the reference implementation. See `README.md`.

mod check;
mod nix;
mod notation;
mod render;
mod strings;

pub use check::{Checker, Outcome};
pub use nix::Nix;
pub use notation::{escape, unescape};
pub use render::render;
