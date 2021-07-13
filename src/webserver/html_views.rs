use rand::SeedableRng;
use rand_pcg::Pcg64Mcg;
use typed_html::dom::DOMTree;
use typed_html::elements::FlowContent;
use typed_html::{html, text};

use crate::seed_system::TrivialSeed;
use crate::simulation::{Runner, StandardRunner};
use crate::simulation_state::CombatState;
use crate::start_and_strategy_ai::SearchState;
use crate::webserver::ServerState;

pub type Element = Box<dyn FlowContent<String>>;

impl ServerState {
  pub fn view(&self) -> DOMTree<String> {
    let state_representation = self
      .search_state
      .as_ref()
      .map(|search_state| search_state.view());
    html! {
      <div id="content">
        {state_representation}
        <pre>{text! (&self.debug_log)}</pre>
      </div>
    }
  }
}

impl CombatState {
  pub fn view(&self) -> Element {
    let monsters = self
      .monsters
      .iter()
      .filter(|monster| !monster.gone)
      .map(|monster| {
        html! {
          <div class="monster">
            {text! ("{}", monster)}
          </div>
        }
      });
    let hand = self.hand.iter().map(|card| {
      html! {
        <div class="card">
          {text! ("{}", card)}
        </div>
      }
    });
    html! {
      <div class="combat-state">
        <div class="player">
          {text! ("{}", self.player)}
        </div>
        <div class="monsters">
          {monsters}
        </div>
        <div class="hand">
          {hand}
        </div>
      </div>
    }
  }
}

impl SearchState {
  pub fn view(&self) -> Element {
    let starting_points = self.starting_points.iter().map(|start| {
      let scores = start.candidate_strategies.iter().map(|strategy| {
        {
          text!(
            "average score {:.6} ({} visits)",
            strategy.total_score / strategy.visits as f64,
            strategy.visits
          )
        }
      });
      let mut hypothetical_evaluated_state = start.state.clone();
      //let next_turn = start.state.turn_number + 1;
      let mut runner = StandardRunner::new(
        &mut hypothetical_evaluated_state,
        TrivialSeed::new(Pcg64Mcg::from_entropy()),
      );
      runner.run_until_unable();
      //let log = runner.debug_log().to_string();
      html! {
        <div class="starting-point">
          <div class="starting-point-heading">
            {text! ("{} visits\n{:?}", start.visits, start.choices)}
            {start.state.view()}
            {hypothetical_evaluated_state.view()}
            //<pre>{text! (log)}</pre>
          </div>
          <div class="strategies">
            {scores}
          </div>
        </div>
      }
    });

    html! {
      <div class="search-state">
        <div class="search-state-heading">
          {text! ("{} visits", self.visits)}
        </div>
        <div class="starting-points">
          {starting_points}
        </div>
      </div>
    }
  }
}
