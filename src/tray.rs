#[cfg(not(target_os = "windows"))]
pub fn init_tray() {
  info!("\"tray\" is currently not supported on your system.");
}

#[cfg(target_os = "windows")]
pub fn init_tray() {
  use std::{
    env,
    ptr,
    thread,
    io::{BufWriter, Write},
    process::{exit, Command, Stdio}
  };

  use winapi::um::wincon::GetConsoleWindow;
  use winapi::um::winuser::ShowWindow;

  thread::spawn(move || {
    if let Ok(mut app) = systray::Application::new() {
      let win = unsafe { GetConsoleWindow() };
      if win != ptr::null_mut() {
        unsafe {
          ShowWindow(win, 0);
        }
      }

      match app.set_icon_from_resource(&"tray_icon".to_string()) {
        Ok(_) => (),
        Err(e) => println!("{}", e)
      };

      //Restart the whole process on sync call for now.
      app.add_menu_item(&"Sync".to_string(), move |_| {
        let mut process = Command::new("powershell")
          .args(&["-Command", "-"])
          .stdin(Stdio::piped())
          .spawn().expect("Could not start powershell.");

        {
          let mut out_stdin = process.stdin.as_mut().expect("Could not collect stdin.");
          let mut writer = BufWriter::new(&mut out_stdin);

          let current_path = env::current_exe().expect("Could not get startup path of executable.");

          //Stop current instance of rclone_ignore.
          writer.write_all("Stop-Process -processname rclone_ignore;".as_bytes())
            .expect("Could not write to powershell process.");

          //Start new instance of rclone_ignore.
          writer.write_all(
            format!("Start-Process -FilePath \"{}\" -ArgumentList \"{}\";",
            current_path.display(), env::args().skip(1).collect::<Vec<_>>().join(" ")).as_bytes()
          )
            .expect("Could not write to powershell process.");
        }

        process.wait().expect("Could not start powershell process.");

        Ok::<_, systray::Error>(())
      }).ok();

      app.add_menu_item(&"Show".to_string(), |_| {
        let window = unsafe { GetConsoleWindow() };
        if window != ptr::null_mut() {
          unsafe {
            ShowWindow(window, 5);
          }
        }

        Ok::<_, systray::Error>(())
      }).ok();

      app.add_menu_item(&"Hide".to_string(), |_| {
        let window = unsafe { GetConsoleWindow() };
        if window != ptr::null_mut() {
          unsafe {
            ShowWindow(window, 0);
          }
        }

        Ok::<_, systray::Error>(())
      }).ok();

      app.add_menu_item(&"Quit".to_string(), |win| {
        win.quit();
        Ok::<_, systray::Error>(())
      }).ok();

      app.wait_for_message();
    }
  });
}
