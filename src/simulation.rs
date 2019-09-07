use rand::{Rng, SeedableRng};
use rand_xoshiro::Xoshiro256StarStar;
use std::any::Any;
type Generator = Xoshiro256StarStar;

pub trait Runner {
  fn gen<F: FnOnce(&mut Generator) -> R, R: Any + Clone>(&mut self, f: F) -> R;
}

pub struct DefaultRunner {
  generator: Generator,
  values: Vec<Box<dyn Any>>,
}

impl Runner for DefaultRunner {
  fn gen<F: FnOnce(&mut Generator) -> R, R: Any + Clone>(&mut self, f: F) -> R {
    let result = (f)(&mut self.generator);
    self.values.push(Box::new(result.clone()));
    result
  }
}

pub struct ReplayRunner {
  values: Vec<Box<dyn Any>>,
}

impl Runner for ReplayRunner {
  fn gen<F: FnOnce(&mut Generator) -> R, R: Any + Clone>(&mut self, _f: F) -> R {
    self
      .values
      .pop()
      .expect("ReplayRunner was prompted for a more values than originally")
      .downcast_ref::<R>()
      .expect("ReplayRunner was prompted for different types values than originally")
      .clone()
  }
}
