//! Builds and serves the book in `docs/book`.
//!
//! Reached through the cargo aliases in `.cargo/config.toml`:
//!
//! ```text
//! cargo build-docs             render to target/book
//! cargo run-docs               render, serve on :3000, reload on edit
//! cargo run-docs -- -p 4000    serve somewhere else
//! cargo clean-docs             throw the rendered output away
//! cargo docs help              this, from the command line
//! ```
//!
//! All of the actual rendering is mdBook's. This exists so that the
//! commands are the same on every machine and so a missing mdBook is a
//! prompt rather than a "command not found".

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};

/// Where `book.toml` lives, relative to the crate root.
const BOOK_DIR: &str = "docs/book";

fn main() -> ExitCode {
  let args: Vec<String> = std::env::args().skip(1).collect();
  let (command, rest) = match args.split_first() {
    Some((first, rest)) => (first.as_str(), rest),
    None => ("serve", &[] as &[String]),
  };

  let result = match command {
    "build" => build(),
    "serve" => serve(rest),
    "clean" => clean(),
    "help" | "-h" | "--help" => {
      print_help();
      Ok(())
    },
    other => Err(format!(
      "unknown command '{other}'; expected build, serve or clean"
    )),
  };

  match result {
    Ok(()) => ExitCode::SUCCESS,
    Err(message) => {
      eprintln!("error: {message}");
      ExitCode::FAILURE
    },
  }
}

fn print_help() {
  println!(
    "Builds and serves the Zuri book.

  cargo build-docs              render the book into target/book
  cargo run-docs                render it and serve with live reload
  cargo run-docs -- -p 4000     serve on a different port
  cargo clean-docs              remove the rendered output
  cargo docs <command>          any of build, serve, clean, help

The book source is markdown under {BOOK_DIR}/src and reads fine without
any of this; these commands only exist to render it."
  );
}

fn build() -> Result<(), String> {
  ensure_mdbook()?;

  println!("building the book");
  run_mdbook(&["build", &book_dir_string()])
}

fn serve(args: &[String]) -> Result<(), String> {
  ensure_mdbook()?;

  let port = parse_port(args)?;

  println!("serving the book at http://localhost:{port}");
  println!("edit anything under {BOOK_DIR}/src and the page reloads");

  run_mdbook(&[
    "serve",
    &book_dir_string(),
    "--port",
    &port.to_string(),
    "--open",
  ])
}

fn clean() -> Result<(), String> {
  let rendered = crate_root().join("target").join("book");

  if !rendered.exists() {
    println!("nothing to clean");
    return Ok(());
  }

  std::fs::remove_dir_all(&rendered)
    .map_err(|e| format!("could not remove {}: {e}", rendered.display()))?;

  println!("removed {}", rendered.display());
  Ok(())
}

/// `--port N` or `-p N`, defaulting to mdBook's own 3000.
fn parse_port(args: &[String]) -> Result<u16, String> {
  let mut iter = args.iter();

  while let Some(arg) = iter.next() {
    if arg == "--port" || arg == "-p" {
      let value = iter
        .next()
        .ok_or_else(|| format!("{arg} needs a port number"))?;

      return value
        .parse()
        .map_err(|_| format!("'{value}' is not a port number"));
    }
  }

  Ok(3000)
}

fn crate_root() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn book_dir() -> PathBuf {
  crate_root().join(BOOK_DIR)
}

fn book_dir_string() -> String {
  book_dir().display().to_string()
}

fn run_mdbook(args: &[&str]) -> Result<(), String> {
  let manifest = book_dir().join("book.toml");
  if !manifest.is_file() {
    return Err(format!("no book.toml at {}", manifest.display()));
  }

  let status = Command::new("mdbook")
    .args(args)
    .current_dir(crate_root())
    .status()
    .map_err(|e| format!("could not run mdbook: {e}"))?;

  if status.success() {
    return Ok(());
  }

  Err(format!("mdbook exited with {status}"))
}

/// Confirms mdBook is available, offering to install it when it is not.
///
/// The offer is a real question with a default of no. Installing a
/// binary on someone's machine is not something a docs command should
/// decide on its own, and a developer who would rather run
/// `cargo install mdbook` themselves gets told exactly that.
fn ensure_mdbook() -> Result<(), String> {
  if mdbook_installed() {
    return Ok(());
  }

  println!("mdbook is not installed, and the book needs it to render.");
  println!();
  println!("  cargo install mdbook");
  println!();
  print!("Run that now? [y/N] ");
  io::stdout().flush().ok();

  let mut answer = String::new();
  io::stdin()
    .read_line(&mut answer)
    .map_err(|e| format!("could not read your answer: {e}"))?;

  if !matches!(answer.trim().to_lowercase().as_str(), "y" | "yes") {
    return Err(
      "mdbook is required to render the book; the markdown in docs/book/src reads fine as it is"
        .to_string(),
    );
  }

  install_mdbook()
}

fn mdbook_installed() -> bool {
  Command::new("mdbook")
    .arg("--version")
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status()
    .map(|status| status.success())
    .unwrap_or(false)
}

fn install_mdbook() -> Result<(), String> {
  let status = Command::new(cargo())
    .args(["install", "mdbook"])
    .status()
    .map_err(|e| format!("could not run cargo install: {e}"))?;

  if !status.success() {
    return Err(format!("cargo install mdbook exited with {status}"));
  }

  if !mdbook_installed() {
    return Err(
      "mdbook installed but is not on PATH; check that ~/.cargo/bin is in your PATH".to_string(),
    );
  }

  Ok(())
}

/// The cargo that invoked us, so an alias run under a specific toolchain
/// installs with that same toolchain rather than whatever `cargo`
/// happens to resolve to.
fn cargo() -> String {
  std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}
