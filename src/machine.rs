//! The Step Crafter's game half: its saved state, its inventory and one server tick.
//!
//! [`crate::runtime`] is the pure truth — the maintenance band, the ingredient resolution and the
//! stock arithmetic, with its tests. This module is the only place that reads the game and writes
//! it back: the block entity's saved compound (the target's pattern, the band, the phase, the asks
//! requesters have left here with what each has been served, and the counter), the 27-slot
//! inventory the stock lives in, and the tick that runs [`crate::runtime::plan`] or serves the head
//! ask of the queue one step at a time.
//!
//! Upstream's stock is the Refined Storage network and its interface is a screen. Neither exists
//! for a Horizon add-on (see `ports/_shared/evidence/stepcrafter/parent-surface.md`), so the stock
//! is the machine's **own 27-slot inventory** — declared as a container with the vanilla chest
//! menu, so a player opens it by right-clicking and the engine saves and spills it — and the
//! configuration is set with `/stepcrafter`.

use mod_api::{
    BlockClick, BlockEntityTickEvent, BlockState, Compound, Direction, Flow, Hand, Host, HostSide,
    HostTransaction, Identifier, InteractionResult, ItemKey, ItemStack, LevelHost, LevelRef,
    MenuButtonEvent, ModContainer, Registrar, RightClickBlockEvent, ServerHost,
    StateForPlacementEvent, Tag,
};

use crate::nbt::{amount, drop_key, int, int_of, list, put, string};
use crate::runtime::{
    self, Band, Outcome, Pattern, Phase, Queue, Slot, Slots, Stock, TargetEntry, Targets,
};

/// The block entity type this mod declares, and the `id` its carried data is tagged with.
pub const TYPE: &str = "step_crafter";
/// The block whose item carries the machine (the package's one block).
pub const BLOCK: &str = "stepcrafter:step_crafter";
/// The menu type the content declares (`data/stepcrafter/horizon/menu/step_crafter_config.json`):
/// the 27 stock slots and the two data slots the screen reads.
pub const MENU: &str = "stepcrafter:step_crafter_config";
/// The machine's inventory: upstream's stock is the network, this port's is its own slots.
pub const SLOTS: usize = 27;
/// The 45-slot pattern container, one `item|min|max|batch|phase|crafted|rc|cells|remainders` string
/// per maintained resource — upstream's `TAG_PATTERNS` list of `ResourceMinMaxAmount` slots.
const KEY_TARGETS: &str = "Targets";
/// The cell the machine's panel last focused; the band buttons step this slot.
const KEY_SELECTED: &str = "Selected";
const KEY_TARGET: &str = "Target";
const KEY_MIN: &str = "Min";
const KEY_MAX: &str = "Max";
const KEY_CELLS: &str = "Cells";
/// What the pattern's consumed items leave behind, `consumed=left:count` per entry.
const KEY_REMAINDERS: &str = "Remainders";
const KEY_RESULT_COUNT: &str = "ResultCount";
const KEY_PHASE: &str = "Phase";
const KEY_CRAFTED: &str = "Crafted";
/// The asks a Step Requester has left with this machine, one `x,y,z|item|remaining` per (requester,
/// item): the list a requester writes into this compound (`merge_data`, the `/data merge block`
/// path) and this machine's tick serves one step at a time.
pub const KEY_REQUESTS: &str = "Requests";
/// What each requester has been served, `x,y,z|item|count`: its own share of the stock.
const KEY_SERVED: &str = "Served";
/// The ask this machine last could not serve, `x,y,z|item`: upstream's failed task belongs to the
/// requester that started it, so the asks behind it are not touched.
const KEY_FAILED: &str = "Failed";
/// 0.2.0–0.2.6's single ask. Operational state, not configuration: a world saved with it loses one
/// pending ask, and its requester re-asks on the next tick.
const KEY_REQUEST: &str = "Request";
/// Declare the block entity type holding the machine's one block, its 27-slot container and the
/// Part that ticks it.
pub fn declare(r: &mut dyn Registrar) {
    let kind = r.block_entity_type(TYPE).blocks(&["step_crafter"]).build();
    let block = r.block_key(BLOCK);
    r.block_container(&block, container());
    // A refusal here is the game's to report at load; the Part is then unattached and the block
    // keeps no state, which the load log states.
    let _ = r.block_entity_part(&kind, crate::StepCrafter);
    // The placement-facing Part answers `state_for_placement` for this block only: the machine
    // takes the face it was placed against (upstream's `FACE_CLICKED`), not the placer's yaw.
    r.block_part(block, crate::StepCrafterPlacement);
}

/// The face name of the pin's `Direction`, in the jar's blockstate spelling.
fn face_name(face: Direction) -> &'static str {
    match face {
        Direction::Down => "down",
        Direction::Up => "up",
        Direction::North => "north",
        Direction::South => "south",
        Direction::West => "west",
        Direction::East => "east",
    }
}

/// The `state_for_placement` rule: a machine placed against a face takes that face as its
/// `direction`. Upstream's `StepCrafterBlock.getDirectionType()` is
/// `DefaultDirectionType.FACE_CLICKED`, so the clicked face — not the placer's yaw — is the value.
/// Answer only for our block: every placement goes through this hook, and a state of another
/// block is ignored by the engine, but claiming it here would stop the next mod and vanilla.
pub fn state_for_placement(
    level: &dyn LevelHost,
    event: &mut StateForPlacementEvent<'_>,
) -> Option<Option<BlockState>> {
    if level.state_block(event.state) != Some(BLOCK) {
        return None;
    }
    let value = runtime::direction_for_face(face_name(event.context.face))?;
    level
        .state_with_property(event.state, "direction", value)
        .map(Some)
}

/// Every face reaches every slot and the machine's own menu shows them: the machine's stock is its
/// inventory, and the player loads and empties it through that menu (or a hopper). The menu is this
/// package's type, so the screen can draw the target cell and the band's buttons beside the slots.
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
        // The *menu type*, not the block: a chest's menu is `minecraft:generic_9x3`; this one has
        // the same 27 slots and the screen that draws the configuration over them.
        menu: Some(MENU.to_owned()),
    }
}

// ---------------------------------------------------------------------------
// The saved configuration
// ---------------------------------------------------------------------------

/// What one machine is configured to do: up to 45 pattern slots, each with its own target, band,
/// pattern and status, plus the ask queue this machine serves for nearby requesters.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    /// The pattern slots — upstream's 45-slot pattern container, the truth of the configuration.
    pub targets: Targets,
    /// The cell the panel last focused (the band buttons edit this slot).
    pub selected: usize,
    /// Where the last step left the machine as a whole.
    pub phase: Phase,
    /// The asks requesters have left here, in the order this machine serves them.
    pub queue: Queue,
    /// How many items this machine has crafted in all (the harness readout).
    pub crafted: u64,
}

impl State {
    /// Whether at least one pattern slot is configured.
    #[must_use]
    pub fn is_configured(&self) -> bool {
        !self.targets.is_empty()
    }

    /// The focused slot, or the first when no cell was focused.
    #[must_use]
    pub fn selected_entry(&self) -> Option<&TargetEntry> {
        self.targets
            .entry(self.selected)
            .or_else(|| self.targets.entry(0))
    }

    /// The focused slot, or the first when no cell was focused.
    pub fn selected_entry_mut(&mut self) -> Option<&mut TargetEntry> {
        let index = self.targets.entry(self.selected).map_or_else(
            || (!self.targets.is_empty()).then_some(0),
            |_| Some(self.selected),
        );
        index.and_then(|index| self.targets.entry_mut(index))
    }

    /// The item the focused slot maintains (the legacy single-target view).
    #[must_use]
    pub fn target(&self) -> Option<&str> {
        self.selected_entry().map(TargetEntry::item)
    }

    /// The focused slot's band (the legacy single-band view).
    #[must_use]
    pub fn band(&self) -> Option<Band> {
        self.selected_entry().and_then(TargetEntry::band)
    }

    /// The focused slot's pattern (the legacy single-pattern view).
    #[must_use]
    pub fn pattern(&self) -> Option<&Pattern> {
        self.selected_entry().map(TargetEntry::pattern)
    }
}

/// The part of a machine's saved state another block reads: what it maintains (every pattern slot),
/// where its last step left it, and the asks it holds.
///
/// A Step Requester reads this every tick and writes the ask list back; the patterns, the bands and
/// the stock stay the machine's own.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Shared {
    /// The first slot's item, kept for the 0.2.1–0.2.8 link and readout.
    pub target: Option<String>,
    /// Every item this machine maintains, for a requester's link scan.
    pub targets: Vec<String>,
    /// Where the machine's last step left it.
    pub phase: Phase,
    /// The asks it holds.
    pub queue: Queue,
}

/// Read the shared part of a block entity's saved compound.
#[must_use]
pub fn shared(nbt: &Compound) -> Shared {
    let targets = Targets::decode(&list(nbt, KEY_TARGETS).unwrap_or_default());
    Shared {
        target: targets
            .entry(0)
            .map(|entry| entry.item().to_owned())
            .or_else(|| string(nbt, KEY_TARGET)),
        targets: targets
            .iter()
            .map(|entry| entry.item().to_owned())
            .collect(),
        phase: string(nbt, KEY_PHASE)
            .as_deref()
            .map_or(Phase::Idle, Phase::of),
        queue: Queue::decode(
            &list(nbt, KEY_REQUESTS).unwrap_or_default(),
            &list(nbt, KEY_SERVED).unwrap_or_default(),
            string(nbt, KEY_FAILED).as_deref(),
        ),
    }
}

/// Read the configuration out of a block entity's saved compound; an empty or foreign compound
/// reads as an unconfigured machine. A 0.2.0–0.2.8 compound's single target becomes one slot.
#[must_use]
pub fn read(nbt: &Compound) -> State {
    let shared = shared(nbt);
    let mut targets = Targets::decode(&list(nbt, KEY_TARGETS).unwrap_or_default());
    if targets.is_empty() {
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
        if let (Some(target), Some(count), Some(cells)) =
            (shared.target.as_deref(), result_count, cells)
            && let Some(pattern) = Pattern::decode(&cells, &remainders, target, count)
            && let Some(entry) = TargetEntry::new(target.to_owned(), band, 1, pattern)
        {
            let _ = targets.set(entry);
        }
    }
    let selected = int(nbt, KEY_SELECTED)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0)
        .min(targets.len().saturating_sub(1));
    State {
        targets,
        selected,
        phase: shared.phase,
        queue: shared.queue,
        crafted: amount(nbt, KEY_CRAFTED),
    }
}

/// Write the configuration back; an unconfigured machine keeps no pattern keys at all.
pub fn write(nbt: &mut Compound, state: &State) {
    let encoded = state.targets.encode();
    if encoded.is_empty() {
        drop_key(nbt, KEY_TARGETS);
    } else {
        put(
            nbt,
            KEY_TARGETS,
            Tag::List(
                encoded
                    .iter()
                    .map(|entry| Tag::String(entry.as_str().into()))
                    .collect(),
            ),
        );
    }
    put(nbt, KEY_SELECTED, int_of(state.selected as u64));
    // The focused slot is mirrored into the legacy single-target keys so a requester's `shared`
    // read and the 0.2.1–0.2.8 tooling see the same configuration. The list above is the truth.
    match state.selected_entry() {
        Some(entry) => {
            put(nbt, KEY_TARGET, Tag::String(entry.item().into()));
            match entry.band() {
                Some(band) => {
                    put(nbt, KEY_MIN, int_of(band.min()));
                    put(nbt, KEY_MAX, int_of(band.max()));
                }
                None => {
                    drop_key(nbt, KEY_MIN);
                    drop_key(nbt, KEY_MAX);
                }
            }
            put(
                nbt,
                KEY_CELLS,
                Tag::List(
                    entry
                        .pattern()
                        .encode()
                        .iter()
                        .map(|cell| Tag::String(cell.as_str().into()))
                        .collect(),
                ),
            );
            let remainders = entry.pattern().encode_remainders();
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
                Tag::Int(i32::try_from(entry.pattern().result_count()).unwrap_or(1)),
            );
        }
        None => {
            drop_key(nbt, KEY_TARGET);
            drop_key(nbt, KEY_MIN);
            drop_key(nbt, KEY_MAX);
            drop_key(nbt, KEY_CELLS);
            drop_key(nbt, KEY_REMAINDERS);
            drop_key(nbt, KEY_RESULT_COUNT);
        }
    }
    put(nbt, KEY_PHASE, Tag::String(state.phase.id().into()));
    let requests = state.queue.encode();
    if requests.is_empty() {
        drop_key(nbt, KEY_REQUESTS);
    } else {
        put(
            nbt,
            KEY_REQUESTS,
            Tag::List(
                requests
                    .iter()
                    .map(|entry| Tag::String(entry.as_str().into()))
                    .collect(),
            ),
        );
    }
    let served = state.queue.encode_served();
    if served.is_empty() {
        drop_key(nbt, KEY_SERVED);
    } else {
        put(
            nbt,
            KEY_SERVED,
            Tag::List(
                served
                    .iter()
                    .map(|entry| Tag::String(entry.as_str().into()))
                    .collect(),
            ),
        );
    }
    match state.queue.failed() {
        Some(failed) => put(
            nbt,
            KEY_FAILED,
            Tag::String(failed.encode().as_str().into()),
        ),
        None => drop_key(nbt, KEY_FAILED),
    }
    // 0.2.6's single ask is superseded by the list; the next tick's list is the only truth.
    drop_key(nbt, KEY_REQUEST);
    put(nbt, KEY_CRAFTED, int_of(state.crafted));
}

// ---------------------------------------------------------------------------
// The inventory, as the runtime's stock
// ---------------------------------------------------------------------------

/// The machine's 27 slots as the rules see them: each stack's item, count and stack limit, with
/// `empty_room` as the room an empty slot offers for the step's result.
///
/// A stack carrying components (a name, an enchantment, damage) is a [`Slot::patched`] slot: the
/// step inserts plain items, so that stack is no room for the result. The predicate is the one the
/// real insert uses ([`ItemStack::is_stackable_with`] against the plain stack), so the room check
/// and [`put_into_slots`] cannot disagree about what a slot will take.
#[must_use]
pub fn slots_of(host: &dyn Host, items: &[Option<ItemStack>], empty_room: u32) -> Slots {
    let mut slots = Slots::new(empty_room);
    for stack in items {
        slots.push(stack.as_ref().and_then(|stack| {
            let key = stack.item();
            let max = host.max_stack_size(key).unwrap_or(64).max(1);
            if stack.is_stackable_with(&ItemStack::of(key)) {
                Slot::new(key.id(host), stack.count(), max)
            } else {
                Slot::patched(key.id(host), stack.count(), max)
            }
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
    // The screen's numbers are the menu's data slots, a view and not saved: publish the focused
    // slot's band every tick, so a reopened menu and its buttons agree with the compound commands
    // write.
    event.entity.data = runtime::machine_data_slots(state.band()).to_vec();
    let before = state.clone();

    // An ask that can never be served is dropped before anything is spent on it: one for an item
    // no slot maintains (a re-targeted machine), or one whose requester is gone (a broken block).
    // A requester whose chunk is out this tick is not "gone" — its ask waits.
    if let Some(level) = host.level(host.dimension()) {
        let maintained: Vec<String> = state
            .targets
            .iter()
            .map(|entry| entry.item().to_owned())
            .collect();
        state.queue.retain(|ask| {
            maintained.iter().any(|item| item == ask.item())
                && requester_present(&*host, level, ask.requester())
        });
    }

    // A Step Requester's ask outranks this machine's own bands for the tick it is served: a band is
    // this machine's timing rule, an ask is somebody else's, and either way one tick is one step
    // (`STEPS_PER_TICK`). The head ask is served with the slot that maintains its item.
    if !state.queue.is_empty() {
        serve_request(host, event, &mut state);
        // Write only a state that moved: a requester that wrote an ask later in this same tick
        // must not have it erased by a write that was read before it landed.
        if state != before {
            write(&mut event.entity.nbt, &state);
        }
        sync_active(host, pos, state.phase.is_crafting());
        return;
    }

    // Upstream walks every configured pattern slot in turn while its step budget lasts. This port
    // keeps that order and spends one step this tick on the first slot that needs one; a slot whose
    // stock is inside its band holds, letting the next slot use the step.
    let mut changed = state != before;
    for _ in 0..runtime::STEPS_PER_TICK {
        let mut stepped = false;
        for entry in state.targets.iter_mut() {
            // A slot with no band is configured but idle: upstream's pattern slot with no min/max
            // does nothing until an operator sets the maintenance range.
            let Some(band) = entry.band() else {
                continue;
            };
            let outcome = {
                let slots = slots_with_rooms(&*host, &event.entity.items, Some(entry.pattern()));
                runtime::plan(&slots, band, entry.phase(), Some(entry.pattern()))
            };
            match &outcome {
                Outcome::Hold => continue,
                Outcome::Finished => {
                    entry.set_phase(Phase::Idle);
                    changed = true;
                    break;
                }
                Outcome::Craft { result_count, .. } => {
                    if apply_craft(host, &mut event.entity.items, &outcome) {
                        entry.add_crafted(u64::from(*result_count));
                        state.crafted = state.crafted.saturating_add(u64::from(*result_count));
                        entry.set_phase(Phase::Crafting);
                    } else {
                        // The real merge rule had no room the room model counted: refuse the step
                        // instead of consuming its ingredients and losing the result.
                        entry.set_phase(Phase::NetworkFull);
                    }
                    changed = true;
                    stepped = true;
                    break;
                }
                Outcome::NotEnoughIngredients { .. } => {
                    entry.set_phase(Phase::NotEnoughIngredients);
                    changed = true;
                    break;
                }
                Outcome::NetworkFull { .. } => {
                    entry.set_phase(Phase::NetworkFull);
                    changed = true;
                    break;
                }
            }
        }
        if !stepped {
            break;
        }
    }

    // The machine's own phase is the aggregate of its slots: crafting when any is, the refusal a
    // slot last met otherwise, idle when they are all inside their bands.
    state.phase = if state
        .targets
        .iter()
        .any(|entry| entry.phase().is_crafting())
    {
        Phase::Crafting
    } else {
        state
            .targets
            .iter()
            .find(|entry| entry.phase().is_refusal())
            .map_or(Phase::Idle, TargetEntry::phase)
    };
    if changed {
        write(&mut event.entity.nbt, &state);
    }
    sync_active(host, pos, state.phase.is_crafting());
}

/// Whether a queued ask's requester is still a Step Requester this world can see. A block the level
/// cannot read (an unloaded chunk) keeps its ask; one that reads as another block (or air) has lost
/// it, and the ask goes with the block.
fn requester_present(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> bool {
    if !level.is_loaded(host, pos) {
        return true;
    }
    level
        .block(host, pos)
        .is_none_or(|block| block.id(host) == crate::requester::BLOCK)
}

/// Serve the head ask with one step of the pattern, band bypassed.
///
/// The step is spent on the head ask and credited to **its** requester (`Served`); a step the
/// machine cannot take names that ask as the failed one and leaves it queued, so the asks behind it
/// are not lost with it — upstream's failed task is the requester's own, not the network's.
fn serve_request(
    host: &mut dyn ServerHost,
    event: &mut BlockEntityTickEvent<'_>,
    state: &mut State,
) {
    let Some(item) = state.queue.head().map(|head| head.item().to_owned()) else {
        return;
    };
    let Some(index) = state.targets.iter().position(|entry| entry.item() == item) else {
        // No slot maintains the head's item any more: the ask waits for reconfiguration.
        return;
    };
    let outcome = {
        let entry = state
            .targets
            .entry_mut(index)
            .expect("the head's item has a slot");
        let slots = slots_with_rooms(&*host, &event.entity.items, Some(entry.pattern()));
        runtime::step_now(&slots, Some(entry.pattern()))
    };
    let entry = state
        .targets
        .entry_mut(index)
        .expect("the head's item has a slot");
    match &outcome {
        Outcome::Craft { result_count, .. } => {
            if apply_craft(host, &mut event.entity.items, &outcome) {
                entry.add_crafted(u64::from(*result_count));
                state.crafted = state.crafted.saturating_add(u64::from(*result_count));
                state.queue.spend(u64::from(*result_count));
                entry.set_phase(Phase::Crafting);
                state.phase = Phase::Crafting;
            } else {
                entry.set_phase(Phase::NetworkFull);
                state.phase = Phase::NetworkFull;
                state.queue.fail();
            }
        }
        Outcome::NotEnoughIngredients { .. } => {
            entry.set_phase(Phase::NotEnoughIngredients);
            state.phase = Phase::NotEnoughIngredients;
            state.queue.fail();
        }
        Outcome::NetworkFull { .. } => {
            entry.set_phase(Phase::NetworkFull);
            state.phase = Phase::NetworkFull;
            state.queue.fail();
        }
        // Nothing to serve: the ask waits for the machine to be configured.
        Outcome::Hold | Outcome::Finished => {}
    }
}

/// Take one step now, whatever the band says: the harness driver that proves a single step without
/// waiting for the stock to fall, and the player's manual step. Answers the outcome's name.
///
/// A refusal is recorded in the machine's `Phase` exactly as a tick-driven attempt records it: the
/// readout means "where the last step left this machine", and a manual step is a step (0.2.4: the
/// component-room refusal has to be readable as `phase=network-full`).
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
    if !state.is_configured() {
        return "unconfigured".to_owned();
    }
    let outcome = {
        let pattern = state.pattern();
        let slots = slots_with_rooms(&*host, &items, pattern);
        runtime::step_now(&slots, pattern)
    };
    let result = match &outcome {
        Outcome::Craft { result_count, .. } => {
            if apply_craft(host, &mut items, &outcome) {
                if let Some(entry) = state.selected_entry_mut() {
                    entry.add_crafted(u64::from(*result_count));
                    entry.set_phase(Phase::Crafting);
                }
                state.crafted = state.crafted.saturating_add(u64::from(*result_count));
                state.phase = Phase::Crafting;
                "crafted"
            } else {
                if let Some(entry) = state.selected_entry_mut() {
                    entry.set_phase(Phase::NetworkFull);
                }
                state.phase = Phase::NetworkFull;
                "network-full"
            }
        }
        // Nothing to record: no pattern, or a run that already ended.
        Outcome::Hold => return "held".to_owned(),
        Outcome::Finished => return "finished".to_owned(),
        Outcome::NotEnoughIngredients { .. } => {
            if let Some(entry) = state.selected_entry_mut() {
                entry.set_phase(Phase::NotEnoughIngredients);
            }
            state.phase = Phase::NotEnoughIngredients;
            "not-enough-ingredients"
        }
        Outcome::NetworkFull { .. } => {
            if let Some(entry) = state.selected_entry_mut() {
                entry.set_phase(Phase::NetworkFull);
            }
            state.phase = Phase::NetworkFull;
            "network-full"
        }
    };
    write(&mut nbt, &state);
    let _ = entity.replace_data(&mut *host, nbt);
    replace_items(host, entity, &items);
    result.to_owned()
}

/// Commit one step: the ingredients out, then the result and what the recipe leaves in.
///
/// This is the dry run the room model promises, made with the **real** merge rule: the whole
/// outcome is applied to a copy of the slots first, and every insert must go in completely. If one
/// does not, the step is refused with nothing consumed — a result the machine counted as crafted
/// can never be silently lost (the 0.2.4 component-agnostic room defect). Answers whether it
/// committed.
fn apply_craft(
    host: &mut dyn ServerHost,
    items: &mut [Option<ItemStack>],
    outcome: &Outcome,
) -> bool {
    let Outcome::Craft {
        consume,
        result,
        result_count,
        remainders,
    } = outcome
    else {
        return false;
    };
    let mut after = items.to_vec();
    take_from_slots(&*host, &mut after, consume);
    let inserted = std::iter::once((result, *result_count)).chain(
        remainders
            .iter()
            .map(|(item, count)| (item, u32::try_from(*count).unwrap_or(u32::MAX))),
    );
    for (item, count) in inserted {
        let Some(key) = item_key(&*host, item) else {
            return false;
        };
        if put_into_slots(&*host, &mut after, key, count) != count {
            return false;
        }
    }
    for (slot, stack) in items.iter_mut().zip(after) {
        *slot = stack;
    }
    true
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

/// The key of an item id this world knows, or nothing.
fn item_key(host: &dyn Host, item: &str) -> Option<ItemKey> {
    Identifier::parse(item)
        .ok()
        .and_then(|id| host.item_key(&id))
}

/// One press on the machine's own screen, on the server (`menu_button`): the target cell takes the
/// item the player carries — resolved into that item's step pattern exactly as `/stepcrafter
/// target` does — and the four buttons step the band. Both write the compound `machine::write`
/// writes for the command, so the screen and `/stepcrafter` cannot disagree.
pub fn press(host: &mut dyn ServerHost, event: &mut MenuButtonEvent<'_>) {
    let Some(press) = runtime::MachinePress::of(event.context.id) else {
        return;
    };
    let carried = {
        let host: &dyn Host = &*host;
        event.carried.as_ref().map(|stack| stack.item().id(host))
    };
    // A carried item on a target cell must have a step to take: a pattern the machine could never
    // craft is no configuration, exactly as `/stepcrafter target` refuses an item with no
    // `minecraft:crafting` recipe. Resolve before touching the saved container.
    let resolved = match (press, carried.as_deref()) {
        (runtime::MachinePress::Cell(_), Some(item)) => {
            match crate::pattern::resolve(&*host, item) {
                Ok(pattern) => Some(pattern),
                Err(_) => return,
            }
        }
        _ => None,
    };
    let mut state = read(&event.entity.nbt);
    let mut editor = runtime::MachineEditor::new(state.targets.clone(), state.selected);
    if !editor.apply(press, carried.as_deref()) {
        return;
    }
    let (targets, selected) = editor.into_parts();
    state.targets = targets;
    state.selected = selected;
    // The editor put a provisional recipe in a newly created slot; replace it with the resolved one
    // so a repointed cell keeps the band but maintains the item the player actually clicked.
    if let (Some(pattern), Some(entry)) = (resolved, state.targets.entry_mut(selected)) {
        entry.set_pattern(pattern);
    }
    state.phase = Phase::Idle;
    write(&mut event.entity.nbt, &state);
    // Publish at once rather than waiting for the next tick: the client redraws the number the
    // player just stepped.
    event.entity.data = runtime::machine_data_slots(state.band()).to_vec();
}

/// The machine's screen open payload (`menu_extra_data`): the target's item id, so the client draws
/// the target cell's icon. The band travels in the menu's data slots; the saved compound is the
/// truth for both.
#[must_use]
pub fn extra_data(host: &dyn Host, level: LevelRef, pos: [i32; 3], menu: &str) -> Option<Vec<u8>> {
    if menu != MENU {
        return None;
    }
    let entity = host.block_entity(level, pos)?;
    let data = entity.data(host)?;
    let state = read(&data);
    Some(runtime::machine_extra_data(&state.targets, state.selected).into_bytes())
}

/// A player's right click on one of this package's two blocks opens its menu.
///
/// **The engine never opens a mod container's menu on its own**, and its empty-hand hook does not
/// fire for a mod block's click at this pin: measured in game on this package, with an empty hand
/// and with an item, a right click on both blocks left `ctl screen` at `playing` — the block's
/// class reaches the engine's own container openers only. The route the campaign has green in game
/// (`bibliocraft`, `rftoolsstorage`, `functionalstorage`) is the mod asking for its own menu:
/// `PlayerRef::open_menu(server, entity)`. The block's template class is therefore not the gate;
/// the call is. Both sides consume the click, so a held item is not used behind the menu.
pub(crate) fn right_click(host: HostSide<'_>, event: &mut RightClickBlockEvent<'_>) -> Flow {
    if event.cancelled {
        return Flow::Continue;
    }
    let click = event.click;
    if click.hand != Hand::Main {
        return Flow::Continue;
    }
    match host {
        HostSide::Client(client) => {
            if !is_ours(&*client, click) {
                return Flow::Continue;
            }
            *event.cancellation_result = InteractionResult::Success;
            Flow::Cancel
        }
        HostSide::Server(server) => {
            if !is_ours(&*server, click) {
                return Flow::Continue;
            }
            let Some(level) = server.level(server.dimension()) else {
                return Flow::Continue;
            };
            let Some(entity) = server.block_entity(level, click.pos) else {
                return Flow::Continue;
            };
            let Some(player) = server.player(click.player) else {
                return Flow::Continue;
            };
            if !player.open_menu(server, entity).unwrap_or(false) {
                return Flow::Continue;
            }
            *event.cancellation_result = InteractionResult::Success;
            Flow::Cancel
        }
    }
}

/// Whether the clicked block is the machine or the requester: both open a menu on a click.
fn is_ours(host: &dyn Host, click: &BlockClick<'_>) -> bool {
    let Some(level) = host.level(click.dimension.key().id()) else {
        return false;
    };
    level
        .block(host, click.pos)
        .map(|block| {
            let id = block.id(host);
            id == BLOCK || id == crate::requester::BLOCK
        })
        .unwrap_or(false)
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
    /// One saved slot with a one-cell pattern; the band is what these compound tests check.
    fn slot(item: &str, band: Band) -> TargetEntry {
        let pattern =
            Pattern::new(vec![vec![item.to_owned()]], item.to_owned(), 1).expect("a test pattern");
        TargetEntry::new(item.to_owned(), Some(band), 1, pattern).expect("a test slot")
    }

    /// back, so a write that does not round-trip loses the pattern or the band.
    #[test]
    fn the_state_round_trips_through_the_saved_compound() {
        let mut nbt = Compound::new();
        let pattern = Pattern::new(
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
        })
        .expect("a test pattern");
        let mut targets = Targets::new();
        assert!(
            targets
                .set(
                    TargetEntry::new(
                        "minecraft:torch".to_owned(),
                        Some(Band::new(8, 16).expect("a band")),
                        1,
                        pattern,
                    )
                    .expect("a test slot")
                )
                .is_ok()
        );
        let state = State {
            targets,
            selected: 0,
            phase: Phase::Crafting,
            queue: {
                let mut queue = Queue::new();
                assert!(
                    queue.ask([222, 64, 0], "minecraft:torch", 4),
                    "a queued ask"
                );
                assert!(
                    queue.ask([222, 64, 2], "minecraft:torch", 4),
                    "a second asker"
                );
                queue
            },
            crafted: 12,
        };
        write(&mut nbt, &state);
        assert_eq!(read(&nbt), state);
        assert_eq!(
            read(&nbt)
                .queue
                .outstanding([222, 64, 0], "minecraft:torch"),
            4,
            "a queued ask must survive the compound between ticks"
        );
        assert_eq!(
            read(&nbt).queue.len(),
            2,
            "the second requester's ask is not overwritten"
        );
        let saved = read(&nbt);
        let left = saved
            .pattern()
            .and_then(|pattern| pattern.remainder_of("minecraft:coal"));
        assert_eq!(
            left,
            Some(("minecraft:coal", 1)),
            "what the recipe leaves must survive the compound too"
        );
    }

    /// The two slots and their different bands survive the same compound; a machine that saves only
    /// its first target loses the second on reload.
    #[test]
    fn the_two_targets_round_trip_through_the_saved_compound() {
        let mut nbt = Compound::new();
        let mut targets = Targets::new();
        assert!(
            targets
                .set(slot("minecraft:torch", Band::new(8, 16).expect("a band")))
                .is_ok()
        );
        assert!(
            targets
                .set(slot("minecraft:stick", Band::new(16, 32).expect("a band")))
                .is_ok()
        );
        let state = State {
            targets,
            selected: 1,
            phase: Phase::Idle,
            queue: Queue::new(),
            crafted: 8,
        };
        write(&mut nbt, &state);
        let saved = read(&nbt);
        assert_eq!(saved.targets.len(), 2, "both pattern slots survive");
        assert_eq!(
            saved.targets.entry(0).and_then(TargetEntry::band),
            Some(Band::new(8, 16).expect("a band"))
        );
        assert_eq!(
            saved.targets.entry(1).and_then(TargetEntry::band),
            Some(Band::new(16, 32).expect("a band"))
        );
        assert_eq!(saved.selected, 1, "the focused cell survives");
        assert_eq!(saved, state);
    }

    /// A 0.2.0–0.2.8 compound with one target and a band reads as one slot, so an existing world is
    /// configured as it was.
    #[test]
    fn a_legacy_single_target_compound_still_reads_as_one_slot() {
        let mut nbt = Compound::new();
        put(&mut nbt, KEY_TARGET, Tag::String("minecraft:torch".into()));
        put(&mut nbt, KEY_MIN, int_of(8));
        put(&mut nbt, KEY_MAX, int_of(16));
        put(
            &mut nbt,
            KEY_CELLS,
            Tag::List(vec![
                Tag::String("minecraft:coal,minecraft:charcoal".into()),
                Tag::String("minecraft:stick".into()),
            ]),
        );
        put(&mut nbt, KEY_RESULT_COUNT, Tag::Int(4));
        let state = read(&nbt);
        assert_eq!(state.targets.len(), 1);
        assert_eq!(state.target(), Some("minecraft:torch"));
        assert_eq!(state.band(), Some(Band::new(8, 16).expect("a band")));
        assert_eq!(
            state.pattern().map(|pattern| pattern.result_count()),
            Some(4)
        );
    }

    /// The shared read is what a Step Requester sees: every maintained item, the phase and the
    /// queue — no pattern, no band, no stock.
    #[test]
    fn the_shared_read_is_the_queue_and_the_phase() {
        let mut nbt = Compound::new();
        let mut targets = Targets::new();
        assert!(
            targets
                .set(slot("minecraft:torch", Band::new(0, 0).expect("a band")))
                .is_ok()
        );
        assert!(
            targets
                .set(slot("minecraft:stick", Band::new(0, 0).expect("a band")))
                .is_ok()
        );
        let mut state = State {
            targets,
            selected: 0,
            phase: Phase::NotEnoughIngredients,
            ..State::default()
        };
        assert!(state.queue.ask([222, 64, 0], "minecraft:torch", 4));
        assert!(state.queue.fail().is_some());
        write(&mut nbt, &state);
        let shared = shared(&nbt);
        assert_eq!(shared.target.as_deref(), Some("minecraft:torch"));
        assert_eq!(shared.targets, vec!["minecraft:torch", "minecraft:stick"]);
        assert_eq!(shared.phase, Phase::NotEnoughIngredients);
        assert_eq!(shared.queue.outstanding([222, 64, 0], "minecraft:torch"), 4);
        assert!(shared.queue.failed_for([222, 64, 0], "minecraft:torch"));
    }

    /// 0.2.6's single ask is operational state, not configuration: a world saved with it reads as a
    /// machine with an empty queue, and its requester asks again on the next tick.
    #[test]
    fn a_legacy_single_ask_is_not_a_queued_ask() {
        let mut nbt = Compound::new();
        put(&mut nbt, KEY_REQUEST, int_of(4));
        assert!(read(&nbt).queue.is_empty());
        let state = State::default();
        write(&mut nbt, &state);
        assert_eq!(
            crate::nbt::int(&nbt, KEY_REQUEST),
            None,
            "the old key is dropped"
        );
    }

    #[test]
    fn an_empty_compound_reads_as_an_unconfigured_machine() {
        let state = read(&Compound::new());
        assert!(!state.is_configured());
        assert_eq!(state.phase, Phase::Idle);
    }

    /// The machine's menu declares the 27 stock slots and the two data slots the screen publishes,
    /// so the engine sizes the menu from the same numbers the panel draws — the two cannot drift.
    #[test]
    fn the_machines_menu_declares_the_slots_the_screen_publishes() {
        let declared = include_str!("../data/stepcrafter/horizon/menu/step_crafter_config.json");
        let published = crate::screen::machine_layout().slots.len();
        assert_eq!(
            published, SLOTS,
            "the screen places the menu's 27 stock slots"
        );
        assert!(
            declared.contains(&format!("\"slots\": {published}")),
            "the menu's slots must be the {published} the screen places: {declared}"
        );
        let data = runtime::machine_data_slots(None).len();
        assert_eq!(data, 2, "min and max");
        assert!(
            declared.contains(&format!("\"data_slots\": {data}")),
            "the menu's data_slots must be the {data} the screen draws: {declared}"
        );
    }
}
