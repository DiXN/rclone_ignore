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

  println!("Fetched data from remote.");

  let (tx, rx) = mpsc::channel();
  let mut watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_millis(200)).expect("Cannot spawn watcher.");
  watcher.watch(root, RecursiveMode::Recursive).expect("Cannot watch directory watcher.");

  let get_included_paths = || WalkBuilder::new(root).hidden(false).build().map(|w| PathBuf::from(w.unwrap().path())).collect::<Vec<PathBuf>>();

  let upload_path = |path: &Path| {
    let relative = path.strip_prefix(root).unwrap();
    let mut is_file = true;

    let relative = if path.is_file() {
      relative.parent().unwrap()
    } else {
      is_file = false;
      relative
    };

    let u_path = if cfg!(target_os = "windows") {
      str::replace(&relative.display().to_string(), "\\", "/")
    } else {
      relative.display().to_string()
    };

    (u_path, is_file)
  };

  let mut legal_paths = get_included_paths();

  loop {
    if let Ok(notify) = rx.recv() {
      match notify {
        DebouncedEvent::Create(ref path) => {
          legal_paths = get_included_paths();

          if legal_paths.contains(path) {
           let (u_path, is_file) = upload_path(path);

           if is_file {
            match rclone!("copy", &path.display().to_string(), &format!("{}/{}", remote_root, u_path)).status() {
              Ok(_) => println!("Created: {}", path.display()),
              Err(e) => println!("{}", e)
            }
           } else {
            match rclone!("mkdir", &format!("{}/{}", remote_root, u_path)).status() {
              Ok(_) => println!("Created: {}", path.display()),
              Err(e) => println!("{}", e)
            }
           }
          }
        },
        DebouncedEvent::Write(ref path) => {
          if legal_paths.contains(path) {
            let (u_path, _) = upload_path(path);
            match rclone!("copy", &path.display().to_string(), &format!("{}/{}", remote_root, u_path)).status() {
              Ok(_) => println!("Updated: {}", path.display()),
              Err(e) => println!("{}", e)
            }
          }
        },
        DebouncedEvent::Rename(ref old_path, ref path) => (),
        DebouncedEvent::Remove(ref path) => {
          let (u_path, is_file) = upload_path(path);

          if is_file {
            match rclone!("delete", &format!("{}/{}", remote_root, u_path)).status() {
              Ok(_) => println!("Deleted: {}", path.display()),
              Err(e) => println!("{}", e)
            }
          } else {
            match rclone!("purge", &format!("{}/{}", remote_root, u_path)).status() {
              Ok(_) => println!("Deleted: {}", path.display()),
              Err(e) => println!("{}", e)
            }
          }
        },
        _ => ()
      }
    }
  }

  Ok(())
}
