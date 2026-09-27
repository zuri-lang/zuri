//! Calling a C function through a signature known only at run time.
//!
//! Each signature is turned into a libffi call interface once, and the
//! interface is reused for every call through it. A variadic function
//! gets one interface per distinct list of argument types after the
//! fixed ones, cached the same way.
//!
//! A call publishes the VM for the duration so a callback can reach it,
//! clears `errno` (and `GetLastError()` on Windows) immediately before
//! the call and records both immediately after, which is the only
//! moment either still says something about the function that ran.

use std::os::raw::c_void;
use std::sync::{Arc, Mutex, OnceLock};

use libffi_sys as raw;
use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;

use super::callback;
use super::convert::{self, Dest, Scratch};
use super::library::Library;
use super::types::{
  Abi, CType, Kind, Signature, TypeRef, builtin, general_registers, wants_even_pair,
};
#[cfg(target_arch = "x86_64")]
use super::wide_return;
use super::{FUNCTION, Fail, state};
use crate::vm::object::ZuriContext;
use crate::vm::value::Value;
use crate::vm::vm::VM;

/// A prepared libffi call interface and the argument types it owns.
pub struct Cif {
  cif: Box<raw::ffi_cif>,
  _arg_types: Vec<*mut raw::ffi_type>,
  /// For each argument libffi is given, the index of the argument it
  /// carries, or `None` for padding that only moves later arguments onto
  /// the registers the platform expects.
  pub slots: Vec<Option<usize>>,
  /// Bytes libffi writes the return value into. Never less than a
  /// register, which is what libffi writes for any small integer.
  pub return_size: usize,
  /// The result is a 128-bit integer returned in `xmm0`, so the call
  /// goes through `wide_return`, and the interface libffi was given
  /// takes the thunk's pointer first and returns nothing.
  pub wide_return: bool,
}

// SAFETY: a prepared cif is read-only from here on, and the types it
// points at are either libffi's statics or owned by the `CType`s the
// signature holds for as long as the cif lives.
unsafe impl Send for Cif {}
unsafe impl Sync for Cif {}

impl Cif {
  /// Prepares an interface for `sig`, with `extra` as the types of the
  /// arguments passed after the fixed ones of a variadic function.
  pub fn new(sig: &Signature, extra: &[TypeRef]) -> Result<Cif, String> {
    let mut arg_types = Vec::new();
    let mut slots = Vec::new();
    let mut fixed = 0usize;
    let mut next_general = 0usize;

    let all: Vec<&TypeRef> = sig.params.iter().chain(extra.iter()).collect();

    let wide_return = returns_in_xmm0(sig);
    if wide_return {
      arg_types.push(&raw mut raw::ffi_type_pointer as *mut raw::ffi_type);
      fixed += 1;
    }

    for (index, ty) in all.iter().enumerate() {
      if ty.is_void() {
        return Err("'void' cannot be the type of an argument".into());
      }

      let ffi_type = ty.ffi_type(sig.abi)?;

      // AArch64 Linux starts a 128-bit integer, or an aggregate aligned
      // to sixteen bytes, on an even-numbered register. libffi does not
      // for aggregates, so an unused register is spent first.
      if let Some(count) = general_registers(ty) {
        if wants_even_pair(ty) && next_general % 2 == 1 && next_general < 8 {
          arg_types.push(&raw mut raw::ffi_type_uint64 as *mut raw::ffi_type);
          slots.push(None);
          next_general += 1;
          if index < sig.params.len() {
            fixed += 1;
          }
        }
        next_general = if next_general + count <= 8 {
          next_general + count
        } else {
          8
        };
      }

      arg_types.push(ffi_type);
      slots.push(Some(index));
      if index < sig.params.len() {
        fixed += 1;
      }
    }

    let return_type = if sig.returns.is_void() || wide_return {
      &raw mut raw::ffi_type_void as *mut raw::ffi_type
    } else {
      sig.returns.ffi_type(sig.abi)?
    };

    let return_size = sig.returns.size().unwrap_or(0).max(size_of::<u64>());

    let mut cif: Box<raw::ffi_cif> = Box::new(unsafe { std::mem::zeroed() });

    let status = unsafe {
      if sig.variadic {
        raw::ffi_prep_cif_var(
          &mut *cif,
          sig.abi.raw(),
          fixed as u32,
          arg_types.len() as u32,
          return_type,
          arg_types.as_mut_ptr(),
        )
      } else {
        raw::ffi_prep_cif(
          &mut *cif,
          sig.abi.raw(),
          arg_types.len() as u32,
          return_type,
          arg_types.as_mut_ptr(),
        )
      }
    };

    if status != raw::ffi_status_FFI_OK {
      return Err(match status {
        raw::ffi_status_FFI_BAD_TYPEDEF => {
          format!("libffi rejected a type in '{}'", sig.describe())
        },
        raw::ffi_status_FFI_BAD_ABI => format!(
          "the '{}' calling convention is not available here",
          sig.abi.name()
        ),
        _ => format!("libffi could not prepare a call to '{}'", sig.describe()),
      });
    }

    Ok(Cif {
      cif,
      _arg_types: arg_types,
      slots,
      return_size,
      wide_return,
    })
  }

  pub fn as_ptr(&self) -> *mut raw::ffi_cif {
    &*self.cif as *const raw::ffi_cif as *mut raw::ffi_cif
  }
}

/// Whether `sig` returns a 128-bit integer under Microsoft x64, which
/// puts all of it in `xmm0`.
fn returns_in_xmm0(sig: &Signature) -> bool {
  if cfg!(not(target_arch = "x86_64")) || !sig.abi.is_win64() {
    return false;
  }

  let mut ty = &sig.returns;
  while let Kind::Enum(e) = &ty.kind {
    ty = &e.underlying;
  }

  matches!(ty.kind, Kind::Int { size: 16, .. })
}

/// A C function bound to a signature.
pub struct ForeignFunction {
  pub name: String,
  pub address: usize,
  pub sig: Arc<Signature>,
  /// Keeps the library loaded while the function can still be called.
  pub library: Option<Arc<Library>>,
  /// Run on a helper thread while the calling isolate keeps answering
  /// callbacks made from other threads.
  pub threaded: bool,
  cif: OnceLock<Result<Arc<Cif>, String>>,
  variadic_cifs: Mutex<FxHashMap<Vec<usize>, Arc<Cif>>>,
}

impl ForeignFunction {
  pub fn new(
    name: String,
    address: usize,
    sig: Arc<Signature>,
    library: Option<Arc<Library>>,
  ) -> ForeignFunction {
    ForeignFunction {
      name,
      address,
      sig,
      library,
      threaded: false,
      cif: OnceLock::new(),
      variadic_cifs: Mutex::new(FxHashMap::default()),
    }
  }

  pub fn from_address(
    address: usize,
    sig: Arc<Signature>,
    library: Option<Arc<Library>>,
    name: String,
  ) -> ForeignFunction {
    ForeignFunction::new(name, address, sig, library)
  }

  /// The same function, run on a helper thread.
  pub fn threaded(&self) -> ForeignFunction {
    let mut f = ForeignFunction::new(
      self.name.clone(),
      self.address,
      self.sig.clone(),
      self.library.clone(),
    );
    f.threaded = true;
    f
  }

  /// Prepares the call interface now, so a signature libffi cannot
  /// express is reported where the function is bound rather than where
  /// it is first called.
  pub fn prepare(&self) -> Result<(), Fail> {
    if self.sig.variadic {
      return Cif::new(&self.sig, &[])
        .map(|_| ())
        .map_err(Fail::from_type);
    }
    self.fixed_cif().map(|_| ())
  }

  fn fixed_cif(&self) -> Result<Arc<Cif>, Fail> {
    self
      .cif
      .get_or_init(|| Cif::new(&self.sig, &[]).map(Arc::new))
      .clone()
      .map_err(Fail::from_type)
  }

  fn variadic_cif(&self, extra: &[TypeRef]) -> Result<Arc<Cif>, Fail> {
    let key: Vec<usize> = extra.iter().map(|t| Arc::as_ptr(t) as usize).collect();

    if let Some(found) = self.variadic_cifs.lock().unwrap().get(&key) {
      return Ok(found.clone());
    }

    let cif = Arc::new(Cif::new(&self.sig, extra).map_err(Fail::from_type)?);
    let mut cache = self.variadic_cifs.lock().unwrap();

    // A program that formats with a different set of types on every call
    // should not grow this without bound.
    if cache.len() > 256 {
      cache.clear();
    }

    cache.insert(key, cif.clone());
    Ok(cif)
  }

  /// Runs a destructor attached with `own()`: a function of one pointer.
  pub fn call_destructor(&self, address: usize) -> Result<(), Fail> {
    let cif = self.fixed_cif()?;
    let mut argument = address;
    let mut values: [*mut c_void; 1] = [&mut argument as *mut usize as *mut c_void];
    let mut ret = [0u64; 4];

    // SAFETY: the cif is this function's own, taking one pointer, and
    // `ret` has room for any result.
    unsafe { raw_call(self.address, &cif, ret.as_mut_ptr() as *mut u8, &mut values) };

    Ok(())
  }
}

/// A callable Zuri value for `function`: a bound method whose receiver
/// holds the function and whose method makes the call.
pub fn function_value(vm: &mut VM, function: Arc<ForeignFunction>) -> Value {
  let name = intern(&function.name);

  // The receiver counts as an argument to the native, so the arity it
  // checks is one more than the C function's. A signature too long for
  // that count is checked by `invoke` instead.
  let (min_arity, variadic) = match u8::try_from(function.sig.params.len() + 1) {
    Ok(n) => (n, function.sig.variadic),
    Err(_) => (1, true),
  };

  let receiver = vm.heap_mut().alloc_ptr(FUNCTION, function);
  let pin = vm.pin_values([receiver]);
  let method = vm
    .heap_mut()
    .alloc_native(crate::vm::object::NativeFunction {
      is_method: true,
      name,
      min_arity,
      variadic,
      func: call_native,
    });
  let receiver = vm.pinned(pin);
  vm.heap_mut().alloc_bound_method(receiver, method)
}

/// A native function's name has to live forever. Symbol names repeat,
/// so each distinct one is kept once.
fn intern(name: &str) -> &'static str {
  static NAMES: OnceLock<Mutex<FxHashSet<&'static str>>> = OnceLock::new();
  let mut names = NAMES
    .get_or_init(|| Mutex::new(FxHashSet::default()))
    .lock()
    .unwrap();

  if let Some(found) = names.get(name) {
    return found;
  }

  let leaked: &'static str = Box::leak(name.to_string().into_boxed_str());
  names.insert(leaked);
  leaked
}

/// The foreign function behind a callable made by `function_value`.
pub fn function_of(value: Value) -> Option<Arc<ForeignFunction>> {
  let receiver = if value.is_bound_method() {
    value.as_bound_method().receiver
  } else {
    value
  };

  if !receiver.is_ptr_type(FUNCTION) {
    return None;
  }

  let cell = receiver.as_ptr_cell().borrow();
  cell.downcast_ref::<Arc<ForeignFunction>>().cloned()
}

fn call_native(ctx: &mut ZuriContext) -> Result<Value, String> {
  let function = {
    let cell = ctx.args[0].as_ptr_cell().borrow();
    cell.downcast_ref::<Arc<ForeignFunction>>().unwrap().clone()
  };

  let args: SmallVec<[Value; 8]> = ctx.args[1..].iter().copied().collect();

  invoke(ctx.vm, &function, &args).map_err(|fail| super::raise(ctx.vm, fail))
}

/// Calls `function` with `args`, converting each to its parameter's
/// type and the result back.
pub fn invoke(vm: &mut VM, function: &ForeignFunction, args: &[Value]) -> Result<Value, Fail> {
  if let Some(lib) = &function.library
    && lib.is_closed()
  {
    return Err(Fail::load(format!(
      "cannot call '{}': its library '{}' has been closed",
      function.name, lib.path
    )));
  }

  let sig = function.sig.clone();
  let fixed = sig.params.len();

  if (!sig.variadic && args.len() != fixed) || (sig.variadic && args.len() < fixed) {
    return Err(Fail::argument(format!(
      "{}() expects {}{} argument{}, got {}",
      function.name,
      if sig.variadic { "at least " } else { "" },
      fixed,
      if fixed == 1 { "" } else { "s" },
      args.len()
    )));
  }

  // Everything below can run a collection, which moves objects, and C
  // reads a bytes argument's own storage for as long as the call lasts.
  // The arguments stay pinned until it returns, and are read back
  // through their pins rather than from `args`.
  let mark = vm.pin_values(args.iter().copied());

  // Calls waiting from other threads are answered first, so a program
  // that makes foreign calls in a loop never starves them.
  let outcome = match callback::service_pending(vm) {
    Ok(_) => {
      let mut scratch = Scratch::default();
      let outcome = invoke_with(vm, function, &sig, mark, args.len(), &mut scratch);
      scratch_release(vm, &mut scratch);
      outcome
    },
    Err(fail) => Err(fail),
  };

  vm.unpin(mark);
  outcome
}

fn scratch_release(vm: &mut VM, scratch: &mut Scratch) {
  for core in scratch.callbacks.drain(..) {
    core.release_on(vm);
  }
}

fn invoke_with(
  vm: &mut VM,
  function: &ForeignFunction,
  sig: &Arc<Signature>,
  mark: usize,
  count: usize,
  scratch: &mut Scratch,
) -> Result<Value, Fail> {
  let fixed = sig.params.len();

  // The types of the arguments past the fixed ones, after C's default
  // promotions, and where the values that go with them are pinned.
  let mut types: SmallVec<[TypeRef; 8]> = sig.params.iter().cloned().collect();
  let mut values: SmallVec<[usize; 8]> = (mark..mark + fixed).collect();

  for pin in mark + fixed..mark + count {
    let arg = vm.pinned(pin);
    let (ty, value) = match convert::typed_argument(vm, arg) {
      Some((declared, inner)) => {
        let promoted = promote(&declared);
        if CType::same(&promoted, &declared) {
          (declared, inner)
        } else {
          // Converted as the declared type first, so its own rules
          // apply (a character, a constant's name, a float's rounding),
          // and then promoted, as C promotes it.
          let staged = scratch.buffer(16);
          convert::store(vm, &declared, inner, staged, &mut Dest::Call(scratch))?;
          let value = convert::promote_bool(convert::load(vm, &declared, staged)?);
          (promoted, value)
        }
      },
      None => (convert::promoted_type(vm, arg)?, convert::promote_bool(arg)),
    };
    types.push(ty);
    values.push(vm.pin_values([value]));
  }

  let cif = if sig.variadic {
    function.variadic_cif(&types[fixed..])?
  } else {
    function.fixed_cif()?
  };

  // One buffer per argument, each at least a register wide.
  let mut storage: SmallVec<[*mut u8; 8]> = SmallVec::new();
  for (i, ty) in types.iter().enumerate() {
    let size = ty.require_size().map_err(Fail::from_type)?.max(16);
    let at = scratch.buffer(size);
    let mut dest = Dest::Call(scratch);
    let value = vm.pinned(values[i]);
    convert::store(vm, ty, value, at, &mut dest).map_err(|fail| annotate(fail, function, i))?;
    storage.push(at);
  }

  let padding = scratch.buffer(16);
  let mut avalues: SmallVec<[*mut c_void; 8]> = cif
    .slots
    .iter()
    .map(|slot| match slot {
      Some(i) => storage[*i] as *mut c_void,
      None => padding as *mut c_void,
    })
    .collect();

  let ret = scratch.buffer(cif.return_size.max(16));

  let (errno, last_error) = if function.threaded {
    threaded_call(vm, function.address, &cif, ret, &mut avalues)?
  } else {
    let _guard = callback::enter(vm);
    unsafe { raw_call(function.address, &cif, ret, &mut avalues) }
  };

  {
    let st = state(vm);
    st.errno = errno;
    st.last_error = last_error;
  }

  if let Some(error) = callback::take_trapped(vm) {
    return Err(Fail::Raised(error));
  }

  convert::copy_back(vm, scratch)?;

  if sig.returns.is_void() {
    return Ok(Value::nil());
  }

  convert::load(vm, &sig.returns, ret)
}

/// Makes the call and returns `errno` and `GetLastError()` as it left
/// them.
///
/// # Safety
///
/// `avalues` must hold one valid pointer per argument of `cif`, and
/// `ret` room for its return value.
unsafe fn raw_call(
  address: usize,
  cif: &Cif,
  ret: *mut u8,
  avalues: &mut [*mut c_void],
) -> (i32, u32) {
  #[cfg(target_arch = "x86_64")]
  if cif.wide_return {
    return unsafe { wide_call(address, cif, ret, avalues) };
  }

  errno::clear();

  unsafe {
    raw::ffi_call(
      cif.as_ptr(),
      Some(std::mem::transmute::<usize, unsafe extern "C" fn()>(
        address,
      )),
      ret as *mut c_void,
      avalues.as_mut_ptr(),
    );
  }

  errno::read()
}

/// `raw_call` for a function returning a 128-bit integer in `xmm0`,
/// made through the thunk that stores it at `ret`.
///
/// # Safety
///
/// As for `raw_call`, with `ret` holding sixteen bytes.
#[cfg(target_arch = "x86_64")]
unsafe fn wide_call(
  address: usize,
  cif: &Cif,
  ret: *mut u8,
  avalues: &mut [*mut c_void],
) -> (i32, u32) {
  let mut call = wide_return::Call {
    target: address,
    dest: ret,
    stack_slots: wide_return::stack_slots(avalues.len()),
  };
  let mut first = &mut call as *mut wide_return::Call;

  let mut values: SmallVec<[*mut c_void; 9]> = SmallVec::with_capacity(avalues.len() + 1);
  values.push(&mut first as *mut *mut wide_return::Call as *mut c_void);
  values.extend_from_slice(avalues);

  let mut unused = 0u64;

  errno::clear();

  unsafe {
    raw::ffi_call(
      cif.as_ptr(),
      Some(std::mem::transmute::<usize, unsafe extern "C" fn()>(
        wide_return::call_thunk(),
      )),
      &mut unused as *mut u64 as *mut c_void,
      values.as_mut_ptr(),
    );
  }

  errno::read()
}

struct SendCall {
  address: usize,
  cif: *const Cif,
  ret: *mut u8,
  avalues: *mut *mut c_void,
  count: usize,
}

// SAFETY: every pointer here addresses memory owned by the calling
// thread's stack frame, which waits for the helper to finish before any
// of it is released.
unsafe impl Send for SendCall {}

/// Runs the call on a helper thread while this one answers callbacks
/// posted from other threads, which is what the call itself may be
/// waiting on.
fn threaded_call(
  vm: &mut VM,
  address: usize,
  cif: &Arc<Cif>,
  ret: *mut u8,
  avalues: &mut [*mut c_void],
) -> Result<(i32, u32), Fail> {
  let inbox = callback::inbox(vm);
  let job = SendCall {
    address,
    cif: Arc::as_ptr(cif),
    ret,
    avalues: avalues.as_mut_ptr(),
    count: avalues.len(),
  };

  std::thread::scope(|scope| {
    let wake = inbox.clone();
    let worker = scope.spawn(move || {
      let job = job;
      let values = unsafe { std::slice::from_raw_parts_mut(job.avalues, job.count) };
      let result = unsafe { raw_call(job.address, &*job.cif, job.ret, values) };
      wake.notify();
      result
    });

    loop {
      callback::service(vm, &inbox)?;

      if worker.is_finished() {
        break;
      }

      inbox.wait(std::time::Duration::from_millis(50));
    }

    // Anything posted between the last service and the worker finishing
    // is answered before the result is used.
    callback::service(vm, &inbox)?;

    worker
      .join()
      .map_err(|_| Fail::ffi("the helper thread running a threaded call panicked"))
  })
}

/// C's default argument promotions, applied to an explicitly typed
/// variadic argument.
fn promote(ty: &TypeRef) -> TypeRef {
  match &ty.kind {
    Kind::Float => builtin("double").unwrap(),
    Kind::Bool => builtin("int").unwrap(),
    Kind::Int { size, signed, .. } if *size < 4 => {
      if *signed {
        builtin("int").unwrap()
      } else {
        builtin("unsigned int").unwrap()
      }
    },
    Kind::Enum(e) => promote(&e.underlying),
    _ => ty.clone(),
  }
}

/// Prefixes a conversion error with the argument it came from.
fn annotate(fail: Fail, function: &ForeignFunction, index: usize) -> Fail {
  let prefix = |message: String| {
    let name = function
      .sig
      .param_names
      .get(index)
      .and_then(|n| n.as_ref())
      .map(|n| format!(" '{n}'"))
      .unwrap_or_default();
    format!(
      "{}() argument {}{name}: {message}",
      function.name,
      index + 1
    )
  };

  match fail {
    Fail::Builtin(class, message) => Fail::Builtin(class, prefix(message)),
    Fail::Ffi(class, message) => Fail::Ffi(class, prefix(message)),
    other => other,
  }
}

/// A signature from a return type and parameter types.
pub fn signature(
  returns: TypeRef,
  params: Vec<TypeRef>,
  variadic: bool,
  abi: Abi,
) -> Result<Signature, Fail> {
  for p in &params {
    if p.is_void() {
      return Err(Fail::type_error("'void' cannot be the type of a parameter"));
    }
    if matches!(p.kind, Kind::Function(_)) {
      return Err(Fail::type_error(format!(
        "a parameter cannot be a function; use a pointer to '{}'",
        p.name
      )));
    }
  }

  if matches!(returns.kind, Kind::Array { .. } | Kind::Function(_)) {
    return Err(Fail::type_error(format!(
      "a function cannot return '{}'",
      returns.name
    )));
  }

  let count = params.len();
  Ok(Signature {
    returns,
    params,
    param_names: vec![None; count],
    variadic,
    abi,
  })
}

/// The platform's `errno` and, on Windows, the thread's last error.
pub mod errno {
  #[cfg(any(target_os = "linux", target_os = "android"))]
  fn location() -> *mut libc::c_int {
    unsafe { libc::__errno_location() }
  }

  #[cfg(target_vendor = "apple")]
  fn location() -> *mut libc::c_int {
    unsafe { libc::__error() }
  }

  #[cfg(windows)]
  fn location() -> *mut libc::c_int {
    unsafe extern "C" {
      fn _errno() -> *mut libc::c_int;
    }
    unsafe { _errno() }
  }

  pub fn clear() {
    unsafe { *location() = 0 };

    #[cfg(windows)]
    unsafe {
      windows_sys::Win32::Foundation::SetLastError(0)
    };
  }

  pub fn read() -> (i32, u32) {
    let errno = unsafe { *location() };

    #[cfg(windows)]
    let last = unsafe { windows_sys::Win32::Foundation::GetLastError() };
    #[cfg(not(windows))]
    let last = 0u32;

    (errno, last)
  }

  pub fn set(value: i32) {
    unsafe { *location() = value };
  }
}
