//! Step Crafter for Horizon: the threshold-maintained step crafting machine.
//!
//! Ported from Step Crafter 1.21.1-0.1.10 (`starforcraft/Step-Crafter`, MIT), a Refined Storage
//! add-on. [`runtime`] is the pure truth — the maintenance band, the ingredient resolution and the
//! stock arithmetic, with its tests. [`pattern`] resolves a target item's own crafting recipe into
//! the machine's step pattern, [`machine`] is the block entity (its saved state, its tick, the
//! 27-slot inventory (a vanilla chest menu) and the tick), and [`commands`] is
//! `/stepcrafter …`.
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
//! stock is the machine's own (27 slots' worth, held as a ledger in its saved compound) and the
//! pattern is resolved from the target item's own `minecraft:crafting` recipe. The network
//! families — the requester, the two managers, the crafting monitor, the colour variants, the slot
//! upgrade, the FE buffer and the GUIs — are the gap list in `notes.md`, `blocked` on that surface.

pub mod commands;
pub mod machine;
pub mod nbt;
pub mod pattern;
pub mod requester;
pub mod runtime;

use mod_api::{
    self, BlockEntityPart, BlockEntityTickEvent, BlockEntityTicksEvent, BlockPart, Flow, Hooks,
    RegisterCommandsEvent, Registrar, RegistrationContext, RunModCommandEvent, ServerHost,
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
}
