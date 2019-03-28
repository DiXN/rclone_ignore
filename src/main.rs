#[macro_use] extern crate log;
#[macro_use] extern crate clap;

use notify::{RecommendedWatcher, Watcher, RecursiveMode, DebouncedEvent};
use ignore::WalkBuilder;
use env_logger::{Builder, Env};
use which::which;
use rayon::prelude::*;
use walkdir::WalkDir;

use std::{
  env,
  ptr,
  thread,
  error::Error,
  time::Duration,
  sync::{mpsc, Arc},
  fs::File,
  io::prelude::*,
  process::{exit, Command, ExitStatus},
  path::{PathBuf, Path}
};

mod pathop;
use crate::pathop::{Op, PathOp};

#[macro_use]
mod args;
use crate::args::{get_options, get_matches};

#[cfg(not(target_os = "windows"))]
fn init_tray(temp_dir: String) {
  info!("\"tray\" is currently not supported on your system.");
}

#[cfg(target_os = "windows")]
fn init_tray(temp_dir: String) {
  thread::spawn(move || {
    if let Ok(mut app) = systray::Application::new() {
      let window = unsafe { kernel32::GetConsoleWindow() };

      let (root, remote_root, _, checkers, tps_limit) = get_options();

      if window != ptr::null_mut() {
        unsafe {
          user32::ShowWindow(window, 0);
        }
      }

      match app.set_icon_from_resource(&"tray_icon".to_string()) {
        Ok(_) => (),
        Err(e) => println!("{}", e)
      };

      app.add_menu_item(&"Sync".to_string(), move |_| {
        match sync(&remote_root, &temp_dir, &root, checkers, tps_limit) {
          Ok(_) => (),
          Err(_) => error!("Sync failed.")
        }
      }).ok();

      app.add_menu_item(&"Show".to_string(), move |_| {
        if window != ptr::null_mut() {
          unsafe {
            user32::ShowWindow(window, 5);
          }
        }
      }).ok();

      app.add_menu_item(&"Hide".to_string(), move |_| {
        if window != ptr::null_mut() {
          unsafe {
            user32::ShowWindow(window, 0);
          }
        }
      }).ok();

      app.add_menu_item(&"Quit".to_string(), |_| exit(0)).ok();
      app.wait_for_message();
    }
  });
}

//Get all paths that are not ignored from a .gitignore or .ignore file.
fn get_included_paths(root: &Path) -> Vec<(bool, PathBuf)> {
  WalkBuilder::new(root).hidden(false).build().map(|w| {
    let path = w.unwrap().into_path();
    let is_file = path.is_file();
    (is_file, path)
  }).collect::<Vec<(bool, PathBuf)>>()
}

//Transform local path to a valid remote path.
fn upload_path(root: &Path, path: &Path, preserve_file: bool) -> String {
  let relative = path.strip_prefix(root).unwrap();

  let relative = if !preserve_file {
    if path.is_file() {
      relative.parent().unwrap().display().to_string()
    } else {
      relative.display().to_string()
    }
  } else {
    if path.is_file() {
      relative.display().to_string()
    } else {
      format!("{}/", relative.display())
    }
  };

  if cfg!(target_os = "windows") {
    str::replace(&relative, "\\", "/")
  } else {
    relative
  }
}

//Write all invalid paths to a temporary file so rclones can ignore those paths during sync.
fn update_sync_ignores(root: &Path, dir: &str) -> Result<(), Box<dyn Error>> {
  let legal_paths = get_included_paths(&root);
  let mut file = File::create(&dir)?;
  //Get all paths starting from the specified local root path.
  let all_paths = WalkDir::new(root).into_iter().map(|p| p.unwrap().into_path()).collect::<Vec<_>>();

  if !all_paths.is_empty() {
    let num_tasks_per_chunk = all_paths.len() / num_cpus::get();
    let legal_paths_arc = Arc::new(legal_paths);

    let ips = crossbeam::scope(|scope| {
      let threads = all_paths.chunks(num_tasks_per_chunk).map(|chunk| {
        let cloned_arr = Arc::clone(&legal_paths_arc);
        scope.spawn(move |_|
          chunk.iter()
            .filter(|&t| !cloned_arr.contains(&(t.is_file(), t.to_path_buf())))
            .map(|ip| format!("{}\n", upload_path(&root, &ip, true))).collect::<Vec<_>>().join("")
        )
      }).collect::<Vec<_>>();

      threads.into_iter().map(|t| t.join().unwrap()).collect::<Vec<_>>().join("")
    });

    write!(file, "{}", ips.unwrap())?;
  }

  Ok(())
}

fn sync(remote_root: &str, dir: &str, root: &Path, checkers: usize, tps_limit: f32, mod_time: bool) -> Result<ExitStatus, std::io::Error> {
  match update_sync_ignores(&root, &dir) {
    Ok(_) => (),
    Err(_) => error!("Could not update sync ignores.")
  };

  let status = if mod_time {
    Command::new("rclone")
    .arg("sync")
    .args(&[&remote_root, &root.display().to_string().as_ref(),
      "--exclude-from", dir, "--progress", "--no-update-modtime", "--checkers",
        &format!("{}", checkers), "--tpslimit", &format!("{}", tps_limit), "--retries", "1"]).status()

  } else {
    Command::new("rclone")
    .arg("sync")
    .args(&[&remote_root, &root.display().to_string().as_ref(),
      "--exclude-from", dir, "--progress", "--checkers",
        &format!("{}", checkers), "--tpslimit", &format!("{}", tps_limit), "--retries", "1"]).status()
  };

  info!("Synced data with remote.");

  status
}

fn main() -> Result<(), Box<dyn Error>> {
  let env = Env::default()
    .filter_or(env_logger::DEFAULT_FILTER_ENV, "info");

  Builder::from_env(env).init();

  let (root, remote_root, ignores, checkers, tps_limit, mod_time) = get_options();
  let root = root.as_path();

  if which("rclone").is_err() {
    exit!("You need to install rclone fist.");
  }

  {
    let matches = get_matches();

    if let Ok(t) = value_t!(matches, "threads", usize) {
      rayon::ThreadPoolBuilder::new().num_threads(t).build_global().unwrap();
    } else {
      rayon::ThreadPoolBuilder::new().num_threads(3).build_global().unwrap();
    };
  }

  let mut dir = env::temp_dir();
  dir.push("rclone_excludes.txt");

  let mut legal_paths = get_included_paths(&root);

  if cfg!(target_os = "windows") {
    init_tray(dir.display().to_string());
  }

  sync(&remote_root, &dir.display().to_string(), root, checkers, tps_limit, mod_time)?;

  let (tx, rx) = mpsc::channel();
  let mut watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_millis(200)).expect("Cannot spawn watcher.");
  watcher.watch(root, RecursiveMode::Recursive).expect("Cannot watch directory watcher.");

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

    let legal_paths_updated = get_included_paths(&root);
    let mut tasks = Vec::new();

    for chunk in paths.chunks(2) {
      //Check for "move".
      if chunk.len() > 1 &&
          chunk[0].op == Op::REMOVE && chunk[1].op == Op::CREATE &&
            legal_paths.iter().filter(|(_, p)| p == &chunk[0].path).next().is_some() &&
              legal_paths_updated.iter().filter(|(_, p)| p == &chunk[1].path).next().is_some() &&
                ignores.matches(&chunk[0].path).is_empty() && ignores.matches(&chunk[1].path).is_empty() {
        let from_u_path = upload_path(&root, &chunk[0].path, true);
        let to_u_path = upload_path(&root, &chunk[1].path, true);

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
                  let u_path = upload_path(&root, &c.path, false);
                  let print_path = upload_path(&root, &c.path, true);

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
                      &format!("{}/{}", remote_root, upload_path(&root, &c.path, false)),
                      &format!("COPY {}", upload_path(&root, &c.path, true))
                    ));
                  }
                }
              },
              Op::RENAME => {
                if let Some(_) = legal_paths_updated.iter().filter(|(_, p)| p == &c.path).next() {
                  let from_u_path = upload_path(&root, &c.old_path, true);
                  let to_u_path = upload_path(&root, &c.path, true);

                  tasks.push(
                    format!("moveto;{};{};{}",
                    &format!("{}/{}", remote_root, &from_u_path),
                    &format!("{}/{}", remote_root, &to_u_path),
                    &format!("RENAME from: {} to: {}", from_u_path, to_u_path)
                  ));

                  //Wait to prevent rename conflicts.
                  thread::sleep(Duration::from_millis(100))
                }
              },
              Op::REMOVE => {
                if let Some((is_file, _)) = legal_paths.iter().filter(|(_, p)| p == &c.path).next() {
                  let u_path = upload_path(&root, &c.path, false);

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
        //Get rclone command and arguments.
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
