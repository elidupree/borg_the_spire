use std::io::{BufRead, Write};

mod communication_mod_state;

fn main() {
  println!("ready");
  eprintln!("Hello BtS");

  let mut file = std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(r#"C:\Users\Eli\Documents\borg_the_spire_log"#)
    .unwrap();

  writeln!(file, "Hello BtS 2").unwrap();

  let input = std::io::stdin();
  let mut input = input.lock();
  let mut failed = false;

  loop {
    let mut buffer = String::new();
    input.read_line(&mut buffer).unwrap();
    if buffer.len() > 3 {
      let interpreted: Result<communication_mod_state::CommunicationState, _> =
        serde_json::from_str(&buffer);
      match interpreted {
        Ok(_state) => writeln!(file, "received state from communication mod").unwrap(),
        Err(err) => {
          writeln!(file, "received non-state from communication mod {:?}", err).unwrap();
          if !failed {
            writeln!(file, "data: {:?}", buffer).unwrap();
          }
          failed = true;
        }
      }
    }
  }
}
