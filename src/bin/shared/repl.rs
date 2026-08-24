use std::{borrow::Cow, env, path::Path};

use nu_ansi_term::{Color, Style};
use reedline::{
  ColumnarMenu, DefaultCompleter, DefaultHinter, DefaultValidator, Emacs, FileBackedHistory,
  KeyCode, KeyModifiers, MenuBuilder, Prompt, PromptEditMode, PromptHistorySearch,
  PromptHistorySearchStatus, Reedline, ReedlineEvent, ReedlineMenu, Signal,
  default_emacs_keybindings,
};
use zuri::vm::vm::VM;

use crate::shared::highlighter::ZuriHighlighter;

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

  pub fn run<F>(&mut self, vm: &mut VM, callback: F)
  where
    F: Fn(&mut VM, String) -> Result<(), ()>,
  {
    loop {
      let sig = self.editor.read_line(&self.prompt);
      match sig {
        Ok(Signal::Success(buffer)) => {
          if let Err(_) = callback(vm, buffer) {
            break;
          }
        },
        Ok(Signal::CtrlD) | Ok(Signal::CtrlC) => {
          let signal = sig.unwrap_or(Signal::CtrlC);
          println!("<KeyboardInterrupt [{:?}]>", signal);
          if matches!(signal, Signal::CtrlC) {
            println!("Type '.exit' to exit the REPL session");
          }
        },
        _ => {
          // println!("Event: {:?}", x);
        },
      };
    }
  }
}
