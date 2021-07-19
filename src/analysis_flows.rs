use crate::ai_utils::starting_choices_made_by_strategy;
use crate::competing_optimizers::StrategyOptimizer;
use crate::condition_strategy::ConditionStrategy;
use crate::representative_sampling::FractalRepresentativeSeedSearch;
use crate::seed_system::{SingleSeed, SingleSeedGenerator};
use crate::seeds_concrete::CombatChoiceLineagesKind;
use crate::simulation::DisplayChoices;
use crate::simulation_state::CombatState;
use crate::start_and_strategy_ai;
use crate::webserver::html_views::Element;
use ordered_float::OrderedFloat;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use typed_html::{html, text};

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug, Default)]
pub struct AnalysisFlowsSpec {
  pub components: Vec<(String, AnalysisComponentSpec)>,
}

#[derive(Default)]
pub struct AnalysisFlows {
  pub starting_state: CombatState,
  pub components: Vec<(String, AnalysisComponent)>,
  pub time_used: Duration,
  pub time_used_for_rendering: Duration,
  // values: HashMap<String, Rc<dyn Any>>,
}

impl AnalysisFlows {
  pub fn new(spec: &AnalysisFlowsSpec, starting_state: CombatState) -> AnalysisFlows {
    let components = spec
      .components
      .iter()
      .map(|(name, spec)| {
        (
          name.clone(),
          AnalysisComponent::new(
            &AnalysisFlowContext {
              starting_state: &starting_state,
            },
            spec.clone(),
          ),
        )
      })
      .collect();
    AnalysisFlows {
      starting_state,
      components,
      time_used: Duration::from_secs(0),
      time_used_for_rendering: Duration::from_secs(0),
    }
  }
  pub fn update_from_spec(&mut self, spec: &AnalysisFlowsSpec) {
    let mut old_components: HashMap<String, _> = self.components.drain(..).collect();
    let new_components = spec
      .components
      .iter()
      .map(|(name, spec)| {
        (
          name.clone(),
          match old_components.remove(name) {
            None => AnalysisComponent::new(
              &AnalysisFlowContext {
                starting_state: &self.starting_state,
              },
              spec.clone(),
            ),
            Some(mut old_component) => {
              if old_component.spec.kind != spec.kind {
                AnalysisComponent::new(
                  &AnalysisFlowContext {
                    starting_state: &self.starting_state,
                  },
                  spec.clone(),
                )
              } else {
                old_component.spec.time_share = spec.time_share;
                old_component
              }
            }
          },
        )
      })
      .collect();
    self.components = new_components;
  }
  pub fn step(&mut self) {
    let best_component = self
      .components
      .iter_mut()
      .min_by_key(|(_, component)| OrderedFloat(component.time_share_used));
    if let Some((_, component)) = best_component {
      let start = Instant::now();
      component.step(&mut AnalysisFlowContext {
        starting_state: &self.starting_state,
      });
      let duration = start.elapsed();
      self.time_used += duration;
      component.time_used += duration;
      component.time_share_used += duration.as_secs_f64() / component.spec.time_share;
    }
  }
  pub fn html_report(&mut self) -> Element {
    let start = Instant::now();
    let reports = self.components.iter().map(|(name, component)| {
      let report = component.html_report(&AnalysisFlowContext {
        starting_state: &self.starting_state,
      });
      html! {
        <div class="analysis-component">
          <div class="analysis-component-name">{text!{"{} ({:.1}/{:.1}s)",name, component.time_used.as_secs_f64(), component.when_started.elapsed().as_secs_f64()}}</div>
          <div class="analysis-component-report">{report}</div>
        </div>
      }
    });
    let result = html! {
      <div class="analysis-components">
        {reports}
      </div>
    };
    let duration = start.elapsed();
    self.time_used_for_rendering += duration;
    result
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
  when_started: Instant,
  time_used: Duration,
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
          when_started: Instant::now(),
          time_used: Duration::from_secs(0),
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
  fn step(&self, _context: &mut AnalysisFlowContext, data: &mut Self::Data) {
    if data.search.visits < 2_000_000_000 {
      data.search.search_step();
    }
  }

  fn html_report(&self, _context: &AnalysisFlowContext, data: &Self::Data) -> Option<Element> {
    Some(data.search.view())
  }
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct FractalRepresentativeSeedSearchComponentSpec {}
pub struct FractalRepresentativeSeedSearchComponentData {
  search: FractalRepresentativeSeedSearch<
    ConditionStrategy,
    SingleSeed<CombatChoiceLineagesKind>,
    SingleSeedGenerator,
  >,
}

impl AnalysisComponentBehavior for FractalRepresentativeSeedSearchComponentSpec {
  type Data = FractalRepresentativeSeedSearchComponentData;

  fn initial_data(&self, context: &AnalysisFlowContext) -> Self::Data {
    let state = context.starting_state.clone();
    FractalRepresentativeSeedSearchComponentData {
      search: FractalRepresentativeSeedSearch::new(
        context.starting_state(),
        SingleSeedGenerator::new(ChaCha8Rng::from_entropy()),
        // TODO: don't duplicate this from competing_optimizers.rs, probably use a generalization
        // like StrategyAndGeneratorSpecification
        Box::new(move |_candidates: &[&ConditionStrategy]| {
          ConditionStrategy::fresh_distinctive_candidate(&state, &mut rand::thread_rng())
          // if candidates.len() < 2 || (rand::random::<f64>() < 0.25) {
          //   FastStrategy::random(&mut rand::thread_rng())
          // } else {
          //   FastStrategy::offspring(
          //     &candidates
          //       .choose_multiple(&mut rand::thread_rng(), 2)
          //       .copied()
          //       .collect::<Vec<_>>(),
          //     &mut rand::thread_rng(),
          //   )
          // }
        }),
      ),
    }
  }

  fn step(&self, context: &mut AnalysisFlowContext, data: &mut Self::Data) {
    data
      .search
      .step(context.starting_state(), &mut ChaCha8Rng::seed_from_u64(0));
  }

  fn html_report(&self, context: &AnalysisFlowContext, data: &Self::Data) -> Option<Element> {
    let mut elements = Vec::new();
    for layer in &data.search.layers {
      let strategies: Vec<_> = layer.strategies().collect();
      let scores = strategies
        .iter()
        .map(|s| format!("{:.3}", s.average))
        .collect::<Vec<_>>();
      let score_with_exploiting = (0..layer.seeds.len())
        .map(|index| {
          strategies
            .iter()
            .map(|s| s.scores[index])
            .max_by_key(|&f| OrderedFloat(f))
            .unwrap()
        })
        .sum::<f64>()
        / (layer.seeds.len() as f64);
      elements.push(html! {
        <div class="fractal_report_row">
          {text!(
            "{}: [{:.3}] {}",
            layer.seeds.len(),
            score_with_exploiting,
            scores.join(", ")
          )}
        </div>
      });
    }
    let mut all_starting_choices: Vec<Vec<_>> = data
      .search
      .layers
      .last()
      .unwrap()
      .strategies()
      .map(|s| starting_choices_made_by_strategy(context.starting_state(), &*s.strategy))
      .collect();
    all_starting_choices.sort();
    all_starting_choices.dedup();
    let all_starting_choices = all_starting_choices
      .into_iter()
      .map(|c| DisplayChoices(&c).to_string())
      .collect::<Vec<_>>()
      .join(", ");
    let meta_starting_choices =
      starting_choices_made_by_strategy(context.starting_state(), &data.search.meta_strategy());
    Some(html! {
      <div class="fractal_report">
        {elements}
        <div class="fractal_report_row">
          {text!(
            "All starting choices: {}",
            all_starting_choices
          )}
        </div>
        <div class="fractal_report_row">
          {text!(
            "Metastrategy starting choices: {}",
            DisplayChoices(&meta_starting_choices)
          )}
        </div>
      </div>
    })
  }
}
