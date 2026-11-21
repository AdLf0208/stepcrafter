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
//! | `info <pos>` | the tooltip: maintaining min–max, the status |
//! | `step <pos>` | a manual step (upstream: click a step-crafting request) |
//! | `take <pos> <item> <count>` | taking the maintained item out of the network |

use mod_api::{
    Command, CommandArgumentValue, CommandInvocation, Host, LevelRef, PlayerRef, ServerHost, Text,
};

use crate::machine;
use crate::pattern;
use crate::runtime::{Band, Phase};

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
            "Usage: /stepcrafter <info|target|configure|step|take> <x> <y> <z> …",
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
            "Usage: /stepcrafter <info|target|configure|step|take> <x> <y> <z> …",
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
        return NO_MACHINE.to_owned();
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
    state.target = Some(pattern.result().to_owned());
    state.pattern = Some(pattern.clone());
    // A new target restarts the run: its band is the operator's to set again.
    state.phase = Phase::Idle;
    machine::write(&mut nbt, &state);
    if !matches!(entity.replace_data(host, nbt), Ok(true)) {
        return "The Step Crafter could not be written.".to_owned();
    }
    format!(
        "stepcrafter target {} {} {} target={} cells={} result={}:{}",
        pos[0],
        pos[1],
        pos[2],
        pattern.result(),
        pattern.cells().len(),
        pattern.result(),
        pattern.result_count()
    )
}

/// `/stepcrafter configure <pos> <min> <max>`: the maintenance band.
fn configure(host: &mut dyn ServerHost, pos: [i32; 3], min: i32, max: i32) -> String {
    let Some(level) = host.level(host.dimension()) else {
        return NO_MACHINE.to_owned();
    };
    if !is_machine(&*host, level, pos) {
        return NO_MACHINE.to_owned();
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
    state.band = Some(band);
    state.phase = Phase::Idle;
    machine::write(&mut nbt, &state);
    if !matches!(entity.replace_data(host, nbt), Ok(true)) {
        return "The Step Crafter could not be written.".to_owned();
    }
    format!(
        "stepcrafter configure {} {} {} min={} max={}",
        pos[0],
        pos[1],
        pos[2],
        band.min(),
        band.max()
    )
}

/// `/stepcrafter step <pos>`: one step now, whatever the band says.
fn step(host: &mut dyn ServerHost, pos: [i32; 3]) -> String {
    let Some(level) = host.level(host.dimension()) else {
        return NO_MACHINE.to_owned();
    };
    if !is_machine(&*host, level, pos) {
        return NO_MACHINE.to_owned();
    }
    let result = machine::step_once(host, level, pos);
    format!(
        "stepcrafter step {} {} {} result={}",
        pos[0], pos[1], pos[2], result
    )
}

/// `/stepcrafter take <pos> <item> <count>`: hand stored items to the command's player.
fn take(host: &mut dyn ServerHost, pos: [i32; 3], item: &str, count: i32) -> String {
    let Some(level) = host.level(host.dimension()) else {
        return NO_MACHINE.to_owned();
    };
    if !is_machine(&*host, level, pos) {
        return NO_MACHINE.to_owned();
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
    let Some(id) = mod_api::Identifier::parse(item).ok() else {
        return format!("stepcrafter: {item} is not an item id");
    };
    let Some(key) = host.item_key(&id) else {
        return format!("stepcrafter: {item} is not an item this world knows");
    };
    let per_stack = host.max_stack_size(key).unwrap_or(64).max(1);
    let mut given = 0;
    while given < take {
        let piece = (take - given).min(u64::from(per_stack));
        let Ok(piece) = u32::try_from(piece) else {
            break;
        };
        let Some(stack) = mod_api::ItemStack::new(key, piece) else {
            break;
        };
        if player.give_stack(host, stack).is_err() {
            break;
        }
        given += u64::from(piece);
    }
    machine::take_from_slots(&*host, &mut items, &[(item.to_owned(), given)]);
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
    let target = state.target.as_deref().unwrap_or("none");
    let (min, max) = state.band.map_or(("-".to_owned(), "-".to_owned()), |band| {
        (band.min().to_string(), band.max().to_string())
    });
    let cells = state
        .pattern
        .as_ref()
        .map_or(0, |pattern| pattern.cells().len());
    let result = state.pattern.as_ref().map_or("none".to_owned(), |pattern| {
        format!("{}:{}", pattern.result(), pattern.result_count())
    });
    format!(
        "stepcrafter {} {} {} target={} min={} max={} stored={} phase={} cells={} result={} crafted={} items={} stock={}",
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
        stock.total(),
        stock_line(&stock),
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
