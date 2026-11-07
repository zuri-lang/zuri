use reedline::{ValidationResult, Validator};
use zuri::compiler::lexer::{Lexer, UNBALANCED_COMMENT, UNTERMINATED_STRING};
use zuri::compiler::token::TokenKind;

/// Decides whether an entry is finished by reading it with the lexer the
/// compiler uses, so a bracket in a string or a quote in a comment means
/// what it does in a program. An entry that more lines could never mend
/// counts as finished, so the compiler can say what is wrong with it.
pub struct ZuriValidator;

impl Validator for ZuriValidator {
  fn validate(&self, line: &str) -> ValidationResult {
    let mut lexer = Lexer::new(line);
    let mut depth = 0;

    loop {
      // The lexer ends an interpolation at its first `}`, whatever else
      // is open inside it, so brackets there are the parser's to judge.
      let counted = !lexer.is_interpolating();

      match lexer.scan().kind {
        TokenKind::Lparen | TokenKind::Lbracket | TokenKind::Lbrace if counted => depth += 1,
        TokenKind::Rparen | TokenKind::Rbracket | TokenKind::Rbrace if counted => {
          depth -= 1;

          if depth < 0 {
            return ValidationResult::Complete;
          }
        },
        TokenKind::Error(message, ..) => {
          return if message == UNTERMINATED_STRING || message == UNBALANCED_COMMENT {
            ValidationResult::Incomplete
          } else {
            ValidationResult::Complete
          };
        },
        TokenKind::Eof => {
          return if depth > 0 || lexer.is_interpolating() {
            ValidationResult::Incomplete
          } else {
            ValidationResult::Complete
          };
        },
        _ => {},
      }
    }
  }
}
