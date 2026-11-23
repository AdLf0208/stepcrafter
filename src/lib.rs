//! Step Crafter for Horizon: the threshold-maintained step crafting machine.
//!
//! Ported from Step Crafter 1.21.1-0.1.10 (`starforcraft/Step-Crafter`, MIT), a Refined Storage
//! add-on. [`runtime`] is the pure truth — the maintenance band, the ingredient resolution and the
//! stock arithmetic, with its tests. [`pattern`] resolves a target item's own crafting recipe into
//! the machine's step pattern, [`machine`] is the crafter's block entity (its saved state, its tick,
//! its 27-slot inventory and its ask queue), [`requester`] the Step Requester's (its filter entries,
//! its screen and the asks it leaves with a nearby machine), and [`commands`] is `/stepcrafter …`.
//!
//! # What this slice is
//!
//! Upstream's core: a machine that keeps a resource in stock. It is told a pattern and a minimum
//! and maximum; when the stock of the pattern's output falls below the minimum it crafts one step
//! at a time until the maximum is reached (`StepCrafterNetworkNode.doWork`), resolving each step's
//! ingredients from the stock (`StepCraftingAlgorithm`).
//!
//! Upstream's stock is a Refined Storage network and its pattern is an RS pattern item. Neither is
//! reachable from a Horizon add-on (`ports/_shared/evidence/stepcrafter/parent-surface.md`), so the
//! stock is the machine's own (its 27 slots, a declared container) and the pattern is resolved from
//! the target item's own `minecraft:crafting` recipe. The requester is re-rooted on a nearby machine
//! and edits its 45 filter entries on its own screen; the two managers, the crafting monitor, the
//! colour variants, the slot upgrade and the FE buffer are the gap list in `notes.md`, `blocked` on
//! that surface.

pub mod commands;
pub mod machine;
pub mod nbt;
pub mod pattern;
pub mod requester;
pub mod runtime;
pub mod screen;

use mod_api::{
    self, BlockEntityPart, BlockEntityTickEvent, BlockEntityTicksEvent, BlockPart, ClientHost,
    Flow, Hooks, HostSide, MenuButtonEvent, MenuExtraDataEvent, RegisterCommandsEvent, Registrar,
    RegistrationContext, RightClickBlockEvent, RunModCommandEvent, ScreenClickEvent,
    ScreenDrawEvent, ScreenEvent, ServerHost,
};

/// The mod keeps no state of its own: a machine's pattern, band, phase and stock all live in its
/// block entity's saved compound.
#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    /// Declare the block entity types the two blocks use, and the Parts that tick them.
    fn register(&self, r: &mut dyn Registrar, _cx: &RegistrationContext) {
        machine::declare(r);
        requester::declare(r);
    }

    /// `/stepcrafter info|target|configure|step|take`.
    fn register_commands(&self, _host: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        commands::declare(event.commands);
    }

    /// The bodies of those commands; one that is not ours is left to the game.
    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        if commands::run(host, event.command) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    /// The requester's filter cells on the client: the entries' resources, one id per line, which
    /// its screen draws as icons. The numbers travel in the menu's data slots; this payload is what
    /// the pin has no data slot for (an item id). The machine's one cell uses the same hook for its
    /// target. Answer only for our blocks.
    fn menu_extra_data(
        &self,
        host: &mut dyn ServerHost,
        event: &mut MenuExtraDataEvent<'_>,
    ) -> Option<Vec<u8>> {
        let level = host.level(host.dimension())?;
        match event.block {
            requester::BLOCK => requester::extra_data(&*host, level, event.pos, event.menu),
            machine::BLOCK => machine::extra_data(&*host, level, event.pos, event.menu),
            _ => None,
        }
    }

    /// A right click on either block opens its menu: the engine never opens a mod container's menu
    /// by itself (`machine::right_click` records the measurement and the route).
    fn right_click_block(&self, host: HostSide, event: &mut RightClickBlockEvent<'_>) -> Flow {
        machine::right_click(host, event)
    }
}

/// The Part the game asks about the machine's block entity: it ticks on the server, each tick
/// running the pure maintenance rule against the machine's own stock.
pub struct StepCrafter;

#[mod_api::horizon_part]
impl BlockPart for StepCrafter {}

#[mod_api::horizon_part]
impl BlockEntityPart for StepCrafter {
    /// The machine's block entity ticks on the server (`EntityBlock.getTicker`).
    fn block_entity_ticks(&self, _host: &dyn mod_api::Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    /// One server tick: the band rule, one step, and the `active` state.
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        machine::tick(host, event);
    }

    /// The machine's own panel: its 27 stock slots, the target cell and the band's buttons.
    fn screen(
        &self,
        _host: &dyn mod_api::Host,
        event: &mut ScreenEvent<'_>,
    ) -> Option<mod_api::Screen> {
        if event.block != machine::BLOCK {
            return None;
        }
        Some(screen::machine_layout())
    }

    /// Draw the machine's target cell and band over the engine's panel and slot frames.
    fn screen_draw(&self, host: &dyn ClientHost, event: &mut ScreenDrawEvent<'_>) {
        if event.block == machine::BLOCK {
            screen::machine_draw(host, event);
        }
    }

    /// A press on the machine's panel over no stock slot: the target cell or a band button.
    fn screen_click(
        &self,
        _host: &dyn ClientHost,
        event: &mut ScreenClickEvent<'_>,
    ) -> Option<i32> {
        if event.block != machine::BLOCK {
            return None;
        }
        screen::machine_button_at(event.at)
    }

    /// The server's half of a machine press: the target cell or a band button.
    fn menu_button(&self, host: &mut dyn ServerHost, event: &mut MenuButtonEvent<'_>) {
        machine::press(host, event);
    }
}

/// The Part the game asks about the requester's block entity: it ticks on the server, each tick
/// asking the crafter it maintains its filter with.
pub struct Requester;

#[mod_api::horizon_part]
impl BlockPart for Requester {}

#[mod_api::horizon_part]
impl BlockEntityPart for Requester {
    /// The requester ticks on the server, the same way the crafter does.
    fn block_entity_ticks(&self, _host: &dyn mod_api::Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    /// One server tick: find the crafter, read its stock, ask or hold.
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        requester::tick(host, event);
    }

    /// The requester's panel: 45 filter cells drawn by [`screen`], the selected entry's band and
    /// batch, and the buttons that step them.
    fn screen(
        &self,
        _host: &dyn mod_api::Host,
        event: &mut ScreenEvent<'_>,
    ) -> Option<mod_api::Screen> {
        if event.block != requester::BLOCK {
            return None;
        }
        Some(screen::layout())
    }

    /// Draw the panel's cells, values and buttons (the client's half of a press).
    fn screen_draw(&self, host: &dyn ClientHost, event: &mut ScreenDrawEvent<'_>) {
        if event.block == requester::BLOCK {
            screen::draw(host, event);
        }
    }

    /// A press on the panel over no slot: a cell or one of the value buttons, as a menu button.
    fn screen_click(
        &self,
        _host: &dyn ClientHost,
        event: &mut ScreenClickEvent<'_>,
    ) -> Option<i32> {
        if event.block != requester::BLOCK {
            return None;
        }
        screen::button_at(event.at)
    }

    /// The server's half of a press: the filter cell or the value button the client sent.
    fn menu_button(&self, host: &mut dyn ServerHost, event: &mut MenuButtonEvent<'_>) {
        requester::press(host, event);
    }
}
