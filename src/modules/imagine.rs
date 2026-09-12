//! Native backing for the `imagine` module: codecs, resampling, the
//! per-pixel kernels and glyph rasterization.
//!
//! # What lives here and what doesn't
//!
//! Everything in this file is O(width x height) work over a whole
//! buffer, or a format decoder. Nothing here knows what a rectangle
//! is, how a line is drawn, what "sepia" means, or which options a
//! PNG accepts. That all lives in `libs/imagine/`, in Zuri, because
//! that is the part of an image library people actually want to read
//! and extend.
//!
//! The split holds because of three primitives rather than a long
//! list of named filters:
//!
//! * `apply_lut` takes four 256-entry tables and rewrites every pixel
//!   through them. Brightness, contrast, gamma, levels, inversion,
//!   thresholding and posterization are all just different tables, and
//!   building a 256-byte table in Zuri costs nothing.
//! * `apply_matrix` takes a 4x5 colour matrix. Grayscale, sepia,
//!   saturation, hue rotation, tinting and channel swaps are all just
//!   different matrices.
//! * `convolve` takes an arbitrary square kernel. Blur, sharpen,
//!   emboss, edge detection and smoothing are all just different
//!   kernels.
//!
//! So adding a filter to Zuri's `imagine` is a Zuri change, not a Rust
//! one. Only the memory-bandwidth-bound loop is native.
//!
//! # Pixel format
//!
//! One format throughout: 8-bit RGBA, straight (not premultiplied),
//! row-major, no padding, so a pixel sits at `(y * width + x) * 4`.
//! Every function here takes and returns that, which is why the Zuri
//! side can hold an image as a plain `bytes` and write pixels into it
//! directly instead of paying a native call per pixel.
//!
//! # AVIF is encode-only
//!
//! Decoding AVIF needs an AV1 decoder, and the only mature one is
//! libdav1d, which is C. Every other format enabled here is pure Rust,
//! and keeping the build free of a C toolchain is worth more than AVIF
//! input. `capabilities()` reports this honestly so the Zuri layer can
//! raise a real error instead of a decode failure.

use std::io::Cursor;

use image::codecs::gif::{GifDecoder, GifEncoder, Repeat};
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder};
use image::imageops::FilterType;
use image::{
  AnimationDecoder, Delay, ExtendedColorType, Frame, ImageEncoder, ImageFormat, ImageReader,
  RgbaImage,
};

use crate::builtins::enforce::ArgType;
use crate::modules::{BuiltinModuleDef, native};
use crate::vm::object::ZuriContext;
use crate::vm::value::Value;
use crate::vm::vm::VM;
use crate::{enforce_arg_count, enforce_arg_range, enforce_arg_type, enforce_arg_type_any_of_opt};

pub static MODULE: BuiltinModuleDef = BuiltinModuleDef {
  name: "_imagine",
  build,
};

/// `ObjPtr` tag for a parsed font face. Named here rather than inline
/// so `enforce_arg_ptr!` and `alloc_ptr` can't drift apart.
const FONT_TAG: &str = "imagine_font";

fn build(vm: &mut VM) -> Vec<(&'static str, Value)> {
  vec![
    // codecs
    ("probe", native(vm, "probe", 1, false, probe)),
    ("decode", native(vm, "decode", 1, true, decode)),
    (
      "decode_frames",
      native(vm, "decode_frames", 1, true, decode_frames),
    ),
    ("encode", native(vm, "encode", 5, false, encode)),
    (
      "encode_frames",
      native(vm, "encode_frames", 3, false, encode_frames),
    ),
    (
      "orientation",
      native(vm, "orientation", 1, false, orientation),
    ),
    (
      "capabilities",
      native(vm, "capabilities", 0, false, capabilities),
    ),
    // geometry
    ("resize", native(vm, "resize", 6, false, resize)),
    ("rotate", native(vm, "rotate", 5, false, rotate)),
    ("flip", native(vm, "flip", 4, false, flip)),
    ("transpose", native(vm, "transpose", 3, false, transpose)),
    ("crop", native(vm, "crop", 7, false, crop)),
    // pixel kernels
    ("apply_lut", native(vm, "apply_lut", 5, false, apply_lut)),
    (
      "apply_matrix",
      native(vm, "apply_matrix", 2, false, apply_matrix),
    ),
    ("convolve", native(vm, "convolve", 8, false, convolve)),
    ("blur", native(vm, "blur", 4, false, blur)),
    ("composite", native(vm, "composite", 10, false, composite)),
    ("quantize", native(vm, "quantize", 5, false, quantize)),
    (
      "premultiply",
      native(vm, "premultiply", 2, false, premultiply),
    ),
    // text
    ("font_load", native(vm, "font_load", 1, false, font_load)),
    ("font_info", native(vm, "font_info", 2, false, font_info)),
    (
      "measure_text",
      native(vm, "measure_text", 4, false, measure_text),
    ),
    (
      "render_text",
      native(vm, "render_text", 4, false, render_text),
    ),
  ]
}

// ---------------------------------------------------------------------------
// small helpers
// ---------------------------------------------------------------------------

/// Builds a Zuri dict from string keys.
///
/// No GC pinning here, deliberately. A `Value` in a Rust local is not a
/// root, but it only needs to be one if a collection can run while the
/// local is holding it, and nothing in this module can make that happen:
/// allocation never collects (the collector runs at safepoints -- the
/// interpreter's dispatch loop, `jit::runtime::zuri_jit_safepoint`,
/// `VM::ensure_stable_for_compiled_entry`, the `gc()` native), and no
/// native here ever calls back into Zuri code to reach one.
///
/// That reasoning is what to re-check before pinning anything in this
/// module, and what stops the pattern being copied somewhere it would be
/// wrong: a native that DOES re-enter Zuri (`call_value`, `instantiate`,
/// constructing an instance) can be collected underneath, and every
/// `Value` it holds across that call has to come back out of `gc_pins`
/// rather than a local. `builtins::list`'s callback natives are the
/// worked example.
fn make_dict(ctx: &mut ZuriContext, pairs: Vec<(&str, Value)>) -> Value {
  let entries: Vec<(Value, Value)> = pairs
    .into_iter()
    .map(|(name, value)| (ctx.heap().alloc_string(name), value))
    .collect();

  ctx.heap().alloc_dict(entries)
}

/// Allocates a list of strings. See `make_dict` on why no pinning.
fn alloc_string_list(ctx: &mut ZuriContext, names: &[&str]) -> Value {
  let items: Vec<Value> = names
    .iter()
    .map(|name| ctx.heap().alloc_string(*name))
    .collect();

  ctx.heap().alloc_list(items)
}

fn num(ctx: &ZuriContext, index: usize) -> f64 {
  ctx.args[index].as_number()
}

/// A non-negative dimension argument, rejecting NaN, infinities and
/// anything that would wrap when it reaches a buffer index.
fn dim(ctx: &ZuriContext, index: usize, what: &str) -> Result<u32, String> {
  let raw = num(ctx, index);
  if !raw.is_finite() || raw < 0.0 || raw > u32::MAX as f64 {
    return Err(format!(
      "{}() got an out-of-range {}: {}",
      ctx.name, what, raw
    ));
  }
  Ok(raw as u32)
}

/// Checks that a buffer really is `width * height` RGBA pixels. Every
/// kernel here indexes without further bounds checks, so this is the
/// one place that has to be right.
fn check_buffer(name: &str, len: usize, width: u32, height: u32) -> Result<(), String> {
  let expected = (width as usize)
    .checked_mul(height as usize)
    .and_then(|n| n.checked_mul(4))
    .ok_or_else(|| format!("{}(): image dimensions {}x{} overflow", name, width, height))?;

  if len != expected {
    return Err(format!(
      "{}(): pixel buffer holds {} bytes, but {}x{} RGBA needs {}",
      name, len, width, height, expected
    ));
  }
  Ok(())
}

/// Turn an RGBA buffer into an `image` view without copying it twice.
fn to_image(name: &str, data: Vec<u8>, width: u32, height: u32) -> Result<RgbaImage, String> {
  RgbaImage::from_raw(width, height, data).ok_or_else(|| {
    format!(
      "{}(): pixel buffer does not match {}x{}",
      name, width, height
    )
  })
}

/// Optional dict lookup by string key.
fn option(options: Value, key: &str) -> Option<Value> {
  if !options.is_dict() {
    return None;
  }
  options.with_dict(|storage| {
    storage
      .entries
      .iter()
      .find(|(k, _)| k.is_string() && k.as_str() == key)
      .map(|(_, v)| *v)
  })
}

fn option_number(options: Value, key: &str, fallback: f64) -> f64 {
  match option(options, key) {
    Some(v) if v.is_number() => v.as_number(),
    _ => fallback,
  }
}

fn option_string(options: Value, key: &str) -> Option<String> {
  match option(options, key) {
    Some(v) if v.is_string() => Some(v.as_str().to_string()),
    _ => None,
  }
}

// ---------------------------------------------------------------------------
// formats
// ---------------------------------------------------------------------------

/// Zuri's format names, which are the lowercase extension-ish names a
/// user would type, mapped onto the crate's own enum. Deliberately not
/// just `ImageFormat::from_extension`, because that accepts spellings
/// (`"jfif"`, `"pbm"`) the Zuri API doesn't document and would then be
/// stuck supporting.
fn format_from_name(name: &str) -> Option<ImageFormat> {
  match name {
    "png" => Some(ImageFormat::Png),
    "jpeg" | "jpg" => Some(ImageFormat::Jpeg),
    "gif" => Some(ImageFormat::Gif),
    "bmp" => Some(ImageFormat::Bmp),
    "tiff" | "tif" => Some(ImageFormat::Tiff),
    "tga" => Some(ImageFormat::Tga),
    "webp" => Some(ImageFormat::WebP),
    "avif" => Some(ImageFormat::Avif),
    "qoi" => Some(ImageFormat::Qoi),
    "ico" => Some(ImageFormat::Ico),
    "pnm" | "ppm" | "pgm" | "pbm" => Some(ImageFormat::Pnm),
    _ => None,
  }
}

fn format_name(format: ImageFormat) -> &'static str {
  match format {
    ImageFormat::Png => "png",
    ImageFormat::Jpeg => "jpeg",
    ImageFormat::Gif => "gif",
    ImageFormat::Bmp => "bmp",
    ImageFormat::Tiff => "tiff",
    ImageFormat::Tga => "tga",
    ImageFormat::WebP => "webp",
    ImageFormat::Avif => "avif",
    ImageFormat::Qoi => "qoi",
    ImageFormat::Ico => "ico",
    ImageFormat::Pnm => "pnm",
    _ => "unknown",
  }
}

/// Formats we can read. AVIF is missing on purpose; see this module's
/// own docs.
const DECODABLE: &[&str] = &[
  "png", "jpeg", "gif", "bmp", "tiff", "tga", "webp", "qoi", "ico", "pnm",
];

const ENCODABLE: &[&str] = &[
  "png", "jpeg", "gif", "bmp", "tiff", "tga", "webp", "avif", "qoi", "ico", "pnm",
];

/// `_imagine.capabilities()`; what this build can actually do, so the
/// Zuri layer never advertises a format it would then fail on.
fn capabilities(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 0);

  let decode_list = alloc_string_list(ctx, DECODABLE);
  let encode_list = alloc_string_list(ctx, ENCODABLE);
  let animated_list = alloc_string_list(ctx, &["gif", "webp"]);

  let dict = make_dict(
    ctx,
    vec![
      ("decode", decode_list),
      ("encode", encode_list),
      ("animated", animated_list),
    ],
  );

  Ok(dict)
}

// ---------------------------------------------------------------------------
// decoding
// ---------------------------------------------------------------------------

/// `_imagine.probe(data)`; format and dimensions from the header
/// alone, without decoding pixels. Returns nil when the data isn't a
/// recognisable image, which is the cheap way for the Zuri layer to
/// answer "is this a PNG" over an upload.
fn probe(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);

  let probed = ctx.args[0].with_bytes(|data| {
    let reader = match ImageReader::new(Cursor::new(data)).with_guessed_format() {
      Ok(reader) => reader,
      Err(_) => return None,
    };
    let format = reader.format()?;
    let (width, height) = reader.into_dimensions().ok()?;
    Some((format, width, height))
  });

  let Some((format, width, height)) = probed else {
    return Ok(Value::nil());
  };

  let format_value = ctx.heap().alloc_string(format_name(format));

  Ok(make_dict(
    ctx,
    vec![
      ("format", format_value),
      ("width", Value::number(width as f64)),
      ("height", Value::number(height as f64)),
    ],
  ))
}

/// `_imagine.decode(data [, format])`; the whole image as straight
/// RGBA. An explicit format skips sniffing, which matters when the
/// caller already knows (a `Content-Type`, a file extension) and wants
/// a mislabelled file to fail loudly rather than be silently decoded
/// as whatever it really is.
fn decode(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_range!(ctx, 1, 2);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);
  enforce_arg_type_any_of_opt!(ctx, 1, [ArgType::String, ArgType::Nil]);

  let explicit = match ctx.args.get(1) {
    Some(v) if v.is_string() => {
      let name = v.as_str().to_lowercase();
      Some(format_from_name(&name).ok_or_else(|| format!("decode(): unknown format '{}'", name))?)
    },
    _ => None,
  };

  if explicit == Some(ImageFormat::Avif) {
    return Err("decode(): AVIF decoding is not supported by this build".to_string());
  }

  let decoded = ctx.args[0].with_bytes(|data| -> Result<(RgbaImage, ImageFormat), String> {
    let reader = match explicit {
      Some(format) => ImageReader::with_format(Cursor::new(data), format),
      None => ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| format!("decode(): {}", e))?,
    };

    let format = reader
      .format()
      .ok_or_else(|| "decode(): unrecognised image format".to_string())?;

    if format == ImageFormat::Avif {
      return Err("decode(): AVIF decoding is not supported by this build".to_string());
    }

    let image = reader
      .decode()
      .map_err(|e| format!("decode(): {}", describe_image_error(&e)))?;

    Ok((image.to_rgba8(), format))
  })?;

  let (image, format) = decoded;
  let width = image.width();
  let height = image.height();

  let pixels = ctx.heap().alloc_bytes(image.into_raw());
  let format_value = ctx.heap().alloc_string(format_name(format));

  let dict = make_dict(
    ctx,
    vec![
      ("pixels", pixels),
      ("width", Value::number(width as f64)),
      ("height", Value::number(height as f64)),
      ("format", format_value),
    ],
  );

  Ok(dict)
}

/// `_imagine.decode_frames(data [, format])`; every frame of an
/// animation, already composited against the previous frame so each
/// one is a complete image. GIF's own disposal and transparency rules
/// are handled by the decoder, which is the whole reason not to expose
/// raw frames to Zuri.
fn decode_frames(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_range!(ctx, 1, 2);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);
  enforce_arg_type_any_of_opt!(ctx, 1, [ArgType::String, ArgType::Nil]);

  let explicit = match ctx.args.get(1) {
    Some(v) if v.is_string() => {
      let name = v.as_str().to_lowercase();
      Some(
        format_from_name(&name)
          .ok_or_else(|| format!("decode_frames(): unknown format '{}'", name))?,
      )
    },
    _ => None,
  };

  // Collected fully before anything is allocated: the `bytes` borrow
  // is live for the whole closure, and allocating under it would let
  // a collection re-enter the same RefCell.
  let frames = ctx.args[0].with_bytes(|data| -> Result<Vec<(Vec<u8>, u32, u32, f64)>, String> {
    let format = match explicit {
      Some(format) => format,
      None => image::guess_format(data).map_err(|e| format!("decode_frames(): {}", e))?,
    };

    let collected = match format {
      ImageFormat::Gif => {
        let decoder = GifDecoder::new(Cursor::new(data))
          .map_err(|e| format!("decode_frames(): {}", describe_image_error(&e)))?;
        decoder.into_frames().collect_frames()
      },
      ImageFormat::WebP => {
        let decoder = image::codecs::webp::WebPDecoder::new(Cursor::new(data))
          .map_err(|e| format!("decode_frames(): {}", describe_image_error(&e)))?;
        decoder.into_frames().collect_frames()
      },
      other => {
        return Err(format!(
          "decode_frames(): {} is not an animated format",
          format_name(other)
        ));
      },
    }
    .map_err(|e| format!("decode_frames(): {}", describe_image_error(&e)))?;

    Ok(
      collected
        .into_iter()
        .map(|frame| {
          let (numerator, denominator) = frame.delay().numer_denom_ms();
          let delay = if denominator == 0 {
            0.0
          } else {
            numerator as f64 / denominator as f64
          };
          let buffer = frame.into_buffer();
          let (width, height) = (buffer.width(), buffer.height());
          (buffer.into_raw(), width, height, delay)
        })
        .collect(),
    )
  })?;

  let mut items: Vec<Value> = Vec::with_capacity(frames.len());

  for (raw, width, height, delay) in frames {
    let pixels = ctx.heap().alloc_bytes(raw);

    items.push(make_dict(
      ctx,
      vec![
        ("pixels", pixels),
        ("width", Value::number(width as f64)),
        ("height", Value::number(height as f64)),
        ("delay", Value::number(delay)),
      ],
    ));
  }

  let list = ctx.heap().alloc_list(items);

  Ok(list)
}

/// `image`'s own error text is often just "The image format could not
/// be determined"; prefixing it with the underlying kind makes a
/// truncated upload distinguishable from an unsupported one.
fn describe_image_error(error: &image::ImageError) -> String {
  match error {
    image::ImageError::IoError(io) if io.kind() == std::io::ErrorKind::UnexpectedEof => {
      "image data ends unexpectedly (truncated file?)".to_string()
    },
    other => other.to_string(),
  }
}

/// `_imagine.orientation(data)`; the EXIF orientation tag, 1 through
/// 8, or 0 when there isn't one. Phone cameras almost always store the
/// sensor's own orientation here rather than rotating the pixels, so a
/// library that ignores it shows a lot of sideways photographs.
///
/// Parsed directly rather than through a crate: this is one tag in one
/// IFD, and the alternative is a dependency for thirty lines.
fn orientation(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);

  let value = ctx.args[0].with_bytes(exif_orientation);
  Ok(Value::number(value as f64))
}

/// Walks a JPEG's segment list for an APP1/Exif block, then that
/// block's IFD0 for tag 0x0112. Returns 0 for anything it can't
/// follow, which the caller treats as "no orientation".
fn exif_orientation(data: &[u8]) -> u16 {
  if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 {
    return 0;
  }

  let mut offset = 2usize;
  while offset + 4 <= data.len() {
    if data[offset] != 0xFF {
      return 0;
    }

    let marker = data[offset + 1];
    // Standalone markers carry no length field.
    if marker == 0xD8 || marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
      offset += 2;
      continue;
    }
    // Start of scan; entropy-coded data follows and there is no more
    // metadata to find.
    if marker == 0xDA || marker == 0xD9 {
      return 0;
    }

    let length = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize;
    if length < 2 || offset + 2 + length > data.len() {
      return 0;
    }

    let segment = &data[offset + 4..offset + 2 + length];
    if marker == 0xE1 && segment.len() > 6 && &segment[..6] == b"Exif\0\0" {
      return tiff_orientation(&segment[6..]);
    }

    offset += 2 + length;
  }

  0
}

/// The TIFF header and IFD0 walk behind an Exif block.
fn tiff_orientation(tiff: &[u8]) -> u16 {
  if tiff.len() < 8 {
    return 0;
  }

  let big_endian = match &tiff[..2] {
    b"MM" => true,
    b"II" => false,
    _ => return 0,
  };

  let read16 = |bytes: &[u8]| -> u16 {
    if big_endian {
      u16::from_be_bytes([bytes[0], bytes[1]])
    } else {
      u16::from_le_bytes([bytes[0], bytes[1]])
    }
  };
  let read32 = |bytes: &[u8]| -> u32 {
    if big_endian {
      u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    } else {
      u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    }
  };

  if read16(&tiff[2..4]) != 42 {
    return 0;
  }

  let ifd = read32(&tiff[4..8]) as usize;
  if ifd + 2 > tiff.len() {
    return 0;
  }

  let count = read16(&tiff[ifd..ifd + 2]) as usize;
  for index in 0..count {
    let entry = ifd + 2 + index * 12;
    if entry + 12 > tiff.len() {
      return 0;
    }
    if read16(&tiff[entry..entry + 2]) == 0x0112 {
      // A SHORT value sits in the first two bytes of the value field.
      return read16(&tiff[entry + 8..entry + 10]);
    }
  }

  0
}

// ---------------------------------------------------------------------------
// encoding
// ---------------------------------------------------------------------------

/// `_imagine.encode(pixels, width, height, format, options)`.
///
/// Options understood, per format: `quality` (JPEG, AVIF),
/// `speed` (AVIF 1-10, GIF 1-30), `compression` (PNG:
/// "default"/"fast"/"best"). Anything else is ignored rather than rejected,
/// so the Zuri layer can pass one options dict through to whichever
/// encoder the caller picked.
fn encode(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 5);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);
  enforce_arg_type!(ctx, 1, ArgType::Number);
  enforce_arg_type!(ctx, 2, ArgType::Number);
  enforce_arg_type!(ctx, 3, ArgType::String);

  let width = dim(ctx, 1, "width")?;
  let height = dim(ctx, 2, "height")?;
  let name = ctx.args[3].as_str().to_lowercase();
  let options = ctx.args[4];

  let format =
    format_from_name(&name).ok_or_else(|| format!("encode(): unknown format '{}'", name))?;

  if width == 0 || height == 0 {
    return Err("encode(): cannot encode an image with a zero dimension".to_string());
  }

  let data = ctx.args[0].with_bytes(|pixels| -> Result<Vec<u8>, String> {
    check_buffer("encode", pixels.len(), width, height)?;
    encode_rgba(pixels, width, height, format, options)
  })?;

  Ok(ctx.heap().alloc_bytes(data))
}

fn encode_rgba(
  pixels: &[u8],
  width: u32,
  height: u32,
  format: ImageFormat,
  options: Value,
) -> Result<Vec<u8>, String> {
  let mut out = Vec::new();

  match format {
    ImageFormat::Png => {
      let compression = match option_string(options, "compression").as_deref() {
        Some("fast") => CompressionType::Fast,
        Some("best") => CompressionType::Best,
        Some("default") | None => CompressionType::Default,
        Some(other) => {
          return Err(format!(
            "encode(): unknown png compression '{}', expected default, fast or best",
            other
          ));
        },
      };

      PngEncoder::new_with_quality(&mut out, compression, PngFilter::Adaptive)
        .write_image(pixels, width, height, ExtendedColorType::Rgba8)
        .map_err(|e| format!("encode(): {}", e))?;
    },

    ImageFormat::Jpeg => {
      let quality = clamp_quality(option_number(options, "quality", 85.0));
      // JPEG has no alpha channel. Flattening against the background
      // the caller asked for (white by default) is the only honest
      // answer; silently dropping alpha turns transparent pixels black.
      let background = option_number(options, "background", 0xFFFFFFFF_u32 as f64) as u32;
      let flattened = flatten(pixels, background);

      JpegEncoder::new_with_quality(&mut out, quality)
        .write_image(&flattened, width, height, ExtendedColorType::Rgb8)
        .map_err(|e| format!("encode(): {}", e))?;
    },

    ImageFormat::Avif => {
      let quality = clamp_quality(option_number(options, "quality", 80.0));
      let speed = option_number(options, "speed", 6.0).clamp(1.0, 10.0) as u8;

      image::codecs::avif::AvifEncoder::new_with_speed_quality(&mut out, speed, quality)
        .write_image(pixels, width, height, ExtendedColorType::Rgba8)
        .map_err(|e| format!("encode(): {}", e))?;
    },

    ImageFormat::Gif => {
      // The GIF encoder wants whole frames, and quantizes internally.
      //
      // The speed is set explicitly because the encoder's own default
      // is 1, the slowest of its 1-30 range, for a quality difference
      // this format's 256 colours mostly cannot show.
      let buffer = to_image("encode", pixels.to_vec(), width, height)?;
      let mut encoder = GifEncoder::new_with_speed(&mut out, gif_speed(options));
      encoder
        .encode(&buffer, width, height, ExtendedColorType::Rgba8)
        .map_err(|e| format!("encode(): {}", e))?;
      drop(encoder);
    },

    other => {
      let buffer = to_image("encode", pixels.to_vec(), width, height)?;
      buffer
        .write_to(&mut Cursor::new(&mut out), other)
        .map_err(|e| format!("encode(): {}", e))?;
    },
  }

  Ok(out)
}

/// The GIF quantizer's effort, 1 (slowest, best) to 30 (fastest).
fn gif_speed(options: Value) -> i32 {
  let raw = option_number(options, "speed", 15.0);

  if !raw.is_finite() {
    return 15;
  }

  raw.clamp(1.0, 30.0) as i32
}

fn clamp_quality(raw: f64) -> u8 {
  if !raw.is_finite() {
    return 85;
  }
  raw.clamp(1.0, 100.0) as u8
}

/// RGBA to RGB, compositing against an opaque background colour given
/// as packed 0xRRGGBBAA (the alpha byte is ignored; a background is
/// opaque by definition).
fn flatten(pixels: &[u8], background: u32) -> Vec<u8> {
  let br = ((background >> 24) & 0xFF) as u32;
  let bg = ((background >> 16) & 0xFF) as u32;
  let bb = ((background >> 8) & 0xFF) as u32;

  let mut out = Vec::with_capacity(pixels.len() / 4 * 3);
  for pixel in pixels.chunks_exact(4) {
    let alpha = pixel[3] as u32;
    if alpha == 255 {
      out.extend_from_slice(&pixel[..3]);
      continue;
    }
    let inverse = 255 - alpha;
    out.push(((pixel[0] as u32 * alpha + br * inverse) / 255) as u8);
    out.push(((pixel[1] as u32 * alpha + bg * inverse) / 255) as u8);
    out.push(((pixel[2] as u32 * alpha + bb * inverse) / 255) as u8);
  }
  out
}

/// `_imagine.encode_frames(frames, format, options)`; an animation,
/// where `frames` is a list of `{pixels, width, height, delay}` dicts.
/// Only GIF is supported: the pure-Rust WebP encoder writes stills
/// only, which `capabilities()` already reports.
fn encode_frames(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 3);
  enforce_arg_type!(ctx, 0, ArgType::List);
  enforce_arg_type!(ctx, 1, ArgType::String);

  let name = ctx.args[1].as_str().to_lowercase();
  if name != "gif" {
    return Err(format!(
      "encode_frames(): {} animations cannot be written by this build, only gif",
      name
    ));
  }

  let options = ctx.args[2];
  let repeat = option_number(options, "repeat", 0.0);

  let entries: Vec<Value> = ctx.args[0].with_list(|items| items.to_vec());
  if entries.is_empty() {
    return Err("encode_frames(): an animation needs at least one frame".to_string());
  }

  let mut frames = Vec::with_capacity(entries.len());
  for (index, entry) in entries.iter().enumerate() {
    if !entry.is_dict() {
      return Err(format!(
        "encode_frames(): frame {} is {}, expected a dict",
        index + 1,
        entry.type_name()
      ));
    }

    let pixels = option(*entry, "pixels")
      .filter(|v| v.is_bytes())
      .ok_or_else(|| format!("encode_frames(): frame {} has no pixels", index + 1))?;
    let width = option_number(*entry, "width", 0.0) as u32;
    let height = option_number(*entry, "height", 0.0) as u32;
    let delay = option_number(*entry, "delay", 100.0).max(0.0);

    let raw = pixels.with_bytes(|data| -> Result<Vec<u8>, String> {
      check_buffer("encode_frames", data.len(), width, height)?;
      Ok(data.to_vec())
    })?;

    let buffer = to_image("encode_frames", raw, width, height)?;
    frames.push(Frame::from_parts(
      buffer,
      0,
      0,
      Delay::from_saturating_duration(std::time::Duration::from_micros((delay * 1000.0) as u64)),
    ));
  }

  let mut out = Vec::new();
  {
    let mut encoder = GifEncoder::new_with_speed(&mut out, gif_speed(options));
    let repeat = if repeat <= 0.0 {
      Repeat::Infinite
    } else {
      Repeat::Finite(repeat.min(u16::MAX as f64) as u16)
    };
    encoder
      .set_repeat(repeat)
      .map_err(|e| format!("encode_frames(): {}", e))?;
    encoder
      .encode_frames(frames)
      .map_err(|e| format!("encode_frames(): {}", e))?;
  }

  Ok(ctx.heap().alloc_bytes(out))
}

// ---------------------------------------------------------------------------
// geometry
// ---------------------------------------------------------------------------

fn filter_from_name(name: &str) -> Option<FilterType> {
  match name {
    "nearest" => Some(FilterType::Nearest),
    "bilinear" | "triangle" => Some(FilterType::Triangle),
    "bicubic" | "catmull_rom" => Some(FilterType::CatmullRom),
    "gaussian" => Some(FilterType::Gaussian),
    "lanczos" | "lanczos3" => Some(FilterType::Lanczos3),
    _ => None,
  }
}

/// `_imagine.resize(pixels, width, height, new_width, new_height, filter)`.
fn resize(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 6);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);
  enforce_arg_type!(ctx, 5, ArgType::String);

  let width = dim(ctx, 1, "width")?;
  let height = dim(ctx, 2, "height")?;
  let new_width = dim(ctx, 3, "width")?;
  let new_height = dim(ctx, 4, "height")?;

  let filter_name = ctx.args[5].as_str().to_lowercase();
  let filter = filter_from_name(&filter_name)
    .ok_or_else(|| format!("resize(): unknown filter '{}'", filter_name))?;

  if new_width == 0 || new_height == 0 {
    return Err("resize(): target dimensions must both be at least 1".to_string());
  }

  let raw = ctx.args[0].with_bytes(|pixels| -> Result<Vec<u8>, String> {
    check_buffer("resize", pixels.len(), width, height)?;
    Ok(pixels.to_vec())
  })?;

  let source = to_image("resize", raw, width, height)?;
  let resized = image::imageops::resize(&source, new_width, new_height, filter);

  Ok(ctx.heap().alloc_bytes(resized.into_raw()))
}

/// `_imagine.rotate(pixels, width, height, degrees, background)`.
///
/// Exact quarter turns go through the lossless paths; anything else is
/// a bilinear resample into a canvas grown to hold the rotated corners,
/// with the gaps filled by `background` (packed 0xRRGGBBAA). Returns
/// the new dimensions alongside the pixels because they change.
fn rotate(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 5);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);

  let width = dim(ctx, 1, "width")?;
  let height = dim(ctx, 2, "height")?;
  let degrees = num(ctx, 3);
  let background = num(ctx, 4);

  if !degrees.is_finite() {
    return Err("rotate(): angle must be a finite number of degrees".to_string());
  }

  let raw = ctx.args[0].with_bytes(|pixels| -> Result<Vec<u8>, String> {
    check_buffer("rotate", pixels.len(), width, height)?;
    Ok(pixels.to_vec())
  })?;

  let source = to_image("rotate", raw, width, height)?;

  // Normalised to [0, 360) so -90 and 270 take the same fast path.
  let normalised = degrees.rem_euclid(360.0);
  let rotated = if (normalised - 0.0).abs() < f64::EPSILON {
    source
  } else if (normalised - 90.0).abs() < f64::EPSILON {
    image::imageops::rotate90(&source)
  } else if (normalised - 180.0).abs() < f64::EPSILON {
    image::imageops::rotate180(&source)
  } else if (normalised - 270.0).abs() < f64::EPSILON {
    image::imageops::rotate270(&source)
  } else {
    rotate_free(&source, normalised, pack_to_rgba(background))
  };

  let new_width = rotated.width();
  let new_height = rotated.height();
  let pixels = ctx.heap().alloc_bytes(rotated.into_raw());

  Ok(make_dict(
    ctx,
    vec![
      ("pixels", pixels),
      ("width", Value::number(new_width as f64)),
      ("height", Value::number(new_height as f64)),
    ],
  ))
}

fn pack_to_rgba(packed: f64) -> [u8; 4] {
  let bits = if packed.is_finite() && packed >= 0.0 {
    packed as u64 as u32
  } else {
    0
  };
  [
    ((bits >> 24) & 0xFF) as u8,
    ((bits >> 16) & 0xFF) as u8,
    ((bits >> 8) & 0xFF) as u8,
    (bits & 0xFF) as u8,
  ]
}

/// Rotation by an arbitrary angle, sampling the source bilinearly.
///
/// Works backwards, from each destination pixel to where it came from
/// in the source, which is what keeps the output free of the holes a
/// forward mapping leaves behind.
fn rotate_free(source: &RgbaImage, degrees: f64, background: [u8; 4]) -> RgbaImage {
  let radians = degrees.to_radians();
  let (sin, cos) = radians.sin_cos();

  let width = source.width() as f64;
  let height = source.height() as f64;

  // The rotated bounding box, from the two half-extents.
  let new_width = (width * cos.abs() + height * sin.abs()).ceil().max(1.0);
  let new_height = (width * sin.abs() + height * cos.abs()).ceil().max(1.0);

  let mut out = RgbaImage::from_pixel(new_width as u32, new_height as u32, image::Rgba(background));

  let source_cx = width / 2.0;
  let source_cy = height / 2.0;
  let dest_cx = new_width / 2.0;
  let dest_cy = new_height / 2.0;

  for y in 0..out.height() {
    for x in 0..out.width() {
      let dx = x as f64 + 0.5 - dest_cx;
      let dy = y as f64 + 0.5 - dest_cy;

      // Inverse rotation; note the sign flip against the forward form.
      let sx = dx * cos + dy * sin + source_cx - 0.5;
      let sy = -dx * sin + dy * cos + source_cy - 0.5;

      if let Some(pixel) = sample_bilinear(source, sx, sy) {
        out.put_pixel(x, y, image::Rgba(pixel));
      }
    }
  }

  out
}

/// Bilinear sample at a fractional source position, or `None` when the
/// position falls outside the image entirely.
///
/// Alpha is weighted along with the colour channels rather than
/// separately, which is right for straight (non-premultiplied) RGBA as
/// long as fully transparent pixels carry a sensible colour. Decoders
/// give us that; `Image` fills new buffers with zeroes, so an edge
/// against transparency darkens very slightly. Premultiplying for the
/// duration of a rotate would cost two extra passes over the buffer
/// for a difference invisible at 8 bits.
fn sample_bilinear(source: &RgbaImage, sx: f64, sy: f64) -> Option<[u8; 4]> {
  let width = source.width() as i64;
  let height = source.height() as i64;

  if sx < -1.0 || sy < -1.0 || sx > width as f64 || sy > height as f64 {
    return None;
  }

  let x0 = sx.floor() as i64;
  let y0 = sy.floor() as i64;
  let fx = sx - x0 as f64;
  let fy = sy - y0 as f64;

  let at = |x: i64, y: i64| -> [f64; 4] {
    if x < 0 || y < 0 || x >= width || y >= height {
      return [0.0; 4];
    }
    let pixel = source.get_pixel(x as u32, y as u32).0;
    [
      pixel[0] as f64,
      pixel[1] as f64,
      pixel[2] as f64,
      pixel[3] as f64,
    ]
  };

  let tl = at(x0, y0);
  let tr = at(x0 + 1, y0);
  let bl = at(x0, y0 + 1);
  let br = at(x0 + 1, y0 + 1);

  let mut out = [0u8; 4];
  for channel in 0..4 {
    let top = tl[channel] * (1.0 - fx) + tr[channel] * fx;
    let bottom = bl[channel] * (1.0 - fx) + br[channel] * fx;
    out[channel] = (top * (1.0 - fy) + bottom * fy).round().clamp(0.0, 255.0) as u8;
  }

  // Everything sampled was outside the image, so this destination
  // pixel keeps the background rather than a black transparent one.
  if out[3] == 0 && (x0 < -1 || y0 < -1 || x0 > width || y0 > height) {
    return None;
  }

  Some(out)
}

/// `_imagine.flip(pixels, width, height, mode)`; mode 1 horizontal,
/// 2 vertical, 3 both.
fn flip(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 4);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);

  let width = dim(ctx, 1, "width")?;
  let height = dim(ctx, 2, "height")?;
  let mode = num(ctx, 3) as i64;

  if !(1..=3).contains(&mode) {
    return Err(format!(
      "flip(): mode must be 1 (horizontal), 2 (vertical) or 3 (both), got {}",
      mode
    ));
  }

  let flipped = ctx.args[0].with_bytes(|pixels| -> Result<Vec<u8>, String> {
    check_buffer("flip", pixels.len(), width, height)?;

    let stride = width as usize * 4;
    let mut out = vec![0u8; pixels.len()];

    for y in 0..height as usize {
      let source_y = if mode >= 2 {
        height as usize - 1 - y
      } else {
        y
      };
      let source_row = &pixels[source_y * stride..source_y * stride + stride];
      let dest_row = &mut out[y * stride..y * stride + stride];

      if mode == 2 {
        dest_row.copy_from_slice(source_row);
        continue;
      }

      for x in 0..width as usize {
        let from = (width as usize - 1 - x) * 4;
        dest_row[x * 4..x * 4 + 4].copy_from_slice(&source_row[from..from + 4]);
      }
    }

    Ok(out)
  })?;

  Ok(ctx.heap().alloc_bytes(flipped))
}

/// `_imagine.transpose(pixels, width, height)`; reflects across the
/// main diagonal, so the result is `height` wide by `width` tall.
fn transpose(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 3);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);

  let width = dim(ctx, 1, "width")?;
  let height = dim(ctx, 2, "height")?;

  let transposed = ctx.args[0].with_bytes(|pixels| -> Result<Vec<u8>, String> {
    check_buffer("transpose", pixels.len(), width, height)?;

    let mut out = vec![0u8; pixels.len()];
    let source_stride = width as usize * 4;
    let dest_stride = height as usize * 4;

    for y in 0..height as usize {
      for x in 0..width as usize {
        let from = y * source_stride + x * 4;
        let to = x * dest_stride + y * 4;
        out[to..to + 4].copy_from_slice(&pixels[from..from + 4]);
      }
    }

    Ok(out)
  })?;

  Ok(ctx.heap().alloc_bytes(transposed))
}

/// `_imagine.crop(pixels, width, height, x, y, crop_width, crop_height)`.
///
/// The rectangle must lie entirely inside the image; clamping a
/// too-large crop would quietly hand back different dimensions than
/// the caller asked for, and the Zuri layer is the right place to
/// decide whether to clamp or refuse.
fn crop(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 7);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);

  let width = dim(ctx, 1, "width")?;
  let height = dim(ctx, 2, "height")?;
  let x = dim(ctx, 3, "x offset")?;
  let y = dim(ctx, 4, "y offset")?;
  let crop_width = dim(ctx, 5, "width")?;
  let crop_height = dim(ctx, 6, "height")?;

  if crop_width == 0 || crop_height == 0 {
    return Err("crop(): the crop rectangle must be at least 1x1".to_string());
  }
  if x + crop_width > width || y + crop_height > height {
    return Err(format!(
      "crop(): {}x{} at ({}, {}) does not fit inside a {}x{} image",
      crop_width, crop_height, x, y, width, height
    ));
  }

  let cropped = ctx.args[0].with_bytes(|pixels| -> Result<Vec<u8>, String> {
    check_buffer("crop", pixels.len(), width, height)?;

    let source_stride = width as usize * 4;
    let dest_stride = crop_width as usize * 4;
    let mut out = vec![0u8; dest_stride * crop_height as usize];

    for row in 0..crop_height as usize {
      let from = (y as usize + row) * source_stride + x as usize * 4;
      let to = row * dest_stride;
      out[to..to + dest_stride].copy_from_slice(&pixels[from..from + dest_stride]);
    }

    Ok(out)
  })?;

  Ok(ctx.heap().alloc_bytes(cropped))
}

// ---------------------------------------------------------------------------
// pixel kernels
// ---------------------------------------------------------------------------

/// Reads a 256-entry lookup table argument, or `None` when the caller
/// passed nil for "leave this channel alone".
fn lut_arg(ctx: &ZuriContext, index: usize) -> Result<Option<[u8; 256]>, String> {
  let value = ctx.args[index];
  if value.is_nil() {
    return Ok(None);
  }
  if !value.is_bytes() {
    return Err(format!(
      "{}() expects argument {} to be bytes or nil, got {}",
      ctx.name,
      index + 1,
      value.type_name()
    ));
  }

  value.with_bytes(|table| {
    if table.len() != 256 {
      return Err(format!(
        "{}(): a lookup table needs exactly 256 entries, got {}",
        ctx.name,
        table.len()
      ));
    }
    let mut out = [0u8; 256];
    out.copy_from_slice(table);
    Ok(Some(out))
  })
}

/// `_imagine.apply_lut(pixels, red, green, blue, alpha)`; rewrites the
/// buffer in place, each channel through its own 256-entry table. A nil
/// table leaves that channel untouched.
///
/// This is the workhorse behind most of `imagine`'s colour adjustments.
/// Building a table costs 256 iterations in Zuri no matter how large
/// the image is, and applying it is one indexed load per channel.
fn apply_lut(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 5);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);

  let red = lut_arg(ctx, 1)?;
  let green = lut_arg(ctx, 2)?;
  let blue = lut_arg(ctx, 3)?;
  let alpha = lut_arg(ctx, 4)?;

  ctx.args[0].with_bytes_mut(|pixels| {
    if pixels.len() % 4 != 0 {
      return Err("apply_lut(): pixel buffer length is not a multiple of 4".to_string());
    }

    for pixel in pixels.chunks_exact_mut(4) {
      if let Some(table) = &red {
        pixel[0] = table[pixel[0] as usize];
      }
      if let Some(table) = &green {
        pixel[1] = table[pixel[1] as usize];
      }
      if let Some(table) = &blue {
        pixel[2] = table[pixel[2] as usize];
      }
      if let Some(table) = &alpha {
        pixel[3] = table[pixel[3] as usize];
      }
    }

    Ok(())
  })?;

  Ok(Value::nil())
}

/// `_imagine.apply_matrix(pixels, matrix)`; rewrites the buffer in
/// place through a 4x5 colour matrix, laid out row-major:
///
/// ```text
/// r' = m0*r  + m1*g  + m2*b  + m3*a  + m4
/// g' = m5*r  + m6*g  + m7*b  + m8*a  + m9
/// b' = m10*r + m11*g + m12*b + m13*a + m14
/// a' = m15*r + m16*g + m17*b + m18*a + m19
/// ```
///
/// The constant column (m4, m9, m14, m19) is in 0-255 units, not
/// normalised, so a matrix that adds 20 to red really does put 20 in
/// m4. Channels are clamped after the multiply, never wrapped.
fn apply_matrix(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);
  enforce_arg_type!(ctx, 1, ArgType::List);

  let mut matrix = [0f32; 20];
  let read = ctx.args[1].with_list(|items| {
    if items.len() != 20 {
      return Err(format!(
        "apply_matrix(): a colour matrix needs 20 numbers, got {}",
        items.len()
      ));
    }
    for (index, item) in items.iter().enumerate() {
      if !item.is_number() {
        return Err(format!(
          "apply_matrix(): entry {} is {}, expected a number",
          index + 1,
          item.type_name()
        ));
      }
      matrix[index] = item.as_number() as f32;
    }
    Ok(())
  });
  read?;

  ctx.args[0].with_bytes_mut(|pixels| {
    if pixels.len() % 4 != 0 {
      return Err("apply_matrix(): pixel buffer length is not a multiple of 4".to_string());
    }

    for pixel in pixels.chunks_exact_mut(4) {
      let r = pixel[0] as f32;
      let g = pixel[1] as f32;
      let b = pixel[2] as f32;
      let a = pixel[3] as f32;

      let nr = matrix[0] * r + matrix[1] * g + matrix[2] * b + matrix[3] * a + matrix[4];
      let ng = matrix[5] * r + matrix[6] * g + matrix[7] * b + matrix[8] * a + matrix[9];
      let nb = matrix[10] * r + matrix[11] * g + matrix[12] * b + matrix[13] * a + matrix[14];
      let na = matrix[15] * r + matrix[16] * g + matrix[17] * b + matrix[18] * a + matrix[19];

      pixel[0] = nr.clamp(0.0, 255.0) as u8;
      pixel[1] = ng.clamp(0.0, 255.0) as u8;
      pixel[2] = nb.clamp(0.0, 255.0) as u8;
      pixel[3] = na.clamp(0.0, 255.0) as u8;
    }

    Ok(())
  })?;

  Ok(Value::nil())
}

/// How a convolution reads pixels off the edge of the image.
#[derive(Clone, Copy, PartialEq)]
enum EdgeMode {
  /// Repeat the nearest edge pixel; the usual choice, and what keeps a
  /// blur from developing a dark halo.
  Clamp,
  /// Treat everything outside as transparent black.
  Transparent,
  /// Wrap to the opposite edge, for tiling images.
  Wrap,
}

fn edge_from_name(name: &str) -> Option<EdgeMode> {
  match name {
    "clamp" => Some(EdgeMode::Clamp),
    "transparent" => Some(EdgeMode::Transparent),
    "wrap" => Some(EdgeMode::Wrap),
    _ => None,
  }
}

/// `_imagine.convolve(pixels, width, height, kernel, divisor, offset, edge, keep_alpha)`.
///
/// The kernel is a flat list of `size * size` numbers with an odd
/// `size`. A divisor of 0 means "sum the kernel", which is what almost
/// every published kernel expects; passing an explicit divisor is for
/// the ones whose weights sum to zero (edge detection).
///
/// `keep_alpha` decides whether the alpha channel goes through the
/// kernel too. Convolving it is right for a blur, where softening a
/// shape's edge is the whole point. It is wrong for anything whose
/// weights do not sum to one: a Laplacian over a uniformly opaque
/// image sums to zero, which would turn the entire result invisible.
fn convolve(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 8);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);
  enforce_arg_type!(ctx, 3, ArgType::List);
  enforce_arg_type!(ctx, 6, ArgType::String);

  let width = dim(ctx, 1, "width")?;
  let height = dim(ctx, 2, "height")?;
  let mut divisor = num(ctx, 4) as f32;
  let offset = num(ctx, 5) as f32;

  let edge_name = ctx.args[6].as_str().to_lowercase();
  let edge = edge_from_name(&edge_name)
    .ok_or_else(|| format!("convolve(): unknown edge mode '{}'", edge_name))?;

  let keep_alpha = ctx.args[7].is_bool() && ctx.args[7].as_bool();

  let kernel = ctx.args[3].with_list(|items| -> Result<Vec<f32>, String> {
    let mut out = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
      if !item.is_number() {
        return Err(format!(
          "convolve(): kernel entry {} is {}, expected a number",
          index + 1,
          item.type_name()
        ));
      }
      out.push(item.as_number() as f32);
    }
    Ok(out)
  })?;

  let size = (kernel.len() as f64).sqrt().round() as usize;
  if size * size != kernel.len() || size == 0 || size % 2 == 0 {
    return Err(format!(
      "convolve(): a kernel must be square with an odd side length, got {} entries",
      kernel.len()
    ));
  }

  if divisor == 0.0 {
    let sum: f32 = kernel.iter().sum();
    divisor = if sum == 0.0 { 1.0 } else { sum };
  }

  let convolved = ctx.args[0].with_bytes(|pixels| -> Result<Vec<u8>, String> {
    check_buffer("convolve", pixels.len(), width, height)?;
    Ok(convolve_buffer(
      pixels, width, height, &kernel, size, divisor, offset, edge, keep_alpha,
    ))
  })?;

  Ok(ctx.heap().alloc_bytes(convolved))
}

#[allow(clippy::too_many_arguments)]
/// Applies `kernel` to every pixel.
///
/// Split into an interior and a border because they want different
/// code. Away from the edges every tap is guaranteed in bounds, so the
/// whole footprint collapses to a set of fixed byte offsets from the
/// centre pixel and the inner loop becomes a multiply-accumulate over
/// those. That is worth separating out: the edge-mode decision used to
/// run per tap per pixel, which for a 3x3 kernel meant nine bounds
/// tests and nine clamps to produce nine multiplies, and it only ever
/// changed the answer on the handful of pixels within `radius` of a
/// side.
///
/// Zero weights are dropped up front rather than skipped in the loop,
/// which matters for the sparse kernels (edge detection, embossing)
/// where most entries are zero.
fn convolve_buffer(
  pixels: &[u8],
  width: u32,
  height: u32,
  kernel: &[f32],
  size: usize,
  divisor: f32,
  offset: f32,
  edge: EdgeMode,
  keep_alpha: bool,
) -> Vec<u8> {
  let channels = if keep_alpha { 3 } else { 4 };
  let width = width as i64;
  let height = height as i64;
  let radius = (size / 2) as i64;
  let stride = width as usize * 4;

  let mut out = vec![0u8; pixels.len()];

  // (dx, dy, weight) for the taps that can actually contribute.
  let mut taps: Vec<(i64, i64, f32)> = Vec::with_capacity(kernel.len());
  for ky in 0..size as i64 {
    for kx in 0..size as i64 {
      let weight = kernel[(ky * size as i64 + kx) as usize];
      if weight != 0.0 {
        taps.push((kx - radius, ky - radius, weight));
      }
    }
  }

  // The same taps as a flat byte offset from the centre pixel, for the
  // interior where no tap can fall outside the image.
  let flat: Vec<(isize, f32)> = taps
    .iter()
    .map(|&(dx, dy, weight)| ((dy * stride as i64 + dx * 4) as isize, weight))
    .collect();

  // One reciprocal instead of a divide per channel per pixel. A divisor
  // of 1 (the common case, and every normalised kernel after the caller
  // folds the sum in) inverts exactly, so nothing moves.
  let inverse = 1.0 / divisor;

  for y in 0..height {
    let row_interior = y >= radius && y < height - radius;

    for x in 0..width {
      let base = y as usize * stride + x as usize * 4;
      let mut sums = [0f32; 4];

      if row_interior && x >= radius && x < width - radius {
        // All four channels, unconditionally and by fixed index. The
        // count is otherwise a runtime value, which is enough to stop
        // the compiler unrolling this at all, and an unrolled four-wide
        // accumulate is exactly the shape it can turn into vector
        // instructions. Working out an alpha that `keep_alpha` then
        // discards costs less than losing that.
        for &(shift, weight) in &flat {
          let tap = (base as isize + shift) as usize;
          let source = &pixels[tap..tap + 4];

          sums[0] += source[0] as f32 * weight;
          sums[1] += source[1] as f32 * weight;
          sums[2] += source[2] as f32 * weight;
          sums[3] += source[3] as f32 * weight;
        }
      } else {
        for &(dx, dy, weight) in &taps {
          let mut sx = x + dx;
          let mut sy = y + dy;

          match edge {
            EdgeMode::Clamp => {
              sx = sx.clamp(0, width - 1);
              sy = sy.clamp(0, height - 1);
            },
            EdgeMode::Wrap => {
              sx = sx.rem_euclid(width);
              sy = sy.rem_euclid(height);
            },
            EdgeMode::Transparent => {
              if sx < 0 || sy < 0 || sx >= width || sy >= height {
                continue;
              }
            },
          }

          let tap = sy as usize * stride + sx as usize * 4;
          for channel in 0..channels {
            sums[channel] += pixels[tap + channel] as f32 * weight;
          }
        }
      }

      for channel in 0..channels {
        out[base + channel] = (sums[channel] * inverse + offset).clamp(0.0, 255.0) as u8;
      }

      if keep_alpha {
        out[base + 3] = pixels[base + 3];
      }
    }
  }

  out
}

/// `_imagine.blur(pixels, width, height, sigma)`; a true Gaussian
/// blur, separable so cost is linear rather than quadratic in the
/// radius. Kept apart from `convolve` because expressing a wide
/// Gaussian as an NxN kernel would be dramatically slower.
fn blur(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 4);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);

  let width = dim(ctx, 1, "width")?;
  let height = dim(ctx, 2, "height")?;
  let sigma = num(ctx, 3);

  if !sigma.is_finite() || sigma <= 0.0 {
    return Err("blur(): sigma must be greater than zero".to_string());
  }

  let blurred = ctx.args[0].with_bytes(|pixels| -> Result<Vec<u8>, String> {
    check_buffer("blur", pixels.len(), width, height)?;
    Ok(gaussian(pixels, width, height, sigma as f32))
  })?;

  Ok(ctx.heap().alloc_bytes(blurred))
}

/// Separable Gaussian: one horizontal pass, then one vertical pass
/// over the result. Alpha is premultiplied for the duration so that
/// blurring a shape against transparency doesn't drag the colour of
/// fully transparent pixels into the visible edge.
/// Box sizes whose successive application approximates a Gaussian of
/// this sigma, one per pass.
///
/// Three box blurs is the usual stopping point: the central limit
/// theorem says repeated box convolution converges on a Gaussian, and
/// by the third pass the error is well under a quantisation step for
/// 8-bit channels, which is why libvips, Pillow and the SVG
/// `feGaussianBlur` spec all settle there too. The sizes come out of
/// solving for the box width whose variance, tripled, matches the
/// Gaussian's; `wl`/`wu` are the odd widths either side of the ideal,
/// and `m` decides how many passes take the smaller one.
fn box_sizes_for_gaussian(sigma: f32, passes: usize) -> Vec<i64> {
  let n = passes as f32;
  let ideal = ((12.0 * sigma * sigma / n) + 1.0).sqrt();

  let mut wl = ideal.floor() as i64;
  if wl % 2 == 0 {
    wl -= 1;
  }
  wl = wl.max(1);
  let wu = wl + 2;

  let wl_f = wl as f32;
  let m_ideal =
    (12.0 * sigma * sigma - n * wl_f * wl_f - 4.0 * n * wl_f - 3.0 * n) / (-4.0 * wl_f - 4.0);
  let m = m_ideal.round() as i64;

  (0..passes)
    .map(|i| if (i as i64) < m { wl } else { wu })
    .collect()
}

/// One horizontal box blur, edge-clamped, over interleaved RGBA floats.
///
/// The window moves one pixel at a time and the running sum moves with
/// it: one add and one subtract per pixel, whatever the radius. That
/// O(1)-per-pixel behaviour is the whole point of going through boxes
/// rather than convolving the Gaussian directly.
fn box_blur_h(src: &[f32], dst: &mut [f32], width: usize, height: usize, radius: i64) {
  if radius <= 0 {
    dst.copy_from_slice(src);
    return;
  }

  let r = radius as usize;
  let scale = 1.0 / (2 * r + 1) as f32;
  let last = width - 1;

  for y in 0..height {
    let row = y * width * 4;
    let mut sum = [0f32; 4];

    // The window starts hanging off the left edge, so the first pixel
    // stands in for everything out there.
    for c in 0..4 {
      sum[c] = src[row + c] * (r + 1) as f32;
    }
    for x in 1..=r.min(last) {
      for c in 0..4 {
        sum[c] += src[row + x * 4 + c];
      }
    }
    if r > last {
      for c in 0..4 {
        sum[c] += src[row + last * 4 + c] * (r - last) as f32;
      }
    }

    for x in 0..width {
      for c in 0..4 {
        dst[row + x * 4 + c] = sum[c] * scale;
      }
      let add = (x + r + 1).min(last);
      let drop = x.saturating_sub(r);
      for c in 0..4 {
        sum[c] += src[row + add * 4 + c] - src[row + drop * 4 + c];
      }
    }
  }
}

/// One vertical box blur, edge-clamped.
///
/// Same running sum as the horizontal pass, but held for a whole row of
/// columns at once and advanced a row at a time. Walking rows rather
/// than columns keeps every access sequential; a column-at-a-time loop
/// touches a new cache line on every step and costs several times more
/// for the identical arithmetic.
fn box_blur_v(src: &[f32], dst: &mut [f32], width: usize, height: usize, radius: i64) {
  if radius <= 0 {
    dst.copy_from_slice(src);
    return;
  }

  let r = radius as usize;
  let scale = 1.0 / (2 * r + 1) as f32;
  let stride = width * 4;
  let last = height - 1;
  let mut sum = vec![0f32; stride];

  for i in 0..stride {
    sum[i] = src[i] * (r + 1) as f32;
  }
  for y in 1..=r.min(last) {
    let row = y * stride;
    for i in 0..stride {
      sum[i] += src[row + i];
    }
  }
  if r > last {
    let row = last * stride;
    for i in 0..stride {
      sum[i] += src[row + i] * (r - last) as f32;
    }
  }

  for y in 0..height {
    let row = y * stride;
    for i in 0..stride {
      dst[row + i] = sum[i] * scale;
    }
    let add = ((y + r + 1).min(last)) * stride;
    let drop = (if y >= r { y - r } else { 0 }) * stride;
    for i in 0..stride {
      sum[i] += src[add + i] - src[drop + i];
    }
  }
}

/// Gaussian blur, approximated by three box blurs per axis.
///
/// Convolving the Gaussian directly costs O(sigma) taps per pixel per
/// axis, which is what made a heavy blur so much dearer than a light
/// one. Boxes make the cost independent of sigma entirely.
///
/// The trade is a little fidelity: three boxes can only land on the
/// discrete variances their widths allow, so the effective sigma comes
/// out within roughly 5-18% of the one asked for (`blur(4)` measures
/// about 3.8). Every library that approximates this way has the same
/// property, it is well under what the eye picks up on a blur, and odd
/// box widths keep the result centred, so nothing shifts by half a
/// pixel. Code that needs an exact kernel should convolve one itself.
///
/// Colour is premultiplied by alpha before blurring and divided back out
/// afterwards. Without that, a transparent pixel's colour (which may be
/// anything at all, since nothing is drawn there) bleeds into its
/// visible neighbours and leaves a dark or coloured fringe along every
/// soft edge.
fn gaussian(pixels: &[u8], width: u32, height: u32, sigma: f32) -> Vec<u8> {
  let width = width as usize;
  let height = height as usize;

  if width == 0 || height == 0 {
    return pixels.to_vec();
  }

  let mut source: Vec<f32> = Vec::with_capacity(pixels.len());
  for pixel in pixels.chunks_exact(4) {
    let alpha = pixel[3] as f32 / 255.0;
    source.push(pixel[0] as f32 * alpha);
    source.push(pixel[1] as f32 * alpha);
    source.push(pixel[2] as f32 * alpha);
    source.push(pixel[3] as f32);
  }

  let mut scratch = vec![0f32; source.len()];
  for size in box_sizes_for_gaussian(sigma, 3) {
    let radius = (size - 1) / 2;
    box_blur_h(&source, &mut scratch, width, height, radius);
    box_blur_v(&scratch, &mut source, width, height, radius);
  }

  let mut out = vec![0u8; pixels.len()];
  for (chunk, dst) in source.chunks_exact(4).zip(out.chunks_exact_mut(4)) {
    let alpha = chunk[3].clamp(0.0, 255.0);
    dst[3] = alpha.round() as u8;

    if alpha <= 0.0 {
      continue;
    }

    let scale = 255.0 / alpha;
    for channel in 0..3 {
      dst[channel] = (chunk[channel] * scale).clamp(0.0, 255.0).round() as u8;
    }
  }

  out
}

/// The Porter-Duff / separable blend modes `composite()` understands.
#[derive(Clone, Copy)]
enum BlendMode {
  Normal,
  Multiply,
  Screen,
  Overlay,
  Darken,
  Lighten,
  ColorDodge,
  ColorBurn,
  HardLight,
  SoftLight,
  Difference,
  Exclusion,
  Add,
  Subtract,
}

fn blend_from_name(name: &str) -> Option<BlendMode> {
  match name {
    "normal" | "over" => Some(BlendMode::Normal),
    "multiply" => Some(BlendMode::Multiply),
    "screen" => Some(BlendMode::Screen),
    "overlay" => Some(BlendMode::Overlay),
    "darken" => Some(BlendMode::Darken),
    "lighten" => Some(BlendMode::Lighten),
    "color_dodge" => Some(BlendMode::ColorDodge),
    "color_burn" => Some(BlendMode::ColorBurn),
    "hard_light" => Some(BlendMode::HardLight),
    "soft_light" => Some(BlendMode::SoftLight),
    "difference" => Some(BlendMode::Difference),
    "exclusion" => Some(BlendMode::Exclusion),
    "add" | "plus" => Some(BlendMode::Add),
    "subtract" => Some(BlendMode::Subtract),
    _ => None,
  }
}

/// One channel of a separable blend, on 0..1 values. The formulas are
/// the ones in the CSS compositing spec, which is also what every
/// image editor implements.
fn blend_channel(mode: BlendMode, backdrop: f32, source: f32) -> f32 {
  match mode {
    BlendMode::Normal => source,
    BlendMode::Multiply => backdrop * source,
    BlendMode::Screen => backdrop + source - backdrop * source,
    BlendMode::Overlay => blend_channel(BlendMode::HardLight, source, backdrop),
    BlendMode::Darken => backdrop.min(source),
    BlendMode::Lighten => backdrop.max(source),
    BlendMode::ColorDodge => {
      if backdrop <= 0.0 {
        0.0
      } else if source >= 1.0 {
        1.0
      } else {
        (backdrop / (1.0 - source)).min(1.0)
      }
    },
    BlendMode::ColorBurn => {
      if backdrop >= 1.0 {
        1.0
      } else if source <= 0.0 {
        0.0
      } else {
        1.0 - ((1.0 - backdrop) / source).min(1.0)
      }
    },
    BlendMode::HardLight => {
      if source <= 0.5 {
        backdrop * (2.0 * source)
      } else {
        let s = 2.0 * source - 1.0;
        backdrop + s - backdrop * s
      }
    },
    BlendMode::SoftLight => {
      if source <= 0.5 {
        backdrop - (1.0 - 2.0 * source) * backdrop * (1.0 - backdrop)
      } else {
        let d = if backdrop <= 0.25 {
          ((16.0 * backdrop - 12.0) * backdrop + 4.0) * backdrop
        } else {
          backdrop.sqrt()
        };
        backdrop + (2.0 * source - 1.0) * (d - backdrop)
      }
    },
    BlendMode::Difference => (backdrop - source).abs(),
    BlendMode::Exclusion => backdrop + source - 2.0 * backdrop * source,
    BlendMode::Add => (backdrop + source).min(1.0),
    BlendMode::Subtract => (backdrop - source).max(0.0),
  }
}

/// `_imagine.composite(dst, dst_w, dst_h, src, src_w, src_h, x, y, opacity, mode)`.
///
/// Draws `src` onto `dst` in place at (x, y), clipped to the
/// destination. `x` and `y` are signed, so a sprite can hang off the
/// top or left edge.
#[allow(clippy::too_many_arguments)]
fn composite(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 10);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);
  enforce_arg_type!(ctx, 3, ArgType::Bytes);
  enforce_arg_type!(ctx, 9, ArgType::String);

  let dest_width = dim(ctx, 1, "width")?;
  let dest_height = dim(ctx, 2, "height")?;
  let source_width = dim(ctx, 4, "width")?;
  let source_height = dim(ctx, 5, "height")?;
  let at_x = num(ctx, 6);
  let at_y = num(ctx, 7);
  let opacity = num(ctx, 8).clamp(0.0, 1.0) as f32;

  if !at_x.is_finite() || !at_y.is_finite() {
    return Err("composite(): position must be finite".to_string());
  }

  let mode_name = ctx.args[9].as_str().to_lowercase();
  let mode = blend_from_name(&mode_name)
    .ok_or_else(|| format!("composite(): unknown blend mode '{}'", mode_name))?;

  // Same object on both sides would deadlock the two RefCell borrows,
  // and is meaningless anyway; the Zuri layer clones before calling.
  if ctx.args[0].as_obj() == ctx.args[3].as_obj() {
    return Err("composite(): source and destination must be different buffers".to_string());
  }

  if opacity <= 0.0 {
    return Ok(Value::nil());
  }

  let source = ctx.args[3].with_bytes(|pixels| -> Result<Vec<u8>, String> {
    check_buffer("composite", pixels.len(), source_width, source_height)?;
    Ok(pixels.to_vec())
  })?;

  ctx.args[0].with_bytes_mut(|dest| -> Result<(), String> {
    check_buffer("composite", dest.len(), dest_width, dest_height)?;
    composite_buffer(
      dest,
      dest_width as i64,
      dest_height as i64,
      &source,
      source_width as i64,
      source_height as i64,
      at_x.floor() as i64,
      at_y.floor() as i64,
      opacity,
      mode,
    );
    Ok(())
  })?;

  Ok(Value::nil())
}

#[allow(clippy::too_many_arguments)]
fn composite_buffer(
  dest: &mut [u8],
  dest_width: i64,
  dest_height: i64,
  source: &[u8],
  source_width: i64,
  source_height: i64,
  at_x: i64,
  at_y: i64,
  opacity: f32,
  mode: BlendMode,
) {
  // Only the overlapping rectangle is touched, computed once rather
  // than tested per pixel.
  let start_x = at_x.max(0);
  let start_y = at_y.max(0);
  let end_x = (at_x + source_width).min(dest_width);
  let end_y = (at_y + source_height).min(dest_height);

  if start_x >= end_x || start_y >= end_y {
    return;
  }

  let dest_stride = dest_width as usize * 4;
  let source_stride = source_width as usize * 4;

  for y in start_y..end_y {
    let source_y = (y - at_y) as usize;
    for x in start_x..end_x {
      let source_x = (x - at_x) as usize;

      let sbase = source_y * source_stride + source_x * 4;
      let source_alpha = source[sbase + 3] as f32 / 255.0 * opacity;
      if source_alpha <= 0.0 {
        continue;
      }

      let dbase = y as usize * dest_stride + x as usize * 4;
      let dest_alpha = dest[dbase + 3] as f32 / 255.0;

      let out_alpha = source_alpha + dest_alpha * (1.0 - source_alpha);
      if out_alpha <= 0.0 {
        dest[dbase..dbase + 4].copy_from_slice(&[0, 0, 0, 0]);
        continue;
      }

      for channel in 0..3 {
        let backdrop = dest[dbase + channel] as f32 / 255.0;
        let source_value = source[sbase + channel] as f32 / 255.0;

        // The blend applies where the two overlap; outside the
        // backdrop it is the source colour on its own. This is the
        // spec's Cs = (1 - ab) x Cs + ab x B(Cb, Cs).
        let blended = (1.0 - dest_alpha) * source_value
          + dest_alpha * blend_channel(mode, backdrop, source_value);

        let value =
          (source_alpha * blended + dest_alpha * backdrop * (1.0 - source_alpha)) / out_alpha;
        dest[dbase + channel] = (value * 255.0).clamp(0.0, 255.0).round() as u8;
      }

      dest[dbase + 3] = (out_alpha * 255.0).clamp(0.0, 255.0).round() as u8;
    }
  }
}

/// `_imagine.quantize(pixels, width, height, colors, dither)`; reduces
/// the image to at most `colors` distinct colours, in place, keeping
/// the RGBA layout. Useful on its own (a posterised look, smaller PNGs)
/// and as a preview of what GIF export will do.
fn quantize(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 5);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);

  let width = dim(ctx, 1, "width")?;
  let height = dim(ctx, 2, "height")?;
  let colors = num(ctx, 3);
  let dither = ctx.args[4].is_bool() && ctx.args[4].as_bool();

  if !(2.0..=256.0).contains(&colors) {
    return Err(format!(
      "quantize(): colour count must be between 2 and 256, got {}",
      colors
    ));
  }

  let raw = ctx.args[0].with_bytes(|pixels| -> Result<Vec<u8>, String> {
    check_buffer("quantize", pixels.len(), width, height)?;
    Ok(pixels.to_vec())
  })?;

  let quantizer = color_quant::NeuQuant::new(if dither { 10 } else { 30 }, colors as usize, &raw);
  let palette = quantizer.color_map_rgba();

  ctx.args[0].with_bytes_mut(|pixels| {
    if dither {
      dither_floyd_steinberg(pixels, width, height, &quantizer, &palette);
    } else {
      for pixel in pixels.chunks_exact_mut(4) {
        let index = quantizer.index_of(pixel);
        pixel.copy_from_slice(&palette[index * 4..index * 4 + 4]);
      }
    }
  });

  Ok(Value::nil())
}

/// Floyd-Steinberg error diffusion over the colour channels. Alpha is
/// snapped rather than diffused: spreading alpha error produces
/// speckled edges, which looks far worse than a hard cut.
fn dither_floyd_steinberg(
  pixels: &mut [u8],
  width: u32,
  height: u32,
  quantizer: &color_quant::NeuQuant,
  palette: &[u8],
) {
  let width = width as usize;
  let height = height as usize;
  let mut errors = vec![0f32; width * height * 3];

  for y in 0..height {
    for x in 0..width {
      let base = (y * width + x) * 4;
      let error_base = (y * width + x) * 3;

      let mut wanted = [0u8; 4];
      for channel in 0..3 {
        wanted[channel] =
          (pixels[base + channel] as f32 + errors[error_base + channel]).clamp(0.0, 255.0) as u8;
      }
      wanted[3] = pixels[base + 3];

      let index = quantizer.index_of(&wanted);
      let chosen = &palette[index * 4..index * 4 + 4];
      pixels[base..base + 4].copy_from_slice(chosen);

      for channel in 0..3 {
        let delta = wanted[channel] as f32 - chosen[channel] as f32;
        let mut spread = |dx: usize, dy: usize, factor: f32| {
          if dx >= width || dy >= height {
            return;
          }
          errors[(dy * width + dx) * 3 + channel] += delta * factor;
        };

        spread(x + 1, y, 7.0 / 16.0);
        if x > 0 {
          spread(x - 1, y + 1, 3.0 / 16.0);
        }
        spread(x, y + 1, 5.0 / 16.0);
        spread(x + 1, y + 1, 1.0 / 16.0);
      }
    }
  }
}

/// `_imagine.premultiply(pixels, undo)`; converts between straight and
/// premultiplied alpha in place. `imagine` stores straight alpha
/// everywhere, so this exists for callers handing pixels to something
/// that wants them premultiplied, and for getting them back.
fn premultiply(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);

  let undo = ctx.args[1].is_bool() && ctx.args[1].as_bool();

  ctx.args[0].with_bytes_mut(|pixels| {
    if pixels.len() % 4 != 0 {
      return Err("premultiply(): pixel buffer length is not a multiple of 4".to_string());
    }

    for pixel in pixels.chunks_exact_mut(4) {
      let alpha = pixel[3] as u32;
      if alpha == 255 {
        continue;
      }
      if alpha == 0 {
        pixel[0] = 0;
        pixel[1] = 0;
        pixel[2] = 0;
        continue;
      }

      for channel in 0..3 {
        pixel[channel] = if undo {
          ((pixel[channel] as u32 * 255 + alpha / 2) / alpha).min(255) as u8
        } else {
          ((pixel[channel] as u32 * alpha + 127) / 255) as u8
        };
      }
    }

    Ok(())
  })?;

  Ok(Value::nil())
}

// ---------------------------------------------------------------------------
// text
// ---------------------------------------------------------------------------

/// A parsed font face, held behind a `Ptr` so a program loads a `.ttf`
/// once and draws with it many times. Parsing is the expensive part;
/// rasterizing a glyph from an already-parsed face is not.
struct FontFace {
  font: fontdue::Font,
  name: String,
}

/// `_imagine.font_load(data)`; parses a TrueType or OpenType face.
fn font_load(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 1);
  enforce_arg_type!(ctx, 0, ArgType::Bytes);

  let parsed = ctx.args[0].with_bytes(|data| {
    fontdue::Font::from_bytes(data, fontdue::FontSettings::default())
      .map_err(|e| format!("font_load(): {}", e))
  })?;

  let name = parsed
    .name()
    .map(|n| n.to_string())
    .unwrap_or_else(|| "unnamed".to_string());

  Ok(
    ctx
      .heap()
      .alloc_ptr(FONT_TAG, FontFace { font: parsed, name }),
  )
}

/// Runs `f` against the face behind a `Ptr` argument.
fn with_font<R>(
  ctx: &ZuriContext,
  index: usize,
  f: impl FnOnce(&FontFace) -> R,
) -> Result<R, String> {
  let value = ctx.args[index];
  if !value.is_ptr_type(FONT_TAG) {
    return Err(format!(
      "{}() expects argument {} to be a font, got {}",
      ctx.name,
      index + 1,
      value.type_name()
    ));
  }

  let object = unsafe { &*value.as_obj() };
  match object {
    crate::vm::object::Obj::Ptr(cell) => {
      let borrowed = cell.borrow();
      let face = borrowed
        .value
        .downcast_ref::<FontFace>()
        .ok_or_else(|| format!("{}(): font handle has been consumed", ctx.name))?;
      Ok(f(face))
    },
    _ => Err(format!("{}(): expected a font handle", ctx.name)),
  }
}

/// `_imagine.font_info(font, size)`; the vertical metrics a caller
/// needs to lay text out itself, in pixels at that size.
fn font_info(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 2);
  enforce_arg_type!(ctx, 1, ArgType::Number);

  let size = text_size(num(ctx, 1))?;
  let (name, metrics) = with_font(ctx, 0, |face| {
    (face.name.clone(), face.font.horizontal_line_metrics(size))
  })?;

  let metrics = metrics.ok_or_else(|| {
    "font_info(): this font has no horizontal metrics, so it cannot lay out text".to_string()
  })?;

  let name_value = ctx.heap().alloc_string(name);
  Ok(make_dict(
    ctx,
    vec![
      ("name", name_value),
      ("ascent", Value::number(metrics.ascent as f64)),
      ("descent", Value::number(metrics.descent as f64)),
      ("line_gap", Value::number(metrics.line_gap as f64)),
      ("line_height", Value::number(metrics.new_line_size as f64)),
    ],
  ))
}

fn text_size(raw: f64) -> Result<f32, String> {
  if !raw.is_finite() || raw <= 0.0 || raw > 4096.0 {
    return Err(format!(
      "text size must be between 1 and 4096 pixels, got {}",
      raw
    ));
  }
  Ok(raw as f32)
}

/// How a rendered block of text places its lines against each other.
#[derive(Clone, Copy, PartialEq)]
enum Align {
  Left,
  Center,
  Right,
}

struct TextOptions {
  tracking: f32,
  line_height: f32,
  align: Align,
}

fn text_options(options: Value) -> Result<TextOptions, String> {
  let align = match option_string(options, "align").as_deref() {
    Some("left") | None => Align::Left,
    Some("center") | Some("centre") => Align::Center,
    Some("right") => Align::Right,
    Some(other) => {
      return Err(format!(
        "unknown text alignment '{}', expected left, center or right",
        other
      ));
    },
  };

  Ok(TextOptions {
    tracking: option_number(options, "tracking", 0.0) as f32,
    line_height: option_number(options, "line_height", 1.0).max(0.0) as f32,
    align,
  })
}

/// One laid-out line: its glyphs' advances are already summed into
/// `width`, and `chars` keeps the text so rasterization doesn't repeat
/// the split.
struct LaidLine {
  text: String,
  width: f32,
}

/// Advance-width layout with kerning. Deliberately simple: no
/// shaping, no bidi, no ligatures. Those need a shaping engine, and a
/// standard library that pretended to do them would be worse than one
/// that says plainly it doesn't.
fn lay_out(face: &FontFace, text: &str, size: f32, options: &TextOptions) -> Vec<LaidLine> {
  let mut lines = Vec::new();

  for raw in text.split('\n') {
    let mut width = 0f32;
    let mut previous: Option<char> = None;

    for character in raw.chars() {
      if let Some(left) = previous {
        width += face
          .font
          .horizontal_kern(left, character, size)
          .unwrap_or(0.0);
        width += options.tracking;
      }
      width += face.font.metrics(character, size).advance_width;
      previous = Some(character);
    }

    lines.push(LaidLine {
      text: raw.to_string(),
      width: width.max(0.0),
    });
  }

  lines
}

/// Total pixel size of a laid-out block, and the baseline of its first
/// line measured from the top.
fn block_size(
  face: &FontFace,
  size: f32,
  lines: &[LaidLine],
  options: &TextOptions,
) -> (f32, f32, f32) {
  let metrics = face.font.horizontal_line_metrics(size);
  let (ascent, natural) = match metrics {
    Some(m) => (m.ascent, m.new_line_size),
    None => (size, size * 1.2),
  };

  let step = natural * options.line_height;
  let width = lines.iter().fold(0f32, |acc, line| acc.max(line.width));
  let height = if lines.len() <= 1 {
    natural
  } else {
    step * (lines.len() - 1) as f32 + natural
  };

  (width, height, ascent)
}

/// `_imagine.measure_text(font, text, size, options)`; the box the
/// same call to `render_text` would produce, without doing the work.
fn measure_text(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 4);
  enforce_arg_type!(ctx, 1, ArgType::String);
  enforce_arg_type!(ctx, 2, ArgType::Number);

  let size = text_size(num(ctx, 2))?;
  let options = text_options(ctx.args[3]).map_err(|e| format!("measure_text(): {}", e))?;
  let text = ctx.args[1].as_str().to_string();

  let measured = with_font(ctx, 0, |face| {
    let lines = lay_out(face, &text, size, &options);
    let (width, height, ascent) = block_size(face, size, &lines, &options);
    (width, height, ascent, lines.len())
  })?;

  let (width, height, ascent, line_count) = measured;

  Ok(make_dict(
    ctx,
    vec![
      ("width", Value::number(width.ceil() as f64)),
      ("height", Value::number(height.ceil() as f64)),
      ("baseline", Value::number(ascent as f64)),
      ("lines", Value::number(line_count as f64)),
    ],
  ))
}

/// `_imagine.render_text(font, text, size, options)`; rasterizes the
/// whole string into an 8-bit coverage mask.
///
/// A mask, not a coloured image, on purpose. Colour, opacity and any
/// blend mode belong to whoever is drawing, and the Zuri side already
/// has to composite anyway; handing back coverage keeps the colour
/// decision on the Zuri side of the boundary and makes one call per
/// `text()` rather than one per glyph.
fn render_text(ctx: &mut ZuriContext) -> Result<Value, String> {
  enforce_arg_count!(ctx, 4);
  enforce_arg_type!(ctx, 1, ArgType::String);
  enforce_arg_type!(ctx, 2, ArgType::Number);

  let size = text_size(num(ctx, 2))?;
  let options = text_options(ctx.args[3]).map_err(|e| format!("render_text(): {}", e))?;
  let text = ctx.args[1].as_str().to_string();

  let rendered = with_font(ctx, 0, |face| render_block(face, &text, size, &options))?;
  let (coverage, width, height, baseline) = rendered;

  let coverage_value = ctx.heap().alloc_bytes(coverage);

  Ok(make_dict(
    ctx,
    vec![
      ("coverage", coverage_value),
      ("width", Value::number(width as f64)),
      ("height", Value::number(height as f64)),
      ("baseline", Value::number(baseline as f64)),
    ],
  ))
}

fn render_block(
  face: &FontFace,
  text: &str,
  size: f32,
  options: &TextOptions,
) -> (Vec<u8>, u32, u32, f32) {
  let lines = lay_out(face, text, size, options);
  let (block_width, block_height, ascent) = block_size(face, size, &lines, options);

  // A glyph can reach past its advance box (an italic f, an overshoot
  // on a round letter), so the mask gets a little slack on each side
  // rather than clipping. The Zuri layer draws the mask at an offset
  // that accounts for it.
  let padding = (size * 0.5).ceil() as i64;
  let width = (block_width.ceil() as i64 + padding * 2).max(1) as usize;
  let height = (block_height.ceil() as i64 + padding * 2).max(1) as usize;

  let mut coverage = vec![0u8; width * height];

  let natural = face
    .font
    .horizontal_line_metrics(size)
    .map(|m| m.new_line_size)
    .unwrap_or(size * 1.2);
  let step = natural * options.line_height;

  for (index, line) in lines.iter().enumerate() {
    let line_baseline = padding as f32 + ascent + step * index as f32;

    let start_x = padding as f32
      + match options.align {
        Align::Left => 0.0,
        Align::Center => (block_width - line.width) / 2.0,
        Align::Right => block_width - line.width,
      };

    let mut pen = start_x;
    let mut previous: Option<char> = None;

    for character in line.text.chars() {
      if let Some(left) = previous {
        pen += face
          .font
          .horizontal_kern(left, character, size)
          .unwrap_or(0.0);
        pen += options.tracking;
      }

      let (metrics, bitmap) = face.font.rasterize(character, size);

      // fontdue hands back the glyph relative to the baseline, with
      // ymin measured upward from it; a descender has a negative ymin.
      let glyph_x = (pen + metrics.xmin as f32).round() as i64;
      let glyph_y = (line_baseline - (metrics.height as f32 + metrics.ymin as f32)).round() as i64;

      for row in 0..metrics.height {
        let target_y = glyph_y + row as i64;
        if target_y < 0 || target_y >= height as i64 {
          continue;
        }
        for column in 0..metrics.width {
          let target_x = glyph_x + column as i64;
          if target_x < 0 || target_x >= width as i64 {
            continue;
          }

          let value = bitmap[row * metrics.width + column];
          if value == 0 {
            continue;
          }

          // Glyphs can touch where they overlap; keep the darker of
          // the two rather than letting the sum wrap.
          let slot = &mut coverage[target_y as usize * width + target_x as usize];
          *slot = (*slot).max(value);
        }
      }

      pen += metrics.advance_width;
      previous = Some(character);
    }
  }

  (
    coverage,
    width as u32,
    height as u32,
    padding as f32 + ascent,
  )
}
