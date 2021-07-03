use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::Duration;

pub fn watch(executable_original: &str, executable_copy: &str, args: &[&str]) {
  // TODO : this isn't the most efficient file watcher system, figure out what is?
  let mut executable_copy = PathBuf::from(executable_original);
  executable_copy.set_extension("bts-copy.exe");
  let mut last_modified = None;
  let mut child: Option<_> = None;
  loop {
    // If the code file has been modified, update it.
    if let Ok(modified) = fs::metadata(&executable_original).and_then(|m| m.modified()) {
      if Some(modified) != last_modified {
        last_modified = Some(modified);
        drop(child);
        while let Err(_) = fs::copy(executable_original, &executable_copy) {
          std::thread::sleep(Duration::from_millis(100));
        }
        child = Some(scopeguard::guard(
          Command::new(&executable_copy).args(args).spawn().unwrap(),
          |mut child| {
            child.kill().unwrap();
            child.wait().unwrap();
          },
        ));
      }
      std::thread::sleep(Duration::from_millis(100));
    }
  }
}
