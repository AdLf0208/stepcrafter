//! The Step Requester's game half: its filter, the crafter it asks, and one server tick.
//!
//! Upstream's requester is a Refined Storage network node: it asks the network's autocrafting
//! component for a batch of a filtered resource and watches the network's storage for it
//! (`StepRequesterNetworkNode.doWork`). A Horizon add-on cannot join that network (see
//! `ports/_shared/evidence/stepcrafter/parent-surface.md`), so this port roots the same rule on the
//! machine that exists here: a **nearby Step Crafter**. Its stock is the storage the requester
//! watches, and the ask is one entry (`x,y,z|item|remaining`) merged into that crafter's saved
//! `Requests` list — the pin's `/data merge block` write path
//! ([`mod_api::BlockEntityRef::merge_data`]) — which the crafter's own tick serves one step at a time
//! and credits to the asker (`Served`), so two requesters on one crafter are two asks, not one count.
//!
//! What crosses between the two blocks is world state only: a list written into the other's compound,
//! and reads of its `Target`, `Phase`, `Requests`, `Served` and item slots. No Rust call crosses a
//! port boundary, and neither block needs the other's crate.

use mod_api::{
    BlockEntityRef, BlockEntityTickEvent, Compound, Host, LevelRef, MenuButtonEvent, ModContainer,
    Registrar, ServerHost, Tag,
};

use crate::machine;
use crate::nbt::{amount, drop_key, int_of, put, string};
use crate::runtime::{self, Ask, Band, Entry, Filter, Filters, Phase};

/// The block entity type this mod declares for upstream's requester.
pub const TYPE: &str = "step_requester";
/// The requester block (upstream's own, with its `active` state).
pub const BLOCK: &str = "stepcrafter:step_requester";
/// The menu type the content declares (`data/stepcrafter/horizon/menu/step_requester_filters.json`): one
/// hidden slot and the five data slots the screen reads.
pub const MENU: &str = "stepcrafter:step_requester_filters";
/// The menu's container slots: upstream's 45 filter slots are ghost entries, not storage, so the
/// container the menu needs is a single hidden slot no face reaches.
const MENU_SLOTS: usize = 1;

/// The cube, in blocks each way, a requester looks for a crafter in. Upstream's reach is the
/// network's; this is the documented re-root: `LINK_RADIUS` blocks along each axis, nearest first.
pub const LINK_RADIUS: i32 = 4;

/// Upstream's `rf` container: one saved string per filter entry.
const KEY_FILTERS: &str = "Filters";
const KEY_REQUESTS: &str = "Requests";
/// The filter cell the screen's value buttons edit, so a reopened screen shows the same entry.
const KEY_SELECTED: &str = "Selected";
/// The 0.2.1–0.2.4 single-entry keys. Read as one entry when a compound has no `Filters`, and
/// dropped on the next write; a world made by the older package keeps its filter.
const KEY_ITEM: &str = "FilterItem";
const KEY_MIN: &str = "FilterMin";
const KEY_MAX: &str = "FilterMax";
const KEY_BATCH: &str = "FilterBatch";
const KEY_PHASE: &str = "Phase";
const KEY_LINK: &str = "Link";
const KEY_BACKOFF: &str = "Backoff";

/// Declare the requester's block entity type, the menu its container opens, and the Part that ticks
/// it.
pub fn declare(r: &mut dyn Registrar) {
    let kind = r
        .block_entity_type(TYPE)
        .blocks(&["step_requester"])
        .build();
    let block = r.block_key(BLOCK);
    r.block_container(&block, container());
    // A refusal is the game's to report at load; the Part is then unattached and the block keeps
    // no state, which the load log states.
    let _ = r.block_entity_part(&kind, crate::Requester);
}

/// The requester's container: the one hidden slot its menu type needs, on no face (a filter entry
/// is a resource, a band and a batch in the saved compound, never an item to pump in and out).
pub(crate) fn container() -> ModContainer {
    ModContainer {
        slots: MENU_SLOTS,
        faces: std::array::from_fn(|_| Vec::new()),
        inputs: Vec::new(),
        outputs: Vec::new(),
        menu: Some(MENU.to_owned()),
    }
}

/// What one requester is configured to maintain.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    /// The filter entries, one per maintained resource (upstream's 45-slot `rf` container).
    pub filters: Filters,
    /// How many asks this requester has issued in all (the harness readout).
    pub requests: u64,
    /// The filter cell the screen's value buttons edit (`0` on a fresh requester).
    pub selected: usize,
}

impl State {
    /// Whether the requester has a filter to maintain.
    #[must_use]
    pub fn is_configured(&self) -> bool {
        !self.filters.is_empty()
    }

    /// The first entry's phase, as the readout's single-entry fields print it.
    #[must_use]
    pub fn phase_id(&self) -> &'static str {
        self.filters.first().map_or("idle", Entry::phase_id)
    }
}

/// Read a requester's configuration out of its saved compound.
///
/// A compound written by 0.2.1–0.2.4 has no `Filters` list but the single entry's keys; it reads as
/// one entry so an existing world keeps its filter. The next tick writes the container form.
#[must_use]
pub fn read(nbt: &Compound) -> State {
    let filters = match crate::nbt::list(nbt, KEY_FILTERS) {
        Some(entries) => Filters::decode(&entries),
        None => legacy_filters(nbt),
    };
    State {
        filters,
        requests: amount(nbt, KEY_REQUESTS),
        selected: usize::try_from(amount(nbt, KEY_SELECTED)).unwrap_or(0),
    }
}

/// The 0.2.4 single filter entry, as the one entry of a container.
fn legacy_filters(nbt: &Compound) -> Filters {
    let Some(item) = string(nbt, KEY_ITEM) else {
        return Filters::new();
    };
    let Some(band) = Band::new(amount(nbt, KEY_MIN), amount(nbt, KEY_MAX)) else {
        return Filters::new();
    };
    let Some(filter) = Filter::new(item, band, amount(nbt, KEY_BATCH)) else {
        return Filters::new();
    };
    let mut filters = Filters::new();
    let _ = filters.set(filter);
    if let Some(entry) = filters.iter_mut().next() {
        entry.set_phase(
            string(nbt, KEY_PHASE)
                .as_deref()
                .map_or(Phase::Idle, Phase::of),
        );
        entry.set_link(string(nbt, KEY_LINK).as_deref().and_then(parse_pos));
        entry.set_backoff(amount(nbt, KEY_BACKOFF).try_into().unwrap_or(u32::MAX));
    }
    filters
}

/// Write a requester's configuration back: the whole container, plus the ask counter.
///
/// The 0.2.4 single-entry keys are superseded, so they are dropped here rather than left as a second
/// truth beside the container.
pub fn write(nbt: &mut Compound, state: &State) {
    let entries = state.filters.encode();
    if entries.is_empty() {
        drop_key(nbt, KEY_FILTERS);
    } else {
        put(
            nbt,
            KEY_FILTERS,
            Tag::List(
                entries
                    .iter()
                    .map(|entry| Tag::String(entry.as_str().into()))
                    .collect(),
            ),
        );
    }
    put(nbt, KEY_REQUESTS, int_of(state.requests));
    put(
        nbt,
        KEY_SELECTED,
        int_of(state.selected.try_into().unwrap_or(u64::MAX)),
    );
    for key in [
        KEY_ITEM,
        KEY_MIN,
        KEY_MAX,
        KEY_BATCH,
        KEY_PHASE,
        KEY_LINK,
        KEY_BACKOFF,
    ] {
        drop_key(nbt, key);
    }
}

/// One server tick of a requester: walk every filter entry and ask or hold for each.
///
/// Upstream's `StepRequesterNetworkNode.doWork` runs its five guards once per non-empty filter slot,
/// each with that slot's own status and failed-task timeout. This is the same walk: an entry's
/// crafter is resolved from the entry's resource, and one entry's refusal or backoff never stops
/// another entry from being served.
pub fn tick(host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
    let me = event.pos;
    let mut state = read(&event.entity.nbt);
    // The screen's numbers are the menu's data slots, which are a view and not saved: publish them
    // every tick, so a press shows as it lands and a reopened menu opens on the selected entry.
    event.entity.data = runtime::view_data_slots(&state.filters, state.selected).to_vec();
    if state.filters.is_empty() {
        machine::sync_active(host, me, false);
        return;
    }
    let Some(level) = host.level(host.dimension()) else {
        machine::sync_active(host, me, false);
        return;
    };

    let mut running = false;
    let mut asked = 0u64;
    for entry in state.filters.iter_mut() {
        if tick_entry(&mut *host, level, me, entry) {
            asked = asked.saturating_add(1);
        }
        running |= entry.phase() == Phase::Crafting;
    }

    state.requests = state.requests.saturating_add(asked);
    write(&mut event.entity.nbt, &state);
    machine::sync_active(host, me, running);
}

/// One entry's tick: find the crafter that maintains its resource, read that crafter's stock, then
/// ask or hold. Answers whether it put an ask into the crafter.
fn tick_entry(host: &mut dyn ServerHost, level: LevelRef, me: [i32; 3], entry: &mut Entry) -> bool {
    let found = resolve(&*host, level, me, entry.filter().item(), entry.link());
    let (crafter_pos, crafter_entity, crafter) = match found {
        Found::Crafter { pos, entity, state } => (pos, entity, state),
        // The crafter is out of the world for its own tick: keep the link and hold, rather than
        // rescanning on one unreadable tick.
        Found::Unreadable => return false,
        Found::NoCrafter => {
            entry.set_link(None);
            // Nothing to ask: a run that lost its crafter is not running any more.
            if entry.phase() == Phase::Crafting {
                entry.set_phase(Phase::Idle);
            }
            return false;
        }
    };

    let item = entry.filter().item().to_owned();
    // The stock is the crafter's single inventory, but the entry's own `stored` is **its** share of
    // it: what its asks produced, never another requester's torches (0.2.7).
    let stock = stock_count(&*host, crafter_entity, &item);
    let stored = crafter.queue.share(me, &item, stock);
    let outstanding = crafter.queue.outstanding(me, &item);
    // The crafter names the ask it could not serve; only that requester reports the refusal and
    // times out — upstream's failed task belongs to the requester that started it.
    let refused = crafter
        .queue
        .failed_for(me, &item)
        .then_some(crafter.phase)
        .filter(|phase| phase.is_refusal());
    let (phase, backoff) = runtime::settle(entry.phase(), entry.backoff(), refused, outstanding);
    let decision = runtime::ask(stored, entry.filter(), phase, outstanding, backoff);

    entry.set_link(Some(crafter_pos));
    entry.set_phase(phase);
    entry.set_backoff(backoff);
    match decision {
        Ask::Hold => false,
        Ask::Finish => {
            entry.set_phase(Phase::Idle);
            false
        }
        Ask::Backoff => {
            entry.set_backoff(backoff.saturating_sub(1));
            false
        }
        Ask::Waiting => {
            entry.set_phase(Phase::Crafting);
            false
        }
        Ask::Request { amount } => {
            entry.set_phase(Phase::Crafting);
            ask_crafter(host, crafter_entity, me, &item, amount);
            true
        }
    }
}

/// One entry as the readout prints it: what it maintains, its own share of its crafter's stock, and
/// how its last tick went.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    /// The entry's resource.
    pub item: String,
    /// The entry's band.
    pub min: u64,
    /// The entry's band.
    pub max: u64,
    /// The entry's largest single ask.
    pub batch: u64,
    /// The entry's own share of the item in the linked crafter: what its asks produced.
    pub stored: u64,
    /// How many items the linked crafter has served this entry, in all.
    pub served: u64,
    /// Whether this entry's ask is queued with the linked crafter (`1`) or not (`0`).
    pub queued: u64,
    /// The entry's phase, `running` while its ask is queued.
    pub phase: &'static str,
    /// The crafter this entry asks right now.
    pub crafter: Option<[i32; 3]>,
    /// Failed-task ticks left for this entry.
    pub backoff: u32,
}

impl View {
    /// The entry as one readout field:
    /// `item|min|max|batch|stored|served|queued|phase|crafter|backoff`.
    #[must_use]
    pub fn encode(&self) -> String {
        let crafter = self
            .crafter
            .map_or_else(|| "none".to_owned(), |[x, y, z]| format!("{x},{y},{z}"));
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            self.item,
            self.min,
            self.max,
            self.batch,
            self.stored,
            self.served,
            self.queued,
            self.phase,
            crafter,
            self.backoff
        )
    }
}

/// Every configured entry as the readout needs it: its crafter, that crafter's stock of its resource
/// (upstream's `storageComponent.get(resource)`) and this entry's own share of it, resolved once.
#[must_use]
pub fn views(host: &dyn Host, level: LevelRef, me: [i32; 3], nbt: &Compound) -> Vec<View> {
    read(nbt)
        .filters
        .iter()
        .map(|entry| {
            let filter = entry.filter();
            let (crafter, stored, served, queued) =
                match resolve(host, level, me, filter.item(), entry.link()) {
                    Found::Crafter { pos, entity, state } => {
                        let stock = stock_count(host, entity, filter.item());
                        (
                            Some(pos),
                            state.queue.share(me, filter.item(), stock),
                            state.queue.served_for(me, filter.item()),
                            state.queue.outstanding(me, filter.item()),
                        )
                    }
                    Found::NoCrafter | Found::Unreadable => (None, 0, 0, 0),
                };
            View {
                item: filter.item().to_owned(),
                min: filter.band().min(),
                max: filter.band().max(),
                batch: filter.batch(),
                stored,
                served,
                queued: u64::from(queued > 0),
                phase: entry.phase_id(),
                crafter,
                backoff: entry.backoff(),
            }
        })
        .collect()
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

/// Put one ask into the crafter's queue: the `/data merge block` write path, which loads the merged
/// compound into the block entity, so the crafter's own next tick serves it.
///
/// The ask is the queue's **list**, so a second requester's ask joins the first's instead of
/// overwriting it (0.2.6's single `Request` `Int` was the last writer's). A requester holds one ask
/// per item: re-asking merges into it.
fn ask_crafter(
    host: &mut dyn ServerHost,
    entity: BlockEntityRef,
    me: [i32; 3],
    item: &str,
    amount: u64,
) {
    let Some(data) = entity.data(&*host) else {
        return;
    };
    let mut queue = machine::shared(&data).queue;
    if !queue.ask(me, item, amount) {
        return;
    }
    let mut patch = Compound::new();
    put(
        &mut patch,
        machine::KEY_REQUESTS,
        Tag::List(
            queue
                .encode()
                .iter()
                .map(|entry| Tag::String(entry.as_str().into()))
                .collect(),
        ),
    );
    let _ = entity.merge_data(host, patch);
}

/// One press on a requester's screen, on the server (`menu_button`): the item the player carries
/// makes or repoints a cell's entry, a cell focuses the entry the value buttons edit, and those
/// buttons step the focused entry's band and batch. The saved container and `/stepcrafter
/// requester` are the same `Filters`, so a press and a command cannot disagree.
pub fn press(host: &mut dyn ServerHost, event: &mut MenuButtonEvent<'_>) {
    let Some(press) = runtime::Press::of(event.context.id) else {
        return;
    };
    let carried = {
        let host: &dyn Host = &*host;
        event.carried.as_ref().map(|stack| stack.item().id(host))
    };
    let state = read(&event.entity.nbt);
    let mut editor = runtime::Editor::new(state.filters, state.selected);
    if !editor.apply(press, carried.as_deref()) {
        return;
    }
    let selected = editor.selected();
    let state = State {
        filters: editor.into_filters(),
        requests: state.requests,
        selected,
    };
    write(&mut event.entity.nbt, &state);
    // Publish at once rather than waiting for the next tick: the client redraws the number the
    // player just stepped.
    event.entity.data = runtime::view_data_slots(&state.filters, state.selected).to_vec();
}

/// The screen's open payload (`menu_extra_data`): the entries' resources in cell order, so the
/// client draws each cell's icon. The numbers are the menu's data slots, which change as the player
/// presses; a resource set from the screen shows its icon again when the menu is reopened.
#[must_use]
pub fn extra_data(host: &dyn Host, level: LevelRef, pos: [i32; 3], menu: &str) -> Option<Vec<u8>> {
    if menu != MENU {
        return None;
    }
    let entity = host.block_entity(level, pos)?;
    let data = entity.data(host)?;
    Some(runtime::view_extra_data(&read(&data).filters).into_bytes())
}

/// Where a requester's crafter is this tick.
enum Found {
    /// The crafter, its handle, and the part of its saved state a requester reads.
    Crafter {
        pos: [i32; 3],
        entity: BlockEntityRef,
        state: machine::Shared,
    },
    /// No Step Crafter maintains the filter inside [`LINK_RADIUS`].
    NoCrafter,
    /// A crafter is there but its block entity cannot be read this tick (it is out for its own
    /// tick), so this tick holds instead of deciding that the crafter is gone.
    Unreadable,
}

/// One position's answer to "is this the crafter my filter needs".
enum Candidate {
    Hit(BlockEntityRef, machine::Shared),
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
    let mut best: Option<([i32; 3], BlockEntityRef, machine::Shared, i32)> = None;
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
    let state = machine::shared(&data);
    if state.target.as_deref() == Some(item) {
        Candidate::Hit(entity, state)
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

/// A second entry: 8 sticks kept between 8 and 16, asked 8 at a time.
#[cfg(test)]
fn sticks() -> Filter {
    Filter::new(
        "minecraft:stick".to_owned(),
        runtime::Band::new(8, 16).expect("a valid band"),
        8,
    )
    .expect("a valid filter")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two filter entries and their own phases, links and timeouts live in one saved compound, so a
    /// write that does not round-trip loses one entry or the crafter the other asks.
    #[test]
    fn the_requester_state_round_trips_through_the_saved_compound() {
        let mut filters = Filters::new();
        filters.set(torches()).expect("room for the first entry");
        filters.set(sticks()).expect("room for the second entry");
        {
            let first = filters.iter_mut().next().expect("the first entry");
            first.set_phase(Phase::Crafting);
            first.set_link(Some([218, 64, 0]));
            first.set_backoff(7);
        }
        let state = State {
            filters,
            requests: 3,
            selected: 1,
        };
        let mut nbt = Compound::new();
        write(&mut nbt, &state);
        assert_eq!(read(&nbt), state);
        assert_eq!(
            read(&nbt).filters.len(),
            2,
            "both filter entries must survive the compound between ticks"
        );
    }

    #[test]
    fn an_empty_compound_reads_as_an_unconfigured_requester() {
        let state = read(&Compound::new());
        assert!(!state.is_configured());
        assert_eq!(state.phase_id(), "idle");
        assert!(state.filters.is_empty());
    }

    /// A world saved by 0.2.1–0.2.4 has the single entry's keys and no `Filters` list; the upgrade
    /// must keep that filter rather than read the requester as unconfigured.
    #[test]
    fn a_legacy_single_filter_compound_still_reads_as_one_entry() {
        let mut nbt = Compound::new();
        put(&mut nbt, KEY_ITEM, Tag::String("minecraft:torch".into()));
        put(&mut nbt, KEY_MIN, int_of(4));
        put(&mut nbt, KEY_MAX, int_of(8));
        put(&mut nbt, KEY_BATCH, int_of(4));
        put(&mut nbt, KEY_PHASE, Tag::String("crafting".into()));
        put(&mut nbt, KEY_LINK, Tag::String("218,64,0".into()));
        put(&mut nbt, KEY_BACKOFF, int_of(7));

        let state = read(&nbt);
        assert_eq!(state.filters.len(), 1);
        let entry = state.filters.first().expect("the legacy entry");
        assert_eq!(entry.filter().item(), "minecraft:torch");
        assert_eq!(
            entry.filter().band(),
            runtime::Band::new(4, 8).expect("a band")
        );
        assert_eq!(entry.filter().batch(), 4);
        assert_eq!(entry.phase(), Phase::Crafting);
        assert_eq!(entry.link(), Some([218, 64, 0]));
        assert_eq!(entry.backoff(), 7);

        // The next write migrates the compound to the container form and drops the old keys.
        write(&mut nbt, &state);
        assert_eq!(read(&nbt), state);
        assert_eq!(
            crate::nbt::list(&nbt, KEY_FILTERS).map(|list| list.len()),
            Some(1)
        );
        assert_eq!(crate::nbt::string(&nbt, KEY_ITEM), None);
    }

    #[test]
    fn the_readout_calls_a_running_ask_running() {
        let mut running = Filters::new();
        running.set(torches()).expect("room for the entry");
        if let Some(entry) = running.iter_mut().next() {
            entry.set_phase(Phase::Crafting);
        }
        assert_eq!(
            State {
                filters: running,
                requests: 0,
                selected: 0
            }
            .phase_id(),
            "running"
        );
        assert_eq!(State::default().phase_id(), "idle");
        let mut blocked = Filters::new();
        blocked.set(torches()).expect("room for the entry");
        if let Some(entry) = blocked.iter_mut().next() {
            entry.set_phase(Phase::NotEnoughIngredients);
        }
        assert_eq!(
            State {
                filters: blocked,
                requests: 0,
                selected: 0
            }
            .phase_id(),
            "not-enough-ingredients"
        );
    }

    #[test]
    fn a_saved_position_round_trips_and_foreign_text_is_refused() {
        assert_eq!(parse_pos("218,64,-3"), Some([218, 64, -3]));
        assert_eq!(parse_pos("218,64"), None);
        assert_eq!(parse_pos("218,64,0,1"), None);
        assert_eq!(parse_pos("a,b,c"), None);
    }

    /// The menu type declares as many data slots as the screen publishes, and the selected cell
    /// survives a save: a reopened screen shows the entry the buttons were editing.
    #[test]
    fn the_menu_declares_the_slots_the_screen_publishes() {
        // The content menu is what the engine sizes the data slots from; the screen publishes an
        // array, so the two are checked against each other rather than against a constant.
        let declared = include_str!("../data/stepcrafter/horizon/menu/step_requester_filters.json");
        let published = runtime::view_data_slots(&Filters::new(), 0).len();
        assert!(
            declared.contains(&format!("\"data_slots\": {published}")),
            "the menu's data_slots must be the {published} the screen publishes: {declared}"
        );
        assert!(
            declared.contains("\"slots\": 1"),
            "the menu has the container's one hidden slot: {declared}"
        );
        let mut filters = Filters::new();
        filters.set(torches()).expect("room for the first");
        filters.set(sticks()).expect("room for the second");
        let state = State {
            filters,
            requests: 0,
            selected: 1,
        };
        let mut nbt = Compound::new();
        write(&mut nbt, &state);
        assert_eq!(read(&nbt), state);
        assert_eq!(
            read(&nbt).selected,
            1,
            "the screen opens on the cell it last edited"
        );
        let empty = read(&Compound::new());
        assert_eq!(empty.selected, 0, "a fresh requester edits its first cell");
    }
}
