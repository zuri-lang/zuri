//! The type model: every C type the module can describe, where each
//! one's bytes go, and what libffi is told about it.
//!
//! Layout is computed here rather than asked of libffi. libffi only
//! knows naturally aligned structs of scalars, which leaves out packed
//! records, bitfields, over-aligned members, unions and 128-bit
//! integers. So a record's size and alignment come from this module,
//! following the C compiler of the platform, and libffi is handed a
//! struct whose size and alignment are already filled in (which it
//! trusts) together with a list of elements chosen to make its own
//! register classification come out the way the compiler's does. That
//! list is the `skeleton` below, and it is built per ABI.

use std::fmt::Write as _;
use std::ptr;
use std::sync::{Arc, LazyLock, Mutex, OnceLock};

use libffi_sys as raw;
use rustc_hash::FxHashMap;

#[cfg(not(all(target_pointer_width = "64", target_endian = "little")))]
compile_error!("the ffi module supports 64-bit little-endian targets only");

/// A shared, immutable handle on a type. Records are the one kind with
/// interior state, and only until they are sealed.
pub type TypeRef = Arc<CType>;

/// The width of a pointer, and of `size_t`, on every supported target.
pub const POINTER_SIZE: usize = 8;

/// `long` is 32 bits on Windows and 64 everywhere else.
#[cfg(windows)]
pub const LONG_SIZE: usize = 4;
#[cfg(not(windows))]
pub const LONG_SIZE: usize = 8;

/// Plain `char` is unsigned on 64-bit Arm Linux and signed on the rest,
/// Apple's Arm platforms included.
pub const CHAR_SIGNED: bool = !cfg!(all(
  target_arch = "aarch64",
  not(target_vendor = "apple"),
  not(windows)
));

/// `wchar_t` is a UTF-16 code unit on Windows and a 32-bit value
/// elsewhere, unsigned on Arm Linux and signed on the rest.
#[cfg(windows)]
pub const WCHAR: (usize, bool) = (2, false);
#[cfg(all(target_arch = "aarch64", not(target_vendor = "apple"), not(windows)))]
pub const WCHAR: (usize, bool) = (4, false);
#[cfg(not(any(windows, all(target_arch = "aarch64", not(target_vendor = "apple")))))]
pub const WCHAR: (usize, bool) = (4, true);

/// How `long double` is represented on this target. Each target
/// constructs one of these.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LongDoubleRepr {
  /// The x87 80-bit extended format, padded to 16 bytes.
  X87,
  /// IEEE 754 binary128.
  Quad,
  /// The same as `double`.
  Double,
}

#[cfg(all(target_arch = "x86_64", not(windows)))]
pub const LONG_DOUBLE: LongDoubleRepr = LongDoubleRepr::X87;
#[cfg(all(target_arch = "aarch64", not(target_vendor = "apple"), not(windows)))]
pub const LONG_DOUBLE: LongDoubleRepr = LongDoubleRepr::Quad;
#[cfg(any(windows, all(target_arch = "aarch64", target_vendor = "apple")))]
pub const LONG_DOUBLE: LongDoubleRepr = LongDoubleRepr::Double;

/// Whether the C compiler of this platform has `_Complex`. MSVC does not.
pub const HAS_COMPLEX: bool = cfg!(not(windows));

/// Records follow MSVC's layout rules on Windows and the Itanium rules
/// (GCC and Clang) elsewhere. They differ only for bitfields.
pub const MSVC_LAYOUT: bool = cfg!(windows);

/// The text encoding a string-carrying pointer converts through.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Encoding {
  Utf8,
  Utf16,
  Utf32,
  /// `wchar_t`: UTF-16 on Windows, UTF-32 elsewhere.
  Wide,
}

impl Encoding {
  pub fn parse(name: &str) -> Option<Encoding> {
    match name {
      "utf-8" | "utf8" => Some(Encoding::Utf8),
      "utf-16" | "utf16" => Some(Encoding::Utf16),
      "utf-32" | "utf32" => Some(Encoding::Utf32),
      "wide" => Some(Encoding::Wide),
      _ => None,
    }
  }

  /// Bytes per code unit.
  pub fn unit(self) -> usize {
    match self {
      Encoding::Utf8 => 1,
      Encoding::Utf16 => 2,
      Encoding::Utf32 => 4,
      Encoding::Wide => WCHAR.0,
    }
  }

  /// The fixed-width encoding `Wide` stands for here.
  pub fn resolve(self) -> Encoding {
    match self {
      Encoding::Wide if WCHAR.0 == 2 => Encoding::Utf16,
      Encoding::Wide => Encoding::Utf32,
      other => other,
    }
  }
}

/// Which calling convention a signature uses.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Abi {
  /// The platform's own C convention.
  Default,
  /// Microsoft x64, available on every x86-64 target.
  Win64,
  /// System V AMD64, available on x86-64 Unix.
  SysV64,
}

impl Abi {
  pub fn parse(name: &str) -> Result<Abi, String> {
    match name {
      "default" | "C" | "c" | "cdecl" | "system" => Ok(Abi::Default),
      "win64" => {
        if cfg!(target_arch = "x86_64") {
          Ok(Abi::Win64)
        } else {
          Err("the 'win64' convention exists only on x86-64".into())
        }
      },
      "sysv64" => {
        if cfg!(all(target_arch = "x86_64", not(windows))) {
          Ok(Abi::SysV64)
        } else if cfg!(target_arch = "x86_64") {
          Err("the 'sysv64' convention is not available on Windows".into())
        } else {
          Err("the 'sysv64' convention exists only on x86-64".into())
        }
      },
      other => Err(format!(
        "unknown calling convention '{other}'; expected 'default', 'win64' or 'sysv64'"
      )),
    }
  }

  pub fn name(self) -> &'static str {
    match self {
      Abi::Default => "default",
      Abi::Win64 => "win64",
      Abi::SysV64 => "sysv64",
    }
  }

  pub fn raw(self) -> raw::ffi_abi {
    match self {
      Abi::Default => raw::ffi_abi_FFI_DEFAULT_ABI,
      #[cfg(target_arch = "x86_64")]
      Abi::Win64 => raw::ffi_abi_FFI_WIN64,
      #[cfg(all(target_arch = "x86_64", not(windows)))]
      Abi::SysV64 => raw::ffi_abi_FFI_UNIX64,
      #[allow(unreachable_patterns)]
      _ => raw::ffi_abi_FFI_DEFAULT_ABI,
    }
  }

  /// Whether a signature using this convention follows the Microsoft
  /// x64 rules for passing aggregates.
  pub fn is_win64(self) -> bool {
    match self {
      Abi::Win64 => true,
      Abi::SysV64 => false,
      Abi::Default => cfg!(windows),
    }
  }
}

/// What a C integer type is for, beyond its width. Only affects how a
/// value is accepted and printed; every role stores the same way.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IntRole {
  Plain,
  /// A C character type: `char`, `signed char`, `unsigned char`,
  /// `wchar_t`, `char16_t`, `char32_t`. Accepts a one-character string
  /// as well as a number.
  Character,
  /// One of Rust's `NonZero` integers: zero is refused.
  NonZero,
  /// `Option` of a `NonZero` integer, which Rust stores in the same
  /// width with zero meaning `None`: `nil` both ways.
  OptionalNonZero,
}

pub struct CType {
  pub kind: Kind,
  /// The name the type prints as: `int`, `struct point *`, `Point`.
  pub name: String,
  pub is_const: bool,
  /// libffi's view of the type, built on first use.
  ffi: OnceLock<FfiType>,
  /// The same for a record passed under the other x86-64 convention
  /// from the platform's own, which classifies aggregates differently.
  ffi_foreign_abi: OnceLock<FfiType>,
}

pub enum Kind {
  Void,
  Bool,
  Int {
    size: usize,
    signed: bool,
    role: IntRole,
  },
  Float,
  Double,
  LongDouble,
  ComplexFloat,
  ComplexDouble,
  /// A Rust `char`: four bytes holding a Unicode scalar value.
  RustChar,
  Pointer(PointerInfo),
  Array {
    element: TypeRef,
    /// `None` for an array of unknown length: a flexible array member,
    /// or an `extern` array declared without a bound.
    length: Option<usize>,
  },
  Record(Arc<Record>),
  Enum(Arc<EnumInfo>),
  Function(Arc<Signature>),
}

pub struct PointerInfo {
  pub target: TypeRef,
  /// Set for the string types: a value of this type converts to and
  /// from a Zuri string in this encoding.
  pub text: Option<Encoding>,
  /// A Rust reference, `NonNull<T>` or `Box<T>`: never null, so `nil`
  /// is refused on the way in.
  pub nonnull: bool,
}

pub struct EnumInfo {
  pub underlying: TypeRef,
  pub constants: Mutex<Vec<(String, i128)>>,
}

impl EnumInfo {
  pub fn value_of(&self, name: &str) -> Option<i128> {
    let constants = self.constants.lock().unwrap();
    constants.iter().find(|(n, _)| n == name).map(|(_, v)| *v)
  }
}

pub struct Signature {
  pub returns: TypeRef,
  pub params: Vec<TypeRef>,
  pub param_names: Vec<Option<String>>,
  /// Arguments after the fixed ones are allowed, C's `...`.
  pub variadic: bool,
  pub abi: Abi,
}

impl Signature {
  pub fn describe(&self) -> String {
    let mut out = String::new();
    let _ = write!(out, "{} (", self.returns.name);

    for (i, p) in self.params.iter().enumerate() {
      if i > 0 {
        out.push_str(", ");
      }
      out.push_str(&p.name);
    }

    if self.variadic {
      if !self.params.is_empty() {
        out.push_str(", ");
      }
      out.push_str("...");
    }

    out.push(')');
    out
  }
}

impl CType {
  pub fn new(kind: Kind, name: impl Into<String>) -> TypeRef {
    Arc::new(CType {
      kind,
      name: name.into(),
      is_const: false,
      ffi: OnceLock::new(),
      ffi_foreign_abi: OnceLock::new(),
    })
  }

  /// A copy of `ty` under another name, which is what a typedef is.
  pub fn renamed(ty: &TypeRef, name: impl Into<String>) -> TypeRef {
    Arc::new(CType {
      kind: ty.kind.share(),
      name: name.into(),
      is_const: ty.is_const,
      ffi: OnceLock::new(),
      ffi_foreign_abi: OnceLock::new(),
    })
  }

  /// `ty` with `const` added.
  pub fn constant(ty: &TypeRef) -> TypeRef {
    if ty.is_const {
      return ty.clone();
    }

    let name = match ty.kind {
      Kind::Pointer(_) => format!("{} const", ty.name),
      _ => format!("const {}", ty.name),
    };

    Arc::new(CType {
      kind: ty.kind.share(),
      name,
      is_const: true,
      ffi: OnceLock::new(),
      ffi_foreign_abi: OnceLock::new(),
    })
  }

  pub fn int(size: usize, signed: bool, name: &str) -> TypeRef {
    CType::new(
      Kind::Int {
        size,
        signed,
        role: IntRole::Plain,
      },
      name,
    )
  }

  pub fn character(size: usize, signed: bool, name: &str) -> TypeRef {
    CType::new(
      Kind::Int {
        size,
        signed,
        role: IntRole::Character,
      },
      name,
    )
  }

  pub fn pointer_to(target: &TypeRef) -> TypeRef {
    let name = pointer_name(target);
    CType::new(
      Kind::Pointer(PointerInfo {
        target: target.clone(),
        text: None,
        nonnull: false,
      }),
      name,
    )
  }

  pub fn pointer_with(
    target: &TypeRef,
    text: Option<Encoding>,
    nonnull: bool,
    name: String,
  ) -> TypeRef {
    CType::new(
      Kind::Pointer(PointerInfo {
        target: target.clone(),
        text,
        nonnull,
      }),
      name,
    )
  }

  pub fn array_of(element: &TypeRef, length: Option<usize>) -> TypeRef {
    let name = match length {
      Some(n) => format!("{}[{n}]", element.name),
      None => format!("{}[]", element.name),
    };
    CType::new(
      Kind::Array {
        element: element.clone(),
        length,
      },
      name,
    )
  }

  pub fn function(sig: Signature) -> TypeRef {
    let name = sig.describe();
    CType::new(Kind::Function(Arc::new(sig)), name)
  }

  pub fn is_void(&self) -> bool {
    matches!(self.kind, Kind::Void)
  }

  pub fn is_record(&self) -> bool {
    matches!(self.kind, Kind::Record(_))
  }

  pub fn record(&self) -> Option<&Arc<Record>> {
    match &self.kind {
      Kind::Record(r) => Some(r),
      _ => None,
    }
  }

  pub fn pointer(&self) -> Option<&PointerInfo> {
    match &self.kind {
      Kind::Pointer(p) => Some(p),
      _ => None,
    }
  }

  /// The signature a function-pointer type points at.
  pub fn function_pointer(&self) -> Option<&Arc<Signature>> {
    match &self.kind {
      Kind::Pointer(p) => match &p.target.kind {
        Kind::Function(sig) => Some(sig),
        _ => None,
      },
      Kind::Function(sig) => Some(sig),
      _ => None,
    }
  }

  /// A kind name for the Zuri side: 'int', 'float', 'pointer',
  /// 'struct' and so on.
  pub fn kind_name(&self) -> &'static str {
    match &self.kind {
      Kind::Void => "void",
      Kind::Bool => "bool",
      Kind::Int { .. } => "int",
      Kind::Float | Kind::Double | Kind::LongDouble => "float",
      Kind::ComplexFloat | Kind::ComplexDouble => "complex",
      Kind::RustChar => "char",
      Kind::Pointer(p) if p.text.is_some() => "string",
      Kind::Pointer(p) if matches!(p.target.kind, Kind::Function(_)) => "function pointer",
      Kind::Pointer(_) => "pointer",
      Kind::Array { .. } => "array",
      Kind::Record(r) if r.variants.get().is_some_and(|v| v.is_some()) => "enum",
      Kind::Record(r) if r.is_union => "union",
      Kind::Record(_) => "struct",
      Kind::Enum(_) => "enum",
      Kind::Function(_) => "function",
    }
  }

  /// Size in bytes, or `None` for a type with no size: `void`, a
  /// function, an array of unknown length and a record that has not
  /// been defined.
  pub fn size(&self) -> Option<usize> {
    match &self.kind {
      Kind::Void | Kind::Function(_) => None,
      Kind::Bool => Some(1),
      Kind::Int { size, .. } => Some(*size),
      Kind::Float => Some(4),
      Kind::Double => Some(8),
      Kind::LongDouble => Some(match LONG_DOUBLE {
        LongDoubleRepr::Double => 8,
        _ => 16,
      }),
      Kind::ComplexFloat => Some(8),
      Kind::ComplexDouble => Some(16),
      Kind::RustChar => Some(4),
      Kind::Pointer(_) => Some(POINTER_SIZE),
      Kind::Array { element, length } => {
        let n = (*length)?;
        element.size().map(|s| s * n)
      },
      Kind::Record(r) => r.layout().ok().map(|l| l.size),
      Kind::Enum(e) => e.underlying.size(),
    }
  }

  pub fn align(&self) -> Option<usize> {
    match &self.kind {
      Kind::Void | Kind::Function(_) => None,
      Kind::Bool => Some(1),
      Kind::Int { size, .. } => Some(*size),
      Kind::Float => Some(4),
      Kind::Double => Some(8),
      Kind::LongDouble => Some(match LONG_DOUBLE {
        LongDoubleRepr::Double => 8,
        _ => 16,
      }),
      Kind::ComplexFloat => Some(4),
      Kind::ComplexDouble => Some(8),
      Kind::RustChar => Some(4),
      Kind::Pointer(_) => Some(POINTER_SIZE),
      Kind::Array { element, .. } => element.align(),
      Kind::Record(r) => r.layout().ok().map(|l| l.align),
      Kind::Enum(e) => e.underlying.align(),
    }
  }

  /// Size, or an error naming why the type has none.
  pub fn require_size(&self) -> Result<usize, String> {
    match &self.kind {
      Kind::Record(r) => r.layout().map(|l| l.size),
      _ => self
        .size()
        .ok_or_else(|| format!("'{}' has no size", self.name)),
    }
  }

  pub fn require_align(&self) -> Result<usize, String> {
    match &self.kind {
      Kind::Record(r) => r.layout().map(|l| l.align),
      _ => self
        .align()
        .ok_or_else(|| format!("'{}' has no alignment", self.name)),
    }
  }

  /// libffi's description of this type, for passing it by value.
  pub fn ffi_type(&self, abi: Abi) -> Result<*mut raw::ffi_type, String> {
    // The skeleton of an aggregate depends on the convention it is
    // passed under, so a record used under an explicit convention
    // other than the platform's own gets a skeleton of its own.
    if abi != Abi::Default && abi.is_win64() != cfg!(windows) && needs_skeleton(self) {
      if self.ffi_foreign_abi.get().is_none() {
        let built = self.skeleton_for(abi)?;
        let _ = self.ffi_foreign_abi.set(built);
      }
      return Ok(self.ffi_foreign_abi.get().unwrap().as_ptr());
    }

    if let Some(ft) = self.ffi.get() {
      return Ok(ft.as_ptr());
    }

    let built = match &self.kind {
      Kind::Void => FfiType::Static(&raw mut raw::ffi_type_void),
      Kind::Bool => FfiType::Static(&raw mut raw::ffi_type_uint8),
      Kind::Int { size, signed, .. } => int_ffi_type(*size, *signed),
      Kind::Float => FfiType::Static(&raw mut raw::ffi_type_float),
      Kind::Double => FfiType::Static(&raw mut raw::ffi_type_double),
      Kind::LongDouble => match LONG_DOUBLE {
        LongDoubleRepr::Double => FfiType::Static(&raw mut raw::ffi_type_double),
        _ => FfiType::Static(&raw mut raw::ffi_type_longdouble),
      },
      Kind::ComplexFloat => complex_ffi_type(false)?,
      Kind::ComplexDouble => complex_ffi_type(true)?,
      Kind::RustChar => FfiType::Static(&raw mut raw::ffi_type_uint32),
      Kind::Pointer(_) => FfiType::Static(&raw mut raw::ffi_type_pointer),
      Kind::Enum(e) => {
        return e.underlying.ffi_type(abi);
      },
      Kind::Array { .. } => {
        return Err(format!(
          "an array cannot be passed by value; pass a pointer to its first element instead of '{}'",
          self.name
        ));
      },
      Kind::Function(_) => {
        return Err(format!(
          "a function cannot be passed by value; use a pointer to '{}'",
          self.name
        ));
      },
      Kind::Record(_) => self.skeleton_for(Abi::Default)?,
    };

    let _ = self.ffi.set(built);
    Ok(self.ffi.get().unwrap().as_ptr())
  }

  fn skeleton_for(&self, abi: Abi) -> Result<FfiType, String> {
    let layout = match &self.kind {
      Kind::Record(r) => r.layout()?,
      _ => unreachable!("only records get a skeleton"),
    };

    if layout.size == 0 {
      return Err(format!(
        "'{}' has no size, and a zero-sized value cannot be passed by value",
        self.name
      ));
    }

    let mut scalars = Vec::new();
    flatten(self, 0, &mut scalars, 0);

    Ok(skeleton(layout.size, layout.align, &scalars, abi))
  }

  /// Whether two types are the same for the purposes of a conversion.
  /// Records, enums and functions compare by identity; everything else
  /// by shape.
  pub fn same(a: &CType, b: &CType) -> bool {
    match (&a.kind, &b.kind) {
      (Kind::Void, Kind::Void)
      | (Kind::Bool, Kind::Bool)
      | (Kind::Float, Kind::Float)
      | (Kind::Double, Kind::Double)
      | (Kind::LongDouble, Kind::LongDouble)
      | (Kind::ComplexFloat, Kind::ComplexFloat)
      | (Kind::ComplexDouble, Kind::ComplexDouble)
      | (Kind::RustChar, Kind::RustChar) => true,
      (
        Kind::Int {
          size: s1,
          signed: g1,
          ..
        },
        Kind::Int {
          size: s2,
          signed: g2,
          ..
        },
      ) => s1 == s2 && g1 == g2,
      (Kind::Pointer(p1), Kind::Pointer(p2)) => {
        p1.text == p2.text && CType::same(&p1.target, &p2.target)
      },
      (
        Kind::Array {
          element: e1,
          length: l1,
        },
        Kind::Array {
          element: e2,
          length: l2,
        },
      ) => l1 == l2 && CType::same(e1, e2),
      (Kind::Record(r1), Kind::Record(r2)) => Arc::ptr_eq(r1, r2),
      (Kind::Enum(e1), Kind::Enum(e2)) => Arc::ptr_eq(e1, e2),
      (Kind::Function(f1), Kind::Function(f2)) => {
        Arc::ptr_eq(f1, f2)
          || (f1.variadic == f2.variadic
            && f1.abi == f2.abi
            && f1.params.len() == f2.params.len()
            && CType::same(&f1.returns, &f2.returns)
            && f1
              .params
              .iter()
              .zip(&f2.params)
              .all(|(x, y)| CType::same(x, y)))
      },
      _ => false,
    }
  }
}

impl Kind {
  /// A second `Kind` describing the same type, for typedefs and
  /// qualified copies. Everything with identity is behind an `Arc`, so
  /// the copy shares it.
  fn share(&self) -> Kind {
    match self {
      Kind::Void => Kind::Void,
      Kind::Bool => Kind::Bool,
      Kind::Int { size, signed, role } => Kind::Int {
        size: *size,
        signed: *signed,
        role: *role,
      },
      Kind::Float => Kind::Float,
      Kind::Double => Kind::Double,
      Kind::LongDouble => Kind::LongDouble,
      Kind::ComplexFloat => Kind::ComplexFloat,
      Kind::ComplexDouble => Kind::ComplexDouble,
      Kind::RustChar => Kind::RustChar,
      Kind::Pointer(p) => Kind::Pointer(PointerInfo {
        target: p.target.clone(),
        text: p.text,
        nonnull: p.nonnull,
      }),
      Kind::Array { element, length } => Kind::Array {
        element: element.clone(),
        length: *length,
      },
      Kind::Record(r) => Kind::Record(r.clone()),
      Kind::Enum(e) => Kind::Enum(e.clone()),
      Kind::Function(f) => Kind::Function(f.clone()),
    }
  }
}

fn needs_skeleton(ty: &CType) -> bool {
  matches!(ty.kind, Kind::Record(_))
}

/// How a pointer to `target` is spelled.
pub fn pointer_name(target: &CType) -> String {
  match &target.kind {
    Kind::Function(sig) => {
      let params: Vec<&str> = sig.params.iter().map(|p| p.name.as_str()).collect();
      let mut list = params.join(", ");
      if sig.variadic {
        if !list.is_empty() {
          list.push_str(", ");
        }
        list.push_str("...");
      }
      format!("{} (*)({})", sig.returns.name, list)
    },
    _ => format!("{} *", target.name),
  }
}

// Records.

/// A member as it was declared, before layout.
#[derive(Clone)]
pub struct FieldSpec {
  pub name: String,
  pub ty: TypeRef,
  /// Width in bits, for a bitfield.
  pub bits: Option<u32>,
  /// An explicit minimum alignment: `_Alignas`, `aligned(n)`.
  pub align: Option<usize>,
}

/// A member after layout.
#[derive(Clone)]
pub struct Field {
  /// Empty for an anonymous struct or union member, whose own members
  /// are reached as if they were this record's.
  pub name: String,
  pub ty: TypeRef,
  /// Byte offset from the start of the record. For a bitfield, the
  /// byte its first bit sits in.
  pub offset: usize,
  /// For a bitfield, the offset of its first bit from the start of the
  /// record and its width.
  pub bits: Option<(usize, u32)>,
}

pub struct Layout {
  pub fields: Vec<Field>,
  pub size: usize,
  pub align: usize,
}

impl Layout {
  /// Finds a member by name, looking through anonymous members, and
  /// returns it with its offset adjusted to this record's start.
  pub fn find(&self, name: &str) -> Option<Field> {
    for field in &self.fields {
      if field.name == name {
        return Some(field.clone());
      }

      if field.name.is_empty()
        && let Some(record) = field.ty.record()
        && let Ok(inner) = record.layout()
        && let Some(mut found) = inner.find(name)
      {
        found.offset += field.offset;
        found.bits = found.bits.map(|(b, w)| (b + field.offset * 8, w));
        return Some(found);
      }
    }

    None
  }

  /// Every named member in declaration order, with anonymous members
  /// flattened in place.
  pub fn flat_fields(&self) -> Vec<Field> {
    let mut out = Vec::new();

    for field in &self.fields {
      if field.name.is_empty() {
        if let Some(record) = field.ty.record()
          && let Ok(inner) = record.layout()
        {
          for mut f in inner.flat_fields() {
            f.offset += field.offset;
            f.bits = f.bits.map(|(b, w)| (b + field.offset * 8, w));
            out.push(f);
          }
        }
        continue;
      }

      out.push(field.clone());
    }

    out
  }
}

/// A Rust enum with fields, laid out per the `repr(C)` rules for them.
pub struct Variants {
  /// The field of this record that holds the discriminant, found at
  /// offset zero in both layouts Rust uses.
  pub tag: TypeRef,
  pub cases: Vec<Variant>,
  /// `repr(C)` and `repr(C, int)` put the payload in a union after the
  /// tag; `repr(int)` alone makes the whole enum a union of structs
  /// that each begin with the tag.
  pub tagged_union: bool,
}

pub struct Variant {
  pub name: String,
  pub discriminant: i128,
  /// The payload struct, laid out as Rust lays it out, tag first when
  /// `tagged_union` is false. `None` for a unit variant.
  pub payload: Option<TypeRef>,
}

pub struct Record {
  pub tag: Option<String>,
  pub is_union: bool,
  draft: Mutex<Draft>,
  layout: OnceLock<Layout>,
  pub variants: OnceLock<Option<Variants>>,
}

struct Draft {
  fields: Vec<FieldSpec>,
  pack: Option<usize>,
  align: Option<usize>,
  /// A body has been given: `struct x { ... }` rather than `struct x;`.
  defined: bool,
  /// Why a record that will never have a layout has none, for the error
  /// that says so.
  opaque: Option<String>,
}

impl Record {
  pub fn new(tag: Option<String>, is_union: bool) -> Arc<Record> {
    Arc::new(Record {
      tag,
      is_union,
      draft: Mutex::new(Draft {
        fields: Vec::new(),
        pack: None,
        align: None,
        defined: false,
        opaque: None,
      }),
      layout: OnceLock::new(),
      variants: OnceLock::new(),
    })
  }

  pub fn is_sealed(&self) -> bool {
    self.layout.get().is_some()
  }

  pub fn is_defined(&self) -> bool {
    self.is_sealed() || self.draft.lock().unwrap().defined
  }

  fn check_open(&self) -> Result<(), String> {
    if self.is_sealed() {
      return Err(format!(
        "'{}' has already been laid out and can no longer change",
        self.display_name()
      ));
    }
    Ok(())
  }

  pub fn display_name(&self) -> String {
    let word = if self.is_union { "union" } else { "struct" };
    match &self.tag {
      Some(t) => format!("{word} {t}"),
      None => format!("anonymous {word}"),
    }
  }

  pub fn add_field(&self, spec: FieldSpec) -> Result<(), String> {
    self.check_open()?;

    if let Some(bits) = spec.bits {
      if !matches!(spec.ty.kind, Kind::Int { .. } | Kind::Bool | Kind::Enum(_)) {
        return Err(format!(
          "bitfield '{}' must have an integer type, not '{}'",
          spec.name, spec.ty.name
        ));
      }

      let width = spec.ty.size().unwrap_or(0) as u32 * 8;
      if bits > width {
        return Err(format!(
          "bitfield '{}' is {bits} bits wide, more than its type '{}' holds",
          spec.name, spec.ty.name
        ));
      }

      if bits == 0 && !spec.name.is_empty() {
        return Err(format!(
          "bitfield '{}' has zero width; only an unnamed bitfield may",
          spec.name
        ));
      }
    }

    let mut draft = self.draft.lock().unwrap();

    if !spec.name.is_empty() && draft.fields.iter().any(|f| f.name == spec.name) {
      return Err(format!(
        "'{}' already has a member named '{}'",
        self.display_name(),
        spec.name
      ));
    }

    if let Some(last) = draft.fields.last()
      && matches!(last.ty.kind, Kind::Array { length: None, .. })
    {
      return Err(format!(
        "'{}' ends in a flexible array member, so nothing can follow it",
        self.display_name()
      ));
    }

    draft.fields.push(spec);
    draft.defined = true;
    Ok(())
  }

  pub fn set_pack(&self, pack: usize) -> Result<(), String> {
    self.check_open()?;

    if !pack.is_power_of_two() {
      return Err(format!("packing must be a power of two, not {pack}"));
    }

    self.draft.lock().unwrap().pack = Some(pack);
    Ok(())
  }

  pub fn set_align(&self, align: usize) -> Result<(), String> {
    self.check_open()?;

    if !align.is_power_of_two() {
      return Err(format!("alignment must be a power of two, not {align}"));
    }

    self.draft.lock().unwrap().align = Some(align);
    Ok(())
  }

  pub fn mark_defined(&self) {
    self.draft.lock().unwrap().defined = true;
  }

  /// Marks the record as one whose layout is never known, with the
  /// reason reported when something needs its size.
  pub fn set_opaque(&self, reason: String) {
    self.draft.lock().unwrap().opaque = Some(reason);
  }

  /// Lays the record out on first use and returns the result; the
  /// record is sealed from then on.
  pub fn layout(&self) -> Result<&Layout, String> {
    if let Some(layout) = self.layout.get() {
      return Ok(layout);
    }

    let draft = self.draft.lock().unwrap();

    if let Some(reason) = &draft.opaque {
      return Err(reason.clone());
    }

    if !draft.defined {
      return Err(format!(
        "'{}' is declared but never defined, so it can only be used through a pointer",
        self.display_name()
      ));
    }

    let computed = compute_layout(self.is_union, &draft)?;
    drop(draft);

    let _ = self.layout.set(computed);
    Ok(self.layout.get().unwrap())
  }
}

fn align_up(value: usize, align: usize) -> usize {
  if align <= 1 {
    return value;
  }
  value.div_ceil(align) * align
}

fn compute_layout(is_union: bool, draft: &Draft) -> Result<Layout, String> {
  let mut fields = Vec::with_capacity(draft.fields.len());
  let mut record_align = 1usize;
  let mut size = 0usize;
  let mut cursor_bits = 0usize;

  // MSVC keeps a run of bitfields in one storage unit of the declared
  // type's size for as long as they fit and share that size.
  let mut unit: Option<(usize, usize, usize)> = None; // (start bits, size bytes, used bits)

  let pack = draft.pack;
  let clamp = |a: usize| match pack {
    Some(p) => a.min(p),
    None => a,
  };

  for spec in &draft.fields {
    let natural_size = spec.ty.require_size().or_else(|e| {
      if matches!(spec.ty.kind, Kind::Array { length: None, .. }) {
        Ok(0)
      } else {
        Err(format!("member '{}': {e}", spec.name))
      }
    })?;
    let natural_align = spec
      .ty
      .require_align()
      .map_err(|e| format!("member '{}': {e}", spec.name))?;

    match spec.bits {
      None => {
        if let Some((start, bytes, _)) = unit.take() {
          cursor_bits = cursor_bits.max(start + bytes * 8);
        }

        let mut align = clamp(natural_align);
        if let Some(explicit) = spec.align {
          align = align.max(explicit);
        }
        record_align = record_align.max(align);

        let offset = if is_union {
          0
        } else {
          align_up(cursor_bits.div_ceil(8), align)
        };

        fields.push(Field {
          name: spec.name.clone(),
          ty: spec.ty.clone(),
          offset,
          bits: None,
        });

        if is_union {
          size = size.max(natural_size);
        } else {
          cursor_bits = (offset + natural_size) * 8;
        }
      },
      Some(width) => {
        let width = width as usize;
        let unit_bytes = natural_size;
        let unit_bits = unit_bytes * 8;

        if is_union {
          if width > 0 {
            if !spec.name.is_empty() || MSVC_LAYOUT {
              record_align = record_align.max(clamp(natural_align));
            }
            size = size.max(width.div_ceil(8));
            fields.push(Field {
              name: spec.name.clone(),
              ty: spec.ty.clone(),
              offset: 0,
              bits: Some((0, width as u32)),
            });
          }
          continue;
        }

        let bit_offset;

        if MSVC_LAYOUT {
          if width == 0 {
            if let Some((start, bytes, _)) = unit.take() {
              cursor_bits = start + bytes * 8;
            }
            continue;
          }

          let fits = match unit {
            Some((_, bytes, used)) => bytes == unit_bytes && used + width <= unit_bits,
            None => false,
          };

          if fits {
            let (start, bytes, used) = unit.unwrap();
            bit_offset = start + used;
            unit = Some((start, bytes, used + width));
          } else {
            if let Some((start, bytes, _)) = unit.take() {
              cursor_bits = start + bytes * 8;
            }
            let align = clamp(natural_align);
            record_align = record_align.max(align);
            let start = align_up(cursor_bits.div_ceil(8), align) * 8;
            bit_offset = start;
            unit = Some((start, unit_bytes, width));
            cursor_bits = start + unit_bits;
          }
        } else {
          let type_align_bits = natural_align * 8;

          if width == 0 {
            cursor_bits = align_up(cursor_bits, type_align_bits);
            continue;
          }

          let packed = pack == Some(1);
          let mut offset = cursor_bits;

          if !packed {
            // A bitfield never straddles a boundary of its type's own
            // alignment; if it would, it starts at the next one.
            let boundary = clamp(natural_align) * 8;
            if offset / boundary != (offset + width - 1) / boundary {
              offset = align_up(offset, boundary);
            }
          }

          if !spec.name.is_empty() {
            record_align = record_align.max(clamp(natural_align));
          }

          bit_offset = offset;
          cursor_bits = offset + width;
          let _ = type_align_bits;
        }

        fields.push(Field {
          name: spec.name.clone(),
          ty: spec.ty.clone(),
          offset: bit_offset / 8,
          bits: Some((bit_offset, width as u32)),
        });
      },
    }
  }

  if let Some((start, bytes, _)) = unit.take() {
    cursor_bits = cursor_bits.max(start + bytes * 8);
  }

  if let Some(explicit) = draft.align {
    record_align = record_align.max(explicit);
  }

  if !is_union {
    size = cursor_bits.div_ceil(8);
  }

  let size = align_up(size, record_align);

  Ok(Layout {
    fields,
    size,
    align: record_align,
  })
}

// libffi types.

/// An `ffi_type` that is either one of libffi's own statics or one this
/// module built and owns.
pub enum FfiType {
  Static(*mut raw::ffi_type),
  Owned(Box<OwnedFfiType>),
}

pub struct OwnedFfiType {
  ty: raw::ffi_type,
  _elements: Vec<*mut raw::ffi_type>,
  _children: Vec<FfiType>,
}

// SAFETY: every pointer in here is either one of libffi's immutable
// statics or points into memory this value owns and never mutates after
// construction, so sharing it between threads is sound.
unsafe impl Send for FfiType {}
unsafe impl Sync for FfiType {}

impl FfiType {
  pub fn as_ptr(&self) -> *mut raw::ffi_type {
    match self {
      FfiType::Static(p) => *p,
      FfiType::Owned(b) => &b.ty as *const raw::ffi_type as *mut raw::ffi_type,
    }
  }

  /// A struct type with its size and alignment already set, which
  /// libffi then takes as given, and `elements` for it to classify.
  fn aggregate(size: usize, align: usize, elements: Vec<FfiType>) -> FfiType {
    let mut pointers: Vec<*mut raw::ffi_type> = elements.iter().map(|e| e.as_ptr()).collect();
    pointers.push(ptr::null_mut());

    let mut owned = Box::new(OwnedFfiType {
      ty: raw::ffi_type {
        size,
        alignment: align as u16,
        type_: raw::FFI_TYPE_STRUCT as u16,
        elements: ptr::null_mut(),
      },
      _elements: pointers,
      _children: elements,
    });

    owned.ty.elements = owned._elements.as_mut_ptr();
    FfiType::Owned(owned)
  }
}

fn int_ffi_type(size: usize, signed: bool) -> FfiType {
  {
    match (size, signed) {
      (1, true) => FfiType::Static(&raw mut raw::ffi_type_sint8),
      (1, false) => FfiType::Static(&raw mut raw::ffi_type_uint8),
      (2, true) => FfiType::Static(&raw mut raw::ffi_type_sint16),
      (2, false) => FfiType::Static(&raw mut raw::ffi_type_uint16),
      (4, true) => FfiType::Static(&raw mut raw::ffi_type_sint32),
      (4, false) => FfiType::Static(&raw mut raw::ffi_type_uint32),
      (8, true) => FfiType::Static(&raw mut raw::ffi_type_sint64),
      (8, false) => FfiType::Static(&raw mut raw::ffi_type_uint64),
      // libffi has no 128-bit integer. Two 64-bit halves at 16-byte
      // alignment classify and align the way `__int128` does on
      // System V and AArch64; `call.rs` supplies the even-register rule
      // AArch64 Linux adds on top.
      _ => FfiType::aggregate(
        16,
        16,
        vec![
          FfiType::Static(&raw mut raw::ffi_type_uint64),
          FfiType::Static(&raw mut raw::ffi_type_uint64),
        ],
      ),
    }
  }
}

#[cfg(not(windows))]
fn complex_ffi_type(double: bool) -> Result<FfiType, String> {
  Ok(if double {
    FfiType::Static(&raw mut raw::ffi_type_complex_double)
  } else {
    FfiType::Static(&raw mut raw::ffi_type_complex_float)
  })
}

#[cfg(windows)]
fn complex_ffi_type(_double: bool) -> Result<FfiType, String> {
  Err(
    "complex numbers cannot be passed by value on Windows, whose C compiler has no _Complex".into(),
  )
}

/// One scalar inside an aggregate, as the ABI classifiers see it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Scalar {
  Int,
  F32,
  F64,
  /// x87 extended precision.
  X87,
  /// IEEE binary128.
  F128,
}

/// Collects every scalar in `ty` as `(offset, size, class)`, offsets
/// taken from the start of the outermost record.
fn flatten(ty: &CType, base: usize, out: &mut Vec<(usize, usize, Scalar)>, depth: usize) {
  // Beyond sixty-four bytes no ABI here passes an aggregate in
  // registers, so the detail stops mattering.
  if out.len() > 64 || depth > 32 {
    return;
  }

  match &ty.kind {
    Kind::Bool | Kind::RustChar | Kind::Pointer(_) => {
      out.push((base, ty.size().unwrap_or(8), Scalar::Int));
    },
    Kind::Int { size, .. } => out.push((base, *size, Scalar::Int)),
    Kind::Enum(e) => flatten(&e.underlying, base, out, depth + 1),
    Kind::Float => out.push((base, 4, Scalar::F32)),
    Kind::Double => out.push((base, 8, Scalar::F64)),
    Kind::LongDouble => match LONG_DOUBLE {
      LongDoubleRepr::X87 => out.push((base, 16, Scalar::X87)),
      LongDoubleRepr::Quad => out.push((base, 16, Scalar::F128)),
      LongDoubleRepr::Double => out.push((base, 8, Scalar::F64)),
    },
    Kind::ComplexFloat => {
      out.push((base, 4, Scalar::F32));
      out.push((base + 4, 4, Scalar::F32));
    },
    Kind::ComplexDouble => {
      out.push((base, 8, Scalar::F64));
      out.push((base + 8, 8, Scalar::F64));
    },
    Kind::Array { element, length } => {
      let Some(n) = length else {
        return;
      };
      let Some(step) = element.size() else {
        return;
      };
      for i in 0..*n {
        if out.len() > 64 {
          break;
        }
        flatten(element, base + i * step, out, depth + 1);
      }
    },
    Kind::Record(r) => {
      let Ok(layout) = r.layout() else {
        return;
      };
      for field in &layout.fields {
        match field.bits {
          Some((bit, width)) => {
            let first = bit / 8;
            let last = (bit + width as usize - 1) / 8;
            for byte in first..=last {
              out.push((base + byte, 1, Scalar::Int));
            }
          },
          None => flatten(&field.ty, base + field.offset, out, depth + 1),
        }
      }
    },
    Kind::Void | Kind::Function(_) => {},
  }
}

fn skeleton(size: usize, align: usize, scalars: &[(usize, usize, Scalar)], abi: Abi) -> FfiType {
  let exact = scalars.iter().all(|&(offset, len, s)| {
    let natural = match s {
      Scalar::Int => len.min(16),
      Scalar::F32 => 4,
      Scalar::F64 => 8,
      Scalar::X87 | Scalar::F128 => 16,
    };
    offset % natural.max(1) == 0
  });

  if abi.is_win64() {
    return win64_skeleton(size, align);
  }

  if cfg!(target_arch = "x86_64") {
    sysv_skeleton(size, align, scalars, exact)
  } else {
    aapcs64_skeleton(size, align, scalars)
  }
}

/// Microsoft x64 decides by size alone: 1, 2, 4 and 8 bytes travel in a
/// register as an integer, anything else by reference.
fn win64_skeleton(size: usize, align: usize) -> FfiType {
  let words = size.div_ceil(8).max(1);
  let elements = (0..words)
    .map(|_| FfiType::Static(&raw mut raw::ffi_type_uint64))
    .collect();
  FfiType::aggregate(size, align, elements)
}

/// A struct libffi classifies as MEMORY without that changing its size:
/// the first element is itself a struct too large for registers.
fn memory_skeleton(size: usize, align: usize) -> FfiType {
  let poison = FfiType::aggregate(33, 1, vec![FfiType::Static(&raw mut raw::ffi_type_uint8)]);
  FfiType::aggregate(size, align, vec![poison])
}

/// System V AMD64: each eightbyte is INTEGER if any integer overlaps
/// it and SSE if only floating point does. Past sixteen bytes, or with a
/// member at an offset its type would never be aligned to, the whole
/// thing goes in memory.
fn sysv_skeleton(
  size: usize,
  align: usize,
  scalars: &[(usize, usize, Scalar)],
  exact: bool,
) -> FfiType {
  if size > 16 || !exact {
    return memory_skeleton(size, align);
  }

  if scalars.iter().any(|&(_, _, s)| s == Scalar::X87) {
    // A long double fills sixteen bytes on its own. Alone it is
    // classified X87 and X87UP; shared with anything else, as in a
    // union, the classes cannot merge and the value goes in memory.
    if scalars.len() == 1 {
      let element = FfiType::Static(&raw mut raw::ffi_type_longdouble);
      return FfiType::aggregate(size, align, vec![element]);
    }
    return memory_skeleton(size, align);
  }

  let words = size.div_ceil(8);
  let mut elements = Vec::new();

  for word in 0..words {
    let start = word * 8;
    let end = (start + 8).min(size);
    let mut has_int = false;
    let mut has_float = false;

    for &(offset, len, scalar) in scalars {
      if offset < end && offset + len > start {
        match scalar {
          Scalar::Int => has_int = true,
          _ => has_float = true,
        }
      }
    }

    let tail = end - start;

    if has_int || (!has_float && word + 1 < words) {
      elements.push(match tail {
        1 => FfiType::Static(&raw mut raw::ffi_type_uint8),
        2 => FfiType::Static(&raw mut raw::ffi_type_uint16),
        3 | 4 => FfiType::Static(&raw mut raw::ffi_type_uint32),
        _ => FfiType::Static(&raw mut raw::ffi_type_uint64),
      });
    } else if has_float {
      elements.push(if tail <= 4 {
        FfiType::Static(&raw mut raw::ffi_type_float)
      } else {
        FfiType::Static(&raw mut raw::ffi_type_double)
      });
    }
  }

  // Every element but the last is eight bytes wide, so libffi places
  // each one at the start of the eightbyte its class was computed for.
  FfiType::aggregate(size, align, elements)
}

/// AAPCS64: a homogeneous floating-point aggregate of up to four members
/// goes in vector registers, anything else of sixteen bytes or less in
/// general registers, and anything larger by reference.
fn aapcs64_skeleton(size: usize, align: usize, scalars: &[(usize, usize, Scalar)]) -> FfiType {
  if let Some(first) = scalars.first() {
    let base = first.2;
    let width = first.1;
    let homogeneous = matches!(base, Scalar::F32 | Scalar::F64 | Scalar::F128)
      && scalars.len() <= 4
      && scalars.len() * width == size
      && scalars
        .iter()
        .enumerate()
        .all(|(i, &(offset, len, s))| s == base && len == width && offset == i * width);

    if homogeneous {
      let element = match base {
        Scalar::F32 => &raw mut raw::ffi_type_float,
        Scalar::F64 => &raw mut raw::ffi_type_double,
        _ => &raw mut raw::ffi_type_longdouble,
      };
      let elements = scalars.iter().map(|_| FfiType::Static(element)).collect();
      return FfiType::aggregate(size, align, elements);
    }
  }

  let words = size.div_ceil(8).max(1);
  let elements = (0..words)
    .map(|_| FfiType::Static(&raw mut raw::ffi_type_uint64))
    .collect();
  FfiType::aggregate(size, align, elements)
}

/// Whether AArch64 Linux starts an argument of `ty` on an even
/// register: its 128-bit integers, and aggregates that need 16-byte
/// alignment and travel in general registers.
pub fn wants_even_pair(ty: &CType) -> bool {
  if cfg!(not(all(
    target_arch = "aarch64",
    not(target_vendor = "apple"),
    not(windows)
  ))) {
    return false;
  }

  match &ty.kind {
    Kind::Int { size: 16, .. } => true,
    Kind::Enum(e) => wants_even_pair(&e.underlying),
    Kind::Record(r) => {
      let Ok(layout) = r.layout() else {
        return false;
      };
      if layout.align != 16 || layout.size > 16 {
        return false;
      }
      let mut scalars = Vec::new();
      flatten(ty, 0, &mut scalars, 0);
      !scalars
        .iter()
        .all(|&(_, _, s)| matches!(s, Scalar::F32 | Scalar::F64 | Scalar::F128))
    },
    _ => false,
  }
}

/// How many general registers an argument of `ty` takes on AArch64, or
/// `None` when it goes in vector registers instead.
pub fn general_registers(ty: &CType) -> Option<usize> {
  match &ty.kind {
    Kind::Float | Kind::Double | Kind::LongDouble | Kind::ComplexFloat | Kind::ComplexDouble => {
      None
    },
    Kind::Int { size: 16, .. } => Some(2),
    Kind::Enum(e) => general_registers(&e.underlying),
    Kind::Record(r) => {
      let layout = r.layout().ok()?;
      let mut scalars = Vec::new();
      flatten(ty, 0, &mut scalars, 0);
      let hfa = !scalars.is_empty()
        && scalars.len() <= 4
        && scalars
          .iter()
          .all(|&(_, _, s)| matches!(s, Scalar::F32 | Scalar::F64 | Scalar::F128))
        && scalars.windows(2).all(|w| w[0].2 == w[1].2);
      if hfa {
        None
      } else if layout.size > 16 {
        Some(1)
      } else {
        Some(layout.size.div_ceil(8).max(1))
      }
    },
    _ => Some(1),
  }
}

// Built-in type names.

/// Every type the module knows by name without being told: C's own,
/// the `<stdint.h>` family, and the Rust primitives.
pub static BUILTINS: LazyLock<FxHashMap<&'static str, TypeRef>> = LazyLock::new(|| {
  let mut m: FxHashMap<&'static str, TypeRef> = FxHashMap::default();

  let void = CType::new(Kind::Void, "void");
  m.insert("void", void.clone());

  let boolean = CType::new(Kind::Bool, "bool");
  m.insert("bool", boolean.clone());
  m.insert("_Bool", CType::renamed(&boolean, "_Bool"));

  m.insert("char", CType::character(1, CHAR_SIGNED, "char"));
  m.insert("signed char", CType::character(1, true, "signed char"));
  m.insert("unsigned char", CType::character(1, false, "unsigned char"));
  m.insert("short", CType::int(2, true, "short"));
  m.insert("unsigned short", CType::int(2, false, "unsigned short"));
  m.insert("int", CType::int(4, true, "int"));
  m.insert("unsigned int", CType::int(4, false, "unsigned int"));
  m.insert("long", CType::int(LONG_SIZE, true, "long"));
  m.insert(
    "unsigned long",
    CType::int(LONG_SIZE, false, "unsigned long"),
  );
  m.insert("long long", CType::int(8, true, "long long"));
  m.insert(
    "unsigned long long",
    CType::int(8, false, "unsigned long long"),
  );
  m.insert("__int128", CType::int(16, true, "__int128"));
  m.insert(
    "unsigned __int128",
    CType::int(16, false, "unsigned __int128"),
  );
  m.insert("__int128_t", CType::int(16, true, "__int128_t"));
  m.insert("__uint128_t", CType::int(16, false, "__uint128_t"));

  for (name, size, signed) in [
    ("int8_t", 1, true),
    ("uint8_t", 1, false),
    ("int16_t", 2, true),
    ("uint16_t", 2, false),
    ("int32_t", 4, true),
    ("uint32_t", 4, false),
    ("int64_t", 8, true),
    ("uint64_t", 8, false),
    ("intmax_t", 8, true),
    ("uintmax_t", 8, false),
    ("intptr_t", 8, true),
    ("uintptr_t", 8, false),
    ("ptrdiff_t", 8, true),
    ("size_t", 8, false),
    ("ssize_t", 8, true),
    ("off_t", 8, true),
    ("time_t", 8, true),
    ("int_least8_t", 1, true),
    ("uint_least8_t", 1, false),
    ("int_least16_t", 2, true),
    ("uint_least16_t", 2, false),
    ("int_least32_t", 4, true),
    ("uint_least32_t", 4, false),
    ("int_least64_t", 8, true),
    ("uint_least64_t", 8, false),
    ("int_fast8_t", 1, true),
    ("uint_fast8_t", 1, false),
    ("int_fast64_t", 8, true),
    ("uint_fast64_t", 8, false),
  ] {
    m.insert(name, CType::int(size, signed, name));
  }

  // The fast 16- and 32-bit types are as wide as a register on glibc
  // and as wide as their names say everywhere else.
  let fast = if cfg!(all(target_os = "linux", target_env = "gnu")) {
    8
  } else {
    4
  };
  m.insert(
    "int_fast16_t",
    CType::int(fast.max(2), true, "int_fast16_t"),
  );
  m.insert(
    "uint_fast16_t",
    CType::int(fast.max(2), false, "uint_fast16_t"),
  );
  m.insert("int_fast32_t", CType::int(fast, true, "int_fast32_t"));
  m.insert("uint_fast32_t", CType::int(fast, false, "uint_fast32_t"));

  m.insert("wchar_t", CType::character(WCHAR.0, WCHAR.1, "wchar_t"));
  m.insert("char16_t", CType::character(2, false, "char16_t"));
  m.insert("char32_t", CType::character(4, false, "char32_t"));

  m.insert("float", CType::new(Kind::Float, "float"));
  m.insert("double", CType::new(Kind::Double, "double"));
  m.insert("long double", CType::new(Kind::LongDouble, "long double"));
  m.insert(
    "float _Complex",
    CType::new(Kind::ComplexFloat, "float _Complex"),
  );
  m.insert(
    "double _Complex",
    CType::new(Kind::ComplexDouble, "double _Complex"),
  );

  m.insert("i8", CType::int(1, true, "i8"));
  m.insert("u8", CType::int(1, false, "u8"));
  m.insert("i16", CType::int(2, true, "i16"));
  m.insert("u16", CType::int(2, false, "u16"));
  m.insert("i32", CType::int(4, true, "i32"));
  m.insert("u32", CType::int(4, false, "u32"));
  m.insert("i64", CType::int(8, true, "i64"));
  m.insert("u64", CType::int(8, false, "u64"));
  m.insert("i128", CType::int(16, true, "i128"));
  m.insert("u128", CType::int(16, false, "u128"));
  m.insert("isize", CType::int(8, true, "isize"));
  m.insert("usize", CType::int(8, false, "usize"));
  m.insert("f32", CType::new(Kind::Float, "f32"));
  m.insert("f64", CType::new(Kind::Double, "f64"));
  m.insert("rust char", CType::new(Kind::RustChar, "char"));

  let void_ptr = CType::pointer_to(&void);
  m.insert("void *", void_ptr);

  let plain_char = m.get("char").unwrap().clone();
  let const_char = CType::constant(&plain_char);
  m.insert(
    "string",
    CType::pointer_with(&const_char, Some(Encoding::Utf8), false, "string".into()),
  );
  let wchar = m.get("wchar_t").unwrap().clone();
  let const_wchar = CType::constant(&wchar);
  m.insert(
    "wstring",
    CType::pointer_with(&const_wchar, Some(Encoding::Wide), false, "wstring".into()),
  );

  m
});

pub fn builtin(name: &str) -> Option<TypeRef> {
  BUILTINS.get(name).cloned()
}

/// A `(ptr, len)` pair as a Rust slice is usually passed over the C ABI.
pub fn slice_of(element: &TypeRef, mutable: bool) -> TypeRef {
  let target = if mutable {
    element.clone()
  } else {
    CType::constant(element)
  };
  let ptr_type = CType::pointer_to(&target);
  let name = format!("slice<{}>", element.name);
  let record = Record::new(Some(name.clone()), false);
  let usize = builtin("usize").unwrap();

  record
    .add_field(FieldSpec {
      name: "ptr".into(),
      ty: ptr_type,
      bits: None,
      align: None,
    })
    .expect("a fresh record accepts its first field");
  record
    .add_field(FieldSpec {
      name: "len".into(),
      ty: usize,
      bits: None,
      align: None,
    })
    .expect("a fresh record accepts its second field");

  CType::new(Kind::Record(record), name)
}
