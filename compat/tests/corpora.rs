//! rnix's test inputs and Nix's language tests. These need a `nix-instantiate`, and the language
//! tests the source of the Nix release that rnix follows; see `README.md`.

use std::{env, path::Path, thread};

use expect_test::{ExpectFile, expect_file};
use rnix_compat::{Checker, Nix, nix_files, par_map};

/// Compares rnix's parse of every `.nix` file under `dir` with Nix's, and records those on which
/// the parsers disagree.
fn check_dir(dir: &Path, expected: ExpectFile) {
    let checker = Checker::new(Nix::new().unwrap()).with_strings();
    let files = nix_files(dir).unwrap();
    let jobs = thread::available_parallelism().map_or(1, |n| n.get());
    let outcomes = par_map(&files, jobs, |file| checker.check_file(file));

    let mut actual = format!("{}\n\n", checker.nix().version());
    for (file, outcome) in files.iter().zip(outcomes) {
        if !outcome.is_agreement() {
            actual.push_str(
                &outcome.describe(&file.strip_prefix(dir).unwrap().display().to_string()),
            );
        }
    }
    expected.assert_eq(&actual);
}

#[test]
fn test_data() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../test_data");
    check_dir(&dir, expect_file![concat!(env!("CARGO_MANIFEST_DIR"), "/test_data.expect")]);
}

#[test]
fn nix_language_tests() {
    let src = env::var_os("RNIX_COMPAT_NIX_SRC")
        .expect("RNIX_COMPAT_NIX_SRC should be the source of the Nix release rnix follows");
    check_dir(
        &Path::new(&src).join("tests/functional/lang"),
        expect_file![concat!(env!("CARGO_MANIFEST_DIR"), "/nix-lang.expect")],
    );
}
