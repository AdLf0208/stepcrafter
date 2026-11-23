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

    /// How many items the stock holds in all (the readout's `items=`).
    #[must_use]
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }
}

/// One occupied slot of the machine's inventory: what it holds, how many, and how many fit.
///
/// The game bounds a slot by the item's own stack limit (`max_stack_size`), so the room a step's
/// result has is a **per-slot** quantity: a full slot of coal leaves no room for a torch, even
/// though the inventory's item count says there is space. That is what [`Slots::room_for`] measures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    item: String,
    count: u32,
    max: u32,
}

impl Slot {
    /// A slot holding `count` of `item`, which fits up to `max`; `None` for an empty item id, a
    /// zero count or a slot that can hold nothing.
    #[must_use]
    pub fn new(item: impl Into<String>, count: u32, max: u32) -> Option<Self> {
        let item = item.into();
        (count > 0 && max > 0 && !item.is_empty()).then_some(Self { item, count, max })
    }

    /// The item this slot holds.
    #[must_use]
    pub fn item(&self) -> &str {
        &self.item
    }

    /// How many of it this slot holds.
    #[must_use]
    pub fn count(&self) -> u32 {
        self.count
    }

    /// How many of it this slot can hold.
    #[must_use]
    pub fn max(&self) -> u32 {
        self.max
    }
}

/// The machine's inventory as the rules see it: one entry per slot, in slot order.
///
/// `empty_room` is the room an **empty** slot offers for the step's result — the result item's
/// stack limit, which only the game knows. A slot that already holds something carries its own
/// limit in its [`Slot`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slots {
    slots: Vec<Option<Slot>>,
    empty_room: u32,
}

impl Slots {
    /// No slots yet; an empty slot will hold up to `empty_room` of the result item.
    #[must_use]
    pub fn new(empty_room: u32) -> Self {
        Self {
            slots: Vec::new(),
            empty_room,
        }
    }

    /// Add one slot, empty (`None`) or holding `slot`.
    pub fn push(&mut self, slot: Option<Slot>) {
        self.slots.push(slot);
    }

    /// The slots, in order.
    #[must_use]
    pub fn slots(&self) -> &[Option<Slot>] {
        &self.slots
    }

    /// How many of `item` the slots hold in all.
    #[must_use]
    pub fn count(&self, item: &str) -> u64 {
        self.slots
            .iter()
            .flatten()
            .filter(|slot| slot.item == item)
            .map(|slot| u64::from(slot.count))
            .sum()
    }

    /// The slots as the counted [`Stock`].
    #[must_use]
    pub fn stock(&self) -> Stock {
        let mut stock = Stock::new();
        for slot in self.slots.iter().flatten() {
            stock.add(&slot.item, u64::from(slot.count));
        }
        stock
    }

    /// Take up to `count` of `item` out of the slots, first slot first, as the machine's own
    /// `take_from_slots` does; answers how many were taken.
    pub fn take(&mut self, item: &str, count: u64) -> u64 {
        let mut left = count;
        for cell in &mut self.slots {
            if left == 0 {
                break;
            }
            let Some(slot) = cell.as_mut() else {
                continue;
            };
            if slot.item != item {
                continue;
            }
            let taken = u64::from(slot.count).min(left);
            slot.count -= u32::try_from(taken).unwrap_or(slot.count);
            left -= taken;
            if slot.count == 0 {
                *cell = None;
            }
        }
        count - left
    }

    /// Put up to `count` of `item` into the slots: the slots already holding it first, then empty
    /// slots, each up to its own limit. Answers how many went in.
    pub fn put(&mut self, item: &str, count: u32) -> u32 {
        let mut left = count;
        for slot in self.slots.iter_mut().flatten() {
            if left == 0 {
                break;
            }
            if slot.item != item {
                continue;
            }
            let added = slot.max.saturating_sub(slot.count).min(left);
            slot.count += added;
            left -= added;
        }
        for cell in &mut self.slots {
            if left == 0 {
                break;
            }
            if cell.is_some() {
                continue;
            }
            let added = left.min(self.empty_room);
            if added > 0 {
                *cell = Slot::new(item, added, self.empty_room);
                left -= added;
            }
        }
        count - left
    }

    /// How many of `item` the slots can still hold: the room left in the slots already holding it
    /// plus one full empty slot's worth per empty slot. Any other item's free room is not room for
    /// this one.
    #[must_use]
    pub fn room_for(&self, item: &str) -> u64 {
        self.slots
            .iter()
            .map(|slot| match slot {
                None => u64::from(self.empty_room),
                Some(slot) if slot.item == item => u64::from(slot.max.saturating_sub(slot.count)),
                Some(_) => 0,
            })
            .sum()
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
    /// The result has no room in the slots the ingredients would leave free.
    NetworkFull {
        item: String,
        /// How many of the result those slots can still hold.
        room: u64,
        /// How many the step makes.
        produced: u32,
    },
}

/// Upstream's `craftOneItemNew`, as a pure decision: the band rule, then the ingredient
/// resolution, then the room for the output. The slots are only read.
#[must_use]
pub fn plan(slots: &Slots, band: Band, phase: Phase, pattern: Option<&Pattern>) -> Outcome {
    // No pattern configured yet: upstream has nothing to maintain in that slot.
    let Some(pattern) = pattern else {
        return Outcome::Hold;
    };
    let stored = slots.count(pattern.result());
    match decide(stored, band, phase) {
        Step::Hold => Outcome::Hold,
        Step::Finish => Outcome::Finished,
        Step::Craft => craft(slots, pattern),
    }
}

/// One step's ingredients out and result in, once the band said to craft:
/// upstream's `prepareCraft` (resolve every cell against the stock, reserving what earlier cells
/// took) and its `canInsertIntoNetwork` room check.
fn craft(slots: &Slots, pattern: &Pattern) -> Outcome {
    let mut remaining = slots.stock();
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
    let produced = pattern.result_count();
    // The room is measured **after** the ingredients are taken: consuming the last item of a slot
    // frees that slot, and only the slots that hold the result (or are empty) have room for it.
    let mut after = slots.clone();
    for (item, count) in &consume {
        after.take(item, *count);
    }
    let room = after.room_for(pattern.result());
    if u64::from(produced) > room {
        return Outcome::NetworkFull {
            item: pattern.result().to_owned(),
            room,
            produced,
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
pub fn step_now(slots: &Slots, pattern: Option<&Pattern>) -> Outcome {
    let Some(pattern) = pattern else {
        return Outcome::Hold;
    };
    craft(slots, pattern)
}

/// One filter entry of a Step Requester: upstream's `ResourceMinMaxAmount` — the resource to keep,
/// its band and the largest amount one request may ask for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filter {
    item: String,
    band: Band,
    batch: u64,
}

impl Filter {
    /// The filter for `batch` of `item` kept between the band's amounts, or `None` for an empty
    /// item id (a filter with nothing to maintain).
    #[must_use]
    pub fn new(item: String, band: Band, batch: u64) -> Option<Self> {
        (!item.is_empty()).then_some(Self { item, band, batch })
    }

    /// The item the filter maintains.
    #[must_use]
    pub fn item(&self) -> &str {
        &self.item
    }

    /// The band the filtered item is kept between.
    #[must_use]
    pub fn band(&self) -> Band {
        self.band
    }

    /// The most a single request asks for; `0` disables the filter, as upstream's `batchSize == 0`
    /// skips its slot.
    #[must_use]
    pub fn batch(&self) -> u64 {
        self.batch
    }
}

/// Ticks a failed request is not retried, upstream's `FAILED_TASK_TIMEOUT_TICKS`.
pub const FAILED_TASK_TIMEOUT_TICKS: u32 = 20;

/// What a Step Requester's filter says to do this tick, upstream's `StepRequesterNetworkNode.doWork`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ask {
    /// The stock is inside the band and nothing is running.
    Hold,
    /// A running fill reached `max`: the phase returns to idle.
    Finish,
    /// A failed request is timing out; ask nothing this tick.
    Backoff,
    /// A request is already outstanding with the crafter; do not ask again.
    Waiting,
    /// Ask a crafter for this many items.
    Request { amount: u64 },
}

/// The requester's rule: start asking below `min`, keep asking to `max`, one batch at a time.
///
/// `stored` is how much of the filter's item the linked crafter holds; `outstanding` is how many
/// items it has already been asked for and not yet made; `backoff` is how many of the 20 failed-task
/// ticks are left.
#[must_use]
pub fn ask(stored: u64, filter: &Filter, phase: Phase, outstanding: u64, backoff: u32) -> Ask {
    // Upstream's guards, in its order: an idle filter only starts below `min`, a running fill is
    // only stopped by `max`, and a finished or disabled entry asks nothing.
    if phase != Phase::Crafting && stored >= filter.band.min {
        return Ask::Hold;
    }
    if phase == Phase::Crafting && stored >= filter.band.max {
        return Ask::Finish;
    }
    if backoff > 0 {
        return Ask::Backoff;
    }
    if outstanding > 0 {
        return Ask::Waiting;
    }
    let needed = filter.band.max.saturating_sub(stored);
    if filter.batch == 0 {
        // Upstream skips a filter slot whose batch size is 0: the entry is configured but off.
        return Ask::Hold;
    }
    Ask::Request {
        amount: needed.min(filter.batch),
    }
}

/// Apply [`Outcome::Craft`] to the slots; answers whether they changed. Every other outcome
/// leaves them as they were.
pub fn apply(slots: &mut Slots, outcome: &Outcome) -> bool {
    let Outcome::Craft {
        consume,
        result,
        result_count,
    } = outcome
    else {
        return false;
    };
    for (item, count) in consume {
        slots.take(item, *count);
    }
    slots.put(result, *result_count);
    true
}

/// Whether a crafter's own phase is a refusal the requester must report, and which one.
///
/// Upstream learns this from the failed task (`startTask` answering empty, or its timeout); here the
/// crafter states it in its saved `Phase`, and the stock still being short of `max` is what says the
/// run is unfinished. `None` means "nothing was refused".
#[must_use]
pub fn refused(crafter: Phase, stored: u64, filter: &Filter) -> Option<Phase> {
    match crafter {
        Phase::NotEnoughIngredients | Phase::NetworkFull if stored < filter.band.max => {
            Some(crafter)
        }
        _ => None,
    }
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

    /// A machine's 27 slots from `(item, count)` pairs — one slot each, 64 to a stack, the rest
    /// empty — so the room an empty slot offers is the same 64.
    fn slots(entries: &[(&str, u32)]) -> Slots {
        let mut slots = Slots::new(64);
        for (item, count) in entries {
            slots.push(Slot::new(*item, *count, 64));
        }
        while slots.slots().len() < 27 {
            slots.push(None);
        }
        slots
    }

    /// A machine whose every slot holds `64` of `(item, count)`'s items, in order, with no empty
    /// slot left.
    fn full_slots(entries: &[(&str, u32)]) -> Slots {
        let mut slots = Slots::new(64);
        for (item, count) in entries {
            slots.push(Slot::new(*item, *count, 64));
        }
        slots
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
        let slots = slots(&[("minecraft:coal", 2), ("minecraft:stick", 2)]);
        let outcome = plan(&slots, band(4, 16), Phase::Idle, Some(&torch()));
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
        let slots = slots(&[("minecraft:charcoal", 1), ("minecraft:stick", 1)]);
        let outcome = plan(&slots, band(4, 16), Phase::Idle, Some(&torch()));
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
        let slots = slots(&[("minecraft:coal", 3)]);
        let before = slots.clone();
        let outcome = plan(&slots, band(4, 16), Phase::Idle, Some(&torch()));
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
        // Every slot holds 64: one coal, one stick and 25 slots of coal, so the step's ingredients
        // are there but nothing is empty and no slot holds a torch.
        let mut slots = Slots::new(64);
        slots.push(Slot::new("minecraft:coal", 64, 64));
        slots.push(Slot::new("minecraft:stick", 64, 64));
        for _ in 0..25 {
            slots.push(Slot::new("minecraft:coal", 64, 64));
        }
        let before = slots.clone();
        // The band asks for torches and the stock is short, so the step is attempted.
        let outcome = plan(&slots, band(1800, 2000), Phase::Idle, Some(&torch()));
        assert_eq!(
            outcome,
            Outcome::NetworkFull {
                item: "minecraft:torch".to_owned(),
                room: 0,
                produced: 4,
            },
            "no slot has room for the 4 torches"
        );
        let mut after = before.clone();
        assert!(!apply(&mut after, &outcome), "a refusal consumes nothing");
        assert_eq!(after, before);
    }

    /// The review's defect: the whole inventory's item count must not stand in for the room the
    /// result actually has. 26 slots of 64 coal and one of 60 sticks hold 1,724 items, so an
    /// aggregate budget of 1,728 says 4 torches fit; no slot is empty and none holds a torch, so
    /// the result has **no** room and the step must not consume the coal and stick it selected.
    #[test]
    fn a_result_that_fits_no_slot_is_refused_even_when_the_counts_add_up() {
        let mut slots = Slots::new(64);
        for _ in 0..26 {
            slots.push(Slot::new("minecraft:coal", 64, 64));
        }
        slots.push(Slot::new("minecraft:stick", 60, 64));
        let before = slots.clone();
        let outcome = plan(&slots, band(4, 16), Phase::Idle, Some(&torch()));
        assert_eq!(
            outcome,
            Outcome::NetworkFull {
                item: "minecraft:torch".to_owned(),
                room: 0,
                produced: 4,
            },
            "1,724 items in 27 full-ish slots leave no room for 4 torches"
        );
        let mut after = before.clone();
        assert!(!apply(&mut after, &outcome), "a refusal consumes nothing");
        assert_eq!(
            after, before,
            "the ingredients stay in the machine when the result does not fit"
        );
    }

    #[test]
    fn a_slot_holding_the_result_offers_only_its_own_free_room() {
        // Three full-ish slots and no empty one: the torch stack allows 1 more (64 - 63), the
        // ingredient slots keep their leftovers, so 4 torches do not fit.
        let slots = full_slots(&[
            ("minecraft:torch", 63),
            ("minecraft:coal", 2),
            ("minecraft:stick", 2),
        ]);
        assert_eq!(
            step_now(&slots, Some(&torch())),
            Outcome::NetworkFull {
                item: "minecraft:torch".to_owned(),
                room: 1,
                produced: 4,
            }
        );
        // Four of room inside the torch's own slot is enough, with no empty slot at all.
        let slots = full_slots(&[
            ("minecraft:torch", 60),
            ("minecraft:coal", 2),
            ("minecraft:stick", 2),
        ]);
        assert_eq!(
            step_now(&slots, Some(&torch())),
            Outcome::Craft {
                consume: vec![
                    ("minecraft:coal".to_owned(), 1),
                    ("minecraft:stick".to_owned(), 1),
                ],
                result: "minecraft:torch".to_owned(),
                result_count: 4,
            },
            "the free room in the torch's own slot holds the step"
        );
    }

    #[test]
    fn consuming_the_last_item_of_a_slot_frees_that_slot_for_the_result() {
        // 26 slots of coal and one stick: the step empties the stick's slot, and that slot's room
        // (64) is what holds the 4 torches — the room is measured after the ingredients are taken.
        let mut slots = Slots::new(64);
        for _ in 0..26 {
            slots.push(Slot::new("minecraft:coal", 64, 64));
        }
        slots.push(Slot::new("minecraft:stick", 1, 64));
        assert_eq!(
            plan(&slots, band(4, 16), Phase::Idle, Some(&torch())),
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
    fn an_unconfigured_machine_holds() {
        let slots = slots(&[("minecraft:coal", 9)]);
        assert_eq!(
            plan(&slots, band(8, 32), Phase::Idle, None),
            Outcome::Hold,
            "no pattern yet: nothing to maintain"
        );
    }

    #[test]
    fn apply_moves_the_stock_by_exactly_the_outcome() {
        let mut slots = slots(&[("minecraft:coal", 2), ("minecraft:stick", 1)]);
        let outcome = plan(&slots, band(8, 16), Phase::Idle, Some(&torch()));
        assert!(apply(&mut slots, &outcome));
        assert_eq!(slots.count("minecraft:coal"), 1);
        assert_eq!(slots.count("minecraft:stick"), 0, "the stick was used up");
        assert_eq!(slots.count("minecraft:torch"), 4);
    }

    #[test]
    fn a_manual_step_crafts_even_inside_the_band() {
        // The band says hold at 8 (idle, at min); asking for a step still crafts one.
        let slots = slots(&[
            ("minecraft:coal", 1),
            ("minecraft:stick", 1),
            ("minecraft:torch", 8),
        ]);
        assert_eq!(
            plan(&slots, band(8, 16), Phase::Idle, Some(&torch())),
            Outcome::Hold
        );
        assert_eq!(
            step_now(&slots, Some(&torch())),
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
        let slots = slots(&[("minecraft:torch", 8)]);
        assert_eq!(
            step_now(&slots, Some(&torch())),
            Outcome::NotEnoughIngredients {
                missing: vec![
                    "minecraft:coal,minecraft:charcoal".to_owned(),
                    "minecraft:stick".to_owned(),
                ],
            }
        );
    }

    #[test]
    fn a_slot_takes_and_puts_within_its_own_limit() {
        let mut slots = full_slots(&[("minecraft:torch", 62)]);
        assert_eq!(slots.room_for("minecraft:torch"), 2);
        assert_eq!(
            slots.room_for("minecraft:coal"),
            0,
            "a slot holding torches is no room for coal"
        );
        assert_eq!(slots.take("minecraft:torch", 3), 3);
        assert_eq!(slots.count("minecraft:torch"), 59);
        assert_eq!(slots.room_for("minecraft:torch"), 5);
        assert_eq!(slots.put("minecraft:torch", 9), 5, "the slot stops at 64");
        assert_eq!(slots.count("minecraft:torch"), 64);
        assert_eq!(
            slots.take("minecraft:torch", 100),
            64,
            "a slot empties once"
        );
        assert_eq!(slots.count("minecraft:torch"), 0);
        assert_eq!(
            slots.room_for("minecraft:torch"),
            64,
            "the emptied slot is an empty slot, which offers its full room"
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

    /// The requester's filter: 4 torches kept between 4 and 8, asking 4 at a time.
    fn torches(batch: u64) -> Filter {
        Filter::new("minecraft:torch".to_owned(), band(4, 8), batch).expect("a valid filter")
    }

    #[test]
    fn a_requester_holds_at_its_minimum_while_idle() {
        assert_eq!(ask(4, &torches(4), Phase::Idle, 0, 0), Ask::Hold);
        assert_eq!(
            ask(6, &torches(4), Phase::Idle, 0, 0),
            Ask::Hold,
            "a stock inside the band and nothing running asks nothing"
        );
    }

    #[test]
    fn a_requester_asks_below_its_minimum() {
        assert_eq!(
            ask(0, &torches(4), Phase::Idle, 0, 0),
            Ask::Request { amount: 4 },
            "a stock below min must be asked for"
        );
    }

    #[test]
    fn a_requester_keeps_asking_above_the_minimum_until_the_maximum() {
        // The hysteresis: once asking started, `min` no longer stops it (upstream CRAFTING).
        assert_eq!(
            ask(4, &torches(4), Phase::Crafting, 0, 0),
            Ask::Request { amount: 4 },
            "a running fill continues between min and max"
        );
    }

    #[test]
    fn a_requester_finishes_at_the_maximum() {
        assert_eq!(ask(8, &torches(4), Phase::Crafting, 0, 0), Ask::Finish);
        assert_eq!(
            ask(8, &torches(4), Phase::Idle, 0, 0),
            Ask::Hold,
            "a finished run that is inside the band holds"
        );
    }

    #[test]
    fn a_batch_caps_each_request() {
        assert_eq!(
            ask(0, &torches(4), Phase::Idle, 0, 0),
            Ask::Request { amount: 4 },
            "8 needed but the batch is 4"
        );
        assert_eq!(
            ask(0, &torches(64), Phase::Idle, 0, 0),
            Ask::Request { amount: 8 },
            "the band's remaining room is smaller than the batch"
        );
    }

    #[test]
    fn a_request_is_not_repeated_while_one_is_outstanding() {
        assert_eq!(
            ask(0, &torches(4), Phase::Crafting, 4, 0),
            Ask::Waiting,
            "the crafter is still working off the last request"
        );
    }

    #[test]
    fn a_failed_task_backs_off_before_asking_again() {
        assert_eq!(
            ask(0, &torches(4), Phase::Crafting, 0, 5),
            Ask::Backoff,
            "a task that failed is not retried until its timeout runs out"
        );
        assert_eq!(
            ask(8, &torches(4), Phase::Crafting, 0, 5),
            Ask::Finish,
            "reaching the maximum ends the run even while a timeout is pending"
        );
    }

    #[test]
    fn a_zero_batch_filter_is_disabled() {
        assert_eq!(
            ask(0, &torches(0), Phase::Idle, 0, 0),
            Ask::Hold,
            "upstream skips a filter entry whose batch size is 0"
        );
        assert!(Filter::new(String::new(), band(4, 8), 4).is_none());
    }

    #[test]
    fn a_crafter_that_refused_is_the_requesters_failed_task() {
        assert_eq!(
            refused(Phase::NotEnoughIngredients, 4, &torches(4)),
            Some(Phase::NotEnoughIngredients),
            "the crafter has too few ingredients and the stock is still short"
        );
        assert_eq!(
            refused(Phase::NetworkFull, 0, &torches(4)),
            Some(Phase::NetworkFull)
        );
        assert_eq!(
            refused(Phase::NotEnoughIngredients, 8, &torches(4)),
            None,
            "a refusal that no longer matters is not reported"
        );
        assert_eq!(refused(Phase::Crafting, 0, &torches(4)), None);
        assert_eq!(refused(Phase::Idle, 0, &torches(4)), None);
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
