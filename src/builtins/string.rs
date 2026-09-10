#![allow(unused)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::LazyLock;

use rustc_hash::FxHashMap;

use pcre2::bytes::{CaptureLocations, Match, Regex, RegexBuilder};

use crate::{
  builtins::{
    MethodTable, build,
    enforce::{
      ArgType, enforce_method_arg_count, enforce_method_arg_range, enforce_method_arg_type,
      enforce_method_arg_type_any_of, enforce_method_arg_type_opt,
    },
    method, method_n, method_opt, to_string,
  },
  vm::{object::ZuriContext, value::Value},
};

pub static STRING_METHODS: LazyLock<MethodTable> = LazyLock::new(|| {
  build(vec![
    method_n("@key", 1, _key),
    method_n("@value", 1, _value),
    method("to_string", to_string),
    method("length", length),
    method("upper", upper),
    method("lower", lower),
    method("is_empty", is_empty),
    method("is_alpha", is_alpha),
    method("is_alnum", is_alnum),
    method("is_number", is_number),
    method("is_lower", is_lower),
    method("is_upper", is_upper),
    method("is_space", is_space),
    method("ord", ord),
    method_opt("trim", 0, trim),
    method_opt("ltrim", 0, ltrim),
    method_opt("rtrim", 0, rtrim),
    method_n("join", 1, join),
    method_n("split", 1, split),
    method_opt("index_of", 1, index_of),
    method_n("starts_with", 1, starts_with),
    method_n("ends_with", 1, ends_with),
    method_n("contains", 1, contains),
    method_n("count", 1, count),
    method_opt("to_number", 0, to_number),
    method("to_list", to_list),
    method("to_bytes", to_bytes),
    method_opt("lpad", 1, lpad),
    method_opt("rpad", 1, rpad),
    method_opt("match", 1, string_match),
    method_opt("matches", 1, string_matches),
    method_opt("replace", 2, replace),
    method_n("replace_with", 2, replace_with),
    method_n("each", 1, each),
    method("case_fold", case_fold),
    method_n("compare", 1, compare),
    method("lines", lines),
    method_n("each_line", 1, each_line),
    method("capitalize", capitalize),
    method("title", title),
    method("ascii", ascii),
  ])
});

// Regex support.

/// Recognizes a Zuri regex literal; a pattern surrounded by two
/// identical non-word delimiter characters, with any modifier letters
/// following the closing delimiter (e.g. `/[a-z]+/mi`). `None` for a
/// plain string, which every regex-accepting method below falls back
/// to treating as a literal substring.
fn parse_regex(s: &str) -> Option<(&str, &str)> {
  let mut chars = s.chars();
  let delim = chars.next()?;
  if delim.is_alphanumeric() || delim == '_' {
    return None;
  }
  let rest = &s[delim.len_utf8()..];
  let close = rest.rfind(delim)?;
  if close == 0 {
    return None; // no room for a pattern between the delimiters
  }
  Some((&rest[..close], &rest[close + delim.len_utf8()..]))
}

/// Compiles a Zuri regex's pattern/modifiers into a genuine PCRE2
/// `Regex`, matching the spec's claim that Zuri regex is built on top
/// of (and compatible with) PCRE2: named groups, backreferences, and
/// lookaround all just work, since this is the real engine rather
/// than a lookalike.
///
/// Returns the compiled regex together with whether the `A` (force
/// pattern anchoring) modifier was given. `PCRE2_ANCHORED` isn't
/// exposed by this crate's safe wrapper, so anchoring is instead
/// enforced by every caller via `find_at_anchored`/`find_all_captures`,
/// which discard a match unless it begins exactly where the search
/// started, matching the real compile option's observable behavior.
///
/// `U` (ungreedy) and `J` (duplicate subpattern names) aren't exposed
/// as builder options either, but PCRE2's own pattern syntax accepts
/// them as inline `(?U)`/`(?J)` option-setting groups, so they're
/// prepended to the pattern instead. `D` (dollar-endonly) has no such
/// inline equivalent and, like any unrecognized modifier letter, is
/// simply accepted and has no effect.
/// How many distinct patterns one thread keeps compiled. A program
/// normally uses a handful of literal patterns over and over, so this
/// is generous; the cap exists only so a program that builds patterns
/// from user input cannot grow the cache without bound. Going over it
/// clears the whole cache rather than evicting one entry, which costs
/// a recompile for the surviving patterns but keeps this to a counter
/// and no bookkeeping on the hot path.
const REGEX_CACHE_LIMIT: usize = 256;

thread_local! {
  /// Compiled patterns, keyed by pattern then modifiers.
  ///
  /// Nested rather than keyed on a combined string so a lookup can
  /// borrow both halves as `&str` and allocate nothing on a hit, which
  /// is the whole point: compiling a PCRE2 pattern costs orders of
  /// magnitude more than matching a short subject against it, and
  /// programs reuse a small set of literal patterns.
  ///
  /// Thread-local because a `Regex` need not be `Send` for this and
  /// every isolate runs on its own OS thread; each gets its own cache
  /// and there is no synchronization anywhere on the path.
  static REGEX_CACHE: RefCell<FxHashMap<String, FxHashMap<String, Rc<(Regex, bool)>>>> =
    RefCell::new(FxHashMap::default());
}

/// Compiles a pattern, or hands back the one already compiled for it.
///
/// The `bool` is the `A` (anchored) modifier, which PCRE2 has no
/// builder option for and which the caller has to honour itself.
fn compile_regex(pattern: &str, modifiers: &str) -> Result<Rc<(Regex, bool)>, String> {
  let cached = REGEX_CACHE.with(|cache| {
    cache
      .borrow()
      .get(pattern)
      .and_then(|by_modifier| by_modifier.get(modifiers))
      .cloned()
  });

  if let Some(hit) = cached {
    return Ok(hit);
  }

  let compiled = Rc::new(build_regex(pattern, modifiers)?);

  REGEX_CACHE.with(|cache| {
    let mut cache = cache.borrow_mut();

    if cache.len() >= REGEX_CACHE_LIMIT {
      cache.clear();
    }

    cache
      .entry(pattern.to_string())
      .or_default()
      .insert(modifiers.to_string(), Rc::clone(&compiled));
  });

  Ok(compiled)
}

fn build_regex(pattern: &str, modifiers: &str) -> Result<(Regex, bool), String> {
  let mut builder = RegexBuilder::new();
  // Zuri strings are always valid UTF-8; matching per-codepoint
  // (rather than per-byte) is what keeps `.` and every byte offset
  // this file hands back to Zuri correct on multi-byte characters,
  // independent of the `u` modifier, which (per the spec's own
  // modifier table) only controls whether \d/\w/\s become Unicode-
  // property-aware instead of ASCII-only.
  builder.utf(true);

  let mut inline_flags = String::new();
  let mut anchored = false;

  for c in modifiers.chars() {
    match c {
      'i' => {
        builder.caseless(true);
      },
      'm' => {
        builder.multi_line(true);
      },
      's' => {
        builder.dotall(true);
      },
      'x' => {
        builder.extended(true);
      },
      'u' => {
        builder.ucp(true);
      },
      'U' => inline_flags.push('U'),
      'J' => inline_flags.push('J'),
      'A' => anchored = true,
      _ => {}, // 'D' and any unrecognized letter: accepted, no-op.
    }
  }

  let full_pattern = if inline_flags.is_empty() {
    pattern.to_string()
  } else {
    format!("(?{}){}", inline_flags, pattern)
  };

  let re = builder
    .build(&full_pattern)
    .map_err(|e| format!("invalid regular expression '{}': {}", pattern, e))?;
  Ok((re, anchored))
}

/// `Regex::find_at`, honoring the `A` modifier: when `anchored` is
/// set, a match found anywhere past `start` is discarded unless it
/// begins exactly at `start`.
fn find_at_anchored<'s>(
  re: &Regex,
  anchored: bool,
  subject: &'s [u8],
  start: usize,
) -> Result<Option<Match<'s>>, String> {
  let found = re.find_at(subject, start).map_err(|e| e.to_string())?;
  Ok(match found {
    Some(m) if !anchored || m.start() == start => Some(m),
    _ => None,
  })
}

/// Collects every non-overlapping match starting at `search_start` as
/// populated `CaptureLocations`, mirroring `Regex::captures_iter`'s own
/// empty-match handling (advance by one byte, and never accept an
/// empty match immediately following a real one) so unanchored
/// behavior matches the crate's own iterator exactly. When `anchored`
/// is set (the `A` modifier), collection stops at the first match that
/// doesn't begin exactly where the previous one ended (or, for the
/// first match, at `search_start`); this is how anchoring is enforced
/// without a real PCRE2_ANCHORED compile option to reach for.
fn find_all_captures(
  re: &Regex,
  anchored: bool,
  subject: &[u8],
  search_start: usize,
) -> Result<Vec<CaptureLocations>, String> {
  let mut out = Vec::new();
  let mut pos = search_start;
  let mut last_match_end: Option<usize> = None;

  while pos <= subject.len() {
    let mut locs = re.capture_locations();
    let found = re
      .captures_read_at(&mut locs, subject, pos)
      .map_err(|e| e.to_string())?;
    let m = match found {
      Some(m) => m,
      None => break,
    };

    if anchored && m.start() != pos {
      break;
    }
    if m.start() == m.end() && Some(m.end()) == last_match_end {
      pos = m.end() + 1;
      continue;
    }

    last_match_end = Some(m.end());
    pos = if m.end() == m.start() {
      m.end() + 1
    } else {
      m.end()
    };
    out.push(locs);
  }

  Ok(out)
}

/// Safe: every offset handed out by this file's regex helpers comes
/// from PCRE2 running in UTF mode (`compile_regex` always sets
/// `.utf(true)`), so it's always aligned to a UTF-8 codepoint
/// boundary.
fn bytes_to_string(b: &[u8]) -> String {
  std::str::from_utf8(b).unwrap().to_string()
}

/// Reads `ctx.args[idx]` as an optional single-character string
/// parameter, defaulting to `default` when the arg wasn't supplied at
/// all (the `variadic` floor from `method_opt` means it's legal for it
/// to be missing).
fn optional_char(ctx: &ZuriContext, idx: usize, default: char) -> Result<char, String> {
  match ctx.args.get(idx) {
    None => Ok(default),
    Some(v) if v.is_string() => v
      .as_str()
      .chars()
      .next()
      .ok_or_else(|| "expected a single character, got an empty string".to_string()),
    Some(v) => Err(format!("expected a string, got {}", v.type_name())),
  }
}

fn optional_offset(ctx: &ZuriContext, idx: usize) -> Result<usize, String> {
  match ctx.args.get(idx) {
    None => Ok(0),
    Some(v) if v.is_number() => Ok(v.as_number().max(0.0) as usize),
    Some(v) => Err(format!("offset must be a number, got {}", v.type_name())),
  }
}

fn expect_width(ctx: &ZuriContext, idx: usize) -> Result<usize, String> {
  let v = ctx.args[idx];
  if !v.is_number() {
    return Err(format!(
      "expected a number for width, got {}",
      v.type_name()
    ));
  }
  Ok(v.as_number().max(0.0) as usize)
}

/// Char-index (not byte-index) offset into `s`, matching how the rest
/// of the VM indexes strings (see `VM::index_get`'s string arm).
fn char_offset_to_byte(s: &str, char_offset: usize) -> usize {
  s.char_indices()
    .nth(char_offset)
    .map(|(b, _)| b)
    .unwrap_or(s.len())
}

fn length(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);

  Ok(Value::number(ctx.args[0].as_str().chars().count() as f64))
}

fn upper(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);

  Ok(
    ctx
      .vm
      .heap_mut()
      .alloc_string(ctx.args[0].as_str().to_uppercase()),
  )
}

fn lower(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);

  Ok(
    ctx
      .vm
      .heap_mut()
      .alloc_string(ctx.args[0].as_str().to_lowercase()),
  )
}

fn is_empty(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);

  let s = ctx.args[0].as_str();
  Ok(Value::bool(s.is_empty()))
}

fn is_alpha(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);

  let s = ctx.args[0].as_str();
  Ok(Value::bool(
    !s.is_empty() && s.chars().all(|c| c.is_alphabetic()),
  ))
}

fn is_alnum(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);

  let s = ctx.args[0].as_str();
  Ok(Value::bool(
    !s.is_empty() && s.chars().all(|c| c.is_alphanumeric()),
  ))
}

fn is_number(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);

  let s = ctx.args[0].as_str();
  Ok(Value::bool(
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()),
  ))
}

fn is_lower(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);

  let s = ctx.args[0].as_str();
  let cased: Vec<char> = s.chars().filter(|c| c.is_alphabetic()).collect();
  Ok(Value::bool(
    !cased.is_empty() && cased.iter().all(|c| c.is_lowercase()),
  ))
}

fn is_upper(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);

  let s = ctx.args[0].as_str();
  let cased: Vec<char> = s.chars().filter(|c| c.is_alphabetic()).collect();
  Ok(Value::bool(
    !cased.is_empty() && cased.iter().all(|c| c.is_uppercase()),
  ))
}

fn is_space(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);

  let s = ctx.args[0].as_str();
  Ok(Value::bool(
    !s.is_empty() && s.chars().all(|c| c.is_whitespace()),
  ))
}

fn ord(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);

  let s = ctx.args[0].as_str();
  if s.chars().count() != 1 {
    return Err(format!(
      "{}() must be called on a single character, got {}",
      ctx.name, s
    ));
  }

  Ok(Value::number(s.chars().nth(0).unwrap() as u32 as f64))
}

fn trim(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 0, 1);
  enforce_method_arg_type_opt!(ctx, 1, ArgType::String);

  let ch = optional_char(ctx, 1, ' ')?;
  let trimmed = ctx.args[0].as_str().trim_matches(ch).to_string();
  Ok(ctx.vm.heap_mut().alloc_string(trimmed))
}

fn ltrim(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 0, 1);
  enforce_method_arg_type_opt!(ctx, 1, ArgType::String);

  let ch = optional_char(ctx, 1, ' ')?;
  let trimmed = ctx.args[0].as_str().trim_start_matches(ch).to_string();
  Ok(ctx.vm.heap_mut().alloc_string(trimmed))
}

fn rtrim(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 0, 1);
  enforce_method_arg_type_opt!(ctx, 1, ArgType::String);

  let ch = optional_char(ctx, 1, ' ')?;
  let trimmed = ctx.args[0].as_str().trim_end_matches(ch).to_string();
  Ok(ctx.vm.heap_mut().alloc_string(trimmed))
}

/// Joins a string, list, or dict's items using `self` as the
/// separator. List/dict items are stringified via `Value`'s own
/// `Display`; the same representation `to_string()` uses everywhere
/// else; rather than requiring every element to already be a string.
fn join(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type_any_of!(ctx, 1, [ArgType::String, ArgType::List, ArgType::Dict]);

  let sep = ctx.args[0].as_str().to_string();
  let other = ctx.args[1];

  let parts: Vec<String> = if other.is_string() {
    other.as_str().chars().map(|c| c.to_string()).collect()
  } else if other.is_list() {
    other.as_list().iter().map(|v| format!("{}", v)).collect()
  } else if other.is_dict() {
    other
      .as_dict()
      .iter()
      .map(|(_, v)| format!("{}", v))
      .collect()
  } else {
    return Err(format!(
      "'{}' expects argument 1 to be a string, list, or dict, got {}",
      ctx.name,
      other.type_name()
    ));
  };

  Ok(ctx.vm.heap_mut().alloc_string(parts.join(&sep)))
}

fn split(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::String);

  let s = ctx.args[0].as_str().to_string();
  let delim = ctx.args[1].as_str().to_string();

  if s.is_empty() {
    return Ok(ctx.vm.heap_mut().alloc_list(Vec::<Value>::new()));
  }

  let parts: Vec<String> = if delim.is_empty() {
    s.chars().map(|c| c.to_string()).collect()
  } else if let Some((pattern, modifiers)) = parse_regex(&delim) {
    let compiled = compile_regex(pattern, modifiers)?;
    let (re, anchored) = (&compiled.0, compiled.1);
    let bytes = s.as_bytes();
    let matches = find_all_captures(&re, anchored, bytes, 0)?;

    let mut parts = Vec::with_capacity(matches.len() + 1);
    let mut last_end = 0usize;
    for locs in &matches {
      let (mstart, mend) = locs.get(0).unwrap();
      parts.push(bytes_to_string(&bytes[last_end..mstart]));
      last_end = mend;
    }
    parts.push(bytes_to_string(&bytes[last_end..]));
    parts
  } else {
    s.split(delim.as_str()).map(|p| p.to_string()).collect()
  };

  let items: Vec<Value> = parts
    .into_iter()
    .map(|p| ctx.vm.heap_mut().alloc_string(p))
    .collect();
  Ok(ctx.vm.heap_mut().alloc_list(items))
}

/// Char-indexed (not byte-indexed) search, matching `index_of`/`match`/
/// every other string method here. `-1` covers both "not found" and
/// "start index past the end", never an error.
fn index_of(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 1, 2);
  enforce_method_arg_type!(ctx, 1, ArgType::String);
  enforce_method_arg_type_opt!(ctx, 2, ArgType::Number);

  let start = optional_offset(ctx, 2)?;
  let ascii = ctx.args[0].str_is_ascii();

  let haystack = ctx.args[0].as_str();
  let needle = ctx.args[1].as_str();

  if needle.is_empty() {
    return Ok(Value::number(-1.0));
  }

  // `start` and the result are both codepoint counts, but the search
  // itself works on bytes. For an all-ASCII haystack the two indexings
  // coincide, so the conversions collapse; otherwise each costs one
  // linear walk, which still beats materialising the whole string as a
  // `Vec<char>` the way this used to.
  let byte_start = if ascii {
    if start > haystack.len() {
      return Ok(Value::number(-1.0));
    }
    start
  } else {
    match haystack.char_indices().nth(start) {
      Some((offset, _)) => offset,
      // `start` sitting exactly at the end can still only match an
      // empty needle, which is already handled above.
      None => return Ok(Value::number(-1.0)),
    }
  };

  match haystack[byte_start..].find(needle) {
    Some(offset) => {
      let found = byte_start + offset;
      let index = if ascii {
        found
      } else {
        haystack[..found].chars().count()
      };
      Ok(Value::number(index as f64))
    },
    None => Ok(Value::number(-1.0)),
  }
}

fn starts_with(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::String);

  Ok(Value::bool(
    ctx.args[0].as_str().starts_with(ctx.args[1].as_str()),
  ))
}

fn ends_with(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::String);

  Ok(Value::bool(
    ctx.args[0].as_str().ends_with(ctx.args[1].as_str()),
  ))
}

fn contains(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::String);

  Ok(Value::bool(
    ctx.args[0].as_str().contains(ctx.args[1].as_str()),
  ))
}

fn count(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::String);

  Ok(Value::number(
    ctx.args[0].as_str().matches(ctx.args[1].as_str()).count() as f64,
  ))
}

fn to_number(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 0, 1);

  let s = ctx.args[0].as_str();
  let base = if ctx.args.len() == 2 {
    enforce_method_arg_type!(ctx, 1, ArgType::Number);
    ctx.args[1].as_number() as u32
  } else {
    10
  };

  if s.contains(".") && base == 10 {
    Ok(Value::number(s.parse::<f64>().unwrap_or(0.0)))
  } else {
    Ok(Value::number(
      i64::from_str_radix(s, base).unwrap_or(0) as f64
    ))
  }
}

fn to_list(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);

  let s = ctx.args[0].as_str().to_string();
  let items: Vec<Value> = s
    .chars()
    .map(|c| ctx.vm.heap_mut().alloc_string(c.to_string()))
    .collect();
  Ok(ctx.vm.heap_mut().alloc_list(items))
}

fn to_bytes(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);

  let bytes = ctx.args[0].as_str().as_bytes().to_vec();
  Ok(ctx.vm.heap_mut().alloc_bytes(bytes))
}

fn lpad(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 1, 2);
  enforce_method_arg_type!(ctx, 1, ArgType::Number);
  enforce_method_arg_type_opt!(ctx, 2, ArgType::String);

  let s = ctx.args[0].as_str().to_string();
  let width = expect_width(ctx, 1)?;
  let fill = optional_char(ctx, 2, ' ')?;
  let len = s.chars().count();
  let result = if width <= len {
    s
  } else {
    let pad: String = std::iter::repeat(fill).take(width - len).collect();
    format!("{}{}", pad, s)
  };
  Ok(ctx.vm.heap_mut().alloc_string(result))
}

fn rpad(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 1, 2);
  enforce_method_arg_type!(ctx, 1, ArgType::Number);
  enforce_method_arg_type_opt!(ctx, 2, ArgType::String);

  let s = ctx.args[0].as_str().to_string();
  let width = expect_width(ctx, 1)?;
  let fill = optional_char(ctx, 2, ' ')?;
  let len = s.chars().count();
  let result = if width <= len {
    s
  } else {
    let pad: String = std::iter::repeat(fill).take(width - len).collect();
    format!("{}{}", s, pad)
  };
  Ok(ctx.vm.heap_mut().alloc_string(result))
}

/// Plain-string `str` -> substring containment past `offset`. Regex
/// `str` -> `false` on no match, else a one-entry dict `{0: match}`,
/// per spec.
fn string_match(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 1, 2);
  enforce_method_arg_type!(ctx, 1, ArgType::String);
  enforce_method_arg_type_opt!(ctx, 2, ArgType::Number);

  let s = ctx.args[0].as_str().to_string();
  let pattern_str = ctx.args[1].as_str().to_string();
  let start = char_offset_to_byte(&s, optional_offset(ctx, 2)?);

  if let Some((pattern, modifiers)) = parse_regex(&pattern_str) {
    let compiled = compile_regex(pattern, modifiers)?;
    let (re, anchored) = (&compiled.0, compiled.1);
    match find_at_anchored(re, anchored, s.as_bytes(), start)? {
      Some(m) => {
        let matched = ctx
          .vm
          .heap_mut()
          .alloc_string(bytes_to_string(m.as_bytes()));
        Ok(
          ctx
            .vm
            .heap_mut()
            .alloc_dict(vec![(Value::number(0.0), matched)]),
        )
      },
      None => Ok(Value::bool(false)),
    }
  } else {
    Ok(Value::bool(
      s[start.min(s.len())..].contains(pattern_str.as_str()),
    ))
  }
}

/// `{group_index: [every match's text for that group]}` for every
/// capture group (group 0 is always the whole match); an empty dict
/// entry list per group when nothing matched at all, never `false`
/// (unlike `match()`, which distinguishes "no match" from "a match").
fn string_matches(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 1, 2);
  enforce_method_arg_type!(ctx, 1, ArgType::String);
  enforce_method_arg_type_opt!(ctx, 2, ArgType::Number);

  let s = ctx.args[0].as_str().to_string();
  let pattern_str = ctx.args[1].as_str().to_string();
  let start = char_offset_to_byte(&s, optional_offset(ctx, 2)?);

  let (pattern, modifiers) = parse_regex(&pattern_str)
    .ok_or_else(|| "matches() expects a regular expression".to_string())?;
  let compiled = compile_regex(pattern, modifiers)?;
  let (re, anchored) = (&compiled.0, compiled.1);

  let bytes = s.as_bytes();
  let all_matches = find_all_captures(&re, anchored, bytes, start)?;
  let num_groups = re.capture_locations().len();
  let mut columns: Vec<Vec<Value>> = vec![Vec::new(); num_groups];

  for locs in &all_matches {
    for g in 0..num_groups {
      let text = match locs.get(g) {
        Some((gs, ge)) => bytes_to_string(&bytes[gs..ge]),
        None => String::new(),
      };
      columns[g].push(ctx.vm.heap_mut().alloc_string(text));
    }
  }

  let mut pairs = Vec::with_capacity(num_groups);
  for (g, col) in columns.into_iter().enumerate() {
    let list_val = ctx.vm.heap_mut().alloc_list(col);
    pairs.push((Value::number(g as f64), list_val));
  }
  Ok(ctx.vm.heap_mut().alloc_dict(pairs))
}

fn replace(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 2, 3);
  enforce_method_arg_type!(ctx, 1, ArgType::String);
  enforce_method_arg_type!(ctx, 2, ArgType::String);
  enforce_method_arg_type_opt!(ctx, 3, ArgType::Bool);

  let s = ctx.args[0].as_str().to_string();
  let pattern_str = ctx.args[1].as_str().to_string();
  let replacement = ctx.args[2].as_str().to_string();
  let use_regex = match ctx.args.get(3) {
    None => true,
    Some(v) => !v.is_falsey(),
  };

  let result = match (use_regex, parse_regex(&pattern_str)) {
    (true, Some((pattern, modifiers))) => {
      let compiled = compile_regex(pattern, modifiers)?;
      let (re, anchored) = (&compiled.0, compiled.1);
      let bytes = s.as_bytes();
      let all_matches = find_all_captures(&re, anchored, bytes, 0)?;

      let mut result = String::new();
      let mut last_end = 0usize;
      for locs in &all_matches {
        let (mstart, mend) = locs.get(0).unwrap();
        result.push_str(&bytes_to_string(&bytes[last_end..mstart]));
        result.push_str(&expand_replacement(&replacement, locs, bytes));
        last_end = mend;
      }
      result.push_str(&bytes_to_string(&bytes[last_end..]));
      result
    },
    _ => s.replace(pattern_str.as_str(), &replacement),
  };

  Ok(ctx.vm.heap_mut().alloc_string(result))
}

/// Expands `$N`/`${N}` capture-group references in a `replace()`
/// replacement string, matching the spec's own `$index` syntax: `$$`
/// is a literal `$`, `$N` (one or more digits) or `${N}` substitutes
/// group `N`'s matched text (empty if that group didn't participate),
/// and a `$` followed by anything else is passed through literally.
fn expand_replacement(replacement: &str, locs: &CaptureLocations, subject: &[u8]) -> String {
  let mut out = String::new();
  let mut chars = replacement.chars().peekable();

  while let Some(c) = chars.next() {
    if c != '$' {
      out.push(c);
      continue;
    }
    match chars.peek() {
      Some('$') => {
        chars.next();
        out.push('$');
      },
      Some('{') => {
        chars.next();
        let mut digits = String::new();
        while let Some(&d) = chars.peek() {
          if d == '}' {
            chars.next();
            break;
          }
          digits.push(d);
          chars.next();
        }
        push_capture_group(&mut out, &digits, locs, subject);
      },
      Some(d) if d.is_ascii_digit() => {
        let mut digits = String::new();
        while let Some(&d) = chars.peek() {
          if !d.is_ascii_digit() {
            break;
          }
          digits.push(d);
          chars.next();
        }
        push_capture_group(&mut out, &digits, locs, subject);
      },
      _ => out.push('$'),
    }
  }

  out
}

fn push_capture_group(out: &mut String, digits: &str, locs: &CaptureLocations, subject: &[u8]) {
  if let Ok(n) = digits.parse::<usize>() {
    if let Some((s, e)) = locs.get(n) {
      out.push_str(&bytes_to_string(&subject[s..e]));
    }
  }
}

/// Calls back into Zuri code once per match; `call_value` already
/// pads missing parameters with `nil` and ignores extras (see its own
/// doc comment), so this can always pass the full `(match, groups...,
/// offset, string)` argument list regardless of how many the callback
/// actually declared.
fn replace_with(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 2);
  enforce_method_arg_type!(ctx, 1, ArgType::String);
  enforce_method_arg_type!(ctx, 2, ArgType::Function);

  let s = ctx.args[0].as_str().to_string();
  let pattern_str = ctx.args[1].as_str().to_string();
  let callback = ctx.args[2];

  let (pattern, modifiers) = parse_regex(&pattern_str)
    .ok_or_else(|| "replace_with() expects a regular expression".to_string())?;
  let compiled = compile_regex(pattern, modifiers)?;
  let (re, anchored) = (&compiled.0, compiled.1);

  let bytes = s.as_bytes();
  let all_matches = find_all_captures(&re, anchored, bytes, 0)?;
  let num_groups = re.capture_locations().len();

  let whole = ctx.vm.heap_mut().alloc_string(s.clone());
  // `callback` and `whole` are both reused across EVERY match below;
  // if either is still Young and a later match's own `call_value`
  // triggers a collection that relocates it, an un-pinned local would
  // go stale from that point on. See `VM::pin_values`'s own docs.
  let mark = ctx.vm.pin_values([callback, whole]);
  let mut result = String::new();
  let mut last_end = 0usize;

  for locs in &all_matches {
    let (mstart, mend) = locs.get(0).unwrap();
    result.push_str(&bytes_to_string(&bytes[last_end..mstart]));

    let mut call_args = Vec::with_capacity(num_groups + 1);
    for g in 0..num_groups {
      call_args.push(match locs.get(g) {
        Some((gs, ge)) => ctx
          .vm
          .heap_mut()
          .alloc_string(bytes_to_string(&bytes[gs..ge])),
        None => Value::nil(),
      });
    }
    call_args.push(Value::number(mstart as f64));
    call_args.push(ctx.vm.pinned(mark + 1));

    let callback = ctx.vm.pinned(mark);
    let replaced = ctx
      .vm
      .call_value(callback, &call_args)
      .map_err(|e| ctx.vm.describe_error(e))?;
    result.push_str(&format!("{}", replaced));

    last_end = mend;
  }
  result.push_str(&bytes_to_string(&bytes[last_end..]));
  ctx.vm.unpin(mark);

  Ok(ctx.vm.heap_mut().alloc_string(result))
}

fn each(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Function);

  let s = ctx.args[0].as_str().to_string();
  // `ctx.args` itself is not a GC root (see `VM::pin_values`'s own
  // docs); pinning the original string here is what keeps the
  // final `Ok(...)` below safe to return, in addition to `callback`
  // needing it for the same reason every other iterate+callback
  // native does.
  let mark = ctx.vm.pin_values([ctx.args[0], ctx.args[1]]);

  for (i, c) in s.chars().enumerate() {
    let char_val = ctx.vm.heap_mut().alloc_string(c.to_string());
    let callback = ctx.vm.pinned(mark + 1);
    ctx
      .vm
      .call_value(callback, &[char_val, Value::number(i as f64)])
      .map_err(|e| ctx.vm.describe_error(e))?;
  }

  let str_val = ctx.vm.pinned(mark);
  ctx.vm.unpin(mark);
  Ok(str_val)
}

/// Full Unicode case folding (per the `CaseFolding.txt` C+F mappings),
/// not just lowercasing: e.g. German `ß` folds to `ss`, matching how
/// two strings should be compared for case-insensitive equality
/// regardless of script.
fn case_fold(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);

  let folded = caseless::default_case_fold_str(ctx.args[0].as_str());
  Ok(ctx.vm.heap_mut().alloc_string(folded))
}

fn compare(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::String);

  let ordering = ctx.args[0].as_str().cmp(ctx.args[1].as_str());
  let n = match ordering {
    std::cmp::Ordering::Less => -1.0,
    std::cmp::Ordering::Equal => 0.0,
    std::cmp::Ordering::Greater => 1.0,
  };
  Ok(Value::number(n))
}

/// Splits on `\n`, also stripping a trailing `\r` from each line (so
/// both Unix and Windows line endings behave the same), matching
/// Rust's own `str::lines()` semantics.
fn lines(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);

  let s = ctx.args[0].as_str().to_string();
  let items: Vec<Value> = s
    .lines()
    .map(|line| ctx.vm.heap_mut().alloc_string(line.to_string()))
    .collect();
  Ok(ctx.vm.heap_mut().alloc_list(items))
}

fn each_line(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Function);

  let s = ctx.args[0].as_str().to_string();
  let lines: Vec<String> = s.lines().map(|l| l.to_string()).collect();
  // See `each`'s own docs on why both the receiver and the callback
  // need to be pinned here, not just read once up front.
  let mark = ctx.vm.pin_values([ctx.args[0], ctx.args[1]]);

  for (i, line) in lines.into_iter().enumerate() {
    let line_val = ctx.vm.heap_mut().alloc_string(line);
    let callback = ctx.vm.pinned(mark + 1);
    ctx
      .vm
      .call_value(callback, &[line_val, Value::number(i as f64)])
      .map_err(|e| ctx.vm.describe_error(e))?;
  }

  let str_val = ctx.vm.pinned(mark);
  ctx.vm.unpin(mark);
  Ok(str_val)
}

fn capitalize(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);

  let s = ctx.args[0].as_str();
  let mut chars = s.chars();
  let result = match chars.next() {
    Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
    None => String::new(),
  };
  Ok(ctx.vm.heap_mut().alloc_string(result))
}

/// Capitalizes the first letter of every word (a maximal run of
/// alphanumeric characters) and lowercases the rest of that word's
/// letters, leaving whitespace and punctuation between words exactly
/// as they were.
fn title(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);

  let s = ctx.args[0].as_str();
  let mut result = String::with_capacity(s.len());
  let mut at_word_start = true;

  for c in s.chars() {
    if c.is_alphanumeric() {
      if at_word_start {
        result.extend(c.to_uppercase());
      } else {
        result.extend(c.to_lowercase());
      }
      at_word_start = false;
    } else {
      result.push(c);
      at_word_start = true;
    }
  }

  Ok(ctx.vm.heap_mut().alloc_string(result))
}

/// Reinterprets the string as a raw byte view: each byte of its UTF-8
/// encoding becomes its own character (codepoints `0..=255`, i.e. a
/// Latin-1-style one-byte-per-char mapping), rather than the decoded
/// sequence of Unicode scalar values `length()`/`each()`/indexing
/// otherwise operate on. Every result is still valid UTF-8 (every
/// codepoint in `0..=255` is), so the return value is an ordinary
/// string; it just may no longer round-trip through the original
/// multi-byte characters if any were present, and its `length()` now
/// reports the original BYTE count rather than the original CHARACTER
/// count. Meant for the rare case where code needs to walk a string
/// byte-for-byte, e.g. one that originated from a byte stream where
/// the "characters" were never meant to be decoded as Unicode at all.
fn ascii(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);

  let result: String = ctx.args[0].as_str().bytes().map(|b| b as char).collect();
  Ok(ctx.vm.heap_mut().alloc_string(result))
}

// @key / @value: iterable protocol decorators.

fn _key(ctx: &mut ZuriContext) -> Result<Value, String> {
  let val = ctx.args[1];
  let obj = ctx.args[0].as_str();

  if obj.is_empty() {
    return Ok(Value::nil());
  }

  if val.is_nil() {
    return Ok(Value::number(0.0));
  }

  if !val.is_number() {
    return Err(format!(
      "strings are numerically indexed, {} given",
      val.type_name()
    ));
  }

  let index = val.as_number() as usize;
  if index < obj.chars().count() - 1 {
    return Ok(Value::number(index as f64 + 1.0));
  }

  Ok(Value::nil())
}

fn _value(ctx: &mut ZuriContext) -> Result<Value, String> {
  if !ctx.args[1].is_number() {
    return Err("strings are numerically indexed".to_string());
  }

  let index = ctx.args[1].as_number();
  let obj = ctx.args[0].as_str();

  if index > -1.0 && index < obj.chars().count() as f64 {
    let v = obj.chars().nth(index as usize).unwrap().to_string();
    return Ok(ctx.vm.heap_mut().alloc_string(v));
  }

  Ok(Value::nil())
}
