//! `/stepcrafter …`: the machine's interface, and the harness's readout.
//!
//! Upstream's controls are the block's screen (a grid of pattern slots with a minimum, a maximum
//! and a batch size each). A Horizon add-on has no screen for a vanilla menu and no network, so
//! this slice's interface is the right-click (load the stock, empty the target) plus these
//! commands, the `/extradisks` and `/ironfurnaces` precedent: the readout is what a scenario
//! parses, and `target`/`configure` are what the screen's slots would have set.
//!
//! | command | upstream's control |
//! |---|---|
//! | `target <pos> <item>` | the pattern slot, with the pattern being the item's crafting recipe |
//! | `configure <pos> <min> <max>` | the slot's min/max fields |
//! | `requester <pos> <item> <min> <max> <batch>` | the requester's filter slot: resource, min, max, batch (add or replace) |
//! | `requester add <pos> <item> <min> <max> <batch>` | another filter slot, the same fields |
//! | `requester remove <pos> <item>` | clearing a filter slot |
//! | `info <pos>` | the tooltip: maintaining min–max, the status (either block) |
//! | `step <pos>` | a manual step (upstream: click a step-crafting request) |
//! | `take <pos> <item> <count>` | taking the maintained item out of the network |

use mod_api::{
    Command, CommandArgumentValue, CommandInvocation, Host, Identifier, LevelRef, PlayerRef,
    ServerHost, Text,
};

use crate::machine;
use crate::pattern;
use crate::requester;
use crate::runtime::{Band, Filter, Phase, TargetEntry};

/// `/stepcrafter` needs the game-master level `/data get block` needs: it reads and writes another
/// position's block entity.
const OPERATOR_LEVEL: u8 = 2;

const NO_MACHINE: &str = "No Step Crafter there.";

/// Declare the commands this mod answers.
pub fn declare(commands: &mut Vec<Command>) {
    commands.push(Command::op(
        "stepcrafter info <pos:block_pos>",
        OPERATOR_LEVEL,
    ));
    commands.push(Command::op(
        "stepcrafter target <pos:block_pos> <item:id>",
        OPERATOR_LEVEL,
    ));
    commands.push(Command::op(
        "stepcrafter configure <pos:block_pos> <min:integer> <max:integer>",
        OPERATOR_LEVEL,
    ));
    commands.push(Command::op(
        "stepcrafter configure-item <pos:block_pos> <item:id> <min:integer> <max:integer>",
        OPERATOR_LEVEL,
    ));
    commands.push(Command::op(
        "stepcrafter requester <pos:block_pos> <item:id> <min:integer> <max:integer> <batch:integer>",
        OPERATOR_LEVEL,
    ));
    commands.push(Command::op(
        "stepcrafter requester add <pos:block_pos> <item:id> <min:integer> <max:integer> <batch:integer>",
        OPERATOR_LEVEL,
    ));
    commands.push(Command::op(
        "stepcrafter requester remove <pos:block_pos> <item:id>",
        OPERATOR_LEVEL,
    ));
    commands.push(Command::op(
        "stepcrafter step <pos:block_pos>",
        OPERATOR_LEVEL,
    ));
    commands.push(Command::op(
        "stepcrafter take <pos:block_pos> <item:id> <count:integer>",
        OPERATOR_LEVEL,
    ));
}

/// Run a `/stepcrafter` command; one that is not ours is left to the game.
pub fn run(host: &mut dyn ServerHost, invocation: &CommandInvocation) -> bool {
    let words: Vec<&str> = invocation.literals.iter().map(String::as_str).collect();
    let ["stepcrafter", verb, ..] = words.as_slice() else {
        return false;
    };
    let Some(pos) = block_pos(invocation) else {
        say(
            host,
            "Usage: /stepcrafter <info|target|configure|requester|step|take> <x> <y> <z> …",
        );
        return true;
    };
    match *verb {
        "info" => {
            let line = readout(host, pos);
            say(host, &line);
        }
        "target" => {
            let Some(item) = id(invocation, "item") else {
                say(host, "Usage: /stepcrafter target <x> <y> <z> <item>");
                return true;
            };
            let line = set_target(host, pos, &item);
            say(host, &line);
        }
        "configure" => {
            let (Some(min), Some(max)) = (integer(invocation, "min"), integer(invocation, "max"))
            else {
                say(
                    host,
                    "Usage: /stepcrafter configure <x> <y> <z> <min> <max>",
                );
                return true;
            };
            let line = configure(host, pos, min, max);
            say(host, &line);
        }
        "configure-item" => {
            let (Some(item), Some(min), Some(max)) = (
                id(invocation, "item"),
                integer(invocation, "min"),
                integer(invocation, "max"),
            ) else {
                say(
                    host,
                    "Usage: /stepcrafter configure-item <x> <y> <z> <item> <min> <max>",
                );
                return true;
            };
            let line = configure_item(host, pos, &item, min, max);
            say(host, &line);
        }
        "requester" => {
            let line = match words.get(2).copied() {
                Some("add") => {
                    let (Some(item), Some(min), Some(max), Some(batch)) = (
                        id(invocation, "item"),
                        integer(invocation, "min"),
                        integer(invocation, "max"),
                        integer(invocation, "batch"),
                    ) else {
                        say(
                            host,
                            "Usage: /stepcrafter requester add <x> <y> <z> <item> <min> <max> <batch>",
                        );
                        return true;
                    };
                    set_filter(host, pos, &item, min, max, batch)
                }
                Some("remove") => {
                    let Some(item) = id(invocation, "item") else {
                        say(
                            host,
                            "Usage: /stepcrafter requester remove <x> <y> <z> <item>",
                        );
                        return true;
                    };
                    remove_filter(host, pos, &item)
                }
                // The 0.2.1–0.2.4 spelling: set (or replace) the entry for this resource.
                _ => {
                    let (Some(item), Some(min), Some(max), Some(batch)) = (
                        id(invocation, "item"),
                        integer(invocation, "min"),
                        integer(invocation, "max"),
                        integer(invocation, "batch"),
                    ) else {
                        say(
                            host,
                            "Usage: /stepcrafter requester <x> <y> <z> <item> <min> <max> <batch>",
                        );
                        return true;
                    };
                    set_filter(host, pos, &item, min, max, batch)
                }
            };
            say(host, &line);
        }
        "step" => {
            let line = step(host, pos);
            say(host, &line);
        }
        "take" => {
            let (Some(item), Some(count)) = (id(invocation, "item"), integer(invocation, "count"))
            else {
                say(host, "Usage: /stepcrafter take <x> <y> <z> <item> <count>");
                return true;
            };
            let line = take(host, pos, &item, count);
            say(host, &line);
        }
        _ => say(
            host,
            "Usage: /stepcrafter <info|target|configure|requester|step|take> <x> <y> <z> …",
        ),
    }
    true
}

/// `/stepcrafter target <pos> <item>`: resolve the item's crafting recipe into the step pattern.
fn set_target(host: &mut dyn ServerHost, pos: [i32; 3], item: &str) -> String {
    let Some(level) = host.level(host.dimension()) else {
        return NO_MACHINE.to_owned();
    };
    if !is_machine(&*host, level, pos) {
        return not_a_crafter(&*host, level, pos);
    }
    let pattern = match pattern::resolve(&*host, item) {
        Ok(pattern) => pattern,
        Err(error) => return error.message(),
    };
    let Some(entity) = host.block_entity(level, pos) else {
        return NO_MACHINE.to_owned();
    };
    let Some(mut nbt) = entity.data(&*host) else {
        return NO_MACHINE.to_owned();
    };
    let mut state = machine::read(&nbt);
    // Each target gets its own slot, with no band until configured (as upstream's fresh pattern
    // slot has no min/max) and its own derived pattern.
    let slot = TargetEntry::new(pattern.result().to_owned(), None, 1, pattern.clone())
        .expect("a resolved pattern's result is an item id");
    let index = match state.targets.set(slot) {
        Ok(index) => index,
        Err(full) => return format!("stepcrafter: {full}"),
    };
    state.selected = index;
    state.phase = Phase::Idle;
    machine::write(&mut nbt, &state);
    if !matches!(entity.replace_data(host, nbt), Ok(true)) {
        return "The Step Crafter could not be written.".to_owned();
    }
    format!(
        "stepcrafter target {} {} {} target={} cells={} result={}:{} remainders={} targets={}",
        pos[0],
        pos[1],
        pos[2],
        pattern.result(),
        pattern.cells().len(),
        pattern.result(),
        pattern.result_count(),
        remainder_line(&pattern),
        state.targets.len()
    )
}

/// What the pattern's consumed items leave behind, as the saved form (`consumed=left:count`),
/// comma-separated; `none` when nothing is left.
fn remainder_line(pattern: &crate::runtime::Pattern) -> String {
    let entries = pattern.encode_remainders();
    if entries.is_empty() {
        "none".to_owned()
    } else {
        entries.join(",")
    }
}

/// `/stepcrafter configure <pos> <min> <max>`: the maintenance band.
fn configure(host: &mut dyn ServerHost, pos: [i32; 3], min: i32, max: i32) -> String {
    let Some(level) = host.level(host.dimension()) else {
        return NO_MACHINE.to_owned();
    };
    if !is_machine(&*host, level, pos) {
        return not_a_crafter(&*host, level, pos);
    }
    let (Ok(min), Ok(max)) = (u64::try_from(min), u64::try_from(max)) else {
        return "stepcrafter: min and max are amounts, not negative numbers".to_owned();
    };
    let Some(band) = Band::new(min, max) else {
        return format!("stepcrafter: min {min} is above max {max}");
    };
    let Some(entity) = host.block_entity(level, pos) else {
        return NO_MACHINE.to_owned();
    };
    let Some(mut nbt) = entity.data(&*host) else {
        return NO_MACHINE.to_owned();
    };
    let mut state = machine::read(&nbt);
    let Some(item) = state.selected_entry().map(|entry| entry.item().to_owned()) else {
        return "stepcrafter: set a target before configuring its band".to_owned();
    };
    let _ = state.targets.retune(&item, band);
    state.phase = Phase::Idle;
    machine::write(&mut nbt, &state);
    if !matches!(entity.replace_data(host, nbt), Ok(true)) {
        return "The Step Crafter could not be written.".to_owned();
    }
    format!(
        "stepcrafter configure {} {} {} min={} max={} targets={}",
        pos[0],
        pos[1],
        pos[2],
        band.min(),
        band.max(),
        state.targets.len()
    )
}

/// `/stepcrafter configure-item <pos> <item> <min> <max>`: retune one named slot's band, leaving
/// the other slots and their patterns alone. This is the per-pattern min/max upstream edits in its
/// pattern-slot screen.
fn configure_item(
    host: &mut dyn ServerHost,
    pos: [i32; 3],
    item: &str,
    min: i32,
    max: i32,
) -> String {
    let Some(level) = host.level(host.dimension()) else {
        return NO_MACHINE.to_owned();
    };
    if !is_machine(&*host, level, pos) {
        return not_a_crafter(&*host, level, pos);
    }
    let (Ok(min), Ok(max)) = (u64::try_from(min), u64::try_from(max)) else {
        return "stepcrafter: min and max are amounts, not negative numbers".to_owned();
    };
    let Some(band) = Band::new(min, max) else {
        return format!("stepcrafter: min {min} is above max {max}");
    };
    let Some(entity) = host.block_entity(level, pos) else {
        return NO_MACHINE.to_owned();
    };
    let Some(mut nbt) = entity.data(&*host) else {
        return NO_MACHINE.to_owned();
    };
    let mut state = machine::read(&nbt);
    if !state.targets.retune(item, band) {
        return format!("stepcrafter: no pattern slot maintains {item}");
    }
    state.phase = Phase::Idle;
    machine::write(&mut nbt, &state);
    if !matches!(entity.replace_data(host, nbt), Ok(true)) {
        return "The Step Crafter could not be written.".to_owned();
    }
    format!(
        "stepcrafter configure-item {} {} {} item={} min={} max={} targets={}",
        pos[0],
        pos[1],
        pos[2],
        item,
        band.min(),
        band.max(),
        state.targets.len()
    )
}

/// `/stepcrafter step <pos>`: one step now, whatever the band says.
fn step(host: &mut dyn ServerHost, pos: [i32; 3]) -> String {
    let Some(level) = host.level(host.dimension()) else {
        return NO_MACHINE.to_owned();
    };
    if !is_machine(&*host, level, pos) {
        return not_a_crafter(&*host, level, pos);
    }
    let result = machine::step_once(host, level, pos);
    format!(
        "stepcrafter step {} {} {} result={}",
        pos[0], pos[1], pos[2], result
    )
}

/// `/stepcrafter take <pos> <item> <count>`: hand stored items to the command's player.
///
/// The **stored stacks themselves** are handed over, split down to the count asked for. A stack's
/// components and damage belong to what the machine holds — a named torch is still that torch, an
/// enchanted or damaged item keeps its state — so rebuilding it from its id would lose them.
fn take(host: &mut dyn ServerHost, pos: [i32; 3], item: &str, count: i32) -> String {
    let Some(level) = host.level(host.dimension()) else {
        return NO_MACHINE.to_owned();
    };
    if !is_machine(&*host, level, pos) {
        return not_a_crafter(&*host, level, pos);
    }
    let Ok(count) = u64::try_from(count) else {
        return "stepcrafter: count is an amount, not a negative number".to_owned();
    };
    let Some(entity) = host.block_entity(level, pos) else {
        return NO_MACHINE.to_owned();
    };
    let mut items = slots_of(&*host, entity);
    let mut stock = machine::stock_of(&*host, &items);
    let held = stock.count(item);
    let take = held.min(count);
    if take == 0 {
        return format!(
            "stepcrafter take {} {} {} item={} stored=0 taken=0",
            pos[0], pos[1], pos[2], item
        );
    }
    let Some(player) = acting_player(host) else {
        return "stepcrafter: only a player can be handed items".to_owned();
    };
    let mut left = u32::try_from(take).unwrap_or(u32::MAX);
    let mut given = 0u64;
    for slot in &mut items {
        if left == 0 {
            break;
        }
        let Some(stack) = slot.as_ref() else {
            continue;
        };
        if stack.item().id(&*host) != item {
            continue;
        }
        let Some(stack) = slot.take() else {
            continue;
        };
        let split = stack.split(left);
        let Some(taken) = split.taken else {
            *slot = split.remainder;
            continue;
        };
        *slot = split.remainder;
        // A refused hand-over leaves the piece in its slot, so `given` counts only what arrived.
        if player.give_stack(host, taken.clone()).is_err() {
            *slot = Some(taken);
            break;
        }
        left -= taken.count();
        given += u64::from(taken.count());
    }
    machine::replace_items(&mut *host, entity, &items);
    stock.remove(item, given);
    format!(
        "stepcrafter take {} {} {} item={} stored={} taken={}",
        pos[0],
        pos[1],
        pos[2],
        item,
        stock.count(item),
        given
    )
}

/// `/stepcrafter info <pos>`: the machine's whole state, as one parseable line.
fn readout(host: &mut dyn ServerHost, pos: [i32; 3]) -> String {
    let Some(level) = host.level(host.dimension()) else {
        return NO_MACHINE.to_owned();
    };
    if is_requester(&*host, level, pos) {
        return requester_readout(host, level, pos);
    }
    if !is_machine(&*host, level, pos) {
        return NO_MACHINE.to_owned();
    }
    let Some(entity) = host.block_entity(level, pos) else {
        return NO_MACHINE.to_owned();
    };
    let nbt = entity.data(&*host).unwrap_or_default();
    let state = machine::read(&nbt);
    let items = slots_of(&*host, entity);
    let stock = machine::stock_of(&*host, &items);
    let target = state.target().unwrap_or("none");
    let (min, max) = state
        .band()
        .map_or(("-".to_owned(), "-".to_owned()), |band| {
            (band.min().to_string(), band.max().to_string())
        });
    let cells = state.pattern().map_or(0, |pattern| pattern.cells().len());
    let result = state.pattern().map_or("none".to_owned(), |pattern| {
        format!("{}:{}", pattern.result(), pattern.result_count())
    });
    let targets: Vec<String> = state
        .targets
        .iter()
        .map(|entry| {
            let (min, max) = entry
                .band()
                .map_or(("-".to_owned(), "-".to_owned()), |band| {
                    (band.min().to_string(), band.max().to_string())
                });
            format!(
                "{}|{}|{}|{}|{}|{}",
                entry.item(),
                min,
                max,
                entry.phase().id(),
                entry.crafted(),
                stock.count(entry.item())
            )
        })
        .collect();
    let targets_line = if targets.is_empty() {
        "none".to_owned()
    } else {
        targets.join(";")
    };
    let asks: Vec<String> = state.queue.encode();
    let asks = if asks.is_empty() {
        "none".to_owned()
    } else {
        asks.join(";")
    };
    let served: Vec<String> = state.queue.encode_served();
    let served = if served.is_empty() {
        "none".to_owned()
    } else {
        served.join(";")
    };
    format!(
        "stepcrafter {} {} {} target={} min={} max={} stored={} phase={} cells={} result={} crafted={} queued={} asks={} served={} items={} stock={} targets={} entries={}",
        pos[0],
        pos[1],
        pos[2],
        target,
        min,
        max,
        stock.count(target),
        state.phase.id(),
        cells,
        result,
        state.crafted,
        state.queue.len(),
        asks,
        served,
        stock.total(),
        stock_line(&stock),
        state.targets.len(),
        targets_line,
    )
}

/// `/stepcrafter info <pos>` for a Step Requester: every filter entry, the crafter it asks, that
/// crafter's stock of the entry's resource (upstream's `storageComponent.get(resource)`) and the
/// entry's own share of it.
///
/// The single-entry fields (`item/min/max/batch/stored/served/queued/phase/crafter/backoff`)
/// describe the first entry; `filters` and `entries` name the whole container.
fn requester_readout(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3]) -> String {
    let Some(entity) = host.block_entity(level, pos) else {
        return NO_MACHINE.to_owned();
    };
    let nbt = entity.data(&*host).unwrap_or_default();
    let state = requester::read(&nbt);
    let views = requester::views(&*host, level, pos, &nbt);
    let first = views.first();
    let item = first.map_or_else(|| "none".to_owned(), |view| view.item.clone());
    let min = first.map_or_else(|| "-".to_owned(), |view| view.min.to_string());
    let max = first.map_or_else(|| "-".to_owned(), |view| view.max.to_string());
    let batch = first.map_or_else(|| "-".to_owned(), |view| view.batch.to_string());
    let stored = first.map_or(0, |view| view.stored);
    let served = first.map_or(0, |view| view.served);
    let queued = first.map_or(0, |view| view.queued);
    let phase = first.map_or("idle", |view| view.phase);
    let backoff = first.map_or(0, |view| view.backoff);
    let crafter = first
        .and_then(|view| view.crafter)
        .map_or_else(|| "none".to_owned(), |[x, y, z]| format!("{x},{y},{z}"));
    let entries: Vec<String> = views.iter().map(requester::View::encode).collect();
    let entries = if entries.is_empty() {
        "none".to_owned()
    } else {
        entries.join(";")
    };
    format!(
        "stepcrafter {} {} {} kind=requester filters={} item={} min={} max={} batch={} stored={} served={} queued={} phase={} crafter={} backoff={} requests={} entries={}",
        pos[0],
        pos[1],
        pos[2],
        state.filters.len(),
        item,
        min,
        max,
        batch,
        stored,
        served,
        queued,
        phase,
        crafter,
        backoff,
        state.requests,
        entries,
    )
}

/// The refusal for a command only a Step Requester answers, at a position that holds something else.
fn ensure_requester(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> Result<(), String> {
    if is_requester(host, level, pos) {
        Ok(())
    } else if is_machine(host, level, pos) {
        Err(
            "stepcrafter: that is a Step Crafter; a filter is set on the requester beside it"
                .to_owned(),
        )
    } else {
        Err(NO_MACHINE.to_owned())
    }
}

/// `/stepcrafter requester [add] <pos> <item> <min> <max> <batch>`: add a filter entry, or replace
/// the one that already maintains this resource (upstream's filter slot with its resource, minimum,
/// maximum and batch size).
fn set_filter(
    host: &mut dyn ServerHost,
    pos: [i32; 3],
    item: &str,
    min: i32,
    max: i32,
    batch: i32,
) -> String {
    let Some(level) = host.level(host.dimension()) else {
        return NO_MACHINE.to_owned();
    };
    if let Err(refusal) = ensure_requester(&*host, level, pos) {
        return refusal;
    }
    let Ok(id) = Identifier::parse(item) else {
        return format!("stepcrafter: {item} is not an item id");
    };
    if host.item_key(&id).is_none() {
        return format!("stepcrafter: {item} is not an item this world knows");
    }
    let (Ok(min), Ok(max), Ok(batch)) =
        (u64::try_from(min), u64::try_from(max), u64::try_from(batch))
    else {
        return "stepcrafter: min, max and batch are amounts, not negative numbers".to_owned();
    };
    let Some(band) = Band::new(min, max) else {
        return format!("stepcrafter: min {min} is above max {max}");
    };
    let Some(filter) = Filter::new(item.to_owned(), band, batch) else {
        return format!("stepcrafter: {item} is not an item id");
    };
    let Some(entity) = host.block_entity(level, pos) else {
        return NO_MACHINE.to_owned();
    };
    let Some(mut nbt) = entity.data(&*host) else {
        return NO_MACHINE.to_owned();
    };
    let mut state = requester::read(&nbt);
    if state.filters.set(filter.clone()).is_err() {
        return format!(
            "stepcrafter: the requester already carries {} filter entries",
            crate::runtime::MAX_FILTER_ENTRIES
        );
    }
    requester::write(&mut nbt, &state);
    if !matches!(entity.replace_data(host, nbt), Ok(true)) {
        return "The Step Requester could not be written.".to_owned();
    }
    format!(
        "stepcrafter requester {} {} {} item={} min={} max={} batch={} filters={}",
        pos[0],
        pos[1],
        pos[2],
        filter.item(),
        filter.band().min(),
        filter.band().max(),
        filter.batch(),
        state.filters.len()
    )
}

/// `/stepcrafter requester remove <pos> <item>`: clear the entry that maintains this resource;
/// removing a resource the requester does not carry changes nothing.
fn remove_filter(host: &mut dyn ServerHost, pos: [i32; 3], item: &str) -> String {
    let Some(level) = host.level(host.dimension()) else {
        return NO_MACHINE.to_owned();
    };
    if let Err(refusal) = ensure_requester(&*host, level, pos) {
        return refusal;
    }
    let Some(entity) = host.block_entity(level, pos) else {
        return NO_MACHINE.to_owned();
    };
    let Some(mut nbt) = entity.data(&*host) else {
        return NO_MACHINE.to_owned();
    };
    let mut state = requester::read(&nbt);
    let removed = state.filters.remove(item);
    requester::write(&mut nbt, &state);
    if !matches!(entity.replace_data(host, nbt), Ok(true)) {
        return "The Step Requester could not be written.".to_owned();
    }
    format!(
        "stepcrafter requester {} {} {} item={} removed={} filters={}",
        pos[0],
        pos[1],
        pos[2],
        item,
        u8::from(removed),
        state.filters.len()
    )
}

/// The stock as `item:count` pairs, comma-separated, in item-id order.
fn stock_line(stock: &crate::runtime::Stock) -> String {
    let entries: Vec<String> = stock
        .entries()
        .map(|(item, count)| format!("{item}:{count}"))
        .collect();
    if entries.is_empty() {
        "empty".to_owned()
    } else {
        entries.join(",")
    }
}

/// A machine's 27 slots as the game holds them, for the readout and `/stepcrafter take`.
fn slots_of(host: &dyn Host, entity: mod_api::BlockEntityRef) -> Vec<Option<mod_api::ItemStack>> {
    (0..machine::SLOTS)
        .map(|slot| entity.item_slot(host, slot as u32).ok().flatten())
        .collect()
}

fn is_machine(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> bool {
    if !level.is_loaded(host, pos) {
        return false;
    }
    level
        .block(host, pos)
        .map(|block| block.id(host) == machine::BLOCK)
        .unwrap_or(false)
}

fn is_requester(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> bool {
    if !level.is_loaded(host, pos) {
        return false;
    }
    level
        .block(host, pos)
        .map(|block| block.id(host) == requester::BLOCK)
        .unwrap_or(false)
}

/// The refusal for a command only a Step Crafter answers, at a position that holds a requester.
fn not_a_crafter(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> String {
    if is_requester(host, level, pos) {
        "stepcrafter: that is a Step Requester; it is configured with /stepcrafter requester"
            .to_owned()
    } else {
        NO_MACHINE.to_owned()
    }
}

fn say(host: &mut dyn ServerHost, line: &str) {
    let _ = host.commands().feedback(Text::literal(line));
}

fn acting_player(host: &mut dyn ServerHost) -> Option<PlayerRef> {
    host.players().acting_player()
}

fn block_pos(invocation: &CommandInvocation) -> Option<[i32; 3]> {
    match invocation.argument("pos")? {
        CommandArgumentValue::BlockPos(pos) => Some(*pos),
        _ => None,
    }
}

fn integer(invocation: &CommandInvocation, name: &str) -> Option<i32> {
    match invocation.argument(name)? {
        CommandArgumentValue::Integer(value) => Some(*value),
        _ => None,
    }
}

/// An `id` argument (`minecraft:torch`); `resource:` arguments arrive as a parsed identifier, so
/// both spellings are accepted.
fn id(invocation: &CommandInvocation, name: &str) -> Option<String> {
    match invocation.argument(name)? {
        CommandArgumentValue::String(value) => Some(value.clone()),
        CommandArgumentValue::Id(value) => Some(value.to_string()),
        _ => None,
    }
}
