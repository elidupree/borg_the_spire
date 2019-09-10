use std::sync::Arc;
use std::ops::{Deref, DerefMut};
use std::iter::FromIterator;
use serde::{Serialize, Deserialize};

/// Currently this is a thin wrapper around Arc, using make_mut() to be a clone-on-write pointer.
///
/// One purpose of not just using Arc is to optimize eq() by automatically comparing equal if the address does
#[derive (Clone, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Debug)]
#[serde (transparent)]
pub struct Cow <T> {
  data: Arc<T>,
}

impl <T: PartialEq> PartialEq<Cow<T>> for Cow<T> {
  fn eq (&self, other: & Self)->bool {
    self.same_identity(other) && self.data == other.data
  }
}

impl <T> Cow <T> {
  pub fn new (inner: T)->Self {
    Cow {
      data: Arc::new (inner)
    }
  }
  
  pub fn same_identity (&self, other: & Self)->bool {
    &*self.data as *const T == &*other.data as *const T
  }
}

impl <T> Deref for Cow <T> {
  type Target = T;
  fn deref (&self)->& T {&*self.data}
}

impl <T: Clone> DerefMut for Cow <T> {
  fn deref_mut (&mut self)->&mut T {Arc::make_mut(&mut self.data)}
}
