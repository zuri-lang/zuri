//! Builds and serves the two books in `docs/`.
//!
//! Reached through the cargo aliases in `.cargo/config.toml`:
//!
//! ```text
//! cargo build-docs              render both into target/
//! cargo run-docs                render, serve on :3000, reload on edit
//! cargo run-docs -- reference   serve the library reference instead
//! cargo run-docs -- -p 4000     serve somewhere else
//! cargo clean-docs              throw the rendered output away
//! cargo docs help               this, from the command line
//! ```
//!
//! All of the actual rendering is mdBook's. This exists so that the
//! commands are the same on every machine and so a missing mdBook is a
//! prompt rather than a "command not found".

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};

/// One rendered book: where its source lives, where mdBook puts the
/// result, and what to call it on the command line.
struct Doc {
  /// What `cargo run-docs -- <name>` selects it by.
  name: &'static str,
  /// Where `book.toml` lives, relative to the crate root.
  dir: &'static str,
  /// What this book owns under `target/`: the rendered HTML, and for a
  /// generated book its pages too. `clean` removes the whole of it.
  out: &'static str,
  /// The Zuri program that writes this book's pages, for a book whose
  /// pages are generated rather than written. `None` for one that is
  /// kept in the repository as source.
  generator: Option<&'static str>,
}

/// Both books, in the order `build` and `clean` walk them. The first is
/// what `serve` picks when nothing is named.
const DOCS: [Doc; 2] = [
  Doc {
    name: "book",
    dir: "docs/book",
    out: "book",
    generator: None,
  },
  Doc {
    name: "reference",
    dir: "docs/reference",
    out: "reference",
    generator: Some("docs/tools/reference/generate.zu"),
  },
];

/// The book named on the command line, or the first one.
///
/// A selector is a bare word, so it never collides with `--port`; an
/// unknown one is an error rather than a silent fall back to the book,
/// which would quietly serve the wrong thing.
fn select(args: &[String]) -> Result<(&'static Doc, Vec<String>), String> {
  let mut chosen = &DOCS[0];
  let mut rest = Vec::new();

  for arg in args {
    if arg.starts_with('-') || rest.last().is_some_and(|last: &String| last == "-p" || last == "--port") {
      rest.push(arg.clone());
      continue;
    }

    match DOCS.iter().find(|doc| doc.name == arg) {
      Some(doc) => chosen = doc,
      None => {
        return Err(format!(
          "unknown book '{arg}'; expected {}",
          DOCS
            .iter()
            .map(|doc| doc.name)
            .collect::<Vec<_>>()
            .join(" or ")
        ));
      },
    }
  }

  Ok((chosen, rest))
}

fn main() -> ExitCode {
  let args: Vec<String> = std::env::args().skip(1).collect();
  let (command, rest) = match args.split_first() {
    Some((first, rest)) => (first.as_str(), rest),
    None => ("serve", &[] as &[String]),
  };

  let result = match command {
    "build" => build(rest),
    "serve" => serve(rest),
    "clean" => clean(),
    "generate" => generate_all(rest),
    "help" | "-h" | "--help" => {
      print_help();
      Ok(())
    },
    other => Err(format!(
      "unknown command '{other}'; expected build, serve, generate or clean"
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
    "Builds and serves the Zuri documentation.

  cargo build-docs              render both books into target/
  cargo run-docs                serve the book with live reload
  cargo run-docs -- reference   serve the library reference instead
  cargo run-docs -- -p 4000     serve on a different port
  cargo docs generate           write the generated pages, render nothing
  cargo clean-docs              remove the rendered output
  cargo docs <command>          any of build, serve, generate, clean, help

The book is markdown under docs/book/src and reads fine unrendered.

The standard library reference is not kept in the repository at all: it
is generated from the doc blocks in libs/ every time, so a page and the
doc block it came from can never disagree. Building or serving it
writes docs/reference/src first."
  );
}

fn build(args: &[String]) -> Result<(), String> {
  ensure_mdbook()?;

  // With no book named, both are rendered; naming one renders only it.
  let chosen: Vec<&Doc> = if args.is_empty() {
    DOCS.iter().collect()
  } else {
    vec![select(args)?.0]
  };

  for doc in chosen {
    generate(doc)?;

    println!("building {}", doc.name);
    run_mdbook(doc, &["build", &dir_string(doc)])?;
  }

  Ok(())
}

/// Writes a generated book's pages before mdBook is asked to render
/// them.
///
/// The reference is read out of `libs/` every time rather than kept in
/// the repository, so there is no chance of a committed page and the
/// doc block it came from disagreeing. That also means a fresh clone
/// has no pages at all until this runs.
///
/// A non-zero exit from the generator means it found a defect in a doc
/// block. It still wrote the pages, so this reports the problem and
/// carries on to render them: a stale book helps nobody, and the
/// generator has already said what is wrong.
fn generate(doc: &Doc) -> Result<(), String> {
  let Some(script) = doc.generator else {
    return Ok(());
  };

  let root = crate_root();
  let zuri = zuri_binary()?;

  println!("generating {} from libs/", doc.name);

  let status = Command::new(&zuri)
    .arg(root.join(script))
    .env("ZURI_ROOT", &root)
    .current_dir(&root)
    .status()
    .map_err(|e| format!("could not run {}: {e}", zuri.display()))?;

  if !status.success() {
    eprintln!("warning: the {} generator reported problems above", doc.name);
  }

  Ok(())
}

/// The `zuri` binary beside this one.
///
/// `cargo build-docs` builds and runs this tool, so its sibling in the
/// same profile directory is the interpreter that was built from the
/// same tree. Falling back to `PATH` would risk generating the
/// reference with a different version of the language than the one
/// being worked on.
fn zuri_binary() -> Result<PathBuf, String> {
  let exe = std::env::current_exe()
    .map_err(|e| format!("could not locate this executable: {e}"))?;

  let candidate = exe.with_file_name(if cfg!(windows) { "zuri.exe" } else { "zuri" });

  if candidate.is_file() {
    return Ok(candidate);
  }

  Err(format!(
    "no zuri binary at {}; run `cargo build` first",
    candidate.display()
  ))
}

fn serve(args: &[String]) -> Result<(), String> {
  ensure_mdbook()?;

  let (doc, rest) = select(args)?;
  let port = parse_port(&rest)?;

  generate(doc)?;

  println!("serving the {} at http://localhost:{port}", doc.name);
  println!("edit anything under {}/src and the page reloads", doc.dir);

  run_mdbook(
    doc,
    &["serve", &dir_string(doc), "--port", &port.to_string(), "--open"],
  )
}

/// `cargo docs generate`: write the generated pages without rendering
/// anything, for a check that does not need mdBook installed.
fn generate_all(args: &[String]) -> Result<(), String> {
  let chosen: Vec<&Doc> = if args.is_empty() {
    DOCS.iter().collect()
  } else {
    vec![select(args)?.0]
  };

  for doc in chosen {
    generate(doc)?;
  }

  Ok(())
}

fn clean() -> Result<(), String> {
  let mut removed = 0;

  for doc in &DOCS {
    let rendered = crate_root().join("target").join(doc.out);

    if !rendered.exists() {
      continue;
    }

    std::fs::remove_dir_all(&rendered)
      .map_err(|e| format!("could not remove {}: {e}", rendered.display()))?;

    println!("removed {}", rendered.display());
    removed += 1;
  }

  if removed == 0 {
    println!("nothing to clean");
  }

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

fn dir_string(doc: &Doc) -> String {
  crate_root().join(doc.dir).display().to_string()
}

fn run_mdbook(doc: &Doc, args: &[&str]) -> Result<(), String> {
  let manifest = crate_root().join(doc.dir).join("book.toml");
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

  println!("mdbook is not installed, and rendering needs it.");
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
      "mdbook is required to render either book; the markdown under docs/ reads fine as it is"
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
