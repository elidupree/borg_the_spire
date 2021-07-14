use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::rc::Rc;

pub struct AnalysisFlows {
  components: HashMap<String, AnalysisComponent>,
  values: HashMap<String, Rc<dyn Any>>,
}

impl AnalysisFlows {
  fn update_from_spec(&mut self, specs: &HashMap<String, AnalysisComponentSpec>) {
    for (name, spec) in specs {
      match self.components.entry(name.clone()) {
        Entry::Vacant(entry) => {
          entry.insert(AnalysisComponent::new(spec.clone()));
        }
        Entry::Occupied(mut entry) => {
          if entry.get().spec.kind != spec.kind {
            *entry.get_mut() = AnalysisComponent::new(spec.clone())
          } else {
            entry.get_mut().spec.time_share = spec.time_share;
          }
        }
      }
    }
  }
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct AnalysisComponentSpec {
  kind: AnalysisComponentKindSpec,
  time_share: f64,
}

#[derive(Debug)]
pub struct AnalysisComponent {
  spec: AnalysisComponentSpec,
  data: Box<dyn Any>,
}

pub struct AnalysisFlowContext {}
impl AnalysisFlowContext {
  pub fn get<T: Any>(&self, name: &str) -> Rc<T> {
    todo!()
  }
}

pub trait AnalysisComponentBehavior {
  type Data: Default;
  fn step(&self, context: &mut AnalysisFlowContext, data: &mut Self::Data);
}

macro_rules! analysis_components {
  ($($Variant: ident,)*) => {
    #[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
    pub enum AnalysisComponentKindSpec {
      $($Variant($Variant),)*
    }

    $(
    impl From<$Variant> for AnalysisComponentKindSpec {
      fn from (source: $Variant)->AnalysisComponentKindSpec {
        AnalysisComponentKindSpec::$Variant(source)
      }
    }
    )*

    impl AnalysisComponent {
      pub fn new(spec: AnalysisComponentSpec) -> AnalysisComponent {
        let data = match spec.kind {
          $(AnalysisComponentKindSpec::$Variant(_) => Box::new(<$Variant as AnalysisComponentBehavior>::Data::default()),)*
        };
        AnalysisComponent {
          spec,
          data,
        }
      }
      pub fn step(&mut self, context: &mut AnalysisFlowContext) {
        match &mut self.spec.kind {
          $(AnalysisComponentKindSpec::$Variant(v) => v.step(context, self.data.downcast_mut().unwrap()),)*
        }
      }
    }
  }
}

analysis_components! {
  CompareStartingPointsComponentSpec,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct CompareStartingPointsComponentSpec {}
#[derive(Debug, Default)]
pub struct CompareStartingPointsComponentData {}
impl AnalysisComponentBehavior for CompareStartingPointsComponentSpec {
  type Data = CompareStartingPointsComponentData;
  fn step(&self, context: &mut AnalysisFlowContext, data: &mut Self::Data) {}
}
