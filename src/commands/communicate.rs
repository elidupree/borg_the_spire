use std::io::BufRead;
use std::path::PathBuf;

pub fn communicate(state_file: PathBuf) {
  println!("ready");

  let input = std::io::stdin();
  let input = input.lock();

  for line in input.lines() {
    let line = line.unwrap();
    if line.len() > 3 {
      if line.starts_with(r#"{"error""#) {
        eprintln!("Received error from communication mod: {}", line);
      }
      let result = std::fs::write(&state_file, line);
      if let Err(e) = result {
        eprintln!("Error writing state to file: {}", e);
      }
    }
  }
}
