use std::collections::HashSet;

use nu_ansi_term::{Color, Style};
use reedline::{AbbrExpandContext, Highlighter, StyledText};

pub static DEFAULT_BUFFER_MATCH_COLOR: Color = Color::Green;
pub static DEFAULT_BUFFER_NEUTRAL_COLOR: Color = Color::White;

pub struct ZuriHighlighter {
  keywords: HashSet<String>,
  match_color: Color,
  neutral_color: Color,
}

impl ZuriHighlighter {
  pub fn new(keywords: Vec<String>) -> ZuriHighlighter {
    ZuriHighlighter {
      keywords: keywords.into_iter().collect(),
      match_color: DEFAULT_BUFFER_MATCH_COLOR,
      neutral_color: DEFAULT_BUFFER_NEUTRAL_COLOR,
    }
  }
}

impl Default for ZuriHighlighter {
  fn default() -> Self {
    ZuriHighlighter::new(vec![])
  }
}

impl Highlighter for ZuriHighlighter {
  // Don't expand an abbreviation while the cursor is inside a string literal.
  fn should_expand_abbr(&self, line: &str, cursor: usize, _context: AbbrExpandContext) -> bool {
    if line.is_empty() || cursor == 0 {
      return true;
    }

    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;
    let mut byte_pos = 0;
    for &byte in line.as_bytes() {
      if byte_pos >= cursor {
        break;
      }

      if escaped {
        escaped = false;
        byte_pos += 1;
        continue;
      }

      match byte {
        b'\\' => escaped = true,
        b'\'' if !in_double => in_single = !in_single,
        b'"' if !in_single => in_double = !in_double,
        _ => {},
      }

      byte_pos += 1;
    }

    !(in_single || in_double)
  }

  fn highlight(&self, line: &str, _cursor: usize) -> StyledText {
    let mut styled_text = StyledText::new();

    if self.keywords.is_empty() {
      styled_text.push((Style::new().fg(self.neutral_color), line.to_string()));
      return styled_text;
    }

    let neutral = Style::new().fg(self.neutral_color);
    let matched = Style::new().fg(self.match_color);
    let is_word_char = |c: char| c.is_alphanumeric() || c == '_';

    #[derive(PartialEq, Clone, Copy)]
    enum Mode {
      Code,
      Single,
      Double,
    }

    let mut mode = Mode::Code;
    let mut escaped = false;
    let mut run_start = 0usize;
    let mut word_start: Option<usize> = None;

    for (byte_idx, ch) in line.char_indices() {
      if escaped {
        escaped = false;
        continue;
      }

      if mode == Mode::Code && word_start.is_none() && is_word_char(ch) {
        if byte_idx > run_start {
          styled_text.push((neutral, line[run_start..byte_idx].to_string()));
        }
        word_start = Some(byte_idx);
      } else if word_start.is_some() && !(mode == Mode::Code && is_word_char(ch)) {
        let ws = word_start.take().unwrap();
        let word = &line[ws..byte_idx];
        let style = if self.keywords.contains(word) { matched } else { neutral };
        styled_text.push((style, word.to_string()));
        run_start = byte_idx;
      }

      match ch {
        '\\' => escaped = true,
        '\'' if mode != Mode::Double => {
          mode = if mode == Mode::Single { Mode::Code } else { Mode::Single };
        },
        '"' if mode != Mode::Single => {
          mode = if mode == Mode::Double { Mode::Code } else { Mode::Double };
        },
        _ => {},
      }
    }

    if let Some(ws) = word_start {
      let word = &line[ws..line.len()];
      let style = if self.keywords.contains(word) { matched } else { neutral };
      styled_text.push((style, word.to_string()));
    } else if line.len() > run_start {
      styled_text.push((neutral, line[run_start..].to_string()));
    }

    styled_text
  }
}
