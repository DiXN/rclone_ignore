extern crate notify;
use notify::{RecommendedWatcher, Watcher, RecursiveMode, DebouncedEvent};

extern crate ignore;
use ignore::WalkBuilder;

use std::{
  process::Command,
  error::Error,
  time::Duration,
  sync::mpsc,
  path::{PathBuf, Path}
};

use std::fs::canonicalize;

macro_rules! rclone {
  () => {{
    Command::new("rclone")
  }};
  ($e:expr) => {{
    rclone!().arg($e)
  }};
  ($($es:expr),+) => {{
    rclone!().args(&[$($es),+])
  }};
}

fn main() -> Result<(), Box<dyn Error>> {
  let root = "./sync";
  let root = &canonicalize(&root).unwrap().display().to_string()[4..];

  let remote_root = "db:/config";

  rclone!("sync", remote_root, root).status()?;

  let (tx, rx) = mpsc::channel();
  let mut watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_millis(200)).expect("Cannot spawn watcher.");
  watcher.watch(root, RecursiveMode::Recursive).expect("Cannot watch directory watcher.");

  let get_included_paths = || WalkBuilder::new(root).hidden(false).build().map(|w| PathBuf::from(w.unwrap().path())).collect::<Vec<PathBuf>>();

  let upload_path = |path: &Path| {
    if cfg!(target_os = "windows") {
      str::replace(&path.strip_prefix(root).unwrap().display().to_string(), "\\", "/")
    } else {
      path.strip_prefix(root).unwrap().display().to_string()
    }
  };

  let mut legal_paths = get_included_paths();

  loop {
    if let Ok(notify) = rx.recv() {

      match notify {
        DebouncedEvent::Create(ref path) => {
          legal_paths = get_included_paths();

          if legal_paths.contains(path) {
           match rclone!("copy", &path.display().to_string(), remote_root).status() {
              Ok(_) => println!("Created: {}", path.display()),
              Err(e) => println!("{}", e)
            }
          }
        },
        DebouncedEvent::Write(ref path) => {
          if legal_paths.contains(path) {
            match rclone!("copy", &path.display().to_string(), remote_root).status() {
              Ok(_) => println!("Updated: {}", path.display()),
              Err(e) => println!("{}", e)
            }
          }
        },
        DebouncedEvent::Rename(ref old_path, ref path) => (),
        DebouncedEvent::Remove(ref path) => {
          match rclone!("delete", &format!("{}/{}", remote_root, upload_path(path))).status() {
            Ok(_) => println!("Deleted: {}", path.display()),
            Err(e) => println!("{}", e)
          }
        },
        _ => ()
      }
    }
  }

  Ok(())
}
