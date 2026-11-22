# Step Crafter — upstream analysis

**Repository:** <https://github.com/starforcraft/Step-Crafter> (MIT, author Ultramega),
cloned read-only to `ports/_shared/upstream/stepcrafter` at commit
`963cd494187ea762b4d78cecd55a45f3cb479812` (2026-11-20, `minecraftVersion=26.1.2`,
`modVersion=26.1.2-1.0.5`). The ATM10 jar is the older `stepcrafter-neoforge-1.21.1-0.1.10.jar`
(kept in `ports/_shared/licence-cache/`); its `data/` and `assets/` are the carried content's
source, its classes the behaviour's second witness.

The port is a **clean-room Rust rewrite**: the Java was read for behaviour (event flow, edge cases,
data shapes), never translated, and no decompilation was used.

## Files read

| file | what it settled |
|---|---|
| `common/stepcrafter/StepCrafterBlockEntity.java` | the block entity's saved keys (`patterns`, `patternResources`, `upgr`, `pri`, `vstm`, `iipc`), the 45 pattern slots and 8 upgrade slots, `preRemoveSideEffects` (drops upgrades and patterns), the menu, the `active` ticker |
| `common/stepcrafter/StepCrafterNetworkNode.java` | **the rule this port is**: `doWork`'s guards (`status != CRAFTING && stored >= min → break`; `CRAFTING && stored >= max → FINISHED`; else one step), `speed + 1` steps per tick, `NETWORK_FULL` / `NOT_ENOUGH_INGREDIENTS` / `EXTERNAL_CONTAINER_FULL` |
| `common/stepcrafter/StepCraftingAlgorithm.java` | the step itself: merge duplicate ingredients, resolve each against the storage with what earlier cells reserved, extract, insert outputs (and remainders), `canInsertIntoNetwork` before extracting |
| `common/stepcrafter/task/StepTaskImpl.java`, `task/StepTaskContainer.java` | the task per pattern request (`amount`, `craftedAmount`, states `READY/RUNNING/COMPLETED/...`) |
| `common/support/ResourceMinMaxAmount.java`, `support/PatternMinMax.java` | the band's validation (non-negative; a resource, min, max, batch, status) |
| `common/DefaultConfig.java`, `common/Config.java` | the defaults this port needed: `DEFAULT_MIN_AMOUNT = 0`, `DEFAULT_MAX_AMOUNT = 0`, `DEFAULT_BATCH_SIZE = 1`, the energy usages (not ported) |
| `common/steprequester/StepRequesterNetworkNode.java` | **the requester's rule (0.2.1)**: `StepRequesterNetworkNode.doWork` — the same hysteretic guards as the crafter (`status != CRAFTING && stored >= min → break`; `CRAFTING && stored >= max → FINISHED`), then a failed-task timeout, then `isAlreadyRunningTask`, then `needed = max - stored`, `batch = min(needed, batchSize)`, `startTask`; empty → `NOT_ENOUGH_INGREDIENTS` + 20 ticks |
| `common/steprequester/StepRequesterBlockEntity.java`, `StepRequesterData.java` | the requester's saved shape: `rf` (45 filter slots of resource+min+max+batch+status), `upgr` (8 upgrade slots), `vsrm`; it has no item storage of its own — the filter is configuration, the goods are the network's |
| `common/support/ResourceMinMaxAmount.java` (re-read) | the filter entry: resource, min, max, batch, status; `batchSize == 0` is skipped by `doWork`, which is why this port's batch 0 disables the entry |
| `common/stepmanager/AbstractStepManagerBlockEntity.java` | the managers walk the network's `StepBlock` containers — network-bound, not in this slice |
| `common/registry/*`, `common/ContentIds.java`, `common/ContentNames.java` | the ids and names the content carries |
| `neoforge/datagen/**` | the data's real shape (recipes, loot, tags) that the jar's `data/` confirms |

## Behaviours extracted (and where they live in Rust)

1. **The band is hysteretic.** Crafting *starts* below `min` and *continues to* `max` — the machine
   does not stop at `min` once it began. `runtime::decide` + `plan`. Proven in game by the 64..128
   band filling to 128 (a machine that stopped at `min` would show 64).
2. **A step resolves ingredients against stock, with alternatives, reserving what earlier cells
   took.** `runtime::craft` (a cell is a candidate set; the first candidate still in stock is
   taken; counting the reservations down as it goes).
3. **The output is checked for room *before* the ingredients are taken** (upstream: `SIMULATE`
   insert, then extract). `runtime::craft`'s capacity check, and the unit test that proves a
   refused step changes nothing.
4. **A failed step is a state, not a crash**: `NOT_ENOUGH_INGREDIENTS` and `NETWORK_FULL` are
   carried in the saved status and shown; when the stock recovers the machine resumes.
   `runtime::Phase`, and the in-game not-enough → restock → resume leg.
5. **One step per tick** without speed upgrades (`speed + 1` with `speed = 0`).
6. **`active`** is the machine's running state.

## Where the rewrite differs (and why)

| upstream | this port | why |
|---|---|---|
| stock = Refined Storage network storage | stock = the machine's own 27-slot inventory | the network surface is absent from the parent port *and* the pin (`ports/_shared/evidence/stepcrafter/parent-surface.md`) |
| pattern = an encoded RS pattern item in a pattern slot | pattern = the target item's own `minecraft:crafting` recipe, resolved at configuration | an RS pattern item is not representable; the recipe is the same object a pattern is encoded from |
| min/max/batch per pattern slot, edited in a GUI | min/max per machine via `/stepcrafter configure`; no batch | batch is the *requester's* request size upstream (`StepRequesterNetworkNode`), and 0.2.1 ships it as `/stepcrafter requester … <batch>` |
| the requester asks the network's autocrafting component; the goods land in network storage | the requester asks the nearest Step Crafter within 4 blocks that maintains the same item, and the goods land in that crafter's own inventory | there is no network (parent-surface.md). The ask is one `Int` (`Request`) written into the crafter's saved compound with `merge_data` — the pin's `/data merge block` path — and spent by the crafter's own tick, one step per tick. World-state interop only; proven in game (`notes.md` §2b, `stepcrafter-e2e/e2e-launcher.log`) |
| a screen with 45 pattern slots + 8 upgrade slots | a 27-slot container with the vanilla chest menu | screens are a later slice; the machine's stock is its inventory |
| FE per machine | none | energy is the RS network's abstraction |
| remainders inserted into the network (buckets) | not produced | recorded as `not ported`; no recipe this slice proves needs one |
| `direction` state | not carried | the model is a cube; no facing behaviour |
| the recipe's five `refinedstorage:` keys | five vanilla keys, same shape | see `notes.md` §6 (a port adaptation) |
