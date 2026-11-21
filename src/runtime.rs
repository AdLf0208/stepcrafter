//! Step Crafter's rules, pure: the maintenance band and one step of the crafting.
//!
//! Upstream (`StepCrafterNetworkNode.doWork`) walks every configured pattern slot and, for each,
//! steps the pattern while its output is short:
//!
//! ```text
//! if status != CRAFTING && stored >= min: break          // nothing to do
//! if status == CRAFTING && stored >= max: status = FINISHED; break
//! result = craftOne(pattern)                            // ingredients out, output in
//! if result is NOT_ENOUGH_INGREDIENTS or NETWORK_FULL: status = result; break
//! ```
//!
//! So the band is hysteretic: crafting **starts** below `min` and **stops** at `max`. That rule,
//! the ingredient resolution and the stock arithmetic are this module's — plain Rust with no
//! `mod_api` types, so `cargo test` exercises them on their own. The game-side half lives in
//! [`crate::machine`], [`crate::pattern`] and [`crate::commands`].

use std::collections::BTreeMap;

/// Steps one tick takes: upstream's `speed + 1` with no speed upgrade installed.
pub const STEPS_PER_TICK: u32 = 1;

/// The maintenance band of one configured pattern: start below `min`, stop at `max`.
///
/// Upstream's `ResourceMinMaxAmount` validates the same way: neither amount may be negative and a
/// band whose maximum is below its minimum cannot be built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Band {
    min: u64,
    max: u64,
}

impl Band {
    /// The band `min..=max`, or `None` when `max < min` (an inverted band maintains nothing).
    #[must_use]
    pub fn new(min: u64, max: u64) -> Option<Self> {
        (max >= min).then_some(Self { min, max })
    }

    /// The amount the stock must fall below before crafting starts.
    #[must_use]
    pub fn min(self) -> u64 {
        self.min
    }

    /// The amount the stock is filled up to once crafting has started.
    #[must_use]
    pub fn max(self) -> u64 {
        self.max
    }
}

/// Upstream's `ResourceStatus` as the machine keeps it between ticks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    /// Nothing is being crafted: the machine acts when the stock falls below `min`.
    #[default]
    Idle,
    /// Crafting is under way: the machine fills up to `max` even above `min`.
    Crafting,
    /// The last step had too few ingredients in stock.
    NotEnoughIngredients,
    /// The last step's output would not fit in the machine's inventory.
    NetworkFull,
}

impl Phase {
    /// The saved name, as [`Phase::of`] reads it back.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Crafting => "crafting",
            Self::NotEnoughIngredients => "not-enough-ingredients",
            Self::NetworkFull => "network-full",
        }
    }

    /// The phase a saved name stands for; an unknown name reads as [`Phase::Idle`].
    #[must_use]
    pub fn of(id: &str) -> Self {
        match id {
            "crafting" => Self::Crafting,
            "not-enough-ingredients" => Self::NotEnoughIngredients,
            "network-full" => Self::NetworkFull,
            _ => Self::Idle,
        }
    }

    /// Whether the block should show its `active` state.
    #[must_use]
    pub fn is_crafting(self) -> bool {
        matches!(self, Self::Crafting)
    }
}

/// What the band says to do, before the ingredients are looked at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The stock is inside the band and nothing is running.
    Hold,
    /// The stock reached `max`: the run is over and the phase returns to idle.
    Finish,
    /// Take one step of the pattern.
    Craft,
}

/// Upstream's three-line rule: start below `min`, keep going to `max`.
#[must_use]
pub fn decide(stored: u64, band: Band, phase: Phase) -> Step {
    // Upstream's two guards, in its order: a running fill (CRAFTING) is only stopped by `max`,
    // and an idle machine (anything else) only starts when the stock is below `min`.
    if phase == Phase::Crafting && stored >= band.max {
        return Step::Finish;
    }
    if phase != Phase::Crafting && stored >= band.min {
        return Step::Hold;
    }
    Step::Craft
}

/// The items the machine holds, by item id: its 27 slots, summed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stock {
    counts: BTreeMap<String, u64>,
}

impl Stock {
    /// An empty stock.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A stock holding these `(item, count)` entries, summed per item.
    #[must_use]
    pub fn of(entries: impl IntoIterator<Item = (String, u64)>) -> Self {
        let mut stock = Self::new();
        for (item, count) in entries {
            stock.add(&item, count);
        }
        stock
    }

    /// Add `count` of `item`; a zero count adds nothing and keeps no entry.
    pub fn add(&mut self, item: &str, count: u64) {
        if count == 0 {
            return;
        }
        *self.counts.entry(item.to_owned()).or_default() += count;
    }

    /// How much of `item` the stock holds.
    #[must_use]
    pub fn count(&self, item: &str) -> u64 {
        self.counts.get(item).copied().unwrap_or(0)
    }

    /// Take up to `count` of `item`; answers how many were actually taken.
    pub fn remove(&mut self, item: &str, count: u64) -> u64 {
        let Some(held) = self.counts.get_mut(item) else {
            return 0;
        };
        let taken = (*held).min(count);
        *held -= taken;
        if *held == 0 {
            self.counts.remove(item);
        }
        taken
    }

    /// Every held item and its count, in item-id order.
    pub fn entries(&self) -> impl Iterator<Item = (&str, u64)> {
        self.counts
            .iter()
            .map(|(item, count)| (item.as_str(), *count))
    }

    /// Whether nothing is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    /// How many items the stock holds in all, which is what the machine's capacity bounds.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }
}

/// One configured step pattern: what a craft consumes, and what it makes.
///
/// Upstream's pattern is a Refined Storage pattern item; here it is the target item's own crafting
/// recipe, resolved from the game's recipe table ([`crate::pattern`]). A cell keeps every item
/// that satisfies it (a tag expands to its members), and one item of the cell is consumed per step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern {
    cells: Vec<Vec<String>>,
    result: String,
    result_count: u32,
}

impl Pattern {
    /// A pattern of these cells, making `result_count` of `result`; `None` when a cell is empty or
    /// names no item, or when the result count is zero (such a pattern could never complete).
    #[must_use]
    pub fn new(cells: Vec<Vec<String>>, result: String, result_count: u32) -> Option<Self> {
        if cells.is_empty()
            || cells.iter().any(|cell| cell.is_empty())
            || result.is_empty()
            || result_count == 0
        {
            return None;
        }
        Some(Self {
            cells,
            result,
            result_count,
        })
    }

    /// The cells, each a list of the items that satisfy it.
    #[must_use]
    pub fn cells(&self) -> &[Vec<String>] {
        &self.cells
    }

    /// The item one step makes.
    #[must_use]
    pub fn result(&self) -> &str {
        &self.result
    }

    /// How many of [`Pattern::result`] one step makes.
    #[must_use]
    pub fn result_count(&self) -> u32 {
        self.result_count
    }

    /// The saved form: one comma-joined candidate list per cell.
    #[must_use]
    pub fn encode(&self) -> Vec<String> {
        self.cells
            .iter()
            .map(|cell| cell.join(","))
            .collect::<Vec<_>>()
    }

    /// A pattern from [`Pattern::encode`]'s form; `None` when it does not admit one ([`Pattern::new`]'s
    /// rules).
    #[must_use]
    pub fn decode(cells: &[String], result: &str, result_count: u32) -> Option<Self> {
        let cells = cells
            .iter()
            .map(|cell| {
                cell.split(',')
                    .map(str::trim)
                    .filter(|id| !id.is_empty())
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        Self::new(cells, result.to_owned(), result_count)
    }
}

/// What one maintenance step does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Inside the band, nothing running.
    Hold,
    /// The run reached `max`: the phase returns to idle.
    Finished,
    /// One step: consume these items, make `result_count` of `result`.
    Craft {
        consume: Vec<(String, u64)>,
        result: String,
        result_count: u32,
    },
    /// A cell of the pattern has no item in stock; the cell's candidates are named.
    NotEnoughIngredients { missing: Vec<String> },
    /// The output would not fit in the machine's inventory.
    NetworkFull {
        item: String,
        stored: u64,
        capacity: u64,
    },
}

/// Upstream's `craftOneItemNew`, as a pure decision: the band rule, then the ingredient
/// resolution, then the room for the output. The stock is only read.
#[must_use]
pub fn plan(
    stock: &Stock,
    band: Band,
    phase: Phase,
    pattern: Option<&Pattern>,
    capacity: u64,
) -> Outcome {
    // No pattern configured yet: upstream has nothing to maintain in that slot.
    let Some(pattern) = pattern else {
        return Outcome::Hold;
    };
    let stored = stock.count(pattern.result());
    match decide(stored, band, phase) {
        Step::Hold => Outcome::Hold,
        Step::Finish => Outcome::Finished,
        Step::Craft => craft(stock, pattern, stored, capacity),
    }
}

/// One step's ingredients out and result in, once the band said to craft:
/// upstream's `prepareCraft` (resolve every cell against the stock, reserving what earlier cells
/// took) and its `canInsertIntoNetwork` room check.
fn craft(stock: &Stock, pattern: &Pattern, stored: u64, capacity: u64) -> Outcome {
    let mut remaining = stock.clone();
    let mut consume: BTreeMap<String, u64> = BTreeMap::new();
    let mut missing = Vec::new();
    for cell in pattern.cells() {
        let held = cell.iter().find(|id| remaining.count(id) > 0);
        let Some(item) = held else {
            // The whole cell is named, so the readout says which alternative set was short.
            missing.push(cell.join(","));
            continue;
        };
        remaining.remove(item, 1);
        *consume.entry(item.clone()).or_default() += 1;
    }
    if !missing.is_empty() {
        return Outcome::NotEnoughIngredients { missing };
    }
    let produced = u64::from(pattern.result_count());
    if stock.total().saturating_add(produced) > capacity {
        return Outcome::NetworkFull {
            item: pattern.result().to_owned(),
            stored,
            capacity,
        };
    }
    Outcome::Craft {
        consume: consume.into_iter().collect(),
        result: pattern.result().to_owned(),
        result_count: pattern.result_count(),
    }
}

/// One step now, whatever the band says: the manual step (`/stepcrafter step`) and the harness
/// driver. The band is the machine's own timing rule; asking for a step is outside it.
#[must_use]
pub fn step_now(stock: &Stock, pattern: Option<&Pattern>, capacity: u64) -> Outcome {
    let Some(pattern) = pattern else {
        return Outcome::Hold;
    };
    let stored = stock.count(pattern.result());
    craft(stock, pattern, stored, capacity)
}

/// Apply [`Outcome::Craft`] to a stock; answers whether the stock changed. Every other outcome
/// leaves it as it was.
pub fn apply(stock: &mut Stock, outcome: &Outcome) -> bool {
    let Outcome::Craft {
        consume,
        result,
        result_count,
    } = outcome
    else {
        return false;
    };
    for (item, count) in consume {
        stock.remove(item, *count);
    }
    stock.add(result, u64::from(*result_count));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn band(min: u64, max: u64) -> Band {
        Band::new(min, max).expect("a valid band")
    }

    /// `minecraft:torch`: one `minecraft:coal` (or charcoal) and one `minecraft:stick`, making 4.
    fn torch() -> Pattern {
        Pattern::new(
            vec![
                vec!["minecraft:coal".to_owned(), "minecraft:charcoal".to_owned()],
                vec!["minecraft:stick".to_owned()],
            ],
            "minecraft:torch".to_owned(),
            4,
        )
        .expect("a valid pattern")
    }

    #[test]
    fn an_inverted_band_cannot_be_built() {
        assert!(Band::new(9, 4).is_none(), "max below min must be refused");
        assert!(Band::new(4, 4).is_some(), "an empty band is legal");
    }

    #[test]
    fn decide_holds_at_the_minimum_while_idle() {
        assert_eq!(decide(8, band(8, 32), Phase::Idle), Step::Hold);
        assert_eq!(decide(20, band(8, 32), Phase::Idle), Step::Hold);
    }

    #[test]
    fn decide_crafts_below_the_minimum() {
        assert_eq!(
            decide(7, band(8, 32), Phase::Idle),
            Step::Craft,
            "a stock below min must be crafted"
        );
    }

    #[test]
    fn decide_keeps_crafting_above_the_minimum_until_the_maximum() {
        // The hysteresis: once a run started, min no longer stops it.
        assert_eq!(
            decide(8, band(8, 32), Phase::Crafting),
            Step::Craft,
            "a running fill continues between min and max"
        );
        assert_eq!(decide(31, band(8, 32), Phase::Crafting), Step::Craft);
    }

    #[test]
    fn decide_finishes_at_the_maximum() {
        assert_eq!(decide(32, band(8, 32), Phase::Crafting), Step::Finish);
        assert_eq!(
            decide(8, band(8, 32), Phase::NotEnoughIngredients),
            Step::Hold,
            "a failed run that is back inside the band holds"
        );
    }

    #[test]
    fn a_step_consumes_every_cell_and_makes_the_result() {
        let stock = Stock::of([
            ("minecraft:coal".to_owned(), 2),
            ("minecraft:stick".to_owned(), 2),
        ]);
        let outcome = plan(&stock, band(4, 16), Phase::Idle, Some(&torch()), 1728);
        assert_eq!(
            outcome,
            Outcome::Craft {
                consume: vec![
                    ("minecraft:coal".to_owned(), 1),
                    ("minecraft:stick".to_owned(), 1),
                ],
                result: "minecraft:torch".to_owned(),
                result_count: 4,
            }
        );
    }

    #[test]
    fn a_step_prefers_the_first_candidate_the_stock_holds() {
        let stock = Stock::of([
            ("minecraft:charcoal".to_owned(), 1),
            ("minecraft:stick".to_owned(), 1),
        ]);
        let outcome = plan(&stock, band(4, 16), Phase::Idle, Some(&torch()), 1728);
        assert_eq!(
            outcome,
            Outcome::Craft {
                consume: vec![
                    ("minecraft:charcoal".to_owned(), 1),
                    ("minecraft:stick".to_owned(), 1),
                ],
                result: "minecraft:torch".to_owned(),
                result_count: 4,
            },
            "the alternative ingredient in stock is the one consumed"
        );
    }

    #[test]
    fn a_missing_ingredient_refuses_the_step_and_names_the_cell() {
        let stock = Stock::of([("minecraft:coal".to_owned(), 3)]);
        let before = stock.clone();
        let outcome = plan(&stock, band(4, 16), Phase::Idle, Some(&torch()), 1728);
        assert_eq!(
            outcome,
            Outcome::NotEnoughIngredients {
                missing: vec!["minecraft:stick".to_owned()],
            }
        );
        let mut after = before.clone();
        assert!(!apply(&mut after, &outcome), "a refusal changes no stock");
        assert_eq!(after, before);
    }

    #[test]
    fn a_full_inventory_refuses_the_step() {
        let stock = Stock::of([
            ("minecraft:coal".to_owned(), 1),
            ("minecraft:stick".to_owned(), 1),
            ("minecraft:torch".to_owned(), 1726),
        ]);
        // The band asks for more torches than the 27-slot inventory can hold: 1726 is below this
        // `min`, so the step is attempted, and its 4 torches would pass the capacity.
        let outcome = plan(&stock, band(1800, 2000), Phase::Idle, Some(&torch()), 1728);
        assert_eq!(
            outcome,
            Outcome::NetworkFull {
                item: "minecraft:torch".to_owned(),
                stored: 1726,
                capacity: 1728,
            },
            "4 torches do not fit beside 1726"
        );
        let mut after = stock.clone();
        assert!(!apply(&mut after, &outcome), "a refusal consumes nothing");
        assert_eq!(after, stock);
    }

    #[test]
    fn an_unconfigured_machine_holds() {
        let stock = Stock::of([("minecraft:coal".to_owned(), 9)]);
        assert_eq!(
            plan(&stock, band(8, 32), Phase::Idle, None, 1728),
            Outcome::Hold,
            "no pattern yet: nothing to maintain"
        );
    }

    #[test]
    fn apply_moves_the_stock_by_exactly_the_outcome() {
        let mut stock = Stock::of([
            ("minecraft:coal".to_owned(), 2),
            ("minecraft:stick".to_owned(), 1),
        ]);
        let outcome = plan(&stock, band(8, 16), Phase::Idle, Some(&torch()), 1728);
        assert!(apply(&mut stock, &outcome));
        assert_eq!(stock.count("minecraft:coal"), 1);
        assert_eq!(stock.count("minecraft:stick"), 0, "the stick was used up");
        assert_eq!(stock.count("minecraft:torch"), 4);
    }

    #[test]
    fn a_manual_step_crafts_even_inside_the_band() {
        // The band says hold at 8 (idle, at min); asking for a step still crafts one.
        let stock = Stock::of([
            ("minecraft:coal".to_owned(), 1),
            ("minecraft:stick".to_owned(), 1),
            ("minecraft:torch".to_owned(), 8),
        ]);
        assert_eq!(
            plan(&stock, band(8, 16), Phase::Idle, Some(&torch()), 1728),
            Outcome::Hold
        );
        assert_eq!(
            step_now(&stock, Some(&torch()), 1728),
            Outcome::Craft {
                consume: vec![
                    ("minecraft:coal".to_owned(), 1),
                    ("minecraft:stick".to_owned(), 1),
                ],
                result: "minecraft:torch".to_owned(),
                result_count: 4,
            }
        );
    }

    #[test]
    fn a_manual_step_still_needs_the_ingredients() {
        let stock = Stock::of([("minecraft:torch".to_owned(), 8)]);
        assert_eq!(
            step_now(&stock, Some(&torch()), 1728),
            Outcome::NotEnoughIngredients {
                missing: vec![
                    "minecraft:coal,minecraft:charcoal".to_owned(),
                    "minecraft:stick".to_owned(),
                ],
            }
        );
    }

    #[test]
    fn a_pattern_round_trips_through_its_saved_form() {
        let pattern = torch();
        let saved = pattern.encode();
        assert_eq!(
            saved,
            vec![
                "minecraft:coal,minecraft:charcoal".to_owned(),
                "minecraft:stick".to_owned()
            ]
        );
        assert_eq!(Pattern::decode(&saved, "minecraft:torch", 4), Some(pattern));
        assert!(Pattern::decode(&["".to_owned()], "minecraft:torch", 4).is_none());
        assert!(Pattern::decode(&saved, "minecraft:torch", 0).is_none());
    }

    #[test]
    fn phases_round_trip_through_their_saved_names() {
        for phase in [
            Phase::Idle,
            Phase::Crafting,
            Phase::NotEnoughIngredients,
            Phase::NetworkFull,
        ] {
            assert_eq!(Phase::of(phase.id()), phase);
        }
        assert_eq!(Phase::of("nonsense"), Phase::Idle);
    }
}
