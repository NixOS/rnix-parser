//! The inputs in `cases.md`. This needs a `nix-instantiate`; see `README.md`.

use std::{fmt::Write, thread};

use expect_test::expect_file;
use rnix_compat::{Checker, Nix, escape, par_map, unescape};

/// The inputs in `cases.md`, with the heading they're under.
fn inputs() -> Vec<(&'static str, String)> {
    let mut inputs = Vec::new();
    let mut heading = "";
    let mut in_block = false;
    for line in include_str!("../cases.md").lines() {
        if in_block {
            match line {
                "```" => in_block = false,
                "" => {}
                _ => inputs.push((heading, unescape(line).unwrap())),
            }
        } else if line == "```nix" {
            in_block = true;
        } else if let Some(text) = line.strip_prefix("### ") {
            heading = text;
        }
    }
    inputs
}

#[test]
fn cases() {
    let checker = Checker::new(Nix::new().unwrap()).with_values();
    let inputs = inputs();
    let jobs = thread::available_parallelism().map_or(1, |n| n.get());
    let outcomes = par_map(&inputs, jobs, |(_, input)| checker.check(input));

    let mut actual = format!("{}\n", checker.nix().version());
    let mut previous = None;
    for ((heading, input), outcome) in inputs.iter().zip(outcomes) {
        if previous != Some(heading) {
            write!(actual, "\n### {heading}\n").unwrap();
            previous = Some(heading);
        }
        actual.push_str(&outcome.describe(&escape(input)));
    }
    expect_file![concat!(env!("CARGO_MANIFEST_DIR"), "/cases.expect")].assert_eq(&actual);
}
