# A one-off script I used for doing some refactoring, committed for posterity

import re

with open ("./src/simulation_state/monsters.rs") as f:
    text = f.read()
    result = re.sub (r'''
impl MonsterBehavior for (\w+) {
  fn make_intent_distribution\(context: &mut IntentChoiceContext\) {
    (.*?)
  }(.*?)
  fn intent_effects\(context: &mut impl IntentEffectsContext\) {
    (.*?)
  }
}''',r'''
intent!{
  pub enum \1Intent {
  }
}
impl MonsterBehavior for \1 {
  type Intent = \1Intent;
  fn make_intent_distribution(context: &mut IntentChoiceContext) {
    use \1Intent::*;
    \2
  }\3
  fn intent_effects(context: &mut impl IntentEffectsContext) {
    use \1Intent::*;
    \4
  }
}''', text, 0, re.DOTALL)
with open ("./src/simulation_state/monsters.rs", "w") as f:
    f.write(result)