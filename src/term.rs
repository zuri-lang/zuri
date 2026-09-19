//! Whether output gets ANSI styling, and the palette when it does.

use std::env;
use std::io::IsTerminal;

use nu_ansi_term::{Color, Style};

/// Whether `stream` should be written with escape codes.
///
/// Two things have to hold: it is a real terminal, and the reader has
/// not asked for plain text by setting `NO_COLOR`. A run piped into a
/// file or another program gets no escape codes either way. This is
/// the same rule `libs/args.zu` applies to the help a command prints,
/// so both halves of the command line agree about when to paint.
pub fn styled(stream: &impl IsTerminal) -> bool {
  stream.is_terminal() && env::var_os("NO_COLOR").is_none()
}

/// The styles the command line's own output is painted with.
///
/// Every field is a plain style when the stream takes no colour, so a
/// caller paints unconditionally rather than branching at each use.
pub struct Palette {
  /// A section heading: `Usage:`, `OPTIONS:`, `COMMANDS:`.
  pub heading: Style,

  /// The name of the thing being described: the executable, a version.
  pub accent: Style,

  /// Text that should not compete for attention: a build stamp, a
  /// closing hint.
  pub muted: Style,
}

impl Palette {
  /// The palette for `stream`, plain throughout when it takes no
  /// colour.
  pub fn for_stream(stream: &impl IsTerminal) -> Self {
    if !styled(stream) {
      return Self {
        heading: Style::new(),
        accent: Style::new(),
        muted: Style::new(),
      };
    }

    Self {
      heading: Style::new().fg(Color::Green).bold(),
      accent: Style::new().fg(Color::Cyan).bold(),
      muted: Style::new().fg(Color::DarkGray),
    }
  }
}
