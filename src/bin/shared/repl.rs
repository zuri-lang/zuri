use std::{borrow::Cow, env, io, path::Path};

use nu_ansi_term::{Color, Style};
use reedline::{
  ColumnarMenu, DefaultCompleter, DefaultHinter, DefaultValidator, Emacs, FileBackedHistory,
  KeyCode, KeyModifiers, MenuBuilder, Prompt, PromptEditMode, PromptHistorySearch,
  PromptHistorySearchStatus, Reedline, ReedlineEvent, ReedlineMenu, Signal, ValidationResult,
  Validator, default_emacs_keybindings,
};
use zuri::vm::vm::VM;

use crate::shared::highlighter::ZuriHighlighter;

/// How many reads in a row may fail before a session gives up. A read
/// fails when the terminal takes over two seconds to answer reedline's
/// cursor query, which a stalled connection can do and then recover
/// from, so this rides out about a minute of that. A terminal that is
/// gone fails at once, so it reaches the limit at once too.
const MAX_READ_FAILURES: u32 = 30;

pub struct ReplPrompt;

impl Prompt for ReplPrompt {
  fn render_prompt_left(&self) -> Cow<'_, str> {
    "".into()
  }

  fn render_prompt_right(&self) -> Cow<'_, str> {
    "".into()
  }

  fn render_prompt_indicator(&self, _: PromptEditMode) -> Cow<'_, str> {
    "%> ".into()
  }

  fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
    Cow::Borrowed(".. ")
  }

  fn render_prompt_history_search_indicator(
    &self,
    history_search: PromptHistorySearch,
  ) -> Cow<'_, str> {
    let prefix = match history_search.status {
      PromptHistorySearchStatus::Passing => "",
      PromptHistorySearchStatus::Failing => "failing ",
    };
    // NOTE: magic strings, given there is logic on how these compose I am not sure if it
    // is worth extracting in to static constant
    Cow::Owned(format!(
      "({}reverse-search: {}) ",
      prefix, history_search.term
    ))
  }
}

impl ReplPrompt {
  pub fn new() -> Self {
    ReplPrompt {}
  }
}

pub struct Repl {
  editor: Reedline,
  prompt: ReplPrompt,
}

impl Repl {
  pub fn new(keywords: Vec<String>) -> Self {
    let history_file = env::current_exe()
      .expect("Could not locate executable path!")
      .parent()
      .unwrap_or(Path::new(""))
      .join("history.txt");

    let mut completed_words = keywords.clone();
    completed_words.push(".exit".to_string());
    completed_words.push(".help".to_string());
    completed_words.push(".credits".to_string());

    let history = Box::new(
      FileBackedHistory::with_file(
        usize::from_str_radix(env!("ZURI_HISTORY_SIZE"), 10).unwrap_or(1000),
        history_file.into(),
      )
      .expect("Error configuring history with file"),
    );

    let completion_menu = Box::new(ColumnarMenu::default().with_name("completion_menu"));
    let mut keybindings = default_emacs_keybindings();
    keybindings.add_binding(
      KeyModifiers::NONE,
      KeyCode::Tab,
      ReedlineEvent::UntilFound(vec![
        ReedlineEvent::Menu("completion_menu".to_string()),
        ReedlineEvent::MenuNext,
      ]),
    );

    let editor = Reedline::create()
      .with_highlighter(Box::new(ZuriHighlighter::new(keywords)))
      .with_completer(Box::new(DefaultCompleter::new(completed_words)))
      .with_menu(ReedlineMenu::EngineCompleter(completion_menu))
      .with_edit_mode(Box::new(Emacs::new(keybindings)))
      .with_validator(Box::new(DefaultValidator))
      .with_history(history)
      .with_hinter(Box::new(
        DefaultHinter::default().with_style(Style::new().italic().fg(Color::LightGray)),
      ));

    let prompt = ReplPrompt::new();

    Self { editor, prompt }
  }

  /// Runs until `callback` ends the session, or fails once the terminal
  /// has stopped being readable.
  pub fn run<F>(&mut self, vm: &mut VM, callback: F) -> io::Result<()>
  where
    F: Fn(&mut VM, String) -> Result<(), ()>,
  {
    let mut failures = 0;

    loop {
      let sig = self.editor.read_line(&self.prompt);
      match sig {
        Ok(Signal::Success(buffer)) => {
          if let Err(_) = callback(vm, buffer) {
            return Ok(());
          }
        },
        Ok(Signal::CtrlD) | Ok(Signal::CtrlC) => {
          let signal = sig.unwrap_or(Signal::CtrlC);
          println!("<KeyboardInterrupt [{:?}]>", signal);
          if matches!(signal, Signal::CtrlC) {
            println!("Type '.exit' to exit the REPL session");
          }
        },
        Err(e) => {
          failures += 1;

          if failures == MAX_READ_FAILURES {
            return Err(e);
          }

          continue;
        },
        _ => {
          // println!("Event: {:?}", x);
        },
      };

      failures = 0;
    }
  }
}

/// Runs a session from standard input when that is not a terminal, so
/// there is nothing for `Repl` to edit on. Statements are split where
/// `Repl` splits them, and the session ends with the input or when
/// `callback` ends it. A statement still open at the end of the input
/// runs as it stands, so its error is reported and not lost.
pub fn run_piped<F>(vm: &mut VM, callback: F) -> io::Result<()>
where
  F: Fn(&mut VM, String) -> Result<(), ()>,
{
  let stdin = io::stdin();
  let mut buffer = String::new();

  loop {
    let at_end = stdin.read_line(&mut buffer)? == 0;

    if !at_end
      && matches!(
        DefaultValidator.validate(&buffer),
        ValidationResult::Incomplete
      )
    {
      continue;
    }

    let statement = buffer.trim_end_matches(['\r', '\n']).to_string();
    buffer.clear();

    if callback(vm, statement).is_err() || at_end {
      return Ok(());
    }
  }
}
