//! Converting between a Zuri number and the wider formats `long double`
//! takes on some platforms.
//!
//! A number is an IEEE double, so going out is exact and coming back
//! rounds to the nearest double, ties to even, the way a C compiler
//! converts `long double` to `double`. Values too large for a double
//! become infinities and values too small become zero or subnormal.

/// The 64-bit pattern of the double nearest to `sign * mantissa *
/// 2^exponent`, where `mantissa` is a 128-bit integer. Rounds to
/// nearest, ties to even.
fn compose(negative: bool, mantissa: u128, exponent: i32) -> f64 {
  let sign = if negative { 1u64 << 63 } else { 0 };

  if mantissa == 0 {
    return f64::from_bits(sign);
  }

  // Normalise so the leading one is bit 127.
  let shift = mantissa.leading_zeros() as i32;
  let m = mantissa << shift;
  // Value is 1.xxx * 2^e with the fraction in the bits below 127.
  let e = exponent + 127 - shift;

  // A double's biased exponent is e + 1023 for normal numbers.
  let biased = e + 1023;

  let (keep, drop_bits) = if biased >= 1 {
    // 53 significant bits survive, the leading one included.
    (53u32, 128 - 53)
  } else {
    // Subnormal: fewer bits survive the further below the range it is.
    let bits = 53 + biased - 1;
    if bits < 0 {
      // Even the rounding bit is out of reach; the answer is zero.
      return f64::from_bits(sign);
    }
    (bits as u32, 128 - bits as u32)
  };

  let mut kept = if keep == 0 { 0 } else { m >> drop_bits };
  let remainder = if drop_bits >= 128 {
    m
  } else {
    m & ((1u128 << drop_bits) - 1)
  };
  let half = 1u128 << (drop_bits - 1);

  if remainder > half || (remainder == half && kept & 1 == 1) {
    kept += 1;
  }

  if biased >= 1 {
    let mut biased = biased as u64;
    if kept >> 53 != 0 {
      kept >>= 1;
      biased += 1;
    }
    if biased >= 2047 {
      return f64::from_bits(sign | 0x7ff0_0000_0000_0000);
    }
    let fraction = (kept as u64) & ((1u64 << 52) - 1);
    f64::from_bits(sign | (biased << 52) | fraction)
  } else {
    // A carry out of the subnormal range lands exactly on the smallest
    // normal number, which the same bit pattern already encodes.
    f64::from_bits(sign | kept as u64)
  }
}

/// Encodes `value` as an x87 80-bit extended float in the first ten of
/// sixteen bytes; the rest is padding and left zero.
pub fn to_x87(value: f64) -> [u8; 16] {
  let bits = value.to_bits();
  let negative = bits >> 63 != 0;
  let exponent = ((bits >> 52) & 0x7ff) as i32;
  let fraction = bits & ((1u64 << 52) - 1);

  let (exp80, mantissa): (u16, u64) = if exponent == 0x7ff {
    if fraction == 0 {
      (0x7fff, 1u64 << 63)
    } else {
      (0x7fff, (1u64 << 63) | (1u64 << 62) | (fraction << 11))
    }
  } else if exponent == 0 {
    if fraction == 0 {
      (0, 0)
    } else {
      // A double subnormal is a normal number in the wider format.
      let shift = fraction.leading_zeros();
      let m = fraction << shift;
      let e = -1022 - (shift as i32 - 11);
      ((e + 16383) as u16, m)
    }
  } else {
    (
      (exponent - 1023 + 16383) as u16,
      (1u64 << 63) | (fraction << 11),
    )
  };

  let mut out = [0u8; 16];
  out[..8].copy_from_slice(&mantissa.to_le_bytes());
  let top = exp80 | if negative { 0x8000 } else { 0 };
  out[8..10].copy_from_slice(&top.to_le_bytes());
  out
}

pub fn from_x87(bytes: &[u8]) -> f64 {
  let mantissa = u64::from_le_bytes(bytes[..8].try_into().unwrap());
  let top = u16::from_le_bytes(bytes[8..10].try_into().unwrap());
  let negative = top & 0x8000 != 0;
  let exp = (top & 0x7fff) as i32;

  if exp == 0x7fff {
    if mantissa << 1 == 0 {
      return if negative {
        f64::NEG_INFINITY
      } else {
        f64::INFINITY
      };
    }
    return f64::NAN;
  }

  // The integer bit is explicit, so the value is mantissa * 2^(e - 63),
  // with denormals using the minimum exponent.
  let e = if exp == 0 { 1 - 16383 } else { exp - 16383 };
  compose(negative, mantissa as u128, e - 63)
}

/// Encodes `value` as IEEE 754 binary128.
pub fn to_quad(value: f64) -> [u8; 16] {
  let bits = value.to_bits();
  let negative = bits >> 63 != 0;
  let exponent = ((bits >> 52) & 0x7ff) as i64;
  let fraction = (bits & ((1u64 << 52) - 1)) as u128;

  let (exp128, frac128): (u128, u128) = if exponent == 0x7ff {
    (
      0x7fff,
      if fraction == 0 {
        0
      } else {
        (1u128 << 111) | (fraction << 60)
      },
    )
  } else if exponent == 0 {
    if fraction == 0 {
      (0, 0)
    } else {
      let shift = fraction.leading_zeros() as i64 - (128 - 52);
      let e = -1022 - shift - 1;
      let normalised = (fraction << (shift + 1)) & ((1u128 << 52) - 1);
      ((e + 16383) as u128, normalised << 60)
    }
  } else {
    ((exponent - 1023 + 16383) as u128, fraction << 60)
  };

  let sign = if negative { 1u128 << 127 } else { 0 };
  (sign | (exp128 << 112) | frac128).to_le_bytes()
}

pub fn from_quad(bytes: &[u8]) -> f64 {
  let bits = u128::from_le_bytes(bytes[..16].try_into().unwrap());
  let negative = bits >> 127 != 0;
  let exp = ((bits >> 112) & 0x7fff) as i32;
  let fraction = bits & ((1u128 << 112) - 1);

  if exp == 0x7fff {
    if fraction == 0 {
      return if negative {
        f64::NEG_INFINITY
      } else {
        f64::INFINITY
      };
    }
    return f64::NAN;
  }

  let (mantissa, e) = if exp == 0 {
    (fraction, 1 - 16383)
  } else {
    (fraction | (1u128 << 112), exp - 16383)
  };

  compose(negative, mantissa, e - 112)
}
