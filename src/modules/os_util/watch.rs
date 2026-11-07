//! The native half of `os.Watcher`.
//!
//! Filesystem events come from `notify`'s recommended watcher, which is
//! the platform's own mechanism: inotify on Linux, FSEvents on macOS,
//! ReadDirectoryChangesW on Windows and kqueue on the BSDs. They arrive
//! on its thread. The exit of a process the watcher follows arrives on a
//! thread waiting for that process, and a trapped signal sends a wake.
//! All three meet in one channel, and `next()` is the only thing that
//! ever waits on it.
//!
//! Only directories are handed to the OS. A file is watched through the
//! directory that holds it, because an editor that saves by renaming a
//! new copy over the original replaces the very inode a direct watch
//! would have been attached to, and the watch would die with it.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use notify::event::{EventKind, MetadataKind, ModifyKind, RenameMode};
use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};

use super::signal;
use crate::modules::isolate_util::pool;

enum Message {
  Fs(notify::Result<notify::Event>),
  Exit(u64),
  Wake,
}

/// How much of a registered path a watcher reports.
#[derive(Clone, Copy, PartialEq)]
enum Scope {
  /// One file, watched through its directory.
  File,
  /// A directory and the entries directly inside it.
  Directory,
  /// A directory and everything under it, however deep.
  Tree,
}

/// One path a caller asked to watch.
struct Registration {
  /// The canonical path, as `paths()` reports it.
  path: PathBuf,
  /// The same path folded for comparison, see `fold`.
  key: PathBuf,
  scope: Scope,
}

impl Registration {
  /// The directory this registration needs watched, and how.
  fn directory(&self) -> (PathBuf, RecursiveMode) {
    match self.scope {
      Scope::File => {
        let parent = self.path.parent().unwrap_or(&self.path).to_path_buf();
        (parent, RecursiveMode::NonRecursive)
      },
      Scope::Directory => (self.path.clone(), RecursiveMode::NonRecursive),
      Scope::Tree => (self.path.clone(), RecursiveMode::Recursive),
    }
  }

  /// Whether an event at `key`, already folded, belongs to this
  /// registration.
  fn covers(&self, key: &Path) -> bool {
    match self.scope {
      Scope::File => key == self.key,
      Scope::Directory => key == self.key || key.parent() == Some(self.key.as_path()),
      Scope::Tree => key.starts_with(&self.key),
    }
  }
}

/// What one OS event says happened to one path, before that is checked
/// against the filesystem.
#[derive(Clone, Copy, PartialEq)]
enum Raw {
  Create,
  Modify,
  Remove,
  /// A rename that did not say which side of it this path was on.
  Moved,
  Rescan,
}

#[derive(PartialEq)]
enum Pending {
  Change(Raw, Option<PathBuf>),
  Exit(u64),
}

/// What `next()` hands back.
pub enum Delivered {
  Change {
    kind: &'static str,
    path: Option<String>,
  },
  Exit(u64),
}

pub enum Outcome {
  Event(Delivered),
  TimedOut,
  /// A trapped signal arrived, and its callback is waiting to run.
  Interrupted,
}

/// The process a watcher follows, in whatever form waiting for it
/// without reaping it takes on this platform.
pub enum FollowTarget {
  /// The process was already reaped, so its exit is already known.
  Exited,
  #[cfg(unix)]
  Pid(u32),
  #[cfg(windows)]
  Handle(usize),
}

pub struct Watcher {
  backend: Option<RecommendedWatcher>,
  sender: Sender<Message>,
  receiver: Receiver<Message>,
  registrations: Vec<Registration>,
  /// The directories the backend is watching right now.
  watched: Vec<(PathBuf, RecursiveMode)>,
  queue: VecDeque<Pending>,
  waker: u64,
  next_follow: u64,
}

impl Watcher {
  pub fn new() -> Result<Watcher, String> {
    #[cfg(target_os = "macos")]
    raise_file_limit();

    let (sender, receiver) = mpsc::channel();

    let events = sender.clone();
    let backend = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
      // Dropped here, on the backend's thread, rather than queued: every
      // open and read in a watched directory produces one, and a watcher
      // nobody is reading would otherwise hold them all.
      if let Ok(event) = &result {
        if ignored(&event.kind) {
          return;
        }
      }

      let _ = events.send(Message::Fs(result));
    })
    .map_err(|e| describe(&e, None))?;

    let wakes = sender.clone();
    let waker = signal::add_waker(move || {
      let _ = wakes.send(Message::Wake);
    });

    Ok(Watcher {
      backend: Some(backend),
      sender,
      receiver,
      registrations: Vec::new(),
      watched: Vec::new(),
      queue: VecDeque::new(),
      waker,
      next_follow: 1,
    })
  }

  fn backend(&mut self) -> Result<&mut RecommendedWatcher, String> {
    self
      .backend
      .as_mut()
      .ok_or_else(|| "this watcher is closed".to_string())
  }

  /// Starts watching `path`. A directory is watched recursively unless
  /// `recursive` is false; a file is watched on its own.
  pub fn add(&mut self, path: &str, recursive: bool) -> Result<(), String> {
    self.backend()?;

    let canonical = canonical(path).map_err(|e| format!("could not watch '{path}': {e}"))?;

    let scope = match (canonical.is_dir(), recursive) {
      (false, _) => Scope::File,
      (true, true) => Scope::Tree,
      (true, false) => Scope::Directory,
    };

    // Whatever has already arrived is taken in first, so a directory
    // the OS has stopped watching since is known to need watching again.
    // A watch that is still alive is left exactly as it is.
    self.drain();

    let key = fold(&canonical);
    let previous = self.registrations.iter().position(|r| r.key == key);

    let registration = Registration {
      path: canonical,
      key,
      scope,
    };

    let replaced = match previous {
      Some(at) => Some(std::mem::replace(&mut self.registrations[at], registration)),
      None => {
        self.registrations.push(registration);
        None
      },
    };

    if let Err(e) = self.reconcile() {
      match replaced {
        Some(old) => {
          let at = previous.unwrap_or(0);
          self.registrations[at] = old;
        },
        None => {
          self.registrations.pop();
        },
      }

      let _ = self.reconcile();

      return Err(describe(&e, Some(path)));
    }

    Ok(())
  }

  /// Stops watching `path`. Returns whether it was being watched.
  pub fn remove(&mut self, path: &str) -> Result<bool, String> {
    self.backend()?;

    // The path may be gone by now, which is often exactly why it is
    // being removed, so its absolute spelling stands in for the
    // canonical one.
    let resolved = canonical(path).unwrap_or_else(|_| absolute(path));
    let key = fold(&resolved);
    let before = self.registrations.len();

    self.registrations.retain(|r| r.key != key);

    if self.registrations.len() == before {
      return Ok(false);
    }

    let _ = self.reconcile();

    Ok(true)
  }

  /// Every path being watched, as it was resolved when it was added.
  pub fn paths(&self) -> Vec<String> {
    self
      .registrations
      .iter()
      .map(|r| r.path.display().to_string())
      .collect()
  }

  /// Starts following a child process. Its exit is reported as an exit
  /// event carrying the returned id.
  pub fn follow(&mut self, target: FollowTarget) -> Result<u64, String> {
    self.backend()?;

    let id = self.next_follow;
    self.next_follow += 1;

    let sender = self.sender.clone();

    match target {
      FollowTarget::Exited => {
        self.queue.push_back(Pending::Exit(id));
      },
      #[cfg(unix)]
      FollowTarget::Pid(pid) => {
        spawn_follower(move || {
          wait_without_reaping(pid);
          let _ = sender.send(Message::Exit(id));
        })?;
      },
      #[cfg(windows)]
      FollowTarget::Handle(handle) => {
        spawn_follower(move || {
          wait_for_handle(handle);
          let _ = sender.send(Message::Exit(id));
        })?;
      },
    }

    Ok(id)
  }

  /// Waits for the next event, for at most `timeout` when one is given.
  ///
  /// On the main thread the wait is cut short by a trapped signal,
  /// since that is the thread its callback runs on. Inside an isolate it
  /// is cut short instead by the isolate being cancelled, which it looks
  /// for at the pool's cancel interval, the way every other wait inside
  /// an isolate does.
  pub fn next(&mut self, timeout: Option<Duration>) -> Result<Outcome, String> {
    self.backend()?;

    let deadline = timeout.and_then(|t| Instant::now().checked_add(t));
    let isolate = pool::in_isolate_context();

    loop {
      self.drain();

      if let Some(event) = self.deliver() {
        return Ok(Outcome::Event(event));
      }

      let interrupted = match isolate {
        true => pool::is_current_cancelled(),
        false => signal::any_pending(),
      };

      if interrupted {
        return Ok(Outcome::Interrupted);
      }

      let left = deadline.map(|d| d.saturating_duration_since(Instant::now()));
      let slice = match isolate {
        true => Some(left.map_or(pool::CANCEL_POLL_INTERVAL, |l| {
          l.min(pool::CANCEL_POLL_INTERVAL)
        })),
        false => left,
      };

      let message = match slice {
        None => match self.receiver.recv() {
          Ok(message) => message,
          // The watcher holds a sender of its own, so the channel
          // cannot close while it is waiting on it.
          Err(_) => return Ok(Outcome::TimedOut),
        },
        Some(slice) => match self.receiver.recv_timeout(slice) {
          Ok(message) => message,
          Err(RecvTimeoutError::Timeout) if left.is_some_and(|l| l <= slice) => {
            return Ok(Outcome::TimedOut);
          },
          // Only the cancel interval passed, so the wait goes on.
          Err(RecvTimeoutError::Timeout) => continue,
          Err(RecvTimeoutError::Disconnected) => return Ok(Outcome::TimedOut),
        },
      };

      self.absorb(message);
    }
  }

  /// Stops every watch. Later calls raise, apart from `close()` itself.
  pub fn close(&mut self) {
    if self.backend.take().is_some() {
      signal::remove_waker(self.waker);
    }

    self.registrations.clear();
    self.watched.clear();
    self.queue.clear();
  }

  /// Takes in everything that has arrived without waiting for more, so
  /// a burst is merged before any of it is handed over.
  fn drain(&mut self) {
    while let Ok(message) = self.receiver.try_recv() {
      self.absorb(message);
    }
  }

  fn absorb(&mut self, message: Message) {
    match message {
      Message::Fs(Ok(event)) => {
        for (raw, path) in translate(event) {
          self.enqueue(raw, path);
        }
      },
      // An error out of the event stream means events were lost, which
      // is what a rescan says.
      Message::Fs(Err(error)) => {
        if error.paths.is_empty() {
          self.enqueue(Raw::Rescan, None);
        }

        for path in error.paths {
          self.enqueue(Raw::Rescan, Some(path));
        }
      },
      Message::Exit(id) => self.queue.push_back(Pending::Exit(id)),
      // Nothing to keep: `next()` looks at the pending signals itself.
      Message::Wake => {},
    }
  }

  fn enqueue(&mut self, raw: Raw, path: Option<PathBuf>) {
    // A directory that went away took its OS watch with it, whether or
    // not anyone asked to hear about the directory itself, and the
    // backend has to be asked again once it is back.
    if raw == Raw::Remove || raw == Raw::Moved {
      if let Some(path) = &path {
        self.forget_directory(path);
      }
    }

    if !self.relevant(raw, path.as_deref()) {
      return;
    }

    let pending = Pending::Change(raw, path);

    // Changes nobody has been handed yet are merged, so a file written
    // in several steps while nobody was reading is reported once.
    if !self.queue.contains(&pending) {
      self.queue.push_back(pending);
    }
  }

  fn relevant(&self, raw: Raw, path: Option<&Path>) -> bool {
    let Some(path) = path else {
      return true;
    };

    let key = fold(path);

    if self.registrations.iter().any(|r| r.covers(&key)) {
      return true;
    }

    // A rescan of a directory above a registration covers it too.
    raw == Raw::Rescan && self.registrations.iter().any(|r| r.key.starts_with(&key))
  }

  /// The next event worth handing over, checked against what the
  /// filesystem holds now.
  fn deliver(&mut self) -> Option<Delivered> {
    while let Some(pending) = self.queue.pop_front() {
      let (raw, path) = match pending {
        Pending::Exit(id) => return Some(Delivered::Exit(id)),
        Pending::Change(raw, path) => (raw, path),
      };

      let exists = path
        .as_deref()
        .is_some_and(|p| p.symlink_metadata().is_ok());

      let kind = match raw {
        Raw::Create if exists => "create",
        Raw::Modify if exists => "modify",
        Raw::Remove if !exists => "remove",
        Raw::Moved if exists => "create",
        Raw::Moved => "remove",
        Raw::Rescan => "rescan",
        _ => continue,
      };

      return Some(Delivered::Change {
        kind,
        path: path.map(|p| p.display().to_string()),
      });
    }

    None
  }

  /// Drops `directory` from what the backend watches, so the next
  /// `add()` or `reconcile()` asks for it again.
  fn forget_directory(&mut self, directory: &Path) {
    let key = fold(directory);
    let mut dropped = Vec::new();

    self.watched.retain(|(dir, _)| {
      let matches = fold(dir) == key;

      if matches {
        dropped.push(dir.clone());
      }

      !matches
    });

    if let Some(backend) = self.backend.as_mut() {
      for dir in dropped {
        let _ = backend.unwatch(&dir);
      }
    }
  }

  /// Brings what the backend watches in line with the registrations.
  ///
  /// Stale watches are dropped before new ones are made. On Linux a
  /// recursive watch and a separate watch on one of its subdirectories
  /// share the same inotify descriptor, so dropping the separate one
  /// after the recursive one exists would take it away from both.
  fn reconcile(&mut self) -> notify::Result<()> {
    let wanted = self.wanted();
    let backend = self
      .backend
      .as_mut()
      .ok_or_else(|| notify::Error::generic("this watcher is closed"))?;

    let mut kept = Vec::new();

    for (dir, mode) in self.watched.drain(..) {
      if wanted.contains(&(dir.clone(), mode)) {
        kept.push((dir, mode));
      } else {
        let _ = backend.unwatch(&dir);
      }
    }

    self.watched = kept;

    for (dir, mode) in wanted {
      if self.watched.contains(&(dir.clone(), mode)) {
        continue;
      }

      backend.watch(&dir, mode)?;
      self.watched.push((dir, mode));
    }

    Ok(())
  }

  /// The directories the registrations need watched, with any one a
  /// recursive watch above it already covers left out.
  fn wanted(&self) -> Vec<(PathBuf, RecursiveMode)> {
    let mut wanted: Vec<(PathBuf, PathBuf, RecursiveMode)> = Vec::new();

    for registration in &self.registrations {
      let (dir, mode) = registration.directory();
      let key = fold(&dir);

      match wanted.iter_mut().find(|(_, other, _)| *other == key) {
        Some(entry) if mode == RecursiveMode::Recursive => entry.2 = mode,
        Some(_) => {},
        None => wanted.push((dir, key, mode)),
      }
    }

    let trees: Vec<PathBuf> = wanted
      .iter()
      .filter(|(_, _, mode)| *mode == RecursiveMode::Recursive)
      .map(|(_, key, _)| key.clone())
      .collect();

    wanted
      .into_iter()
      .filter(|(_, key, _)| {
        !trees
          .iter()
          .any(|tree| key != tree && key.starts_with(tree))
      })
      .map(|(dir, _, mode)| (dir, mode))
      .collect()
  }
}

impl Drop for Watcher {
  fn drop(&mut self) {
    self.close();
  }
}

/// Raises the soft limit on open files as far as the hard limit and the
/// kernel allow, once per process. FSEvents refuses a stream that would
/// watch more directories than about a twelfth of that limit, and macOS
/// starts every process at 256, which leaves room for only 21.
#[cfg(target_os = "macos")]
fn raise_file_limit() {
  static ONCE: std::sync::Once = std::sync::Once::new();

  ONCE.call_once(|| unsafe {
    let mut limit = std::mem::zeroed::<libc::rlimit>();

    if libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) != 0 {
      return;
    }

    let mut most: libc::c_int = 0;
    let mut size = std::mem::size_of::<libc::c_int>();
    let found = libc::sysctlbyname(
      c"kern.maxfilesperproc".as_ptr(),
      (&mut most as *mut libc::c_int).cast(),
      &mut size,
      std::ptr::null_mut(),
      0,
    );

    if found != 0 || most <= 0 {
      return;
    }

    let wanted = limit.rlim_max.min(most as libc::rlim_t);

    if wanted > limit.rlim_cur {
      limit.rlim_cur = wanted;
      libc::setrlimit(libc::RLIMIT_NOFILE, &limit);
    }
  });
}

/// Whether an event of `kind` is one nobody is told about: reading a
/// file, or opening one, changes nothing.
fn ignored(kind: &EventKind) -> bool {
  matches!(
    kind,
    EventKind::Access(_) | EventKind::Modify(ModifyKind::Metadata(MetadataKind::AccessTime))
  )
}

/// The paths one OS event touched, and what it says happened to each.
fn translate(event: notify::Event) -> Vec<(Raw, Option<PathBuf>)> {
  if event.need_rescan() {
    if event.paths.is_empty() {
      return vec![(Raw::Rescan, None)];
    }

    return event
      .paths
      .into_iter()
      .map(|p| (Raw::Rescan, Some(p)))
      .collect();
  }

  if ignored(&event.kind) {
    return Vec::new();
  }

  let raw = match event.kind {
    EventKind::Access(_) => return Vec::new(),
    EventKind::Modify(ModifyKind::Name(RenameMode::Both)) if event.paths.len() == 2 => {
      let mut paths = event.paths.into_iter();
      let from = paths.next();
      let to = paths.next();

      return vec![(Raw::Remove, from), (Raw::Create, to)];
    },
    EventKind::Modify(ModifyKind::Name(RenameMode::From)) => Raw::Remove,
    EventKind::Modify(ModifyKind::Name(RenameMode::To)) => Raw::Create,
    EventKind::Modify(ModifyKind::Name(_)) => Raw::Moved,

    EventKind::Create(_) => Raw::Create,
    EventKind::Remove(_) => Raw::Remove,
    EventKind::Modify(_) | EventKind::Any | EventKind::Other => Raw::Modify,
  };

  event.paths.into_iter().map(|p| (raw, Some(p))).collect()
}

/// `path` resolved through every link, the way the runtime names files
/// everywhere else.
fn canonical(path: &str) -> std::io::Result<PathBuf> {
  crate::builtins::file::canonical_path(path).map(PathBuf::from)
}

fn absolute(path: &str) -> PathBuf {
  std::path::absolute(path).unwrap_or_else(|_| PathBuf::from(path))
}

/// `path` in the form two spellings of the same name compare equal in.
/// Windows and macOS look names up without regard to case, and the
/// events they deliver carry a name's case as it is on disk, which need
/// not be the case it was added with.
fn fold(path: &Path) -> PathBuf {
  #[cfg(any(windows, target_vendor = "apple"))]
  return PathBuf::from(path.to_string_lossy().to_lowercase());

  #[cfg(not(any(windows, target_vendor = "apple")))]
  path.to_path_buf()
}

/// The message a failed watch raises with.
fn describe(error: &notify::Error, path: Option<&str>) -> String {
  let target = path.map(|p| format!(" '{p}'")).unwrap_or_default();

  match &error.kind {
    notify::ErrorKind::PathNotFound => {
      format!("could not watch{target}: No such file or directory")
    },
    #[cfg(target_os = "macos")]
    notify::ErrorKind::MaxFilesWatch => format!(
      "could not watch{target}: the system's limit on watched directories was reached \
       (raise the hard limit on open files, `ulimit -H -n`)"
    ),
    #[cfg(not(target_os = "macos"))]
    notify::ErrorKind::MaxFilesWatch => format!(
      "could not watch{target}: the system's limit on watched directories was reached \
       (on Linux, raise fs.inotify.max_user_watches)"
    ),
    notify::ErrorKind::Io(e) => format!("could not watch{target}: {e}"),
    notify::ErrorKind::Generic(text) => format!("could not watch{target}: {text}"),
    _ => format!("could not watch{target}: {error}"),
  }
}

fn spawn_follower(body: impl FnOnce() + Send + 'static) -> Result<(), String> {
  std::thread::Builder::new()
    .name("zuri-watch-follow".to_string())
    .spawn(body)
    .map(|_| ())
    .map_err(|e| format!("could not follow the process: {e}"))
}

/// Blocks until the child `pid` exits, leaving it to be reaped by
/// whoever reads its status. `WNOWAIT` is what makes the wait leave the
/// child alone, so `Process.wait()` still gets the exit status.
#[cfg(unix)]
fn wait_without_reaping(pid: u32) {
  let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };

  loop {
    let result = unsafe {
      libc::waitid(
        libc::P_PID,
        pid as libc::id_t,
        &mut info,
        libc::WEXITED | libc::WNOWAIT,
      )
    };

    if result == 0 || std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
      return;
    }
  }
}

/// Blocks until the process behind `handle` exits, then closes the
/// handle, which is a duplicate this thread owns.
#[cfg(windows)]
fn wait_for_handle(handle: usize) {
  use windows_sys::Win32::Foundation::CloseHandle;
  use windows_sys::Win32::System::Threading::{INFINITE, WaitForSingleObject};

  unsafe {
    WaitForSingleObject(handle as _, INFINITE);
    CloseHandle(handle as _);
  }
}
