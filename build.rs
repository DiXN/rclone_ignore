#[cfg(target_os = "windows")]
use windres::Build;

#[cfg(target_os = "windows")]
fn main() {
  Build::new().compile("src/windows/rclone_ignore.rc").unwrap();
}

#[cfg(not(target_os = "windows"))]
fn main() { }