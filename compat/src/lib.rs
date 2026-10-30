//! Compares rnix's parser with Nix's, the reference implementation. See `README.md`.

mod check;
mod corpus;
mod nix;
mod notation;
mod render;
mod strings;

pub use check::{Checker, Outcome};
pub use corpus::{nix_files, par_map};
pub use nix::Nix;
pub use notation::{escape, unescape};
pub use render::render;
