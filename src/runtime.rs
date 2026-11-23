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
///
/// A stack also carries components (a name, an enchantment, damage), and the step inserts a plain
/// item: merging it into a named stack would give it that name. So a patched slot is no room for a
/// plain insert and [`Slots::put`] never merges into one; the machine derives the flag from the
/// same `ItemStack::is_stackable_with` the real insert uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    item: String,
    count: u32,
    max: u32,
    plain: bool,
}

impl Slot {
    /// A slot holding `count` of `item` with no component patch (or damage): a plain insert may
    /// merge into it. `None` for an empty item id, a zero count or a slot that can hold nothing.
    #[must_use]
    pub fn new(item: impl Into<String>, count: u32, max: u32) -> Option<Self> {
        Self::make(item, count, max, true)
    }

    /// A slot holding `count` of `item` **with** a component patch or damage (a name, an
    /// enchantment, a damaged tool): a plain insert of the same item must not merge into it.
    #[must_use]
    pub fn patched(item: impl Into<String>, count: u32, max: u32) -> Option<Self> {
        Self::make(item, count, max, false)
    }

    fn make(item: impl Into<String>, count: u32, max: u32, plain: bool) -> Option<Self> {
        let item = item.into();
        (count > 0 && max > 0 && !item.is_empty()).then_some(Self {
            item,
            count,
            max,
            plain,
        })
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

    /// Whether the stack carries no components, so a plain item may merge into it.
    #[must_use]
    pub fn is_plain(&self) -> bool {
        self.plain
    }
}

/// The machine's inventory as the rules see it: one entry per slot, in slot order.
///
/// `default_room` is the room an **empty** slot offers for an item a step inserts — that item's
/// stack limit, which only the game knows. A step inserts more than one item (the result and what
/// the recipe leaves behind), and each has its own limit, so [`Slots::set_empty_room`] names one
/// per item; the default covers the rest. A slot that already holds something carries its own limit
/// in its [`Slot`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slots {
    slots: Vec<Option<Slot>>,
    default_room: u32,
    empty_rooms: BTreeMap<String, u32>,
}

impl Slots {
    /// No slots yet; an empty slot will hold up to `empty_room` of an item not named by
    /// [`Slots::set_empty_room`].
    #[must_use]
    pub fn new(empty_room: u32) -> Self {
        Self {
            slots: Vec::new(),
            default_room: empty_room,
            empty_rooms: BTreeMap::new(),
        }
    }

    /// The room an empty slot offers `item`: its own stack limit, in place of the default.
    pub fn set_empty_room(&mut self, item: &str, room: u32) {
        if room > 0 {
            self.empty_rooms.insert(item.to_owned(), room);
        }
    }

    /// The room an empty slot offers `item`.
    fn room_of_empty_slot(&self, item: &str) -> u32 {
        self.empty_rooms
            .get(item)
            .copied()
            .unwrap_or(self.default_room)
            .max(1)
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

    /// Put up to `count` of a plain `item` into the slots: the plain stacks already holding it
    /// first, then empty slots, each up to its own limit. Answers how many went in. A stack with a
    /// component patch is not a plain stack: merging into it would launder its components onto the
    /// inserted item.
    pub fn put(&mut self, item: &str, count: u32) -> u32 {
        let mut left = count;
        for slot in self.slots.iter_mut().flatten() {
            if left == 0 {
                break;
            }
            if slot.item != item || !slot.plain {
                continue;
            }
            let added = slot.max.saturating_sub(slot.count).min(left);
            slot.count += added;
            left -= added;
        }
        let room = self.room_of_empty_slot(item);
        for cell in &mut self.slots {
            if left == 0 {
                break;
            }
            if cell.is_some() {
                continue;
            }
            let added = left.min(room);
            if added > 0 {
                *cell = Slot::new(item, added, room);
                left -= added;
            }
        }
        count - left
    }

    /// How many of `item` the slots can still hold: the room left in the slots already holding it
    /// plus one full empty slot's worth per empty slot. Any other item's free room is not room for
    /// this one, and neither is a stack carrying components: the step inserts a **plain** item, so
    /// only a plain stack of it can take the insert.
    #[must_use]
    pub fn room_for(&self, item: &str) -> u64 {
        let room = u64::from(self.room_of_empty_slot(item));
        self.slots
            .iter()
            .map(|slot| match slot {
                None => room,
                Some(slot) if slot.item == item && slot.plain => {
                    u64::from(slot.max.saturating_sub(slot.count))
                }
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
///
/// `remainders` is what those consumed items leave in the grid — vanilla's `getRemainingItems`: a
/// milk bucket leaves a bucket, so a cake step that used three of them leaves three buckets. It is
/// keyed by the consumed item, because the pattern keeps every candidate of a cell and the step
/// picks one of them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern {
    cells: Vec<Vec<String>>,
    remainders: BTreeMap<String, (String, u32)>,
    result: String,
    result_count: u32,
}

impl Pattern {
    /// A pattern of these cells, making `result_count` of `result`; `None` when a cell is empty or
    /// names no item, or when the result count is zero (such a pattern could never complete).
    #[must_use]
    pub fn new(cells: Vec<Vec<String>>, result: String, result_count: u32) -> Option<Self> {
        Self::new_with_remainders(cells, BTreeMap::new(), result, result_count)
    }

    /// [`Pattern::new`] with what the consumed items leave behind: each `(consumed, left, count)`.
    #[must_use]
    pub fn new_with_remainders(
        cells: Vec<Vec<String>>,
        remainders: BTreeMap<String, (String, u32)>,
        result: String,
        result_count: u32,
    ) -> Option<Self> {
        if cells.is_empty()
            || cells.iter().any(|cell| cell.is_empty())
            || result.is_empty()
            || result_count == 0
        {
            return None;
        }
        Some(
            Self {
                cells,
                remainders: BTreeMap::new(),
                result,
                result_count,
            }
            .with_remainders(remainders),
        )
    }

    /// The same pattern with what its consumed items leave behind; an entry with an empty consumed
    /// or leftover id, or a zero count, is dropped.
    #[must_use]
    pub fn with_remainders(mut self, remainders: BTreeMap<String, (String, u32)>) -> Self {
        self.remainders = remainders
            .into_iter()
            .filter(|(consumed, (left, count))| {
                !consumed.is_empty() && !left.is_empty() && *count > 0
            })
            .collect();
        self
    }

    /// The cells, each a list of the items that satisfy it.
    #[must_use]
    pub fn cells(&self) -> &[Vec<String>] {
        &self.cells
    }

    /// What each consumed item leaves behind: its leftover item and how many per consumed item.
    #[must_use]
    pub fn remainders(&self) -> &BTreeMap<String, (String, u32)> {
        &self.remainders
    }

    /// What consuming `item` leaves behind, when it leaves anything.
    #[must_use]
    pub fn remainder_of(&self, item: &str) -> Option<(&str, u32)> {
        self.remainders
            .get(item)
            .map(|(left, count)| (left.as_str(), *count))
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

    /// The saved form of [`Pattern::remainders`]: `consumed=left:count` per entry.
    #[must_use]
    pub fn encode_remainders(&self) -> Vec<String> {
        self.remainders
            .iter()
            .map(|(consumed, (left, count))| format!("{consumed}={left}:{count}"))
            .collect()
    }

    /// A pattern from [`Pattern::encode`] and [`Pattern::encode_remainders`]'s forms; `None` when a
    /// part of it does not admit one ([`Pattern::new_with_remainders`]'s rules, or a saved entry
    /// that is not `consumed=left:count`).
    #[must_use]
    pub fn decode(
        cells: &[String],
        remainders: &[String],
        result: &str,
        result_count: u32,
    ) -> Option<Self> {
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
        let mut left = BTreeMap::new();
        for entry in remainders {
            let (consumed, rest) = entry.split_once('=')?;
            let (leftover, count) = match rest.rsplit_once(':') {
                Some((leftover, count)) => (leftover, count.parse::<u32>().ok()?),
                None => (rest, 1),
            };
            left.insert(
                consumed.trim().to_owned(),
                (leftover.trim().to_owned(), count),
            );
        }
        Self::new_with_remainders(cells, left, result.to_owned(), result_count)
    }
}

/// What one maintenance step does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Inside the band, nothing running.
    Hold,
    /// The run reached `max`: the phase returns to idle.
    Finished,
    /// One step: consume these items, make `result_count` of `result`, and leave `remainders`
    /// (`item`, `count`) in the slots — what the consumed ingredients leave behind.
    Craft {
        consume: Vec<(String, u64)>,
        result: String,
        result_count: u32,
        remainders: Vec<(String, u64)>,
    },
    /// A cell of the pattern has no item in stock; the cell's candidates are named.
    NotEnoughIngredients { missing: Vec<String> },
    /// The step's result, or what the recipe leaves, has no room in the slots the ingredients would
    /// leave free.
    NetworkFull {
        item: String,
        /// How many of the item those slots can still hold.
        room: u64,
        /// How many the step would insert (the result, or what the recipe leaves).
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
/// took), its `canInsertIntoNetwork` room check, and `getRemainingItems` (what the ingredients
/// leave in the grid).
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
    // What the consumed items leave behind, summed per leftover item: three milk buckets leave
    // three buckets, however many cells they came from.
    let mut left: BTreeMap<String, u64> = BTreeMap::new();
    for (item, count) in &consume {
        if let Some((leftover, per)) = pattern.remainder_of(item) {
            *left.entry(leftover.to_owned()).or_default() += u64::from(per) * count;
        }
    }
    let produced = pattern.result_count();
    // The room is measured **after** the ingredients are taken: consuming the last item of a slot
    // frees that slot, and only the slots that hold an inserted item (or are empty) have room for
    // it. The result goes in first, then the remainders, one at a time, each through the same rule.
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
    after.put(pattern.result(), produced);
    let mut remainders = Vec::with_capacity(left.len());
    for (item, count) in left {
        let count = u32::try_from(count).unwrap_or(u32::MAX);
        let room = after.room_for(&item);
        if u64::from(count) > room {
            return Outcome::NetworkFull {
                item,
                room,
                produced: count,
            };
        }
        after.put(&item, count);
        remainders.push((item, u64::from(count)));
    }
    Outcome::Craft {
        consume: consume.into_iter().collect(),
        result: pattern.result().to_owned(),
        result_count: pattern.result_count(),
        remainders,
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
        remainders,
    } = outcome
    else {
        return false;
    };
    for (item, count) in consume {
        slots.take(item, *count);
    }
    slots.put(result, *result_count);
    for (item, count) in remainders {
        slots.put(item, u32::try_from(*count).unwrap_or(u32::MAX));
    }
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

/// How many filter entries one requester carries: upstream's `StepRequesterBlockEntity.FILTERS`
/// (`9 * 5`), the container its screen shows and its node walks every tick.
pub const MAX_FILTER_ENTRIES: usize = 45;

/// A requester's filter container is full: upstream's 45 slots hold no room for another resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FiltersFull;

impl std::fmt::Display for FiltersFull {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "all {MAX_FILTER_ENTRIES} filter entries are taken")
    }
}

impl std::error::Error for FiltersFull {}

/// One configured filter entry: upstream's `ResourceMinMaxAmount` (the resource, its band and its
/// batch) plus the two pieces of state this re-root keeps per entry — the crafter it linked and how
/// many failed-task ticks it is backing off — and its own status, as upstream keeps one per slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    filter: Filter,
    phase: Phase,
    link: Option<[i32; 3]>,
    backoff: u32,
}

impl Entry {
    /// A fresh entry for `filter`: idle, unlinked and not backing off.
    #[must_use]
    pub fn new(filter: Filter) -> Self {
        Self {
            filter,
            phase: Phase::Idle,
            link: None,
            backoff: 0,
        }
    }

    /// The resource, band and batch this entry maintains.
    #[must_use]
    pub fn filter(&self) -> &Filter {
        &self.filter
    }

    /// Keep the entry's crafter, phase and timeout while its resource's band or batch changes: a
    /// stepped value is the same resource asked of the same crafter.
    pub fn set_filter(&mut self, filter: Filter) {
        self.filter = filter;
    }

    /// Where this entry's last tick left it.
    #[must_use]
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// Record where a tick left this entry.
    pub fn set_phase(&mut self, phase: Phase) {
        self.phase = phase;
    }

    /// The crafter this entry asks, kept so the scan runs once and is re-validated every tick.
    #[must_use]
    pub fn link(&self) -> Option<[i32; 3]> {
        self.link
    }

    /// Remember (or forget) the crafter this entry asks.
    pub fn set_link(&mut self, link: Option<[i32; 3]>) {
        self.link = link;
    }

    /// Failed-task ticks left before this entry asks again.
    #[must_use]
    pub fn backoff(&self) -> u32 {
        self.backoff
    }

    /// Set the failed-task ticks this entry still has to wait.
    pub fn set_backoff(&mut self, ticks: u32) {
        self.backoff = ticks;
    }

    /// The phase as the readout prints it: upstream's task state is `running` while its CRAFTING
    /// status is set, which is also how the brief names it.
    #[must_use]
    pub fn phase_id(&self) -> &'static str {
        if self.phase == Phase::Crafting {
            "running"
        } else {
            self.phase.id()
        }
    }

    /// The entry's saved form, `item|min|max|batch|phase|link|backoff`; `|` is not an item id, a
    /// phase name or a position, so the fields cannot run together.
    #[must_use]
    pub fn encode(&self) -> String {
        let link = match self.link {
            Some([x, y, z]) => format!("{x},{y},{z}"),
            None => "none".to_owned(),
        };
        format!(
            "{}|{}|{}|{}|{}|{}|{}",
            self.filter.item,
            self.filter.band.min,
            self.filter.band.max,
            self.filter.batch,
            self.phase.id(),
            link,
            self.backoff
        )
    }

    /// Read one saved entry; anything malformed is refused rather than guessed at.
    #[must_use]
    pub fn decode(text: &str) -> Option<Self> {
        let parts: Vec<&str> = text.split('|').collect();
        let [item, min, max, batch, phase, link, backoff] = parts.as_slice() else {
            return None;
        };
        let band = Band::new(min.parse().ok()?, max.parse().ok()?)?;
        let filter = Filter::new((*item).to_owned(), band, batch.parse().ok()?)?;
        Some(Self {
            filter,
            phase: Phase::of(phase),
            link: if *link == "none" {
                None
            } else {
                Some(parse_link(link)?)
            },
            backoff: backoff.parse().ok()?,
        })
    }
}

/// `x,y,z` as an entry saves its linked crafter.
fn parse_link(text: &str) -> Option<[i32; 3]> {
    let mut parts = text.split(',');
    let x = parts.next()?.parse().ok()?;
    let y = parts.next()?.parse().ok()?;
    let z = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some([x, y, z])
}

/// A requester's configured filter entries: upstream's 45-slot container, one entry per resource.
///
/// Upstream addresses entries by slot index and skips an empty slot or a zero batch. This port keeps
/// the entries the operator configured, in the order they were added, and treats a set of the same
/// resource as a replacement of that entry — the resource is the entry's identity, its saved slot
/// index is not.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filters {
    entries: Vec<Entry>,
}

impl Filters {
    /// An empty container: a requester that maintains nothing yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add `filter`, or replace the entry that already maintains its resource, and answer its
    /// index. A replacement starts idle and unlinked, as setting a new band restarts the run.
    ///
    /// # Errors
    ///
    /// [`FiltersFull`] when a new resource would be a 46th entry.
    pub fn set(&mut self, filter: Filter) -> Result<usize, FiltersFull> {
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.filter.item == filter.item)
        {
            self.entries[index] = Entry::new(filter);
            return Ok(index);
        }
        if self.entries.len() >= MAX_FILTER_ENTRIES {
            return Err(FiltersFull);
        }
        self.entries.push(Entry::new(filter));
        Ok(self.entries.len() - 1)
    }

    /// Remove the entry that maintains `item`; answers whether one was there.
    pub fn remove(&mut self, item: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.filter.item != item);
        self.entries.len() != before
    }

    /// How many entries are configured.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the requester maintains nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The first entry, the one the readout's legacy fields describe.
    #[must_use]
    pub fn first(&self) -> Option<&Entry> {
        self.entries.first()
    }

    /// The configured entries in the order they were added.
    pub fn iter(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter()
    }

    /// The configured entries, for a tick that updates each one.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Entry> {
        self.entries.iter_mut()
    }

    /// The entry at `index` (a filter cell), if one is configured there.
    #[must_use]
    pub fn entry(&self, index: usize) -> Option<&Entry> {
        self.entries.get(index)
    }

    /// Replace the entry at `index` with a fresh one for `filter`: a new resource on that cell keeps
    /// the cell and drops the old resource's crafter, phase and timeout.
    pub fn replace(&mut self, index: usize, filter: Filter) {
        if let Some(entry) = self.entries.get_mut(index) {
            *entry = Entry::new(filter);
        }
    }

    /// Set the entry at `index` to `filter`, keeping the state it has already: a band or batch the
    /// player stepped is the same resource on the same crafter.
    pub fn retune(&mut self, index: usize, filter: Filter) {
        if let Some(entry) = self.entries.get_mut(index) {
            entry.set_filter(filter);
        }
    }

    /// Every entry as its saved string, for a `List` in the block entity's compound.
    #[must_use]
    pub fn encode(&self) -> Vec<String> {
        self.entries.iter().map(Entry::encode).collect()
    }

    /// Read saved entries; a malformed one is skipped, so the others survive it.
    #[must_use]
    pub fn decode(texts: &[String]) -> Self {
        Self {
            entries: texts
                .iter()
                .filter_map(|text| Entry::decode(text))
                .collect(),
        }
    }
}

/// The screen's filter grid: upstream's `FILTERS = 9 * 5` cells, drawn 9 across and 5 down.
pub const SCREEN_COLUMNS: usize = 9;
/// The filter grid's rows.
pub const SCREEN_ROWS: usize = 5;

/// The largest amount one screen press reaches. The screen's numbers travel in the menu's data
/// slots, which are 16-bit on the wire, and an amount a player steps to is a stock band, not a
/// count of items: a command may still set more (the band is saved, not the view).
pub const SCREEN_MAX: u64 = 4096;

/// One press on a requester's filter screen, as [`Editor::apply`] takes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    /// A filter cell: select it, and give its entry the carried item when one is carried.
    Cell(u8),
    /// The selected entry's minimum, down one.
    MinDown,
    /// The selected entry's minimum, up one.
    MinUp,
    /// The selected entry's maximum, down one.
    MaxDown,
    /// The selected entry's maximum, up one.
    MaxUp,
    /// The selected entry's batch, down one.
    BatchDown,
    /// The selected entry's batch, up one.
    BatchUp,
    /// Drop the selected entry.
    Remove,
}

/// The menu button ids the screen answers, as `screen_click` sends them and `menu_button` reads
/// them: cell `i` is `100 + i`, the value buttons follow.
pub const FIRST_CELL_BUTTON: i32 = 100;

impl Press {
    /// The press a menu button id stands for; `None` for a button the screen does not draw.
    #[must_use]
    pub fn of(button: i32) -> Option<Self> {
        let cell = button - FIRST_CELL_BUTTON;
        if (0..MAX_FILTER_ENTRIES as i32).contains(&cell) {
            return Some(Self::Cell(u8::try_from(cell).ok()?));
        }
        match button {
            200 => Some(Self::MinDown),
            201 => Some(Self::MinUp),
            202 => Some(Self::MaxDown),
            203 => Some(Self::MaxUp),
            204 => Some(Self::BatchDown),
            205 => Some(Self::BatchUp),
            206 => Some(Self::Remove),
            _ => None,
        }
    }

    /// The button id this press is sent as.
    #[must_use]
    pub fn button(self) -> i32 {
        match self {
            Self::Cell(cell) => FIRST_CELL_BUTTON + i32::from(cell),
            Self::MinDown => 200,
            Self::MinUp => 201,
            Self::MaxDown => 202,
            Self::MaxUp => 203,
            Self::BatchDown => 204,
            Self::BatchUp => 205,
            Self::Remove => 206,
        }
    }
}

/// A requester's screen over its saved filter container: which entry the value buttons edit, plus
/// the container itself. The screen and `/stepcrafter requester` edit the same [`Filters`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Editor {
    filters: Filters,
    selected: usize,
}

impl Editor {
    /// The screen over `filters`, with `selected` focused (clamped to the next free cell).
    #[must_use]
    pub fn new(filters: Filters, selected: usize) -> Self {
        let selected = selected.min(filters.len());
        Self { filters, selected }
    }

    /// The container the screen edits.
    #[must_use]
    pub fn filters(&self) -> &Filters {
        &self.filters
    }

    /// The container, as the block entity saves it.
    #[must_use]
    pub fn into_filters(self) -> Filters {
        self.filters
    }

    /// The entry the value buttons edit; `len()` means "the next cell a click would fill".
    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// How many entries the requester carries.
    #[must_use]
    pub fn count(&self) -> usize {
        self.filters.len()
    }

    /// Apply one press, with the item the player carries (`None` for an empty hand). Answers
    /// whether the configuration changed.
    pub fn apply(&mut self, press: Press, carried: Option<&str>) -> bool {
        match press {
            Press::Cell(cell) => self.click(usize::from(cell), carried),
            Press::MinDown => self.step(Field::Min, Direction::Down),
            Press::MinUp => self.step(Field::Min, Direction::Up),
            Press::MaxDown => self.step(Field::Max, Direction::Down),
            Press::MaxUp => self.step(Field::Max, Direction::Up),
            Press::BatchDown => self.step(Field::Batch, Direction::Down),
            Press::BatchUp => self.step(Field::Batch, Direction::Up),
            Press::Remove => self.remove(),
        }
    }

    /// A cell's press: select it, and make (or repoint) its entry from the carried item.
    fn click(&mut self, cell: usize, carried: Option<&str>) -> bool {
        if cell >= MAX_FILTER_ENTRIES {
            return false;
        }
        let Some(item) = carried else {
            // An empty hand selects; it cannot make an entry, and a cell past the next free one
            // has no entry to select.
            if cell > self.filters.len() || cell == self.selected {
                return false;
            }
            self.selected = cell;
            return true;
        };
        // The resource is an entry's identity, as `/stepcrafter requester add` replaces rather than
        // duplicating one: two entries for one item are refused, and the click changes nothing.
        if self
            .filters
            .iter()
            .enumerate()
            .any(|(index, entry)| index != cell && entry.filter().item() == item)
        {
            return false;
        }
        if cell < self.filters.len() {
            let Some(entry) = self.filters.entry(cell) else {
                return false;
            };
            if entry.filter().item() == item {
                // The same resource on its own slot: this click only focuses it.
                if self.selected == cell {
                    return false;
                }
                self.selected = cell;
                return true;
            }
            // Another resource on this slot keeps the band and batch the player set for it, and
            // drops the link, phase and timeout, which belonged to the old resource's crafter.
            let Some(filter) = Filter::new(
                item.to_owned(),
                entry.filter().band(),
                entry.filter().batch(),
            ) else {
                return false;
            };
            self.filters.replace(cell, filter);
            self.selected = cell;
            return true;
        }
        if cell > self.filters.len() {
            return false;
        }
        // The next free cell: upstream's empty filter slot, at 0..0 with the batch off.
        let Some(band) = Band::new(0, 0) else {
            return false;
        };
        let Some(filter) = Filter::new(item.to_owned(), band, 0) else {
            return false;
        };
        if self.filters.set(filter).is_err() {
            return false;
        }
        self.selected = cell;
        true
    }

    /// One value button on the selected entry.
    fn step(&mut self, field: Field, direction: Direction) -> bool {
        let Some(entry) = self.filters.entry(self.selected) else {
            return false;
        };
        let filter = entry.filter();
        let (min, max, batch) = (filter.band().min(), filter.band().max(), filter.batch());
        let up = direction == Direction::Up;
        let (min, max, batch) = match field {
            // The band stays valid: `min` never passes `max`, `max` never drops below it, and no
            // amount is negative.
            Field::Min if up && min < max => (min + 1, max, batch),
            Field::Min if !up && min > 0 => (min - 1, max, batch),
            Field::Max if up && max < SCREEN_MAX => (min, max + 1, batch),
            Field::Max if !up && max > min => (min, max - 1, batch),
            Field::Batch if up && batch < SCREEN_MAX => (min, max, batch + 1),
            Field::Batch if !up && batch > 0 => (min, max, batch - 1),
            Field::Min | Field::Max | Field::Batch => return false,
        };
        let Some(band) = Band::new(min, max) else {
            return false;
        };
        let Some(filter) = Filter::new(filter.item().to_owned(), band, batch) else {
            return false;
        };
        // The resource is unchanged, so the entry keeps the crafter it linked and its timeout.
        self.filters.retune(self.selected, filter);
        true
    }

    /// Drop the entry the buttons were editing; the selection stays inside the container.
    fn remove(&mut self) -> bool {
        if self.selected >= self.filters.len() {
            return false;
        }
        let item = self
            .filters
            .entry(self.selected)
            .map(|entry| entry.filter().item().to_owned());
        let Some(item) = item else {
            return false;
        };
        let removed = self.filters.remove(&item);
        self.selected = self.selected.min(self.filters.len());
        removed
    }

    /// The menu's data slots: the count, the selected cell and that entry's band and batch, which
    /// is what the screen draws and the client refreshes as the entry changes.
    #[must_use]
    pub fn data_slots(&self) -> [i32; 5] {
        view_data_slots(&self.filters, self.selected)
    }

    /// What the entries' cells show: the resources in order, which the screen's open payload
    /// carries (the numbers are the data slots).
    #[must_use]
    pub fn extra_data(&self) -> String {
        view_extra_data(&self.filters)
    }
}

/// A requester's menu data slots without an editor: the count, the selected cell and that entry's
/// band and batch. A block entity publishes these every tick, which is how the screen's numbers
/// follow a press.
#[must_use]
pub fn view_data_slots(filters: &Filters, selected: usize) -> [i32; 5] {
    let selected = selected.min(filters.len());
    let count = slot_value(filters.len() as u64);
    let Some(entry) = filters.entry(selected) else {
        return [count, slot_value(selected as u64), 0, 0, 0];
    };
    [
        count,
        slot_value(selected as u64),
        slot_value(entry.filter().band().min()),
        slot_value(entry.filter().band().max()),
        slot_value(entry.filter().batch()),
    ]
}

/// The open screen's payload: the entries' resources in cell order, one namespaced id per line.
#[must_use]
pub fn view_extra_data(filters: &Filters) -> String {
    let items: Vec<&str> = filters.iter().map(|entry| entry.filter().item()).collect();
    items.join("\n")
}

/// The three values a screen button steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
    Min,
    Max,
    Batch,
}

/// Which way a value button steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Direction {
    Down,
    Up,
}

/// An amount as a menu data slot: the wire's 16 bits, never a wrapped negative.
fn slot_value(amount: u64) -> i32 {
    i32::try_from(amount.min(u64::try_from(i16::MAX).unwrap_or(0))).unwrap_or(0)
}

/// Whether `text` is a namespaced item id (`namespace:path`), the only thing a screen payload
/// carries for a cell.
fn is_item_id(text: &str) -> bool {
    match text.split_once(':') {
        Some((namespace, path)) => {
            !namespace.is_empty() && !path.is_empty() && !text.chars().any(char::is_whitespace)
        }
        None => false,
    }
}

/// The resources an open screen's payload lists, in cell order. An empty payload and a malformed
/// one both list nothing, so a screen without entries draws empty cells rather than guessing.
#[must_use]
pub fn extra_items(data: &[u8]) -> Vec<String> {
    let Ok(text) = std::str::from_utf8(data) else {
        return Vec::new();
    };
    let items: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    if items.iter().all(|item| is_item_id(item)) {
        items
    } else {
        Vec::new()
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

    /// Vanilla's `getRemainingItems` for the one item a step in these tests consumes: a milk bucket
    /// leaves a bucket.
    fn buckets() -> BTreeMap<String, (String, u32)> {
        BTreeMap::from([(
            "minecraft:milk_bucket".to_owned(),
            ("minecraft:bucket".to_owned(), 1),
        )])
    }

    /// `minecraft:cake` (`MMM/SES/WWW`): 3 milk buckets, 2 sugar, 1 egg and 3 wheat, making one
    /// cake — and leaving the three buckets behind.
    fn cake() -> Pattern {
        let cell = |id: &str| vec![id.to_owned()];
        Pattern::new_with_remainders(
            vec![
                cell("minecraft:milk_bucket"),
                cell("minecraft:milk_bucket"),
                cell("minecraft:milk_bucket"),
                cell("minecraft:sugar"),
                cell("minecraft:egg"),
                cell("minecraft:sugar"),
                cell("minecraft:wheat"),
                cell("minecraft:wheat"),
                cell("minecraft:wheat"),
            ],
            buckets(),
            "minecraft:cake".to_owned(),
            1,
        )
        .expect("a valid pattern")
    }

    /// One milk bucket making one cake, leaving a bucket: cake's rule with a single cell, for the
    /// room test.
    fn bucket_step() -> Pattern {
        Pattern::new_with_remainders(
            vec![vec!["minecraft:milk_bucket".to_owned()]],
            buckets(),
            "minecraft:cake".to_owned(),
            1,
        )
        .expect("a valid pattern")
    }

    #[test]
    fn a_step_returns_the_bucket_the_recipe_leaves() {
        let slots = slots(&[
            ("minecraft:milk_bucket", 3),
            ("minecraft:sugar", 2),
            ("minecraft:egg", 1),
            ("minecraft:wheat", 3),
        ]);
        let outcome = step_now(&slots, Some(&cake()));
        assert_eq!(
            outcome,
            Outcome::Craft {
                consume: vec![
                    ("minecraft:egg".to_owned(), 1),
                    ("minecraft:milk_bucket".to_owned(), 3),
                    ("minecraft:sugar".to_owned(), 2),
                    ("minecraft:wheat".to_owned(), 3),
                ],
                result: "minecraft:cake".to_owned(),
                result_count: 1,
                remainders: vec![("minecraft:bucket".to_owned(), 3)],
            },
            "the three milk buckets leave three buckets in the machine"
        );
        let mut after = slots.clone();
        assert!(apply(&mut after, &outcome));
        assert_eq!(after.count("minecraft:milk_bucket"), 0, "the milk is gone");
        assert_eq!(after.count("minecraft:cake"), 1);
        assert_eq!(
            after.count("minecraft:bucket"),
            3,
            "a bucket recipe leaves its buckets, not destroys them"
        );
    }

    #[test]
    fn a_remainder_that_fits_no_slot_refuses_the_step() {
        // One slot holds the milk bucket. Taking it frees the slot and the cake goes in; nothing is
        // empty then and no slot holds a bucket, so what the recipe leaves has nowhere to go and the
        // step must not consume the milk.
        let slots = full_slots(&[("minecraft:milk_bucket", 1)]);
        let before = slots.clone();
        let outcome = step_now(&slots, Some(&bucket_step()));
        assert_eq!(
            outcome,
            Outcome::NetworkFull {
                item: "minecraft:bucket".to_owned(),
                room: 0,
                produced: 1,
            },
            "no slot has room for the bucket the milk leaves"
        );
        let mut after = before.clone();
        assert!(!apply(&mut after, &outcome), "a refusal consumes nothing");
        assert_eq!(after, before);
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
                remainders: Vec::new(),
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
                remainders: Vec::new(),
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
                remainders: Vec::new(),
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
                remainders: Vec::new(),
            }
        );
    }

    /// The last silent-loss path: the room check counted a stack of the result by item id, so a
    /// **named** torch stack offered the 4 items of room its count left, while the insert merges
    /// only into a stack with no components. The step must be refused, not counted and lost.
    #[test]
    fn a_named_stack_of_the_result_is_no_room_for_a_plain_step() {
        // 60 named torches (4 of nominal room) and the torch recipe's ingredients in every other
        // slot: no slot is empty, so the only stack that could take the result is the named one.
        let mut slots = Slots::new(64);
        slots.push(Slot::patched("minecraft:torch", 60, 64));
        for _ in 0..25 {
            slots.push(Slot::new("minecraft:coal", 64, 64));
        }
        slots.push(Slot::new("minecraft:stick", 64, 64));
        assert_eq!(
            slots.room_for("minecraft:torch"),
            0,
            "a named stack of the result is no room for a plain one"
        );
        assert_eq!(
            step_now(&slots, Some(&torch())),
            Outcome::NetworkFull {
                item: "minecraft:torch".to_owned(),
                room: 0,
                produced: 4,
            },
            "the four torches would be counted and then lost beside the named stack"
        );
    }

    #[test]
    fn a_plain_result_is_a_new_stack_beside_a_named_one() {
        // One empty slot: the four torches do fit, but they must not be merged into the named
        // stack (that would launder its name onto them). They become their own plain stack.
        let mut slots = Slots::new(64);
        slots.push(Slot::patched("minecraft:torch", 60, 64));
        slots.push(Slot::new("minecraft:coal", 2, 64));
        slots.push(Slot::new("minecraft:stick", 2, 64));
        slots.push(None);
        let outcome = step_now(&slots, Some(&torch()));
        assert!(
            matches!(outcome, Outcome::Craft { .. }),
            "the free slot and the ingredients hold the step: {outcome:?}"
        );
        let mut after = slots.clone();
        assert!(apply(&mut after, &outcome));
        let stored: Vec<(u32, bool)> = after
            .slots()
            .iter()
            .flatten()
            .filter(|slot| slot.item() == "minecraft:torch")
            .map(|slot| (slot.count(), slot.is_plain()))
            .collect();
        assert_eq!(
            stored,
            vec![(60, false), (4, true)],
            "the named stack keeps its name and count and the plain result gets its own slot"
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
                remainders: Vec::new(),
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
    fn an_empty_slot_offers_each_inserted_item_its_own_room() {
        // A step inserts the result and what the recipe leaves; their stack limits differ, so an
        // empty slot's room for a bucket (16) is not its room for a cake (1).
        let mut slots = Slots::new(1);
        slots.push(None);
        slots.set_empty_room("minecraft:bucket", 16);
        assert_eq!(slots.room_for("minecraft:cake"), 1);
        assert_eq!(slots.room_for("minecraft:bucket"), 16);
        assert_eq!(slots.put("minecraft:bucket", 3), 3);
        assert_eq!(slots.count("minecraft:bucket"), 3);
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
        assert_eq!(
            Pattern::decode(&saved, &[], "minecraft:torch", 4),
            Some(pattern)
        );
        assert!(Pattern::decode(&["".to_owned()], &[], "minecraft:torch", 4).is_none());
        assert!(Pattern::decode(&saved, &[], "minecraft:torch", 0).is_none());
        // The remainders are part of the configuration: a restart must not forget that the milk
        // buckets leave buckets.
        let pattern = cake();
        let cells = pattern.encode();
        let saved_remainders = pattern.encode_remainders();
        assert_eq!(
            saved_remainders,
            vec!["minecraft:milk_bucket=minecraft:bucket:1".to_owned()]
        );
        assert_eq!(
            Pattern::decode(&cells, &saved_remainders, "minecraft:cake", 1),
            Some(pattern)
        );
        assert!(
            Pattern::decode(&cells, &["nonsense".to_owned()], "minecraft:cake", 1).is_none(),
            "a saved remainder that is not consumed=left:count is refused"
        );
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

    /// A second entry of its own: 8 sticks kept between 8 and 16, asked 8 at a time.
    fn sticks() -> Filter {
        Filter::new("minecraft:stick".to_owned(), band(8, 16), 8).expect("a valid filter")
    }

    #[test]
    fn a_second_filter_entry_is_kept_beside_the_first() {
        let mut filters = Filters::new();
        assert_eq!(filters.set(torches(4)), Ok(0));
        assert_eq!(
            filters.set(sticks()),
            Ok(1),
            "a second resource gets its own entry, as upstream's next filter slot"
        );
        assert_eq!(filters.len(), 2, "the requester keeps both filter entries");
        assert!(!filters.is_empty());
        let items: Vec<&str> = filters.iter().map(|entry| entry.filter().item()).collect();
        assert_eq!(items, ["minecraft:torch", "minecraft:stick"]);
    }

    #[test]
    fn a_filter_entry_is_replaced_by_its_own_resource() {
        let mut filters = Filters::new();
        assert_eq!(filters.set(torches(4)), Ok(0));
        let wider = Filter::new("minecraft:torch".to_owned(), band(8, 16), 4).expect("a filter");
        assert_eq!(
            filters.set(wider),
            Ok(0),
            "the same resource keeps its index"
        );
        assert_eq!(filters.len(), 1, "a resource names one entry, not two");
        assert_eq!(
            filters.first().map(|entry| entry.filter().band()),
            Some(band(8, 16))
        );
        assert_eq!(
            filters.first().map(Entry::phase),
            Some(Phase::Idle),
            "a new band restarts the entry's run"
        );
    }

    #[test]
    fn a_requester_carries_at_most_the_containers_45_entries() {
        let mut filters = Filters::new();
        for n in 0..MAX_FILTER_ENTRIES {
            let filter =
                Filter::new(format!("minecraft:test_{n}"), band(1, 2), 1).expect("a valid filter");
            assert!(filters.set(filter).is_ok());
        }
        assert_eq!(filters.len(), MAX_FILTER_ENTRIES);
        let overflow =
            Filter::new("minecraft:one_too_many".to_owned(), band(1, 2), 1).expect("a filter");
        assert_eq!(
            filters.set(overflow),
            Err(FiltersFull),
            "upstream's container has 45 filter slots"
        );
        assert_eq!(
            filters.len(),
            MAX_FILTER_ENTRIES,
            "the refused entry took no slot"
        );
    }

    #[test]
    fn a_removed_entry_leaves_the_others() {
        let mut filters = Filters::new();
        filters.set(torches(4)).expect("room for the first");
        filters.set(sticks()).expect("room for the second");
        assert!(filters.remove("minecraft:stick"));
        assert_eq!(filters.len(), 1);
        assert_eq!(
            filters.first().map(|entry| entry.filter().item()),
            Some("minecraft:torch")
        );
        assert!(
            !filters.remove("minecraft:stick"),
            "removing an absent resource changes nothing"
        );
    }

    #[test]
    fn a_filter_entry_round_trips_through_its_saved_form() {
        let mut entry = Entry::new(torches(4));
        entry.set_phase(Phase::Crafting);
        entry.set_link(Some([221, 64, 0]));
        entry.set_backoff(7);
        let text = entry.encode();
        assert_eq!(Entry::decode(&text), Some(entry));
        assert!(
            Entry::decode("minecraft:torch|8|4|4|idle|none|0").is_none(),
            "min above max"
        );
        assert!(Entry::decode("nonsense").is_none());
        assert!(
            Entry::decode("minecraft:torch|4|8|4|idle|221,64|0").is_none(),
            "short position"
        );
        assert_eq!(
            Entry::decode("minecraft:torch|4|8|4|idle|none|0").map(|entry| entry.phase_id()),
            Some("idle")
        );
    }

    #[test]
    fn the_filter_container_round_trips_through_its_saved_form() {
        let mut filters = Filters::new();
        filters.set(torches(4)).expect("room for the first");
        filters.set(sticks()).expect("room for the second");
        let texts = filters.encode();
        assert_eq!(texts.len(), 2, "one saved string per entry");
        assert_eq!(Filters::decode(&texts), filters);
        assert!(
            Filters::decode(&["nonsense".to_owned()]).is_empty(),
            "a malformed entry is skipped, not guessed at"
        );
    }

    /// The screen's buttons are one vocabulary: a press that reaches the server must be the press
    /// the client sent, or a click edits a different entry than the one under the pointer.
    #[test]
    fn a_press_round_trips_through_its_button_id() {
        assert_eq!(Press::of(100), Some(Press::Cell(0)));
        assert_eq!(Press::of(144), Some(Press::Cell(44)));
        for (button, press) in [
            (200, Press::MinDown),
            (201, Press::MinUp),
            (202, Press::MaxDown),
            (203, Press::MaxUp),
            (204, Press::BatchDown),
            (205, Press::BatchUp),
            (206, Press::Remove),
        ] {
            assert_eq!(Press::of(button), Some(press));
            assert_eq!(press.button(), button);
        }
        assert_eq!(Press::Cell(3).button(), 103);
        assert_eq!(Press::of(145), None, "a 46th cell is not a filter slot");
        assert_eq!(Press::of(207), None);
        assert_eq!(Press::of(-1), None);
    }

    /// Upstream's screen edits a filter slot: a cell with nothing carried selects it, and a click
    /// with a resource carried is how an entry is made at all.
    #[test]
    fn a_click_on_an_empty_cell_with_a_carried_item_adds_the_entry() {
        let mut editor = Editor::new(Filters::new(), 0);
        assert!(editor.apply(Press::Cell(0), Some("minecraft:torch")));
        assert_eq!(editor.count(), 1);
        let entry = editor.filters().first().expect("the new entry");
        assert_eq!(entry.filter().item(), "minecraft:torch");
        assert_eq!(entry.filter().band(), band(0, 0));
        assert_eq!(entry.filter().batch(), 0);
        assert_eq!(editor.selected(), 0);
        assert!(
            !editor.apply(Press::Cell(0), None),
            "an empty hand on the cell that is already focused changes nothing"
        );
        assert_eq!(editor.count(), 1, "an empty hand never adds an entry");
        assert!(
            editor.apply(Press::Cell(1), None),
            "an empty hand focuses the next free cell, the one a click would fill"
        );
        assert_eq!(editor.selected(), 1);
        assert_eq!(editor.count(), 1);
    }

    /// A click on a filled cell selects that entry; it is what the value buttons then edit.
    #[test]
    fn a_click_on_a_filled_cell_selects_it() {
        let mut filters = Filters::new();
        filters.set(torches(4)).expect("room for the first");
        filters.set(sticks()).expect("room for the second");
        let mut editor = Editor::new(filters, 0);
        assert!(editor.apply(Press::Cell(1), None));
        assert_eq!(editor.selected(), 1);
        assert_eq!(
            editor.filters().first().map(|e| e.filter().item()),
            Some("minecraft:torch"),
            "selecting the second entry did not touch the first"
        );
    }

    /// Upstream's slot takes another resource when one is clicked onto it: the entry keeps its band
    /// and batch, because those are what the player set for *that* slot.
    #[test]
    fn a_click_with_another_item_repoints_the_entry_and_keeps_its_band() {
        let mut filters = Filters::new();
        filters.set(torches(4)).expect("room for the entry");
        let mut editor = Editor::new(filters, 0);
        assert!(editor.apply(Press::Cell(0), Some("minecraft:stick")));
        assert_eq!(editor.count(), 1);
        let entry = editor.filters().first().expect("the entry");
        assert_eq!(entry.filter().item(), "minecraft:stick");
        assert_eq!(entry.filter().band(), band(4, 8));
        assert_eq!(entry.filter().batch(), 4);
    }

    /// The resource is an entry's identity, as the command's add-or-replace rule says: a click that
    /// would leave two entries maintaining one item is refused and both entries stay as they were.
    #[test]
    fn a_click_that_would_give_two_entries_one_resource_is_refused() {
        let mut filters = Filters::new();
        filters.set(torches(4)).expect("room for the first");
        filters.set(sticks()).expect("room for the second");
        let mut editor = Editor::new(filters, 1);
        assert!(!editor.apply(Press::Cell(0), Some("minecraft:stick")));
        let before = Editor::new(editor.filters().clone(), 1);
        assert_eq!(editor, before, "the refused click changed nothing");
        assert_eq!(editor.count(), 2);
    }

    /// A step keeps the band valid: `min` never passes `max` and `max` never drops below `min`, the
    /// same rule `/stepcrafter configure` answers "min 9 is above max 4" with.
    #[test]
    fn a_step_keeps_the_band_valid() {
        let mut filters = Filters::new();
        filters.set(torches(4)).expect("room for the entry");
        let mut editor = Editor::new(filters, 0);
        assert!(editor.apply(Press::MinUp, None));
        assert!(editor.apply(Press::MinUp, None));
        assert!(editor.apply(Press::MinUp, None));
        assert!(editor.apply(Press::MinUp, None));
        let entry = editor.filters().first().expect("the entry");
        assert_eq!(entry.filter().band(), band(8, 8), "four ups reached max");
        assert!(!editor.apply(Press::MinUp, None), "min cannot pass max");
        assert!(
            !editor.apply(Press::MaxDown, None),
            "max cannot drop below min"
        );
        assert!(editor.apply(Press::MaxUp, None));
        assert_eq!(
            editor.filters().first().expect("the entry").filter().band(),
            band(8, 9)
        );
    }

    /// No amount is negative: a fresh entry is 0..0 with batch 0, and pressing every `-` on it
    /// changes nothing (upstream reads a zero batch as a disabled slot, not a negative one).
    #[test]
    fn a_step_never_goes_below_zero_and_a_zero_batch_stays_disabled() {
        let mut filters = Filters::new();
        filters
            .set(Filter::new("minecraft:stick".to_owned(), band(0, 0), 0).expect("a filter"))
            .expect("room for the entry");
        let mut editor = Editor::new(filters, 0);
        assert!(!editor.apply(Press::MinDown, None));
        assert!(!editor.apply(Press::MaxDown, None));
        assert!(!editor.apply(Press::BatchDown, None));
        let entry = editor.filters().first().expect("the entry");
        assert_eq!(entry.filter().band(), band(0, 0));
        assert_eq!(entry.filter().batch(), 0);
        // And a step that is allowed does move: the same entry's maximum, then its minimum.
        assert!(editor.apply(Press::MaxUp, None));
        assert!(editor.apply(Press::MinUp, None));
        let entry = editor.filters().first().expect("the entry");
        assert_eq!(entry.filter().band(), band(1, 1));
    }

    /// A zero batch is upstream's disabled slot; one press makes the entry ask, which is how a
    /// filter set up in a screen starts working.
    #[test]
    fn a_batch_step_enables_an_entry() {
        let mut filters = Filters::new();
        filters
            .set(Filter::new("minecraft:stick".to_owned(), band(0, 0), 0).expect("a filter"))
            .expect("room for the entry");
        let mut editor = Editor::new(filters, 0);
        assert!(editor.apply(Press::BatchUp, None));
        let entry = editor.filters().first().expect("the entry");
        assert_eq!(entry.filter().batch(), 1);
        assert_eq!(entry.filter().band(), band(0, 0));
    }

    /// Remove drops the cell the buttons were editing and keeps the others, and the selection stays
    /// inside the container.
    #[test]
    fn a_remove_press_drops_the_selected_entry_and_keeps_the_others() {
        let mut filters = Filters::new();
        filters.set(torches(4)).expect("room for the first");
        filters.set(sticks()).expect("room for the second");
        let mut editor = Editor::new(filters, 0);
        assert!(editor.apply(Press::Remove, None));
        assert_eq!(editor.count(), 1);
        assert_eq!(
            editor.filters().first().map(|e| e.filter().item()),
            Some("minecraft:stick")
        );
        assert_eq!(editor.selected(), 0);
        assert!(editor.apply(Press::Remove, None));
        assert_eq!(editor.count(), 0);
        assert!(!editor.apply(Press::Remove, None), "nothing left to remove");
    }

    /// A cell past the next free one is a blank frame: it has no entry to select and no resource to
    /// make one from.
    #[test]
    fn a_cell_beyond_the_next_free_one_changes_nothing() {
        let mut filters = Filters::new();
        filters.set(torches(4)).expect("room for the entry");
        let mut editor = Editor::new(filters, 0);
        assert!(!editor.apply(Press::Cell(7), Some("minecraft:stick")));
        assert_eq!(editor.count(), 1);
        assert_eq!(editor.selected(), 0);
        assert!(
            editor.apply(Press::Cell(1), Some("minecraft:stick")),
            "the next free cell takes the entry"
        );
        assert_eq!(editor.count(), 2);
        assert_eq!(editor.selected(), 1);
    }

    /// A container with no entries has nothing to step: the screen's buttons do nothing.
    #[test]
    fn a_step_on_an_empty_requester_changes_nothing() {
        let mut editor = Editor::new(Filters::new(), 0);
        for press in [
            Press::MinUp,
            Press::MaxUp,
            Press::BatchUp,
            Press::MinDown,
            Press::Remove,
        ] {
            assert!(!editor.apply(press, None));
        }
        assert_eq!(editor.count(), 0);
    }

    /// The data slots are the screen's live numbers: the count, the selected cell and that entry's
    /// band and batch, which is what the client draws as the player presses.
    #[test]
    fn the_data_slots_are_the_selected_entry() {
        let mut filters = Filters::new();
        filters.set(torches(4)).expect("room for the first");
        filters.set(sticks()).expect("room for the second");
        let editor = Editor::new(filters, 1);
        assert_eq!(editor.data_slots(), [2, 1, 8, 16, 8]);
        let empty = Editor::new(Filters::new(), 0);
        assert_eq!(
            empty.data_slots(),
            [0, 0, 0, 0, 0],
            "an unconfigured requester shows an empty first cell"
        );
    }

    /// The screen's payload lists the entries' resources in cell order; a payload that is not that
    /// list draws no icons rather than guessing at one.
    #[test]
    fn the_screen_payload_lists_every_entrys_resource() {
        let mut filters = Filters::new();
        filters.set(torches(4)).expect("room for the first");
        filters.set(sticks()).expect("room for the second");
        let payload = Editor::new(filters, 0).extra_data();
        assert_eq!(
            extra_items(payload.as_bytes()),
            vec!["minecraft:torch", "minecraft:stick"]
        );
        assert!(extra_items(b"").is_empty());
        assert!(extra_items(b"nonsense").is_empty(), "a bare id-less line");
        assert_eq!(
            extra_items(b"minecraft:torch\nminecraft:stick\n"),
            vec!["minecraft:torch", "minecraft:stick"]
        );
    }

    /// The screen and `/stepcrafter requester` are one container: an entry the command adds is in
    /// the screen's cells, and an entry the screen makes is in the saved form the command reads.
    #[test]
    fn the_editor_and_the_command_are_one_container() {
        let mut filters = Filters::new();
        filters.set(torches(4)).expect("room for the entry");
        let mut editor = Editor::new(filters, 0);
        assert_eq!(editor.data_slots()[0], 1, "the command's entry is a cell");
        assert!(editor.apply(Press::Cell(1), Some("minecraft:stick")));
        assert!(editor.apply(Press::MaxUp, None), "the new entry's max");
        assert!(editor.apply(Press::BatchUp, None), "and its batch");
        let saved = editor.into_filters();
        let read_back = Filters::decode(&saved.encode());
        assert_eq!(read_back, saved, "the screen's edit is the saved container");
        assert_eq!(read_back.len(), 2);
        assert_eq!(
            read_back.first().map(|e| e.filter().item()),
            Some("minecraft:torch")
        );
        assert_eq!(
            read_back.iter().nth(1).map(|e| e.filter().band()),
            Some(band(0, 1)),
            "the stick entry is what the screen set: max 1"
        );
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
