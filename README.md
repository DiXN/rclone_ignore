# rclone_ignore  · [![Build Status](https://travis-ci.org/DiXN/rclone_ignore.svg?branch=master)](https://travis-ci.org/DiXN/rclone_ignore)

*rclone_ignore* is a small [rclone](https://github.com/ncw/rclone) wrapper with support for file watching and ignoring paths specified in *.gitignore* files.

## Prerequisites

* [rclone](https://github.com/ncw/rclone)
* correctly configured remote in [rclone](https://github.com/ncw/rclone)

## Usage

```
USAGE:
    rclone_ignore.exe [FLAGS] [OPTIONS] --local-root <local-root> --remote-root <remote-root> [--] [sync-args]...

FLAGS:
    -a, --autostart    Runs rclone_ignore on system startup
    -h, --help         Prints help information
    -V, --version      Prints version information

OPTIONS:
    -i, --ignores <ignores>...         Ignores custom glob patterns
    -l, --local-root <local-root>      Specifies local root path for sync
    -r, --remote-root <remote-root>    Specifies remote root path for sync [remote:/path]
    -t, --threads <threads>            Defines maximum amount of concurrently running commands

ARGS:
    <sync-args>...    Specifies arguments for sync
```

* *local-root* and *remote-root* are mandatory flags where *local-root* is a path in your local file system and *remote-root* is the path from where you want to sync files from on the remote.
* *autostart* is currently only supported on Windows.
* Arguments that should be passed to `rclone sync` must be specified after `--`.

### Example
```
cargo run --local-root c:/sync --remote-root db:/ --threads 3 -- --progress
```

## Tray

To enable system tray on Windows build with the feature *tray*

```
cargo build --features tray
```