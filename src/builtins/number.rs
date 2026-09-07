use std::sync::LazyLock;

use num_bigint::BigInt;
use num_traits::FromPrimitive;

use crate::{
  builtins::{
    MethodTable, build,
    enforce::{ArgType, enforce_method_arg_count, enforce_method_arg_type},
    method, method_n, to_string,
  },
  vm::{object::ZuriContext, value::Value},
};

// NOTE ON SCOPE: the doc these were transcribed from calls these
// `math.sin(n)`-style free functions on an importable `math` module.
// That module's resolution (how `import math` maps a name onto a
// native) isn't in any file available here, and grafting a guess at
// it on top of an unseen system risked being flatly wrong. This repo
// already has number INSTANCE methods (`.abs()`, `.max()`), so these
// are added the same way; `n.sin()`, `n.factorial()`, etc. `sum()`/
// `product()` are left out entirely: they operate on a (possibly
// nested) ITERABLE, not a single receiver number, so they don't fit
// this dispatch model at all; those two only make sense as free
// functions on whatever `math` turns out to be.
pub static NUMBER_METHODS: LazyLock<MethodTable> = LazyLock::new(|| {
  build(vec![
    // methods
    method("to_string", to_string),
    method("to_bool", to_bool),
    method("to_bigint", to_bigint),
    method("abs", abs),
    method("chr", chr),
    method("bin", bin),
    method("hex", hex),
    method("oct", oct),
    method("int", int),
    method_n("max", 1, max),
    method_n("min", 1, min),
    method("factorial", factorial),
    method("sin", sin),
    method("cos", cos),
    method("tan", tan),
    method("sinh", sinh),
    method("cosh", cosh),
    method("tanh", tanh),
    method("asin", asin),
    method("acos", acos),
    method("atan", atan),
    method_n("atan2", 1, atan2),
    method("asinh", asinh),
    method("acosh", acosh),
    method("atanh", atanh),
    method("exp", exp),
    method("expm1", expm1),
    method("ceil", ceil),
    method("round", round),
    method("log", log),
    method("log2", log2),
    method("log10", log10),
    method("log1p", log1p),
    method("cbrt", cbrt),
    method("sign", sign),
    method("floor", floor),
    method("is_nan", is_nan),
    method("is_inf", is_inf),
    method("is_finite", is_finite),
    method("trunc", trunc),
    method("sqrt", sqrt),
    method("fraction", fraction),
    method_n("fixed", 1, fixed),
  ])
});

fn to_bool(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  Ok(Value::bool(ctx.args[0].as_number() >= 0.0))
}

/// The counterpart to `bigint.to_number()`. Only an exact integer has a
/// bigint form, so anything fractional, infinite or NaN is rejected rather
/// than quietly rounded; that keeps the conversion lossless in both
/// directions.
fn to_bigint(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);

  let n = ctx.args[0].as_number();
  match BigInt::from_f64(n) {
    Some(big) if n.fract() == 0.0 => Ok(ctx.vm.heap_mut().alloc_bigint(big)),
    _ => Err(format!("cannot convert {} to a bigint, it is not an integer", n)),
  }
}

fn abs(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  Ok(Value::number(ctx.args[0].as_number().abs()))
}

fn chr(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let c = char::from_u32(ctx.args[0].as_number() as u32)
    .unwrap_or('\0')
    .to_string();
  Ok(ctx.heap().alloc_string(c))
}

fn bin(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let c = format!("{:b}", ctx.args[0].as_number() as u64);
  Ok(ctx.heap().alloc_string(c))
}

fn hex(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let c = format!("{:x}", ctx.args[0].as_number() as u64);
  Ok(ctx.heap().alloc_string(c))
}

fn oct(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let c = format!("{:o}", ctx.args[0].as_number() as u64);
  Ok(ctx.heap().alloc_string(c))
}

fn int(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  Ok(Value::number((ctx.args[0].as_number() as i64) as f64))
}

fn max(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  Ok(Value::number(
    ctx.args[0].as_number().max(ctx.args[1].as_number()),
  ))
}

fn min(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  Ok(Value::number(
    ctx.args[0].as_number().min(ctx.args[1].as_number()),
  ))
}

fn factorial(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let n = ctx.args[0].as_number();
  if n < 0.0 || n.fract() != 0.0 {
    return Err(format!(
      "'{}' expects a non-negative integer, got {}",
      ctx.name, n
    ));
  }
  let mut result = 1.0f64;
  let mut i = 2.0f64;
  while i <= n {
    result *= i;
    i += 1.0;
  }
  Ok(Value::number(result))
}

/// Emits `fn $name(ctx) -> Result<Value, String>` calling the given
/// `f64` method with no further arguments; covers every single-
/// argument (just the receiver) math wrapper below in one place
/// instead of hand-writing the same three lines dozens of times.
macro_rules! unary_math {
  ($name:ident, $f:ident) => {
    fn $name(ctx: &mut ZuriContext) -> Result<Value, String> {
      enforce_method_arg_count!(ctx, 0);
      Ok(Value::number(ctx.args[0].as_number().$f()))
    }
  };
}

unary_math!(sin, sin);
unary_math!(cos, cos);
unary_math!(tan, tan);
unary_math!(sinh, sinh);
unary_math!(cosh, cosh);
unary_math!(tanh, tanh);
unary_math!(asin, asin);
unary_math!(acos, acos);
unary_math!(atan, atan);
unary_math!(asinh, asinh);
unary_math!(acosh, acosh);
unary_math!(atanh, atanh);
unary_math!(exp, exp);
unary_math!(expm1, exp_m1);
unary_math!(ceil, ceil);
unary_math!(round, round);
unary_math!(log, ln);
unary_math!(log2, log2);
unary_math!(log10, log10);
unary_math!(cbrt, cbrt);
unary_math!(floor, floor);
unary_math!(trunc, trunc);
unary_math!(sqrt, sqrt);

fn log1p(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  Ok(Value::number(ctx.args[0].as_number().ln_1p()))
}

/// `y.atan2(x)`; per spec, the RECEIVER is the y-coordinate and the
/// argument is the x-coordinate (matching `f64::atan2`'s own
/// `self=y, x` convention directly).
fn atan2(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Number);
  Ok(Value::number(
    ctx.args[0].as_number().atan2(ctx.args[1].as_number()),
  ))
}

/// `-1`/`0`/`1`, with `0`/`-0` preserved for a zero input (unlike
/// `f64::signum`, which returns `+/-1.0` even for zero); per spec.
fn sign(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let n = ctx.args[0].as_number();
  if n == 0.0 {
    Ok(Value::number(if n.is_sign_negative() { -0.0 } else { 0.0 }))
  } else if n > 0.0 {
    Ok(Value::number(1.0))
  } else {
    Ok(Value::number(-1.0))
  }
}

fn is_nan(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  Ok(Value::bool(ctx.args[0].as_number().is_nan()))
}

fn is_inf(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  Ok(Value::bool(ctx.args[0].as_number().is_infinite()))
}

fn is_finite(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  Ok(Value::bool(ctx.args[0].as_number().is_finite()))
}

/// Fractional part as a WHOLE number by shifting the digits after the
/// decimal point left of it; per spec's own example, `1.92.fraction()
/// == 92`, not `0.92`.
///
/// Formats at a fixed 10 decimal places (rounding) rather than
/// `f64`'s own shortest-round-trip `Display`: `1.92_f64.fract()` is
/// actually `0.9199999999999999` (binary/decimal conversion noise,
/// not a real fraction), and reading THAT string verbatim turned
/// `1.92.fraction()` into `9200000000000000` instead of `92`.
/// Rounding to 10 places lands past where that noise starts without
/// losing any digit a person would have actually typed.
fn fraction(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let n = ctx.args[0].as_number();
  let frac = n.fract().abs();
  if frac == 0.0 {
    return Ok(Value::number(0.0));
  }
  let s = format!("{:.10}", frac);
  let digits = s.split('.').nth(1).unwrap_or("0").trim_end_matches('0');
  if digits.is_empty() {
    return Ok(Value::number(0.0));
  }
  let whole: f64 = digits.parse().unwrap_or(0.0);
  Ok(Value::number(whole))
}

/// Rounds to `n` decimal places, half away from zero, matching what
/// `round()` does at a scale.
///
/// Everything stays in `f64`. The obvious integer version has to cast
/// through `u64`, which cannot hold a negative number, panics once the
/// scale passes `10^19`, and quietly loses every non-finite input.
fn fixed(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let n = ctx.args[0].as_number();
  if !n.is_finite() {
    return Ok(Value::number(n));
  }

  // Past 17 places an f64 has no digits left to round, so the scaling
  // would only introduce noise.
  let places = ctx.args[1].as_number().clamp(0.0, 17.0) as i32;
  let precision = 10f64.powi(places);

  Ok(Value::number((n * precision).round() / precision))
}
