#[cfg(target_os = "windows")]
fn main() {
  embed_resource::compile("src/windows/rclone_ignore.rc");
}

#[cfg(not(target_os = "windows"))]
fn main() { }