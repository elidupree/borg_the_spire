use crate::competing_optimizers::StrategyOptimizer;
use crate::representative_sampling::FractalRepresentativeSeedSearch;
use crate::seed_system::{SingleSeed, SingleSeedGenerator};
use crate::seeds_concrete::CombatChoiceLineagesKind;
use crate::simulation_state::CombatState;
use crate::start_and_strategy_ai;
use crate::start_and_strategy_ai::FastStrategy;
use crate::webserver::html_views::Element;
use ordered_float::OrderedFloat;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::time::Instant;
use typed_html::{html, text};

#[derive(Default)]
pub struct AnalysisFlows {
  starting_state: CombatState,
  components: HashMap<String, AnalysisComponent>,
  // values: HashMap<String, Rc<dyn Any>>,
}

impl AnalysisFlows {
  pub fn update_from_spec(&mut self, specs: &HashMap<String, AnalysisComponentSpec>) {
    for (name, spec) in specs {
      match self.components.entry(name.clone()) {
        Entry::Vacant(entry) => {
          entry.insert(AnalysisComponent::new(
            &AnalysisFlowContext {
              starting_state: &self.starting_state,
            },
            spec.clone(),
          ));
        }
        Entry::Occupied(mut entry) => {
          if entry.get().spec.kind != spec.kind {
            *entry.get_mut() = AnalysisComponent::new(
              &AnalysisFlowContext {
                starting_state: &self.starting_state,
              },
              spec.clone(),
            )
          } else {
            entry.get_mut().spec.time_share = spec.time_share;
          }
        }
      }
    }
  }
  pub fn step(&mut self) {
    let best_component = self
      .components
      .values_mut()
      .min_by_key(|component| OrderedFloat(component.time_share_used));
    if let Some(component) = best_component {
      let start = Instant::now();
      component.step(&mut AnalysisFlowContext {
        starting_state: &self.starting_state,
      });
      component.time_share_used += start.elapsed().as_secs_f64() / component.spec.time_share;
    }
  }
  pub fn html_report(&self) -> Element {
    let reports = self.components.iter().map(|(name, component)| {
      let report = component.html_report(&AnalysisFlowContext {
        starting_state: &self.starting_state,
      });
      html! {
        <div class="analysis-component">
          <div class="analysis-component-name">{text!{name}}</div>
          <div class="analysis-component-report">{report}</div>
        </div>
      }
    });
    html! {
      <div class="analysis-components">
        {reports}
      </div>
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
  time_share_used: f64,
}

pub struct AnalysisFlowContext<'a> {
  starting_state: &'a CombatState,
}
impl<'a> AnalysisFlowContext<'a> {
  pub fn starting_state(&self) -> &'a CombatState {
    self.starting_state
  }
  // pub fn get<T: Any>(&self, name: &str) -> Rc<T> {
  //   todo!()
  // }
}

pub trait AnalysisComponentBehavior {
  type Data;
  fn initial_data(&self, context: &AnalysisFlowContext) -> Self::Data;
  fn step(&self, context: &mut AnalysisFlowContext, data: &mut Self::Data);
  fn html_report(&self, context: &AnalysisFlowContext, data: &Self::Data) -> Option<Element>;
}

macro_rules! analysis_components {
  ($($Variant: ident($SpecStruct: ident),)*) => {
    #[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
    pub enum AnalysisComponentKindSpec {
      $($Variant($SpecStruct),)*
    }

    $(
    impl From<$SpecStruct> for AnalysisComponentKindSpec {
      fn from (source: $SpecStruct)->AnalysisComponentKindSpec {
        AnalysisComponentKindSpec::$Variant(source)
      }
    }
    )*

    impl AnalysisComponent {
      pub fn new(context: &AnalysisFlowContext, spec: AnalysisComponentSpec) -> AnalysisComponent {
        let data: Box<dyn Any> = match &spec.kind {
          $(AnalysisComponentKindSpec::$Variant(v) => Box::new(v.initial_data(context)),)*
        };
        AnalysisComponent {
          spec,
          data,
          time_share_used: 0.0,
        }
      }
      pub fn step(&mut self, context: &mut AnalysisFlowContext) {
        match &mut self.spec.kind {
          $(AnalysisComponentKindSpec::$Variant(v) => v.step(context, self.data.downcast_mut().unwrap()),)*
        }
      }
      pub fn html_report(&self, context: &AnalysisFlowContext) -> Option<Element> {
        match &self.spec.kind {
          $(AnalysisComponentKindSpec::$Variant(v) => v.html_report(context, self.data.downcast_ref().unwrap()),)*
        }
      }
    }
  }
}

analysis_components! {
  CompareStartingPoints(CompareStartingPointsComponentSpec),
  FractalRepresentativeSeedSearch(FractalRepresentativeSeedSearchComponentSpec),
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct CompareStartingPointsComponentSpec {}
#[derive(Debug)]
pub struct CompareStartingPointsComponentData {
  search: start_and_strategy_ai::SearchState,
}
impl AnalysisComponentBehavior for CompareStartingPointsComponentSpec {
  type Data = CompareStartingPointsComponentData;
  fn initial_data(&self, context: &AnalysisFlowContext) -> Self::Data {
    CompareStartingPointsComponentData {
      search: start_and_strategy_ai::SearchState::new(context.starting_state().clone()),
    }
  }
  fn step(&self, _context: &mut AnalysisFlowContext, _data: &mut Self::Data) {}

  fn html_report(&self, _context: &AnalysisFlowContext, data: &Self::Data) -> Option<Element> {
    Some(data.search.view())
  }
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct FractalRepresentativeSeedSearchComponentSpec {}
pub struct FractalRepresentativeSeedSearchComponentData {
  search: FractalRepresentativeSeedSearch<
    FastStrategy,
    SingleSeed<CombatChoiceLineagesKind>,
    SingleSeedGenerator,
  >,
}

impl AnalysisComponentBehavior for FractalRepresentativeSeedSearchComponentSpec {
  type Data = FractalRepresentativeSeedSearchComponentData;

  fn initial_data(&self, context: &AnalysisFlowContext) -> Self::Data {
    FractalRepresentativeSeedSearchComponentData {
      search: FractalRepresentativeSeedSearch::new(
        context.starting_state(),
        SingleSeedGenerator::new(ChaCha8Rng::from_entropy()),
        // TODO: don't duplicate this from competing_optimizers.rs, probably use a generalization
        // like StrategyAndGeneratorSpecification
        Box::new(|candidates: &[&FastStrategy]| {
          if candidates.len() < 2 {
            FastStrategy::random(&mut rand::thread_rng())
          } else {
            FastStrategy::offspring(
              &candidates
                .choose_multiple(&mut rand::thread_rng(), 2)
                .copied()
                .collect::<Vec<_>>(),
              &mut rand::thread_rng(),
            )
          }
        }),
      ),
    }
  }

  fn step(&self, context: &mut AnalysisFlowContext, data: &mut Self::Data) {
    data
      .search
      .step(context.starting_state(), &mut ChaCha8Rng::seed_from_u64(0));
  }

  fn html_report(&self, _context: &AnalysisFlowContext, _data: &Self::Data) -> Option<Element> {
    unimplemented!()
  }
}
