use clap::{Arg, App, ArgMatches, AppSettings};
use globset::{Glob, GlobSet, GlobSetBuilder, Error as Glob_Error};

use std::{
  str,
  env,
  error::Error as Std_Error,
  process::{exit, Command, Stdio, ExitStatus},
  io::{BufWriter, Write},
  fs::canonicalize,
  path::{PathBuf, Path}
};

macro_rules! exit {
  ($e:expr) => {{
    error!("{}", $e);
    exit(1);
  }};
}

pub fn get_matches() -> ArgMatches<'static> {
  App::new("rclone_ignore")
    .about("Ignores glob patterns specified in a `.gitignore` or `.ignore` file for usage with rclone")
    .setting(AppSettings::TrailingVarArg)
    .arg(
      Arg::with_name("local-root")
        .short("l")
        .long("local-root")
        .takes_value(true)
        .max_values(1)
        .required(true)
        .help("Specifies local root path for sync")
    )
    .arg(
      Arg::with_name("remote-root")
        .short("r")
        .long("remote-root")
        .takes_value(true)
        .max_values(1)
        .required(true)
        .help("Specifies remote root path for sync [remote:/path]")
    )
    .arg(
      Arg::with_name("threads")
        .short("t")
        .long("threads")
        .takes_value(true)
        .max_values(1)
        .help("Defines maximum amount of concurrently running commands")
    )
    .arg(
      Arg::with_name("ignores")
        .short("i")
        .long("ignores")
        .takes_value(true)
        .min_values(1)
        .multiple(true)
        .help("Ignores custom glob patterns")
    )
    .arg(
      Arg::with_name("autostart")
        .short("a")
        .long("autostart")
        .help("Runs rclone_ignore on system startup")
    )
    .arg(
      Arg::with_name("sync-args")
        .multiple(true)
        .help("Specifies arguments for sync")
    )
    .get_matches()
}


fn get_ignores() -> Result<GlobSet, Glob_Error> {
  let mut builder = GlobSetBuilder::new();

  //Pre defined ignores.
  builder.add(Glob::new("*desktop.ini")?);
  builder.add(Glob::new("*Thumbs.db")?);
  builder.add(Glob::new("*.DS_Store")?);

  let matches = get_matches();

  if matches.is_present("ignores") {
    if let Ok(ignores) = values_t!(matches, "ignores", String) {
      for ignore in ignores {
        builder.add(Glob::new(&ignore)?);
      }
    } else {
      error!("\"ignores\" are invalid.");
    }
  }

  Ok(builder.build()?)
}

#[cfg(not(target_os = "windows"))]
#[allow(unused_variables)]
fn autostart(lr: &Path, rr: &str, matches: &ArgMatches) -> Result<ExitStatus, Box<Std_Error>> {
  info!("\"autostart\" is currently not supported on your system.");

  let mut process = Command::new("echo")
    .arg("\"autostart\" is currently not supported on your system.")
    .spawn()?;

  Ok(process.wait()?)
}

#[cfg(target_os = "windows")]
fn autostart(lr: &Path, rr: &str, matches: &ArgMatches) -> Result<ExitStatus, Box<Std_Error>> {
  let auto_cmd = Command::new("powershell")
    .args(&["-Command", "[environment]::getfolderpath(\"Startup\")"])
    .output()?;

  let auto_path = str::from_utf8(&auto_cmd.stdout)?.trim();

  let mut process = Command::new("powershell")
    .args(&["-Command", "-"])
    .stdin(Stdio::piped())
    .spawn()?;

  {
    let mut out_stdin = process.stdin.as_mut().expect("Could not collect stdin.");

    let mut writer = BufWriter::new(&mut out_stdin);

    match env::current_exe() {
      Ok(exe_path) => {
        //reference: https://stackoverflow.com/a/47340271
        writer.write_all("$WshShell = New-Object -comObject WScript.Shell;".as_bytes())?;
        writer.write_all(format!("$Shortcut = $WshShell.CreateShortcut(\"{}\\rclone_ignore.lnk\");", auto_path).as_bytes())?;
        writer.write_all(format!("$Shortcut.TargetPath = \"{}\";", exe_path.display()).as_bytes())?;
        writer.write_all("$Shortcut.WindowStyle = 7;".as_bytes())?;

        let local_root_str = lr.display().to_string();

        let local_root_str = match local_root_str.chars().nth(local_root_str.len() - 1) {
          Some(l_char) => {
            if l_char != '\\' {
              format!("{}\\", local_root_str)
            } else {
              local_root_str
            }
          },
          None => panic!("Out of range."),
        };

        let mut arguments_str = String::new();

        arguments_str.push_str(&format!("--local-root {} --remote-root {} ", &local_root_str[4..], rr));

        if let Ok(t) = value_t!(matches, "threads", usize) {
          arguments_str.push_str(&format!("--threads {} ", t));
        }

        if let Ok(c) = value_t!(matches, "checkers", usize) {
          arguments_str.push_str(&format!("--checkers {} ", c));
        }

        if let Ok(ignores) = values_t!(matches, "ignores", String) {
          arguments_str.push_str("--ignores ");

          for ignore in ignores {
            arguments_str.push_str(&format!("{} ", ignore));
          }
        }

        if let Ok(sa) = value_t!(matches, "sync-args", String) {
          arguments_str.push_str(" -- ");
          arguments_str.push_str(&sa);
        }

        writer.write_all(format!("$Shortcut.Arguments = \"{}\";", arguments_str).as_bytes())?;
        writer.write_all("$Shortcut.Save();".as_bytes())?;
      },
      Err(e) => panic!(e)
    };
  }

  Ok(process.wait()?)
}

pub fn get_options() -> (PathBuf, String, GlobSet, String) {
  let matches = get_matches();

  let root = if let Ok(lr) = value_t!(matches, "local-root", String) {
    lr
  } else {
    exit!("\"local-root\" is invalid.");
  };

  if !Path::new(&root).exists() {
    exit!("\"local-root\" does not exist locally.");
  }

  let root = &canonicalize(&root).unwrap();

  let remote_root = if let Ok(rr) = value_t!(matches, "remote-root", String) {
    rr
  } else {
    exit!("\"remote-root\" is invalid.");
  };

  let ignores = get_ignores().expect("Cannot get ignores.");

  if matches.is_present("autostart") {
    if cfg!(target_os = "windows") {
      match autostart(&root, &remote_root, &matches) {
        Ok(a) => if a.success() {
          info!("Autostart set.");
        } else {
          error!("Failed to set autostart.")
        },
        Err(e) => error!("{}", e)
      }
    } else {
      info!("\"autostart\" is currently not supported on your system.");
    }
  }

  let sync_args = if let Ok(sa) = value_t!(matches, "sync-args", String) {
    sa
  } else {
    String::from("")
  };

  (PathBuf::from(root), remote_root, ignores, sync_args)
}
