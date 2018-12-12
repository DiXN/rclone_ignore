use clap::{Arg, App, ArgMatches};
use globset::{Glob, GlobSet, GlobSetBuilder, Error};

use std::{
  process::exit,
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
    .get_matches()
}

fn get_ignores() -> Result<GlobSet, Error> {
  let mut builder = GlobSetBuilder::new();

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

pub fn get_options() -> (PathBuf, String, GlobSet) {
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

  if let Ok(t) = value_t!(matches, "threads", usize) {
    rayon::ThreadPoolBuilder::new().num_threads(t).build_global().unwrap();
  } else {
    rayon::ThreadPoolBuilder::new().num_threads(3).build_global().unwrap();
  };

  (PathBuf::from(root), remote_root, ignores)
}
