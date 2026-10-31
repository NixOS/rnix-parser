//! Nix's side of the comparison: `nix-instantiate`.

use std::{
    env,
    ffi::OsString,
    io::{self, Write},
    process::{Command, Stdio},
};

/// Nix looks variables up while it parses, so every input is parsed inside a `with`, which makes
/// free variables legal.
const PREFIX: &str = "with { };\n";

/// Moves the positions in an error back to where they are in the input, above the line that
/// [`PREFIX`] adds: `«stdin»:2:3` is `«stdin»:1:3`, like in Nix's own tests.
fn above_prefix(error: &str) -> String {
    const STDIN: &str = "«stdin»:";
    let mut out = String::new();
    let mut rest = error;
    while let Some(start) = rest.find(STDIN) {
        let after = &rest[start + STDIN.len()..];
        let digits = after.bytes().take_while(u8::is_ascii_digit).count();
        out.push_str(&rest[..start + STDIN.len()]);
        match after[..digits].parse::<usize>() {
            Ok(line) if line > 1 => out.push_str(&(line - 1).to_string()),
            _ => out.push_str(&after[..digits]),
        }
        rest = &after[digits..];
    }
    out.push_str(rest);
    out
}

/// A `nix-instantiate`: `$RNIX_COMPAT_NIX_INSTANTIATE`, or the one on `$PATH`.
pub struct Nix {
    program: OsString,
    version: String,
}

impl Nix {
    pub fn new() -> io::Result<Self> {
        let program =
            env::var_os("RNIX_COMPAT_NIX_INSTANTIATE").unwrap_or_else(|| "nix-instantiate".into());
        let output = Command::new(&program).arg("--version").output()?;
        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(Self { program, version })
    }

    /// What `nix-instantiate --version` says, e.g. `nix-instantiate (Nix) 2.35.3`.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Parses `source`, returning Nix's rendering of the expression (`nix-instantiate --parse`),
    /// or the first line of the error.
    pub fn parse(&self, source: &str) -> Result<String, String> {
        self.parse_bytes(source.as_bytes())
    }

    /// Like [`parse`](Self::parse), for input that may not be UTF-8: Nix reads bytes.
    pub fn parse_bytes(&self, source: &[u8]) -> Result<String, String> {
        let stdin = [PREFIX.as_bytes(), source].concat();
        let shown = self.run(&["--parse"], &stdin).map_err(|error| above_prefix(&error))?;
        let expr = shown.strip_prefix("(with { }; ").and_then(|s| s.strip_suffix(')'));
        Ok(expr.unwrap_or(&shown).to_string())
    }

    /// Evaluates `source` deeply, returning its value as JSON, or the first line of the error.
    pub fn eval_json(&self, source: &str) -> Result<String, String> {
        self.run(&["--eval", "--strict", "--json"], source.as_bytes())
    }

    fn run(&self, args: &[&str], stdin: &[u8]) -> Result<String, String> {
        let mut child = Command::new(&self.program)
            .args(args)
            .args(["--store", "dummy://", "-"])
            // Relative and home paths come out the same on every machine.
            .current_dir("/")
            .env("HOME", "/homeless-shelter")
            // No nix.conf: settings like `lint-url-literals` change what Nix accepts.
            .env("NIX_CONF_DIR", "/nonexistent")
            .env("NIX_USER_CONF_FILES", "")
            // rnix parses pipe operators, which Nix only does with this experimental feature.
            .env("NIX_CONFIG", "extra-experimental-features = pipe-operators")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("can't run {}: {e}", self.program.display()));
        // Nix reads all of its input before it writes anything. If it exits without reading it,
        // the exit status says why.
        let _ = child.stdin.take().unwrap().write_all(stdin);
        let output = child.wait_with_output().unwrap();
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim_end().to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let error = stderr.lines().map(str::trim).find(|line| line.starts_with("error:"));
            Err(error.unwrap_or(stderr.trim()).to_string())
        }
    }
}
