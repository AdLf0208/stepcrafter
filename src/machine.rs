//! The Step Crafter's game half: its saved state, its inventory and one server tick.
//!
//! [`crate::runtime`] is the pure truth — the maintenance band, the ingredient resolution and the
//! stock arithmetic, with its tests. This module is the only place that reads the game and writes
//! it back: the block entity's saved compound (the target's pattern, the band, the phase and the
//! counter), the 27-slot inventory the stock lives in, and the tick that runs
//! [`crate::runtime::plan`].
//!
//! Upstream's stock is the Refined Storage network and its interface is a screen. Neither exists
//! for a Horizon add-on (see `ports/_shared/evidence/stepcrafter/parent-surface.md`), so the stock
//! is the machine's **own 27-slot inventory** — declared as a container with the vanilla chest
//! menu, so a player opens it by right-clicking and the engine saves and spills it — and the
//! configuration is set with `/stepcrafter`.

use mod_api::{
    BlockEntityTickEvent, Compound, Host, HostTransaction, Identifier, ItemKey, ItemStack,
    LevelRef, ModContainer, Registrar, ServerHost, Tag,
};

use crate::nbt::{amount, drop_key, int, int_of, list, put, string};
use crate::runtime::{self, Band, Outcome, Pattern, Phase, Slot, Slots, Stock};

/// The block entity type this mod declares, and the `id` its carried data is tagged with.
pub const TYPE: &str = "step_crafter";
/// The block whose item carries the machine (the package's one block).
pub const BLOCK: &str = "stepcrafter:step_crafter";
/// The machine's inventory: upstream's stock is the network, this port's is its own slots.
pub const SLOTS: usize = 27;

const KEY_TARGET: &str = "Target";
const KEY_MIN: &str = "Min";
const KEY_MAX: &str = "Max";
const KEY_CELLS: &str = "Cells";
/// What the pattern's consumed items leave behind, `consumed=left:count` per entry.
const KEY_REMAINDERS: &str = "Remainders";
const KEY_RESULT_COUNT: &str = "ResultCount";
const KEY_PHASE: &str = "Phase";
const KEY_CRAFTED: &str = "Crafted";
/// How many items a Step Requester has asked this machine for and not yet been given: the ask a
/// requester writes into this compound (`merge_data`, the `/data merge block` path) and this
/// machine's tick spends one step at a time.
pub const KEY_REQUEST: &str = "Request";
/// Declare the block entity type holding the machine's one block, its 27-slot container and the
/// Part that ticks it.
pub fn declare(r: &mut dyn Registrar) {
    let kind = r.block_entity_type(TYPE).blocks(&["step_crafter"]).build();
    let block = r.block_key(BLOCK);
    r.block_container(&block, container());
    // A refusal here is the game's to report at load; the Part is then unattached and the block
    // keeps no state, which the load log states.
    let _ = r.block_entity_part(&kind, crate::StepCrafter);
}

/// Every face reaches every slot and the vanilla chest menu shows them: the machine's stock is
/// its inventory, and the player loads and empties it through that menu (or a hopper).
fn container() -> ModContainer {
    let all: Vec<usize> = (0..SLOTS).collect();
    ModContainer {
        slots: SLOTS,
        faces: [
            all.clone(),
            all.clone(),
            all.clone(),
            all.clone(),
            all.clone(),
            all,
        ],
        inputs: (0..SLOTS).collect(),
        outputs: (0..SLOTS).collect(),
        menu: Some("minecraft:chest".to_owned()),
    }
}

// ---------------------------------------------------------------------------
// The saved configuration
// ---------------------------------------------------------------------------

/// What one machine is configured to do.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    /// The item kept in stock (the pattern's own result).
    pub target: Option<String>,
    /// The maintenance band, absent until `/stepcrafter configure`.
    pub band: Option<Band>,
    /// The step pattern, absent until `/stepcrafter target`.
    pub pattern: Option<Pattern>,
    /// Where the last step left the machine.
    pub phase: Phase,
    /// How many items a requester has asked for and not yet been given.
    pub request: u64,
    /// How many items this machine has crafted in all (the harness readout).
    pub crafted: u64,
}

impl State {
    /// Whether the machine has both a pattern and a band.
    #[must_use]
    pub fn is_configured(&self) -> bool {
        self.pattern.is_some() && self.band.is_some()
    }
}

/// Read the configuration out of a block entity's saved compound; an empty or foreign compound
/// reads as an unconfigured machine.
#[must_use]
pub fn read(nbt: &Compound) -> State {
    let target = string(nbt, KEY_TARGET);
    let band = match (int(nbt, KEY_MIN), int(nbt, KEY_MAX)) {
        (Some(min), Some(max)) => u64::try_from(min)
            .ok()
            .zip(u64::try_from(max).ok())
            .and_then(|(min, max)| Band::new(min, max)),
        _ => None,
    };
    let result_count = int(nbt, KEY_RESULT_COUNT).and_then(|count| u32::try_from(count).ok());
    let cells = list(nbt, KEY_CELLS);
    let remainders = list(nbt, KEY_REMAINDERS).unwrap_or_default();
    let pattern = match (target.as_deref(), result_count) {
        (Some(target), Some(count)) => {
            cells.and_then(|cells| Pattern::decode(&cells, &remainders, target, count))
        }
        _ => None,
    };
    State {
        target,
        band,
        pattern,
        phase: string(nbt, KEY_PHASE)
            .as_deref()
            .map_or(Phase::Idle, Phase::of),
        request: amount(nbt, KEY_REQUEST),
        crafted: amount(nbt, KEY_CRAFTED),
    }
}

/// Write the configuration back; an unconfigured machine keeps no pattern keys at all.
pub fn write(nbt: &mut Compound, state: &State) {
    match &state.target {
        Some(target) => put(nbt, KEY_TARGET, Tag::String(target.as_str().into())),
        None => drop_key(nbt, KEY_TARGET),
    }
    match state.band {
        Some(band) => {
            put(nbt, KEY_MIN, int_of(band.min()));
            put(nbt, KEY_MAX, int_of(band.max()));
        }
        None => {
            drop_key(nbt, KEY_MIN);
            drop_key(nbt, KEY_MAX);
        }
    }
    match &state.pattern {
        Some(pattern) => {
            put(
                nbt,
                KEY_CELLS,
                Tag::List(
                    pattern
                        .encode()
                        .iter()
                        .map(|cell| Tag::String(cell.as_str().into()))
                        .collect(),
                ),
            );
            let remainders = pattern.encode_remainders();
            if remainders.is_empty() {
                drop_key(nbt, KEY_REMAINDERS);
            } else {
                put(
                    nbt,
                    KEY_REMAINDERS,
                    Tag::List(
                        remainders
                            .iter()
                            .map(|entry| Tag::String(entry.as_str().into()))
                            .collect(),
                    ),
                );
            }
            put(
                nbt,
                KEY_RESULT_COUNT,
                Tag::Int(i32::try_from(pattern.result_count()).unwrap_or(1)),
            );
        }
        None => {
            drop_key(nbt, KEY_CELLS);
            drop_key(nbt, KEY_REMAINDERS);
            drop_key(nbt, KEY_RESULT_COUNT);
        }
    }
    put(nbt, KEY_PHASE, Tag::String(state.phase.id().into()));
    put(nbt, KEY_REQUEST, int_of(state.request));
    put(nbt, KEY_CRAFTED, int_of(state.crafted));
}

// ---------------------------------------------------------------------------
// The inventory, as the runtime's stock
// ---------------------------------------------------------------------------

/// The machine's 27 slots as the rules see them: each stack's item, count and stack limit, with
/// `empty_room` as the room an empty slot offers for the step's result.
#[must_use]
pub fn slots_of(host: &dyn Host, items: &[Option<ItemStack>], empty_room: u32) -> Slots {
    let mut slots = Slots::new(empty_room);
    for stack in items {
        slots.push(stack.as_ref().and_then(|stack| {
            let max = host.max_stack_size(stack.item()).unwrap_or(64).max(1);
            Slot::new(stack.item().id(host), stack.count(), max)
        }));
    }
    slots
}

/// The room an empty slot offers a step's result: the result item's own stack limit.
#[must_use]
pub fn result_room(host: &dyn Host, pattern: Option<&Pattern>) -> u32 {
    pattern
        .and_then(|pattern| item_limit(host, pattern.result()))
        .unwrap_or(64)
        .max(1)
}

/// The machine's 27 slots as the rules see them, with each item a step inserts — the result and
/// what the recipe leaves behind — carrying its own stack limit as the room an empty slot offers it.
#[must_use]
pub fn slots_with_rooms(
    host: &dyn Host,
    items: &[Option<ItemStack>],
    pattern: Option<&Pattern>,
) -> Slots {
    let mut slots = slots_of(host, items, result_room(host, pattern));
    if let Some(pattern) = pattern {
        let inserted = std::iter::once(pattern.result())
            .chain(pattern.remainders().values().map(|(left, _)| left.as_str()));
        for item in inserted {
            if let Some(max) = item_limit(host, item) {
                slots.set_empty_room(item, max);
            }
        }
    }
    slots
}

/// An item's stack limit, or `None` for an id this world does not know.
fn item_limit(host: &dyn Host, item: &str) -> Option<u32> {
    Identifier::parse(item)
        .ok()
        .and_then(|id| host.item_key(&id))
        .and_then(|key| host.max_stack_size(key))
        .map(|max| max.max(1))
}

/// The machine's 27 slots as one stock: how much of each item it holds, in all.
#[must_use]
pub fn stock_of(host: &dyn Host, items: &[Option<ItemStack>]) -> Stock {
    let mut stock = Stock::new();
    for stack in items.iter().flatten() {
        if stack.count() > 0 {
            stock.add(&stack.item().id(host), u64::from(stack.count()));
        }
    }
    stock
}

/// Take `consume` out of the slots: what each entry names, spread over every slot that holds it,
/// oldest slot first, exactly as the runtime's step decided.
pub fn take_from_slots(
    host: &dyn Host,
    items: &mut [Option<ItemStack>],
    consume: &[(String, u64)],
) {
    for (item, count) in consume {
        let mut left = *count;
        for slot in items.iter_mut() {
            if left == 0 {
                break;
            }
            let Some(stack) = slot.as_mut() else {
                continue;
            };
            if stack.item().id(host) != *item {
                continue;
            }
            let taken = u64::from(stack.count()).min(left);
            let remaining = stack.count() - u32::try_from(taken).unwrap_or(stack.count());
            left -= taken;
            if remaining == 0 {
                *slot = None;
            } else {
                let _ = stack.set_count(remaining);
            }
        }
    }
}

/// Put `count` of `item` into the slots: existing stacks first, then the first empty slot.
///
/// A stack is only merged into when its components match too: the step makes a plain item, and
/// growing a named or enchanted stack with it would launder those components onto the result.
/// Answers how many went in (all of them, unless the slots are full).
pub fn put_into_slots(
    host: &dyn Host,
    items: &mut [Option<ItemStack>],
    item: ItemKey,
    count: u32,
) -> u32 {
    let max = host.max_stack_size(item).unwrap_or(64).max(1);
    let plain = ItemStack::of(item);
    let mut left = count;
    for slot in items.iter_mut() {
        if left == 0 {
            break;
        }
        let Some(stack) = slot.as_mut() else {
            continue;
        };
        if !stack.is_stackable_with(&plain) {
            continue;
        }
        let room = max.saturating_sub(stack.count());
        let added = room.min(left);
        if added > 0 && stack.grow(added).is_ok() {
            left -= added;
        }
    }
    for slot in items.iter_mut() {
        if left == 0 {
            break;
        }
        if slot.is_some() {
            continue;
        }
        let added = left.min(max);
        if let Some(stack) = ItemStack::new(item, added) {
            *slot = Some(stack);
            left -= added;
        }
    }
    count - left
}

// ---------------------------------------------------------------------------
// The tick
// ---------------------------------------------------------------------------

/// One server tick of a machine: the maintenance rule against the machine's own inventory.
///
/// The configuration is read from and written to `event.entity.nbt` and the stock lives in
/// `event.entity.items` — the block entity the game hands this tick, whose fields are what the
/// engine saves. A `block_entity(..).replace_data` made during a tick is overwritten when the tick
/// ends, so neither is written that way.
pub fn tick(host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
    let pos = event.pos;
    if event.entity.items.len() < SLOTS {
        event.entity.items.resize(SLOTS, None);
    }
    let mut state = read(&event.entity.nbt);

    // A Step Requester's ask outranks this machine's own band for the tick it is served: the band
    // is this machine's timing rule, an ask is somebody else's, and either way one tick is one step
    // (`STEPS_PER_TICK`). The step itself is the same one `/stepcrafter step` takes — the ask
    // bypasses the band, not the pattern.
    if state.request > 0 {
        serve_request(host, event, &mut state);
        write(&mut event.entity.nbt, &state);
        sync_active(host, pos, state.phase.is_crafting());
        return;
    }

    let Some(band) = state.band else {
        sync_active(host, pos, false);
        return;
    };

    let mut changed = false;
    for _ in 0..runtime::STEPS_PER_TICK {
        let slots = slots_with_rooms(&*host, &event.entity.items, state.pattern.as_ref());
        let outcome = runtime::plan(&slots, band, state.phase, state.pattern.as_ref());
        match &outcome {
            Outcome::Hold => break,
            Outcome::Finished => {
                state.phase = Phase::Idle;
                changed = true;
                break;
            }
            Outcome::Craft { result_count, .. } => {
                apply_craft(host, &mut event.entity.items, &outcome);
                state.crafted = state.crafted.saturating_add(u64::from(*result_count));
                state.phase = Phase::Crafting;
                changed = true;
            }
            Outcome::NotEnoughIngredients { .. } => {
                state.phase = Phase::NotEnoughIngredients;
                changed = true;
                break;
            }
            Outcome::NetworkFull { .. } => {
                state.phase = Phase::NetworkFull;
                changed = true;
                break;
            }
        };
    }

    if changed {
        write(&mut event.entity.nbt, &state);
    }
    sync_active(host, pos, state.phase.is_crafting());
}

/// Serve one item of a requester's ask: one step of the pattern, band bypassed.
///
/// The ask is spent by what the step produced (`Request` counts items, not steps); a step the
/// machine cannot take drops the rest of the ask, so the requester reads the refusal from this
/// machine's `phase` instead of waiting for items that will never come. A refusal produces
/// nothing, so neither `crafted` nor the ask is served by it: only a step that fits spends.
fn serve_request(
    host: &mut dyn ServerHost,
    event: &mut BlockEntityTickEvent<'_>,
    state: &mut State,
) {
    let slots = slots_with_rooms(&*host, &event.entity.items, state.pattern.as_ref());
    let outcome = runtime::step_now(&slots, state.pattern.as_ref());
    match &outcome {
        Outcome::Craft { result_count, .. } => {
            apply_craft(host, &mut event.entity.items, &outcome);
            state.crafted = state.crafted.saturating_add(u64::from(*result_count));
            state.request = state.request.saturating_sub(u64::from(*result_count));
            state.phase = Phase::Crafting;
        }
        Outcome::NotEnoughIngredients { .. } => {
            state.request = 0;
            state.phase = Phase::NotEnoughIngredients;
        }
        Outcome::NetworkFull { .. } => {
            state.request = 0;
            state.phase = Phase::NetworkFull;
        }
        Outcome::Hold | Outcome::Finished => {
            // No pattern configured, nothing to step: the ask cannot be answered.
            state.request = 0;
        }
    }
}

/// Take one step now, whatever the band says: the harness driver that proves a single step without
/// waiting for the stock to fall, and the player's manual step. Answers the outcome's name.
pub fn step_once(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3]) -> String {
    let Some(entity) = host.block_entity(level, pos) else {
        return "no-block-entity".to_owned();
    };
    let Some(mut nbt) = entity.data(&*host) else {
        return "no-block-entity".to_owned();
    };
    let mut items: Vec<Option<ItemStack>> = (0..SLOTS)
        .map(|slot| entity.item_slot(&*host, slot as u32).ok().flatten())
        .collect();
    let mut state = read(&nbt);
    let Some(_band) = state.band else {
        return "unconfigured".to_owned();
    };
    let slots = slots_with_rooms(&*host, &items, state.pattern.as_ref());
    let outcome = runtime::step_now(&slots, state.pattern.as_ref());
    match &outcome {
        Outcome::Craft { result_count, .. } => {
            apply_craft(host, &mut items, &outcome);
            state.crafted = state.crafted.saturating_add(u64::from(*result_count));
            state.phase = Phase::Crafting;
            write(&mut nbt, &state);
            let _ = entity.replace_data(&mut *host, nbt);
            replace_items(host, entity, &items);
            "crafted".to_owned()
        }
        Outcome::Hold => "held".to_owned(),
        Outcome::Finished => "finished".to_owned(),
        Outcome::NotEnoughIngredients { .. } => "not-enough-ingredients".to_owned(),
        Outcome::NetworkFull { .. } => "network-full".to_owned(),
    }
}

/// Commit one step: the ingredients out, then the result and what the recipe leaves in.
fn apply_craft(host: &mut dyn ServerHost, items: &mut [Option<ItemStack>], outcome: &Outcome) {
    let Outcome::Craft {
        consume,
        result,
        result_count,
        remainders,
    } = outcome
    else {
        return;
    };
    take_from_slots(&*host, items, consume);
    put_item(host, items, result, *result_count);
    for (item, count) in remainders {
        put_item(host, items, item, u32::try_from(*count).unwrap_or(u32::MAX));
    }
}

/// Write a machine's inventory back, one transactional slot write per changed slot.
pub fn replace_items(
    host: &mut dyn ServerHost,
    entity: mod_api::BlockEntityRef,
    items: &[Option<ItemStack>],
) {
    let committed: Result<(), ()> = host.transaction(|host, tx| {
        for (slot, stack) in items.iter().enumerate() {
            let _ = entity.set_item_slot(host, tx, slot as u32, stack.clone());
        }
        Ok(())
    });
    if committed.is_err() {
        return;
    }
    let _ = entity.set_changed(host);
}

/// Put one item into slots: existing stacks that hold the same components first, then the first
/// empty slot. The id comes from the pattern, so an id this world does not know puts nothing in
/// (the step's own room check already held the space). Used for the result and for what the recipe
/// leaves.
fn put_item(
    host: &mut dyn ServerHost,
    items: &mut [Option<ItemStack>],
    item: &str,
    count: u32,
) {
    if let Ok(id) = Identifier::parse(item)
        && let Some(key) = host.item_key(&id)
    {
        put_into_slots(&*host, items, key, count);
    }
}

/// The `active` state shows the machine is crafting (upstream's `active` is the network node's;
/// this machine has no network, so its own run is what it reports).
pub(crate) fn sync_active(host: &mut dyn ServerHost, pos: [i32; 3], active: bool) {
    let _ = host.set_block_property(pos, "active", if active { "true" } else { "false" });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The saved compound is the machine's configuration: the readout and the tick both read it
    /// back, so a write that does not round-trip loses the pattern or the band.
    #[test]
    fn the_state_round_trips_through_the_saved_compound() {
        let mut nbt = Compound::new();
        let state = State {
            target: Some("minecraft:torch".to_owned()),
            band: Band::new(8, 16),
            pattern: Pattern::new(
                vec![
                    vec!["minecraft:coal".to_owned(), "minecraft:charcoal".to_owned()],
                    vec!["minecraft:stick".to_owned()],
                ],
                "minecraft:torch".to_owned(),
                4,
            )
            .map(|pattern| {
                pattern.with_remainders(std::collections::BTreeMap::from([(
                    "minecraft:coal".to_owned(),
                    ("minecraft:coal".to_owned(), 1),
                )]))
            }),
            phase: Phase::Crafting,
            request: 4,
            crafted: 12,
        };
        write(&mut nbt, &state);
        assert_eq!(read(&nbt), state);
        assert_eq!(
            read(&nbt).request,
            4,
            "an outstanding ask must survive the compound between ticks"
        );
        let saved = read(&nbt);
        let left = saved
            .pattern
            .as_ref()
            .and_then(|pattern| pattern.remainder_of("minecraft:coal"));
        assert_eq!(
            left,
            Some(("minecraft:coal", 1)),
            "what the recipe leaves must survive the compound too"
        );
    }

    #[test]
    fn an_empty_compound_reads_as_an_unconfigured_machine() {
        let state = read(&Compound::new());
        assert!(!state.is_configured());
        assert_eq!(state.phase, Phase::Idle);
    }
}
