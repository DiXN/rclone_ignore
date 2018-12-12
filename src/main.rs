#[macro_use] extern crate log;
#[macro_use] extern crate clap;

use notify::{RecommendedWatcher, Watcher, RecursiveMode, DebouncedEvent};
use ignore::WalkBuilder;
use env_logger::{Builder, Env};
use which::which;
use rayon::prelude::*;

use std::{
  process::{exit, Command},
  error::Error,
  time::Duration,
  sync::mpsc,
  path::{PathBuf, Path}
};

mod pathop;
use crate::pathop::{Op, PathOp};

#[macro_use]
mod args;
use crate::args::get_options;

fn main() -> Result<(), Box<dyn Error>> {
  let env = Env::default()
    .filter_or(env_logger::DEFAULT_FILTER_ENV, "info");

  Builder::from_env(env).init();

  let (root, remote_root, ignores) = get_options();
  let root = root.as_path();

  if which("rclone").is_err() {
    exit!("You need to install rclone fist.");
  }

  Command::new("rclone").arg("copy").args(&[&remote_root, &root.display().to_string(), "--progress", "--checkers", "128", "--retries", "1"]).status()?;

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
    let mut paths = Vec::new();

    let matcher = |notify: &DebouncedEvent, paths: &mut Vec<PathOp>| -> bool {
      let mut skip = false;
      match notify {
        DebouncedEvent::Create(ref path) => paths.push(PathOp::new(path, path, Op::CREATE)),
        DebouncedEvent::Write(ref path) => paths.push(PathOp::new(path, path, Op::WRITE)),
        DebouncedEvent::Rename(ref old_path, ref path) => paths.push(PathOp::new(old_path, path, Op::RENAME)),
        DebouncedEvent::Remove(ref path) => paths.push(PathOp::new(path, path, Op::REMOVE)),
        DebouncedEvent::Chmod(ref path) => paths.push(PathOp::new(path, path, Op::CHMOD)),
        _ => skip = true,
      }

      skip
    };

    if let Ok(notify) = rx.recv() {
      if matcher(&notify, &mut paths) { continue; }
      while let Ok(nf) = rx.recv_timeout(Duration::from_millis(500)) {
        matcher(&nf, &mut paths);
      }
    }

    let legal_paths_updated = get_included_paths();
    let mut tasks = Vec::new();

    for chunk in paths.chunks(2) {
      if chunk.len() > 1 &&
          chunk[0].op == Op::REMOVE && chunk[1].op == Op::CREATE &&
            legal_paths.iter().filter(|(_, p)| p == &chunk[0].path).next().is_some() &&
              legal_paths_updated.iter().filter(|(_, p)| p == &chunk[1].path).next().is_some() &&
                ignores.matches(&chunk[0].path).is_empty() && ignores.matches(&chunk[1].path).is_empty() {
        let from_u_path = upload_path(&chunk[0].path, true);
        let to_u_path = upload_path(&chunk[1].path, true);

        tasks.push(
          format!("moveto;{};{};{}",
          &format!("{}/{}", remote_root, &from_u_path),
          &format!("{}/{}", remote_root, &to_u_path),
          &format!("MOVE from: {} to: {}", from_u_path, to_u_path)
        ));
      } else {
        for c in chunk {
          if ignores.matches(&c.path).is_empty() {
            match &c.op {
              Op::CREATE => {
                if let Some((is_file, _)) = legal_paths_updated.iter().filter(|(_, p)| p == &c.path).next() {
                  let u_path = upload_path(&c.path, false);
                  let print_path = upload_path(&c.path, true);

                  if *is_file {
                    tasks.push(
                      format!("copy;{};{};{}",
                      &c.path.display().to_string(),
                      &format!("{}/{}", remote_root, &u_path),
                      &format!("COPY {}", print_path)
                    ));
                  } else {
                    tasks.push(
                      format!("mkdir;{};{}",
                      &format!("{}/{}", remote_root, &u_path),
                      &format!("MKDIR {}", print_path)
                    ));
                  }
                }
              },
              Op::WRITE => {
                if let Some((is_file, _)) = legal_paths_updated.iter().filter(|(_, p)| p == &c.path).next() {
                  if *is_file {
                    tasks.push(
                      format!("copy;{};{};{}",
                      &c.path.display().to_string(),
                      &format!("{}/{}", remote_root, upload_path(&c.path, false)),
                      &format!("COPY {}", upload_path(&c.path, true))
                    ));
                  }
                }
              },
              Op::RENAME => {
                if let Some(_) = legal_paths_updated.iter().filter(|(_, p)| p == &c.path).next() {
                  let from_u_path = upload_path(&c.old_path, true);
                  let to_u_path = upload_path(&c.path, true);

                  tasks.push(
                    format!("moveto;{};{};{}",
                    &format!("{}/{}", remote_root, &from_u_path),
                    &format!("{}/{}", remote_root, &to_u_path),
                    &format!("RENAME from: {} to: {}", from_u_path, to_u_path)
                  ));
                }
              },
              Op::REMOVE => {
                if let Some((is_file, _)) = legal_paths.iter().filter(|(_, p)| p == &c.path).next() {
                  let u_path = upload_path(&c.path, false);

                  if *is_file {
                    tasks.push(
                      format!("delete;{};{}",
                      &format!("{}/{}", remote_root, u_path),
                      &format!("DELETE {}", u_path),
                    ));
                  } else {
                    tasks.push(
                      format!("purge;{};{}",
                      &format!("{}/{}", remote_root, u_path),
                      &format!("PURGE {}", u_path),
                    ));
                  }
                }
              }
              _ => (),
            };
          }
        }
      }
    }

    if tasks.len() > 0 {
      tasks.into_par_iter().for_each(|t: String| {
        let split = t.split(";").collect::<Vec<&str>>();

        match Command::new("rclone").args(&split[0..split.len() - 1]).status() {
          Ok(s) => {
            if s.success() {
              info!("{} => successfull.", split[split.len() - 1]);
            } else {
              error!("{} => unsucessfull.", split[split.len() - 1]);
            }
          },
          Err(e) => error!("{}", e)
        };
      });
    }

    legal_paths = legal_paths_updated;
  }

  Ok(())
}
