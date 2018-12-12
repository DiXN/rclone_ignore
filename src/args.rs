use clap::{Arg, App, ArgMatches};

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
    .get_matches()
}