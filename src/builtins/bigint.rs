use std::sync::LazyLock;

use num_bigint::{BigInt, Sign};
use num_traits::{Signed, ToPrimitive, Zero};

use crate::{
  builtins::{
    MethodTable, build,
    enforce::{
      ArgType, enforce_method_arg_count, enforce_method_arg_range, enforce_method_arg_type,
      enforce_method_arg_type_any_of, enforce_method_arg_type_opt,
    },
    method, method_n, method_opt,
  },
  vm::{object::ZuriContext, value::Value},
};

pub static BIGINT_METHODS: LazyLock<MethodTable> = LazyLock::new(|| {
  build(vec![
    // conversions
    method_opt("to_string", 0, to_string),
    method("to_number", to_number),
    method("to_bool", to_bool),
    method_opt("to_bytes", 0, to_bytes),
    method("bin", bin),
    method("hex", hex),
    method("oct", oct),
    // sign and comparison
    method("abs", abs),
    method("sign", sign),
    method_n("max", 1, max),
    method_n("min", 1, min),
    // arithmetic
    method_n("pow", 1, pow),
    method("sqrt", sqrt),
    method("cbrt", cbrt),
    method_n("nth_root", 1, nth_root),
    method_n("gcd", 1, gcd),
    method_n("lcm", 1, lcm),
    method_n("modpow", 2, modpow),
    method_n("modinv", 1, modinv),
    // bits
    method("bits", bits),
    method_n("bit", 1, bit),
    method_n("set_bit", 2, set_bit),
    method("trailing_zeros", trailing_zeros),
    // predicates
    method("is_zero", is_zero),
    method("is_even", is_even),
    method("is_odd", is_odd),
  ])
});

/// Every method here takes the receiver as argument 0.
fn this<'a>(ctx: &'a ZuriContext<'_>) -> &'a BigInt {
  ctx.args[0].as_bigint()
}

/// `Display` for a bigint tags the value with a trailing `n` so it reads back
/// as a bigint literal, but `to_string()` is a conversion rather than a repr:
/// callers want the plain digits they can slice, pad or concatenate. The
/// optional radix covers the bases `bin()`/`oct()`/`hex()` don't.
fn to_string(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 0, 1);
  enforce_method_arg_type_opt!(ctx, 1, ArgType::Number);

  let text = if ctx.args.len() > 1 {
    this(ctx).to_str_radix(expect_radix(ctx.args[1].as_number())?)
  } else {
    this(ctx).to_string()
  };

  Ok(ctx.vm.heap_mut().alloc_string(text))
}

/// Lossy by nature: a bigint past `2^53` lands on the nearest double, and one
/// past the double range lands on an infinity rather than silently reading as
/// zero.
fn to_number(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);

  let big = this(ctx);
  let n = big.to_f64().unwrap_or_else(|| {
    if big.is_negative() {
      f64::NEG_INFINITY
    } else {
      f64::INFINITY
    }
  });

  Ok(Value::number(n))
}

/// The bigint's truthiness, exactly as `if` would read it: only zero is
/// falsy.
fn to_bool(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  Ok(Value::bool(!this(ctx).is_zero()))
}

/// Two's-complement bytes, so the sign survives the round trip. Big-endian by
/// default; pass `'little'` for the other order.
fn to_bytes(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_range!(ctx, 0, 1);
  enforce_method_arg_type_opt!(ctx, 1, ArgType::String);

  let little = if ctx.args.len() > 1 {
    match ctx.args[1].as_str() {
      "little" => true,
      "big" => false,
      other => {
        return Err(format!(
          "byte order must be 'big' or 'little', '{}' given",
          other
        ));
      },
    }
  } else {
    false
  };

  let bytes = if little {
    this(ctx).to_signed_bytes_le()
  } else {
    this(ctx).to_signed_bytes_be()
  };

  Ok(ctx.vm.heap_mut().alloc_bytes(bytes))
}

fn bin(ctx: &mut ZuriContext) -> Result<Value, String> {
  radix_string(ctx, 2)
}

fn hex(ctx: &mut ZuriContext) -> Result<Value, String> {
  radix_string(ctx, 16)
}

fn oct(ctx: &mut ZuriContext) -> Result<Value, String> {
  radix_string(ctx, 8)
}

fn radix_string(ctx: &mut ZuriContext, radix: u32) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let text = this(ctx).to_str_radix(radix);
  Ok(ctx.vm.heap_mut().alloc_string(text))
}

fn abs(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let v = this(ctx).abs();
  Ok(ctx.vm.heap_mut().alloc_bigint(v))
}

/// `1`, `-1` or `0`, as a plain number so it drops straight into arithmetic.
fn sign(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);

  Ok(Value::number(match this(ctx).sign() {
    Sign::Plus => 1.0,
    Sign::Minus => -1.0,
    Sign::NoSign => 0.0,
  }))
}

fn max(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::BigInt);

  let v = this(ctx).max(ctx.args[1].as_bigint()).clone();
  Ok(ctx.vm.heap_mut().alloc_bigint(v))
}

fn min(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::BigInt);

  let v = this(ctx).min(ctx.args[1].as_bigint()).clone();
  Ok(ctx.vm.heap_mut().alloc_bigint(v))
}

/// The method form of `**`, and like `**` it needs a non-negative exponent
/// small enough to be worth attempting.
fn pow(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type_any_of!(ctx, 1, [ArgType::Number, ArgType::BigInt]);

  let exponent = expect_u32(ctx, 1, "exponent")?;
  let v = this(ctx).pow(exponent);
  Ok(ctx.vm.heap_mut().alloc_bigint(v))
}

/// Integer square root, truncated towards zero.
fn sqrt(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);

  let big = this(ctx);
  if big.is_negative() {
    return Err("cannot take the square root of a negative bigint".to_string());
  }

  let v = big.sqrt();
  Ok(ctx.vm.heap_mut().alloc_bigint(v))
}

/// Integer cube root, truncated towards zero. Negatives are fine here.
fn cbrt(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  let v = this(ctx).cbrt();
  Ok(ctx.vm.heap_mut().alloc_bigint(v))
}

fn nth_root(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type_any_of!(ctx, 1, [ArgType::Number, ArgType::BigInt]);

  let n = expect_u32(ctx, 1, "root")?;
  if n == 0 {
    return Err("the zeroth root of a bigint is not defined".to_string());
  }

  let big = this(ctx);
  if big.is_negative() && n % 2 == 0 {
    return Err("cannot take an even root of a negative bigint".to_string());
  }

  let v = big.nth_root(n);
  Ok(ctx.vm.heap_mut().alloc_bigint(v))
}

/// Greatest common divisor, always non-negative. `0.gcd(0)` is `0`.
fn gcd(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::BigInt);

  let v = greatest_common_divisor(this(ctx).clone(), ctx.args[1].as_bigint().clone());
  Ok(ctx.vm.heap_mut().alloc_bigint(v))
}

/// Least common multiple, always non-negative. Zero on either side gives `0`,
/// which keeps the identity `gcd * lcm == |a * b|` intact.
fn lcm(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::BigInt);

  let a = this(ctx).clone();
  let b = ctx.args[1].as_bigint().clone();

  let v = if a.is_zero() || b.is_zero() {
    BigInt::zero()
  } else {
    let divisor = greatest_common_divisor(a.clone(), b.clone());
    ((a / divisor) * b).abs()
  };

  Ok(ctx.vm.heap_mut().alloc_bigint(v))
}

/// `(self ** exponent) % modulus`, computed without ever materialising the
/// full power. The result follows the sign of the modulus.
fn modpow(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 2);
  enforce_method_arg_type!(ctx, 1, ArgType::BigInt);
  enforce_method_arg_type!(ctx, 2, ArgType::BigInt);

  let exponent = ctx.args[1].as_bigint();
  let modulus = ctx.args[2].as_bigint();

  if modulus.is_zero() {
    return Err("bigint modpow modulus cannot be zero".to_string());
  }

  // num-bigint refuses a negative exponent outright, so raise the receiver's
  // modular inverse to the positive exponent instead, which is the same value.
  let v = if exponent.is_negative() {
    match this(ctx).modinv(modulus) {
      Some(inverse) => inverse.modpow(&-exponent, modulus),
      None => {
        return Err(
          "bigint modpow with a negative exponent needs a receiver that is invertible modulo \
           the modulus"
            .to_string(),
        );
      },
    }
  } else {
    this(ctx).modpow(exponent, modulus)
  };

  Ok(ctx.vm.heap_mut().alloc_bigint(v))
}

/// The `x` solving `self * x == 1 (mod modulus)`, or `nil` when the two are
/// not coprime and no such `x` exists.
fn modinv(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::BigInt);

  let modulus = ctx.args[1].as_bigint();
  if modulus.is_zero() {
    return Err("bigint modinv modulus cannot be zero".to_string());
  }

  match this(ctx).modinv(modulus) {
    Some(v) => Ok(ctx.vm.heap_mut().alloc_bigint(v)),
    None => Ok(Value::nil()),
  }
}

/// Bits in the magnitude, ignoring the sign. Zero has none.
fn bits(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  Ok(Value::number(this(ctx).bits() as f64))
}

/// Reads one bit of the two's-complement representation, so a negative
/// receiver reports the infinitely repeating sign bits above its magnitude.
fn bit(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 1);
  enforce_method_arg_type!(ctx, 1, ArgType::Number);

  let index = expect_bit_index(ctx.args[1].as_number())?;
  Ok(Value::bool(this(ctx).bit(index)))
}

/// Returns a new bigint with the given bit set or cleared; the receiver is
/// left alone.
fn set_bit(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 2);
  enforce_method_arg_type!(ctx, 1, ArgType::Number);
  enforce_method_arg_type!(ctx, 2, ArgType::Bool);

  let index = expect_bit_index(ctx.args[1].as_number())?;
  let value = ctx.args[2].as_bool();

  let mut v = this(ctx).clone();
  v.set_bit(index, value);

  Ok(ctx.vm.heap_mut().alloc_bigint(v))
}

/// Count of least-significant zero bits, or `nil` for zero, which has no
/// meaningful answer.
fn trailing_zeros(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);

  Ok(match this(ctx).trailing_zeros() {
    Some(n) => Value::number(n as f64),
    None => Value::nil(),
  })
}

fn is_zero(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  Ok(Value::bool(this(ctx).is_zero()))
}

fn is_even(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  Ok(Value::bool(!this(ctx).bit(0)))
}

fn is_odd(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_method_arg_count!(ctx, 0);
  Ok(Value::bool(this(ctx).bit(0)))
}

fn greatest_common_divisor(mut a: BigInt, mut b: BigInt) -> BigInt {
  a = a.abs();
  b = b.abs();

  while !b.is_zero() {
    let r = &a % &b;
    a = b;
    b = r;
  }

  a
}

fn expect_radix(radix: f64) -> Result<u32, String> {
  if radix.fract() != 0.0 || !(2.0..=36.0).contains(&radix) {
    return Err(format!(
      "radix must be a integer from 2 to 36, {} given",
      radix
    ));
  }

  Ok(radix as u32)
}

fn expect_bit_index(index: f64) -> Result<u64, String> {
  if index.fract() != 0.0 || index < 0.0 || index > u64::MAX as f64 {
    return Err(format!(
      "bit index must be a non-negative integer, {} given",
      index
    ));
  }

  Ok(index as u64)
}

/// Both `pow` and `nth_root` want a small non-negative integer, from
/// either a plain number or a bigint.
fn expect_u32(ctx: &ZuriContext<'_>, idx: usize, label: &str) -> Result<u32, String> {
  let arg = ctx.args[idx];

  let value = if arg.is_bigint() {
    arg.as_bigint().to_u32()
  } else {
    let n = arg.as_number();
    if n.fract() == 0.0 && (0.0..=u32::MAX as f64).contains(&n) {
      Some(n as u32)
    } else {
      None
    }
  };

  value.ok_or_else(|| format!("{} must be a integer ranged 0..={}", label, u32::MAX))
}
