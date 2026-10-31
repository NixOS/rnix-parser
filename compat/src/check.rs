//! Comparing rnix's parse of one input with Nix's.

use std::{fmt::Write, fs, path::Path};

use crate::{
    Nix, escape, render,
    values::{self, Difference},
};

/// How rnix's parse of an input compares with Nix's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Both accept the input and parse it to the same expression.
    Same,
    /// Both reject the input.
    BothReject { nix: String },
    /// Nix accepts the input and rnix doesn't.
    RnixRejects { rnix: String },
    /// Both accept the input but parse it to different expressions. `rnix` is Nix's parse of
    /// rnix's [`render`]ing.
    TreeDiffers { nix: String, rnix: String },
    /// Both parse the input to the same expression, but a string literal in it has another
    /// value. The values are JSON.
    StringDiffers { literal: String, nix: String, rnix: String },
    /// Both parse the input to the same expression, but a number literal in it has another type
    /// or value. The types and values are JSON.
    NumberDiffers { literal: String, nix: String, rnix: String },
    /// Nix rejects rnix's rendering of input it accepts: a bug in [`render`].
    RenderRejected { nix: String, rendering: String },
    /// Nix can't evaluate the literals of input it accepts, with the interpolations in strings
    /// replaced: a bug in the comparison of values.
    ValuesRejected { nix: String },
    /// rnix accepts the input and Nix doesn't.
    RnixAccepts { nix: String },
    /// The input is a file that can't be read.
    Unreadable { error: String },
}

impl Outcome {
    /// The kind of outcome, for reports.
    pub fn kind(&self) -> &'static str {
        match self {
            Outcome::Same => "same",
            Outcome::BothReject { .. } => "both-reject",
            Outcome::RnixRejects { .. } => "rnix-rejects",
            Outcome::TreeDiffers { .. } => "tree-differs",
            Outcome::StringDiffers { .. } => "string-differs",
            Outcome::NumberDiffers { .. } => "number-differs",
            Outcome::RenderRejected { .. } => "render-rejected",
            Outcome::ValuesRejected { .. } => "values-rejected",
            Outcome::RnixAccepts { .. } => "rnix-accepts",
            Outcome::Unreadable { .. } => "unreadable",
        }
    }

    /// Whether both parsers accept the input and agree, or both reject it.
    pub fn is_agreement(&self) -> bool {
        matches!(self, Outcome::Same | Outcome::BothReject { .. })
    }

    /// Whether rnix parses input that Nix accepts differently from Nix (the first goal in
    /// `README.md`; rnix accepting input that Nix rejects is the second), compat has a bug, or a
    /// file couldn't be compared at all.
    pub fn is_failure(&self) -> bool {
        matches!(
            self,
            Outcome::RnixRejects { .. }
                | Outcome::TreeDiffers { .. }
                | Outcome::StringDiffers { .. }
                | Outcome::NumberDiffers { .. }
                | Outcome::RenderRejected { .. }
                | Outcome::ValuesRejected { .. }
                | Outcome::Unreadable { .. }
        )
    }

    fn details(&self) -> Vec<(&'static str, &str)> {
        match self {
            Outcome::Same => vec![],
            Outcome::BothReject { nix }
            | Outcome::RnixAccepts { nix }
            | Outcome::ValuesRejected { nix } => vec![("nix", nix)],
            Outcome::RnixRejects { rnix } => vec![("rnix", rnix)],
            Outcome::TreeDiffers { nix, rnix } => vec![("nix", nix), ("rnix", rnix)],
            Outcome::StringDiffers { literal, nix, rnix }
            | Outcome::NumberDiffers { literal, nix, rnix } => {
                vec![("literal", literal), ("nix", nix), ("rnix", rnix)]
            }
            Outcome::RenderRejected { nix, rendering } => {
                vec![("nix", nix), ("rendering", rendering)]
            }
            Outcome::Unreadable { error } => vec![("error", error)],
        }
    }

    /// The outcome for `input`, for reports: the kind and `input` on one line, then the details.
    pub fn describe(&self, input: &str) -> String {
        let mut out = format!("{:<16}{input}\n", self.kind());
        for (name, value) in self.details() {
            writeln!(out, "    {name}: {}", escape(value)).unwrap();
        }
        out
    }
}

/// Compares rnix's parses with a [`Nix`]'s.
pub struct Checker {
    nix: Nix,
    values: bool,
}

impl Checker {
    pub fn new(nix: Nix) -> Self {
        Self { nix, values: false }
    }

    /// Also compares the values of string and number literals, which takes one more evaluation
    /// by Nix.
    pub fn with_values(self) -> Self {
        Self { values: true, ..self }
    }

    pub fn nix(&self) -> &Nix {
        &self.nix
    }

    /// Compares rnix's parse of `source` with Nix's.
    pub fn check(&self, source: &str) -> Outcome {
        let parse = rnix::Root::parse(source);
        let rnix = parse.errors().first().map(ToString::to_string);
        match (self.nix.parse(source), rnix) {
            (Err(nix), Some(_)) => Outcome::BothReject { nix },
            (Err(nix), None) => Outcome::RnixAccepts { nix },
            (Ok(_), Some(rnix)) => Outcome::RnixRejects { rnix },
            (Ok(nix), None) => {
                let rendering = render(&parse.syntax());
                match self.nix.parse(&rendering) {
                    Err(nix) => Outcome::RenderRejected { nix, rendering },
                    Ok(rnix) if rnix != nix => Outcome::TreeDiffers { nix, rnix },
                    Ok(_) if !self.values => Outcome::Same,
                    Ok(_) => match values::compare(&self.nix, &parse.syntax()) {
                        Ok(Some(Difference { literal, number: false, nix, rnix })) => {
                            Outcome::StringDiffers { literal, nix, rnix }
                        }
                        Ok(Some(Difference { literal, number: true, nix, rnix })) => {
                            Outcome::NumberDiffers { literal, nix, rnix }
                        }
                        Ok(None) => Outcome::Same,
                        Err(nix) => Outcome::ValuesRejected { nix },
                    },
                }
            }
        }
    }

    /// Compares rnix's parse of the file at `path` with Nix's. rnix only parses UTF-8, so if Nix
    /// accepts a file that isn't, rnix rejects it.
    pub fn check_file(&self, path: &Path) -> Outcome {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) => return Outcome::Unreadable { error: error.to_string() },
        };
        match String::from_utf8(bytes) {
            Ok(source) => self.check(&source),
            Err(error) => match self.nix.parse_bytes(error.as_bytes()) {
                Ok(_) => {
                    Outcome::RnixRejects { rnix: format!("not UTF-8: {}", error.utf8_error()) }
                }
                Err(nix) => Outcome::BothReject { nix },
            },
        }
    }
}
