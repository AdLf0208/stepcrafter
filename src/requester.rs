//! The Step Requester's game half: its filter, the crafter it asks, and one server tick.
//!
//! Upstream's requester is a Refined Storage network node: it asks the network's autocrafting
//! component for a batch of a filtered resource and watches the network's storage for it
//! (`StepRequesterNetworkNode.doWork`). A Horizon add-on cannot join that network (see
//! `ports/_shared/evidence/stepcrafter/parent-surface.md`), so this port roots the same rule on the
//! machine that exists here: a **nearby Step Crafter**. Its stock is the storage the requester
//! watches, and the ask is a `Request` count written into that crafter's saved compound — the pin's
//! `/data merge block` write path ([`mod_api::BlockEntityRef::merge_data`]) — which the crafter's
//! own tick spends one step at a time.
//!
//! What crosses between the two blocks is world state only: one `Int` written into the other's
//! compound, and reads of its `Target`, `Phase`, `Request` and item slots. No Rust call crosses a
//! port boundary, and neither block needs the other's crate.

use mod_api::{
    BlockEntityRef, BlockEntityTickEvent, Compound, Host, LevelRef, Registrar, ServerHost, Tag,
};

use crate::machine;
use crate::nbt::{amount, drop_key, int_of, put, string};
use crate::runtime::{self, Ask, Filter, Phase};

/// The block entity type this mod declares for upstream's requester.
pub const TYPE: &str = "step_requester";
/// The requester block (upstream's own, with its `active` state).
pub const BLOCK: &str = "stepcrafter:step_requester";

/// The cube, in blocks each way, a requester looks for a crafter in. Upstream's reach is the
/// network's; this is the documented re-root: `LINK_RADIUS` blocks along each axis, nearest first.
pub const LINK_RADIUS: i32 = 4;

const KEY_ITEM: &str = "FilterItem";
const KEY_MIN: &str = "FilterMin";
const KEY_MAX: &str = "FilterMax";
const KEY_BATCH: &str = "FilterBatch";
const KEY_PHASE: &str = "Phase";
const KEY_LINK: &str = "Link";
const KEY_BACKOFF: &str = "Backoff";
const KEY_REQUESTS: &str = "Requests";

/// Declare the requester's block entity type and the Part that ticks it.
pub fn declare(r: &mut dyn Registrar) {
    let kind = r
        .block_entity_type(TYPE)
        .blocks(&["step_requester"])
        .build();
    // A refusal is the game's to report at load; the Part is then unattached and the block keeps
    // no state, which the load log states.
    let _ = r.block_entity_part(&kind, crate::Requester);
}

/// What one requester is configured to maintain.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    /// The filter entry, absent until `/stepcrafter requester`.
    pub filter: Option<Filter>,
    /// Where the last tick left it; upstream's filter-slot status.
    pub phase: Phase,
    /// The crafter it asks, kept so the scan runs once and re-validated every tick.
    pub link: Option<[i32; 3]>,
    /// Failed-task ticks left before the next ask, upstream's `failedTaskTimeouts`.
    pub backoff: u32,
    /// How many asks this requester has issued (the harness readout).
    pub requests: u64,
}

impl State {
    /// Whether the requester has a filter to maintain.
    #[must_use]
    pub fn is_configured(&self) -> bool {
        self.filter.is_some()
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
}

/// Read a requester's configuration out of its saved compound.
#[must_use]
pub fn read(nbt: &Compound) -> State {
    let filter = match (
        string(nbt, KEY_ITEM),
        amount(nbt, KEY_MIN),
        amount(nbt, KEY_MAX),
    ) {
        (Some(item), min, max) => {
            let band = runtime::Band::new(min, max);
            band.and_then(|band| Filter::new(item, band, amount(nbt, KEY_BATCH)))
        }
        (None, _, _) => None,
    };
    State {
        filter,
        phase: string(nbt, KEY_PHASE)
            .as_deref()
            .map_or(Phase::Idle, Phase::of),
        link: string(nbt, KEY_LINK).as_deref().and_then(parse_pos),
        backoff: amount(nbt, KEY_BACKOFF).try_into().unwrap_or(u32::MAX),
        requests: amount(nbt, KEY_REQUESTS),
    }
}

/// Write a requester's configuration back; an unconfigured requester keeps no filter keys at all.
pub fn write(nbt: &mut Compound, state: &State) {
    match &state.filter {
        Some(filter) => {
            put(nbt, KEY_ITEM, Tag::String(filter.item().into()));
            put(nbt, KEY_MIN, int_of(filter.band().min()));
            put(nbt, KEY_MAX, int_of(filter.band().max()));
            put(nbt, KEY_BATCH, int_of(filter.batch()));
        }
        None => {
            drop_key(nbt, KEY_ITEM);
            drop_key(nbt, KEY_MIN);
            drop_key(nbt, KEY_MAX);
            drop_key(nbt, KEY_BATCH);
        }
    }
    put(nbt, KEY_PHASE, Tag::String(state.phase.id().into()));
    match state.link {
        Some([x, y, z]) => {
            let text = format!("{x},{y},{z}");
            put(nbt, KEY_LINK, Tag::String(text.as_str().into()));
        }
        None => drop_key(nbt, KEY_LINK),
    }
    put(nbt, KEY_BACKOFF, int_of(u64::from(state.backoff)));
    put(nbt, KEY_REQUESTS, int_of(state.requests));
}

/// One server tick of a requester: find its crafter, read that crafter's stock, then ask or hold.
pub fn tick(host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
    let me = event.pos;
    let mut state = read(&event.entity.nbt);
    let Some(filter) = state.filter.clone() else {
        machine::sync_active(host, me, false);
        return;
    };
    let Some(level) = host.level(host.dimension()) else {
        machine::sync_active(host, me, false);
        return;
    };

    let (crafter_pos, crafter_entity, crafter) =
        match resolve(&*host, level, me, filter.item(), state.link) {
            Found::Crafter { pos, entity, state } => (pos, entity, state),
            Found::Unreadable => {
                // The crafter is out of the world for its own tick: keep the link and hold, rather
                // than rescanning on one unreadable tick.
                machine::sync_active(host, me, false);
                return;
            }
            Found::NoCrafter => {
                state.link = None;
                // Nothing to ask: a run that lost its crafter is not running any more.
                if state.phase == Phase::Crafting {
                    state.phase = Phase::Idle;
                }
                write(&mut event.entity.nbt, &state);
                machine::sync_active(host, me, false);
                return;
            }
        };
    state.link = Some(crafter_pos);

    let stored = stock_count(&*host, crafter_entity, filter.item());
    // A crafter that refused the ask (its own phase says so, and it has nothing outstanding) is
    // upstream's failed task: report it, and stop asking for its 20-tick timeout.
    if state.phase == Phase::Crafting
        && crafter.request == 0
        && let Some(refusal) = runtime::refused(crafter.phase, stored, &filter)
    {
        state.phase = refusal;
        state.backoff = runtime::FAILED_TASK_TIMEOUT_TICKS;
    }

    match runtime::ask(stored, &filter, state.phase, crafter.request, state.backoff) {
        Ask::Hold => {}
        Ask::Finish => state.phase = Phase::Idle,
        Ask::Backoff => state.backoff = state.backoff.saturating_sub(1),
        Ask::Waiting => state.phase = Phase::Crafting,
        Ask::Request { amount } => {
            state.phase = Phase::Crafting;
            state.requests = state.requests.saturating_add(1);
            ask_crafter(host, crafter_entity, amount);
        }
    }

    write(&mut event.entity.nbt, &state);
    machine::sync_active(host, me, state.phase == Phase::Crafting);
}

/// The crafter a requester asks, and how much of its filter that crafter holds: what the readout
/// needs, resolved once.
#[must_use]
pub fn crafter_and_stock(
    host: &dyn Host,
    level: LevelRef,
    me: [i32; 3],
    nbt: &Compound,
) -> (Option<[i32; 3]>, u64) {
    let state = read(nbt);
    let Some(filter) = state.filter else {
        return (None, 0);
    };
    match resolve(host, level, me, filter.item(), state.link) {
        Found::Crafter { pos, entity, .. } => (Some(pos), stock_count(host, entity, filter.item())),
        Found::NoCrafter | Found::Unreadable => (None, 0),
    }
}

/// The filtered item's stock in a crafter's inventory: upstream's `storageComponent.get(resource)`.
#[must_use]
pub fn stock_count(host: &dyn Host, entity: BlockEntityRef, item: &str) -> u64 {
    (0..machine::SLOTS)
        .filter_map(|slot| entity.item_slot(host, slot as u32).ok().flatten())
        .filter(|stack| stack.item().id(host) == item)
        .map(|stack| u64::from(stack.count()))
        .sum()
}

/// Put one ask into the crafter's saved compound: the `/data merge block` write path, which loads
/// the merged compound into the block entity, so its own next tick reads the ask.
fn ask_crafter(host: &mut dyn ServerHost, entity: BlockEntityRef, amount: u64) {
    let mut data = Compound::new();
    put(&mut data, machine::KEY_REQUEST, int_of(amount));
    let _ = entity.merge_data(host, data);
}

/// Where a requester's crafter is this tick.
enum Found {
    /// The crafter, its handle, and what the requester needs to know about it.
    Crafter {
        pos: [i32; 3],
        entity: BlockEntityRef,
        state: Crafter,
    },
    /// No Step Crafter maintains the filter inside [`LINK_RADIUS`].
    NoCrafter,
    /// A crafter is there but its block entity cannot be read this tick (it is out for its own
    /// tick), so this tick holds instead of deciding that the crafter is gone.
    Unreadable,
}

/// The part of a crafter's saved state a requester reads: how many items it has been asked for and
/// how its last step went. The pattern, the band and the stock are the crafter's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Crafter {
    request: u64,
    phase: Phase,
}

impl From<machine::State> for Crafter {
    fn from(state: machine::State) -> Self {
        Self {
            request: state.request,
            phase: state.phase,
        }
    }
}

/// One position's answer to "is this the crafter my filter needs".
enum Candidate {
    Hit(BlockEntityRef, Crafter),
    Unreadable,
    Miss,
}

/// The cached link while it still holds, else the nearest crafter inside [`LINK_RADIUS`].
fn resolve(
    host: &dyn Host,
    level: LevelRef,
    me: [i32; 3],
    item: &str,
    link: Option<[i32; 3]>,
) -> Found {
    if let Some(pos) = link {
        match inspect(host, level, pos, item) {
            Candidate::Hit(entity, state) => {
                return Found::Crafter { pos, entity, state };
            }
            Candidate::Unreadable => return Found::Unreadable,
            Candidate::Miss => {}
        }
    }
    let mut best: Option<([i32; 3], BlockEntityRef, Crafter, i32)> = None;
    let mut unreadable = false;
    for dx in -LINK_RADIUS..=LINK_RADIUS {
        for dy in -LINK_RADIUS..=LINK_RADIUS {
            for dz in -LINK_RADIUS..=LINK_RADIUS {
                let pos = [me[0] + dx, me[1] + dy, me[2] + dz];
                if pos == me || !is_crafter(host, level, pos) {
                    continue;
                }
                match inspect(host, level, pos, item) {
                    Candidate::Hit(entity, state) => {
                        let distance = dx * dx + dy * dy + dz * dz;
                        if best.as_ref().is_none_or(|(_, _, _, best)| distance < *best) {
                            best = Some((pos, entity, state, distance));
                        }
                    }
                    Candidate::Unreadable => unreadable = true,
                    Candidate::Miss => {}
                }
            }
        }
    }
    match best {
        Some((pos, entity, state, _)) => Found::Crafter { pos, entity, state },
        // A crafter that is merely out for its tick is not a missing crafter.
        None if unreadable => Found::Unreadable,
        None => Found::NoCrafter,
    }
}

/// Whether the block at `pos` is a Step Crafter with a block entity this filter could ask.
fn inspect(host: &dyn Host, level: LevelRef, pos: [i32; 3], item: &str) -> Candidate {
    if !is_crafter(host, level, pos) {
        return Candidate::Miss;
    }
    let Some(entity) = host.block_entity(level, pos) else {
        return Candidate::Unreadable;
    };
    let Some(data) = entity.data(host) else {
        return Candidate::Unreadable;
    };
    let state = machine::read(&data);
    if state.target.as_deref() == Some(item) {
        Candidate::Hit(entity, Crafter::from(state))
    } else {
        // A crafter that maintains something else cannot be asked for this item: the ask would
        // spend this requester's items on another pattern.
        Candidate::Miss
    }
}

fn is_crafter(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> bool {
    if !level.is_loaded(host, pos) {
        return false;
    }
    level
        .block(host, pos)
        .map(|block| block.id(host) == machine::BLOCK)
        .unwrap_or(false)
}

/// `x,y,z` as saved, or nothing when the text is not a position.
fn parse_pos(text: &str) -> Option<[i32; 3]> {
    let mut parts = text.split(',');
    let x = parts.next()?.parse().ok()?;
    let y = parts.next()?.parse().ok()?;
    let z = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some([x, y, z])
}

/// The `minecraft:torch` filter the tests use: 4 kept between 4 and 8, asked 4 at a time.
#[cfg(test)]
fn torches() -> Filter {
    Filter::new(
        "minecraft:torch".to_owned(),
        runtime::Band::new(4, 8).expect("a valid band"),
        4,
    )
    .expect("a valid filter")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The requester's configuration lives in its saved compound, so a write that does not
    /// round-trip loses the filter or the crafter it asks between ticks.
    #[test]
    fn the_requester_state_round_trips_through_the_saved_compound() {
        let mut nbt = Compound::new();
        let state = State {
            filter: Some(torches()),
            phase: Phase::Crafting,
            link: Some([218, 64, 0]),
            backoff: 7,
            requests: 3,
        };
        write(&mut nbt, &state);
        assert_eq!(read(&nbt), state);
    }

    #[test]
    fn an_empty_compound_reads_as_an_unconfigured_requester() {
        let state = read(&Compound::new());
        assert!(!state.is_configured());
        assert_eq!(state.phase, Phase::Idle);
        assert_eq!(state.link, None);
    }

    #[test]
    fn the_readout_calls_a_running_ask_running() {
        let running = State {
            phase: Phase::Crafting,
            ..State::default()
        };
        assert_eq!(running.phase_id(), "running");
        let idle = State::default();
        assert_eq!(idle.phase_id(), "idle");
        let blocked = State {
            phase: Phase::NotEnoughIngredients,
            ..State::default()
        };
        assert_eq!(blocked.phase_id(), "not-enough-ingredients");
    }

    #[test]
    fn a_saved_position_round_trips_and_foreign_text_is_refused() {
        assert_eq!(parse_pos("218,64,-3"), Some([218, 64, -3]));
        assert_eq!(parse_pos("218,64"), None);
        assert_eq!(parse_pos("218,64,0,1"), None);
        assert_eq!(parse_pos("a,b,c"), None);
    }
}
