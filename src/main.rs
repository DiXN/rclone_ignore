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
  let root = "F:\\sync";
  let root = &canonicalize(&root).unwrap().display().to_string()[4..];

  let remote_root = "db:/";

  rclone!("copy", remote_root, root, "--progress", "--checkers", "128", "--retries", "1").status()?;

  println!("Fetched data from remote.");

  let (tx, rx) = mpsc::channel();
  let mut watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_millis(200)).expect("Cannot spawn watcher.");
  watcher.watch(root, RecursiveMode::Recursive).expect("Cannot watch directory watcher.");

  let get_included_paths = || WalkBuilder::new(root).hidden(false).build().map(|w| {
    let path = w.unwrap().into_path();
    let is_file = path.is_file();
    (is_file, path)
  }).collect::<Vec<(bool, PathBuf)>>();

  let upload_path = |path: &Path, preserve_file: bool| {
    let relative = path.strip_prefix(root).unwrap();

    let relative = if !preserve_file {
      if path.is_file() {
        relative.parent().unwrap()
      } else {
        relative
      }
    } else {
      relative
    };

    if cfg!(target_os = "windows") {
      str::replace(&relative.display().to_string(), "\\", "/")
    } else {
      relative.display().to_string()
    }
  };

  let mut legal_paths = get_included_paths();

  loop {
    if let Ok(notify) = rx.recv() {
      match notify {
        DebouncedEvent::NoticeWrite(_) => continue,
        DebouncedEvent::NoticeRemove(_) => continue,
        DebouncedEvent::Create(ref path) => {
          legal_paths = get_included_paths();

          if let Some(lp) = legal_paths.iter().filter(|(_, p)| p == path).next() {
           let u_path = upload_path(path, false);

           if lp.0 {
            match rclone!("copy", &path.display().to_string(), &format!("{}/{}", remote_root, &u_path)).status() {
              Ok(_) => println!("Created: {}", path.display()),
              Err(e) => println!("{}", e)
            }
           } else {
            match rclone!("mkdir", &format!("{}/{}", remote_root, &u_path)).status() {
              Ok(_) => println!("Created: {}", path.display()),
              Err(e) => println!("{}", e)
            }
           }
          }
        },
        DebouncedEvent::Write(ref path) => {
          if let Some(lp) = legal_paths.iter().filter(|(_, p)| p == path).next() {
            if lp.0 {
              match rclone!("copy", &path.display().to_string(), &format!("{}/{}", remote_root, upload_path(path, false))).status() {
                Ok(_) => println!("Updated: {}", path.display()),
                Err(e) => println!("{}", e)
              }
            }
          }
        },
        DebouncedEvent::Rename(ref from_path, ref to_path) => {
          if let Some(_) = legal_paths.iter().filter(|(_, p)| p == from_path).next() {
            match rclone!("moveto", &format!("{}/{}", remote_root, upload_path(from_path, true)), &format!("{}/{}", remote_root, upload_path(to_path, true))).status() {
              Ok(_) => println!("Renamed {} to {}", from_path.display(), to_path.display()),
              Err(e) => println!("{}", e)
            }
          }

          legal_paths = get_included_paths();
        },
        DebouncedEvent::Remove(ref path) => {
          if let Some(lp) = legal_paths.iter().filter(|(_, p)| p == path).next() {
            let u_path = upload_path(path, false);

            if lp.0 {
              match rclone!("delete", &format!("{}/{}", remote_root, u_path)).status() {
                Ok(_) => println!("Deleted: {}", path.display()),
                Err(e) => println!("{}", e)
              }
            } else {
              match rclone!("purge", &format!("{}/{}", remote_root, u_path)).status() {
                Ok(_) => println!("Purged: {}", path.display()),
                Err(e) => println!("{}", e)
              }
            }
          }
        },
        _ => ()
      }
    }
  }

  Ok(())
}
