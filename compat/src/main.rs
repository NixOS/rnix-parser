//! `rnix-compat`: compares rnix's parse of Nix files with Nix's. See `README.md`.

use std::{collections::BTreeMap, env, fs, path::PathBuf, process::ExitCode, thread};

use rnix_compat::{Checker, Nix, escape, nix_files, par_map};

const USAGE: &str = "\
usage: rnix-compat [--values] [--jobs N] [--report FILE] [-e EXPR]... [PATH]...

Compares rnix's parse of every .nix file under each PATH, and of each EXPR,
with Nix's. Prints every input with an outcome other than `same` or
`both-reject` (to FILE, with --report), then how often each outcome occurred.
Exits with status 1 if rnix parses any input that Nix accepts differently,
if compat itself fails on one (render-rejected, values-rejected), or if a file
can't be read.

  --values  also compare the values of string and number literals
  --jobs N  compare N inputs at a time (default: one per CPU)
";

enum Input {
    Expr(String),
    File(PathBuf),
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(message) => {
            eprintln!("rnix-compat: {message}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let mut values = false;
    let mut jobs = thread::available_parallelism().map_or(1, |n| n.get());
    let mut report = None;
    let mut inputs = Vec::new();
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{arg} needs a value\n\n{USAGE}"));
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(ExitCode::SUCCESS);
            }
            "--values" => values = true,
            "--jobs" => jobs = value()?.parse().map_err(|e| format!("--jobs: {e}"))?,
            "--report" => report = Some(PathBuf::from(value()?)),
            "-e" => inputs.push(Input::Expr(value()?)),
            _ if arg.starts_with('-') => return Err(format!("unknown option {arg}\n\n{USAGE}")),
            _ => {
                let files = nix_files(arg.as_ref()).map_err(|e| format!("{arg}: {e}"))?;
                inputs.extend(files.into_iter().map(Input::File));
            }
        }
    }
    if inputs.is_empty() {
        return Err(format!("nothing to compare\n\n{USAGE}"));
    }

    let nix = Nix::new().map_err(|e| format!("can't run nix-instantiate: {e}"))?;
    eprintln!("comparing {} inputs with {}", inputs.len(), nix.version());
    let checker = if values { Checker::new(nix).with_values() } else { Checker::new(nix) };
    let results = par_map(&inputs, jobs, |input| match input {
        Input::Expr(source) => (escape(source), checker.check(source)),
        Input::File(path) => (path.display().to_string(), checker.check_file(path)),
    });

    let mut counts = BTreeMap::new();
    let mut listing = String::new();
    for (label, outcome) in &results {
        *counts.entry(outcome.kind()).or_insert(0) += 1;
        if !outcome.is_agreement() {
            listing.push_str(&outcome.describe(label));
        }
    }
    match report {
        Some(path) => fs::write(&path, listing).map_err(|e| format!("{}: {e}", path.display()))?,
        None => print!("{listing}"),
    }
    for (kind, count) in counts {
        println!("{count:>8} {kind}");
    }

    let failed = results.iter().any(|(_, outcome)| outcome.is_failure());
    Ok(if failed { ExitCode::FAILURE } else { ExitCode::SUCCESS })
}
