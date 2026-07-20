#![allow(unused)]

use std::sync::LazyLock;

use regex::Regex;

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
    method("is_alpha", is_alpha),
    method("is_alnum", is_alnum),
    method("is_number", is_number),
    method("is_lower", is_lower),
    method("is_upper", is_upper),
    method("is_space", is_space),
    method_opt("trim", 0, trim),
    method_opt("ltrim", 0, ltrim),
    method_opt("rtrim", 0, rtrim),
    method_n("join", 1, join),
    method_n("split", 1, split),
    method_opt("index_of", 1, index_of),
    method_n("starts_with", 1, starts_with),
    method_n("ends_with", 1, ends_with),
    method_n("count", 1, count),
    method("to_number", to_number),
    method("to_list", to_list),
    method("to_bytes", to_bytes),
    method_opt("lpad", 1, lpad),
    method_opt("rpad", 1, rpad),
    method_opt("match", 1, string_match),
    method_opt("matches", 1, string_matches),
    method_opt("replace", 2, replace),
    method_n("replace_with", 2, replace_with),
    method_n("each", 1, each),
  ])
});

//-----------------------------------------------------------------------------------
// Regex support
//-----------------------------------------------------------------------------------

/// Recognizes a Zuri regex literal -- a pattern surrounded by two
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

/// Compile a Zuri regex's pattern/modifiers into a `regex::Regex`. See
/// this file's module-level caveat: the `regex` crate is NOT PCRE2 --
/// no backreferences, no lookaround, no named groups -- and only
/// `i`/`m`/`s`/`x`/`U`/`u` of Zuri's documented modifiers have a direct
/// equivalent here; `A`/`D`/`J` are accepted but ignored.
fn compile_regex(pattern: &str, modifiers: &str) -> Result<Regex, String> {
  let flags: String = modifiers
    .chars()
    .filter(|c| matches!(c, 'i' | 'm' | 's' | 'x' | 'u' | 'U'))
    .collect();
  let full = if flags.is_empty() {
    pattern.to_string()
  } else {
    format!("(?{}){}", flags, pattern)
  };
  Regex::new(&full).map_err(|e| format!("invalid regular expression '{}': {}", pattern, e))
}

static NUMBER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d+(\.\d+)?").unwrap());

//-----------------------------------------------------------------------------------
// Small shared arg helpers
//-----------------------------------------------------------------------------------

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

//-----------------------------------------------------------------------------------
// Implementations
//-----------------------------------------------------------------------------------

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
/// `Display` -- the same representation `to_string()` uses everywhere
/// else -- rather than requiring every element to already be a string.
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
    let re = compile_regex(pattern, modifiers)?;
    re.split(&s).map(|p| p.to_string()).collect()
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

  let haystack: Vec<char> = ctx.args[0].as_str().chars().collect();
  let needle: Vec<char> = ctx.args[1].as_str().chars().collect();
  let start = optional_offset(ctx, 2)?;

  if needle.is_empty() || start + needle.len() > haystack.len() {
    return Ok(Value::number(-1.0));
  }

  for i in start..=haystack.len() - needle.len() {
    if haystack[i..i + needle.len()] == needle[..] {
      return Ok(Value::number(i as f64));
    }
  }
  Ok(Value::number(-1.0))
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

fn count(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::String);

  Ok(Value::number(
    ctx.args[0].as_str().matches(ctx.args[1].as_str()).count() as f64,
  ))
}

fn to_number(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);

  let s = ctx.args[0].as_str();
  match NUMBER_RE.find(s) {
    Some(m) => Ok(Value::number(m.as_str().parse::<f64>().unwrap_or(0.0))),
    None => Ok(Value::number(0.0)),
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
    let re = compile_regex(pattern, modifiers)?;
    match re.find_at(&s, start) {
      Some(m) => {
        let matched = ctx.vm.heap_mut().alloc_string(m.as_str().to_string());
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
/// capture group (group 0 is always the whole match) -- an empty dict
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
  let re = compile_regex(pattern, modifiers)?;

  let num_groups = re.captures_len();
  let mut columns: Vec<Vec<Value>> = vec![Vec::new(); num_groups];

  for caps in re.captures_iter(&s[start..]) {
    for g in 0..num_groups {
      let text = caps.get(g).map(|m| m.as_str()).unwrap_or("");
      columns[g].push(ctx.vm.heap_mut().alloc_string(text.to_string()));
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
      let re = compile_regex(pattern, modifiers)?;
      // Zuri's `$index` capture-group syntax matches the `regex`
      // crate's own `$N` replacement syntax directly.
      re.replace_all(&s, replacement.as_str()).into_owned()
    },
    _ => s.replace(pattern_str.as_str(), &replacement),
  };

  Ok(ctx.vm.heap_mut().alloc_string(result))
}

/// Calls back into Zuri code once per match -- `call_value` already
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
  let re = compile_regex(pattern, modifiers)?;

  let whole = ctx.vm.heap_mut().alloc_string(s.clone());
  let mut result = String::new();
  let mut last_end = 0;

  for caps in re.captures_iter(&s) {
    let m = caps.get(0).unwrap();
    result.push_str(&s[last_end..m.start()]);

    let mut call_args = vec![ctx.vm.heap_mut().alloc_string(m.as_str().to_string())];
    for g in 1..caps.len() {
      call_args.push(match caps.get(g) {
        Some(gm) => ctx.vm.heap_mut().alloc_string(gm.as_str().to_string()),
        None => Value::nil(),
      });
    }
    call_args.push(Value::number(m.start() as f64));
    call_args.push(whole);

    let replaced = ctx
      .vm
      .call_value(callback, &call_args)
      .map_err(|e| ctx.vm.describe_exception(e))?;
    result.push_str(&format!("{}", replaced));

    last_end = m.end();
  }
  result.push_str(&s[last_end..]);

  Ok(ctx.vm.heap_mut().alloc_string(result))
}

fn each(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Function);

  let s = ctx.args[0].as_str().to_string();
  let callback = ctx.args[1];

  for (i, c) in s.chars().enumerate() {
    let char_val = ctx.vm.heap_mut().alloc_string(c.to_string());
    ctx
      .vm
      .call_value(callback, &[char_val, Value::number(i as f64)])
      .map_err(|e| ctx.vm.describe_exception(e))?;
  }

  Ok(ctx.args[0])
}

//-----------------------------------------------------------------------------------
// Iterable Decorators (@key / @value)
//-----------------------------------------------------------------------------------

fn _key(ctx: &mut ZuriContext) -> Result<Value, String> {
  let val = ctx.args[1];
  let obj = ctx.args[0].as_str();

  if val.is_nil() {
    if obj.is_empty() {
      return Ok(Value::bool(false));
    }
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
