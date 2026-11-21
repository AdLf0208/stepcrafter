# Step Crafter — port notes

Ported from **Step Crafter 1.21.1-0.1.10** (`starforcraft/Step-Crafter`, **MIT**, author
Ultramega), a **Refined Storage** add-on. Upstream analysis against the public repository at
commit `963cd494187ea762b4d78cecd55a45f3cb479812` (2026-11-20, `minecraftVersion=26.1.2`,
`modVersion=26.1.2-1.0.5`) and the ATM10 jar `stepcrafter-neoforge-1.21.1-0.1.10.jar` (kept as
evidence under `ports/_shared/licence-cache/`). The port targets Horizon **26.3** at pin
`0a875da91dbb0fdcb7ba8c42c11341aa229af51a` + horizon-mixins `e59da9094f0c2fdda6c627fe9eff22dec6a906f0`,
`api = "0.2"`, package format `native-1`, launcher 0.2.92.

**Class + tier:** `content` / **machines** (`ports/ledger.csv`; provenance
`ports/_shared/source-map/atm10-sources.csv`). The slice is a block entity with a container and a
tick — the machines shape, no entities, renderers or dimensions.

**Licence rank 0** (MIT, read from the jar's `META-INF/neoforge.mods.toml`; upstream's
`LICENSE.txt` travels inside the package as `LICENSE`). Publishable under MIT with this source.

**Skills gate:** `Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices`

---

## 0. The parent surface, measured before any code (ruling 2026-11-21)

The dispatch called the parent `mekanism`; the artefact says **Refined Storage**
(`META-INF/neoforge.mods.toml`: `modId = "refinedstorage"`, `type = "required"`,
`versionRange = "2.0.4"`), and the jar references **zero** `mekanism:` ids. The full measurement —
every parent class the add-on binds to, and what `ports/refinedstorage` and the pin actually ship —
is `ports/_shared/evidence/stepcrafter/parent-surface.md`. In one line:

- the **network half** (`AutocraftingNetworkComponent`, `PatternProvider`, `RootStorage`,
  `UpgradeContainer`, the network-node base, the grid screens, and the `refinedstorage:autocrafter`
  / `:detector` / `:upgrade` / `:pattern` items three of its recipes name) is **absent** from the
  parent port *and* from the pin, and no Horizon mod can call another port's Rust — so the
  requester, the two managers, the monitor, the colour variants, the slot upgrade, the FE buffer
  and the GUIs are `blocked(no-parent-addon-surface)`, exactly as `mifa` was;
- the **core mechanic** — *keep a resource between a minimum and a maximum by stepping its own
  crafting pattern* — is the add-on's own rule (`StepCrafterNetworkNode.doWork`,
  `StepCraftingAlgorithm`), and it needs only the pin's recipe surface
  (`recipes_of_type`, `Recipe.ingredients/results`, `Ingredient::item_keys`). That is what this
  port ships, the way `extradisks` (also an RS add-on, ledger `ported-partial`) ships its storage
  without a network.

## 1. The jar's families, and what this slice does with each

Inventory: `unzip -l` of the jar — 51 blockstates, 52 item models, 714 files, `data/stepcrafter/`
with 5 base recipes, 51 colouring recipes, 51 loot tables, 3 item tags, 1 block tag, and the
`en_us` lang.

| family (from the jar) | this slice |
|---|---|
| `step_crafter` block + block entity (the machine, `active` state, red/`side_active` art) | **ported** — the block, its 27-slot inventory, the `active` state, the band rule, `recipes`/`loot`/`lang`/`tab` |
| its pattern system (a Refined Storage *pattern item* per slot, min/max/batch per slot) | **ported differently**: the pattern is the target item's own `minecraft:crafting` recipe (`/stepcrafter target <pos> <item>`), because an RS pattern item is not representable here; the band is `/stepcrafter configure <pos> <min> <max>` |
| `step_requester` block + entity (asks *other* autocrafters for a resource) | `blocked(no-parent-addon-surface)` — there is no autocrafting network to request from |
| `step_crafter_manager`, `step_requester_manager` (+16 colours each), `step_crafting_monitor` (+16 colours) | `blocked(no-parent-addon-surface)` — each is a mixin into Refined Storage's grid screen, which the parent port does not ship (0 hits for `wireless_grid`) |
| `slot_upgrade` item (a slot upgrade, `Items.INSTANCE.getSlotUpgrade`) | `not ported` — upgrades are the RS `UpgradeContainer`; the machine has 27 slots and no upgrade slots |
| the 51 colouring recipes + the three colour tags | `not ported` — they recolour the managers/monitor, which are blocked |
| FE buffer (`energyUsage` per machine, `SimpleNetworkNode`) | `not ported` — energy is the RS network's; this machine's stock is its inventory |
| GUI (screens, side-button widgets, the preview screen, slots with min/max editors) | `not ported` — the interface is the vanilla chest menu (the container) plus `/stepcrafter`; screens are a later slice |
| `active` state, `direction` state | `active` ported; `direction` **not ported** (the model is a cube; no facing behaviour in this slice) |
| advancements, JEI/EMI plugins, config (`energyUsage`, `speedMultiplier`, defaults) | `not ported` — client/optional surface; defaults are documented where the port needs a number |

## 2. The slice: what it is (version 0.2.0)

The machine is told **what to keep in stock** and **between which amounts**:

- `/stepcrafter target <pos> <item>` resolves the item's own `minecraft:crafting` recipe into a
  **step pattern** — one cell per non-empty recipe cell, each cell the set of items that satisfy
  it (a tag expands to its members) — and stores it in the block entity's saved compound.
- `/stepcrafter configure <pos> <min> <max>` sets the maintenance **band** (upstream's
  `ResourceMinMaxAmount`, whose inverted form is refused).
- The block entity ticks every tick, exactly as `StepCrafterNetworkNode.doWork` does:
  `if phase != crafting && stored >= min → hold`; `if phase == crafting && stored >= max → finish`;
  otherwise **one step**: resolve each cell against the stock, take one item of each, put the
  result in. So the band is **hysteretic** — it starts below `min` and fills to `max`.
- The **stock is the machine's own 27-slot inventory** (a declared container with the vanilla chest
  menu: right-click opens it, a hopper reaches it, the engine saves it and spills it when the block
  breaks). Upstream's stock is the network; this is the same quantity, bounded.
- `/stepcrafter info` prints one parseable line; `/stepcrafter step` takes one step now (the
  band's timing bypassed — the manual step); `/stepcrafter take` hands the target back.
- The `active` block state is true while the machine is crafting.

## 3. TDD: red, then green

The red transcript is `ports/_shared/evidence/stepcrafter/red-unit.log` — the rule and the step
were stubs returning `Hold`, so:

```
assertion `left == right` failed: a stock below min must be crafted
  left: Hold
 right: Craft
...
test result: FAILED. 4 passed; 8 failed
```

Green today: **17 tests**, `sdk-test.log`. They cover the rule (`decide_holds_at_the_minimum_while_idle`,
`decide_crafts_below_the_minimum`, `decide_keeps_crafting_above_the_minimum_until_the_maximum`,
`decide_finishes_at_the_maximum`), the step (`a_step_consumes_every_cell_and_makes_the_result`,
`a_step_prefers_the_first_candidate_the_stock_holds`,
`a_missing_ingredient_refuses_the_step_and_names_the_cell`, `a_full_inventory_refuses_the_step`,
`apply_moves_the_stock_by_exactly_the_outcome`, `a_pattern_round_trips_through_its_saved_form`),
the manual step (`a_manual_step_crafts_even_inside_the_band`,
`a_manual_step_still_needs_the_ingredients`), and the saved compound.

The in-game leg then found two things a unit test cannot. (1) A stock kept as a **ledger in the
block entity's saved compound** lost entries between ticks: a stepped machine reported `crafted=4`
and an empty stock, and a later read of the compound held neither the ingredients nor the product,
while the same tick's `Int`/`String` keys survived — measured with a per-tick log written into the
compound (`read=2 craft->4`, then `read=4`, then `read=0`). Re-checking the ledger format did not
explain it, so the stock was moved to where the engine owns it: a **declared container**, whose
slots are the block entity's `items` field, which the engine saves, spills and shows in a menu.
(2) A container block must let the engine spill its contents rather than carry a
`block_entity_data` payload (HANDOVER §9d.23); the payload was tried, did not restore the block
entity, and was removed. Both are recorded under §6.

## 4. In-game evidence (what the transcript proves)

`ports/_shared/evidence/stepcrafter-e2e/` — three logs, all `--strict` green:

| log | verdict | what it is |
|---|---|---|
| `e2e-launcher.log` | `PASS` + `EXIT:0` | the scenario below, on the game the launcher built with the package, plus an in-run control leg on the same Nightly built without it |
| `e2e-control.log` | `FAIL` + `EXIT:1` | the same script with `GAME` = the Nightly **without** the package: `setblock stepcrafter:step_crafter` leaves air |
| `e2e-selftest.log` | `FAIL` + `EXIT:1` | `SELFTEST=1` inverts the first headline assertion (`stored=16`) and the run ends non-zero |

The scenario, in order (each assertion is parsed from a reply, never "no error"):

1. the block places and reads back `stepcrafter:step_crafter`; the item registers; the unported
   `stepcrafter:step_crafter_manager` is refused by `setblock` and by `give`;
2. the carried recipe crafts **one** Step Crafter from 4 iron, 1 redstone, 2 gold, 1 crafter and
   1 hopper in a real table and the grid is consumed; the same items in another pattern craft
   nothing (`tools/craft_check.py`);
3. a fresh machine reads `target=none min=- max=- stored=0 phase=idle cells=0 crafted=0`, and its
   container is empty;
4. `target … minecraft:torch` → `cells=2 result=minecraft:torch:4`; `minecraft:bedrock` →
   "has no minecraft:crafting recipe"; `minecraft:not_a_thing` → "is not an item this world knows";
5. `configure 8 16` works; `configure 9 4` is refused ("min 9 is above max 4");
6. the container is loaded with 8 coal + 8 sticks (`ctl container`, the hopper path) and the
   machine reads `stored=0 … stock=minecraft:coal:8,minecraft:stick:8`; the band then set to 8..16,
   the tick fills **0 → 16 in steps of 4**, consuming exactly **4 coal and 4 sticks**
   (`crafted=16`); 60 more ticks change nothing (no overshoot);
7. `/stepcrafter take … minecraft:torch 8` → `stored=8 taken=8`, and the inventory gains 8;
8. drain the sticks, take the target to 0 → `phase=not-enough-ingredients` with `crafted=16`
   unchanged; restock 4 sticks → it resumes to `stored=16 crafted=32`;
9. `/stepcrafter step` on a band of 0..0 crafts one step (`stored=4 crafted=4`), and a second step
   with no ingredients answers `not-enough-ingredients`;
10. band 64..128 with 32+32 loaded: `active=true` while it runs, `active=false` once idle, and it
    stops at **128** — past `min` (64), which is the hysteresis;
11. a survival break with a pickaxe drops the machine's own item **and spills the 8 torches**
    (`drops: minecraft:torch:8, stepcrafter:step_crafter:1`); placing the item again gives an
    unconfigured machine (the recorded limit, §6);
12. a non-operator's `/stepcrafter info` is refused.

## 5. Packaging

| item | value |
|---|---|
| file | `ports/stepcrafter/stepcrafter.hmod` |
| size | **33,589 B** |
| sha256 | `32b80ec228f8ac978b4413f6e207b79ca1660aa986ca4f13acb8607de970abad` |
| entries | 23 (`111,459 B` uncompressed), **no `Cargo.lock`** |
| version | `0.2.0` (`horizon-mod.toml` + `Cargo.toml`) |
| build | `cargo horizon-mod build`, twice, `rm -f Cargo.lock` before each → `cmp` identical (`sdk-build-twice.log`) |
| pin | `0a875da  made for` / `0b14f8a  fits` (`sdk-fits-0a875da.log`, `sdk-fits-0b14f8a.log`, each printing the sha) |

## 6. Known limits and deviations (recorded, not implied)

- **The stock is the machine's own inventory, not a network** (`blocked(no-parent-addon-surface)`):
  1,728 items in all, and no network of other machines shares it. Upstream's stock is the RS
  network's storage.
- **The pattern comes from the item's crafting recipe, not from an RS pattern item.** `Recipe`
  gives the non-empty cells of the shaped recipe (or the shapeless list) and `Ingredient::item_keys`
  the accepted items, so a step consumes exactly what the recipe declares and nothing else. A
  pattern with a mod's own component predicate (`IngredientKeysError::NotEnumerable`) is refused as
  `UnsupportedRecipe` rather than silently never matching.
- **The configuration does not travel on the item.** A broken machine's item is the plain block
  item and its stock spills (`drops: minecraft:torch:8, stepcrafter:step_crafter:1`); a moved
  machine is re-targeted. Upstream's pattern *is* an item and drops, so it also loses nothing —
  but this port's pattern is derived, so there is nothing to drop. A `block_entity_data` payload
  was tried and removed: it did not restore the block entity, and HANDOVER §9d.23 forbids it on a
  block whose contents the engine already spills.
- **`crafting remainders` are not produced** (a bucket recipe leaves its bucket upstream in the
  network): the cells are consumed and the result inserted. No recipe the port's own e2e uses has
  one; record it as `not ported` rather than pretend.
- **`direction` state, the GUI, the colours, the requester, the managers, the monitor, the slot
  upgrade, the FE buffer** — see §1's table, each with its reason.
- **Recipe adapted (deviation, not a defect):** upstream's `step_crafter.json` is
  `ECE/AMA/EDE` with `refinedstorage:` ingredients (`quartz_enriched_iron`, `construction_core`,
  `advanced_processor`, `autocrafter`, `destruction_core`). This package's own game cannot name
  another mod's items in an isolated install, and `refinedstorage:autocrafter` does not exist in
  the parent port's content either, so the shape is kept and the five keys are re-rooted on
  vanilla materials this port's game has: `E = minecraft:iron_ingot`, `C = minecraft:redstone`,
  `A = minecraft:gold_ingot`, `M = minecraft:crafter`, `D = minecraft:hopper`. A user notices a
  recipe they can craft without Refined Storage; `notes.md` §4 proves it in a real table.

## 7. Behaviour, and where it comes from

| behaviour | upstream | this port |
|---|---|---|
| the band rule (start below min, fill to max, hold at max) | `StepCrafterNetworkNode.doWork`, three guards | `runtime::decide`, `runtime::plan` |
| one step = resolve every cell, take, insert | `StepCraftingAlgorithm.prepareCraft/extractResolvedInputs/insertIntoNetwork` | `runtime::craft` (candidate resolution, reservation by counting), `machine::take_from_slots` / `put_into_slots` |
| `NETWORK_FULL` | `canInsertIntoNetwork` before extracting | `runtime::craft`'s capacity check (the whole inventory, 27 × 64) |
| `NOT_ENOUGH_INGREDIENTS` | `prepareCraft` returns null | `Outcome::NotEnoughIngredients` naming the short cell |
| status per pattern slot | `ResourceStatus` (`CRAFTING`, `FINISHED`, `NOT_ENOUGH_INGREDIENTS`, `NETWORK_FULL`) | `runtime::Phase`, saved as `Phase` |
| `active` block state | the RS network node's `active` property | true while the machine is crafting |
| one step per tick without speed upgrades | `for j < speed + 1` with `speed = 0` | `runtime::STEPS_PER_TICK = 1` |
| the click | `ExtendedMenuProvider` opens a screen | the vanilla chest menu over the 27-slot container |

## 8. Upstream defects fixed / adaptations

None claimed: nothing in the jar's data contradicted its code. The recipe re-rooting in §6 is a
**port adaptation** (the parent's items are not representable in an isolated install), recorded as
a deviation, not as an upstream defect.

## 9. Files

| file | what it is |
|---|---|
| `src/runtime.rs` | the pure rule: `Band`, `Phase`, `Step`, `decide`, `Stock`, `Pattern`, `plan`, `step_now`, `apply` — no `mod_api` types |
| `src/pattern.rs` | the target item's crafting recipe resolved into a `Pattern` (one host crossing, at configuration) |
| `src/machine.rs` | the block entity: saved compound, the 27-slot inventory as a stock, the tick, the manual step |
| `src/commands.rs` | `/stepcrafter info|target|configure|step|take` |
| `src/lib.rs` | the one `impl Hooks` and the `Part` the game ticks |
| `tools/e2e.sh`, `tools/craft_check.py` | the in-game leg of §4 |
| `LICENSE` | upstream's MIT text (`starforcraft/Step-Crafter`) |
| `assets/`, `data/` | the carried content: one block (states `active`), its model/textures, loot, recipe, tag, lang, creative tab |

Textures are upstream's own (MIT) — `step_crafter/{top,side_active,side_inactive}.png`, byte-copied
from the jar; the two models are re-authored for this package (a cube with those three textures
and a vanilla bottom, since upstream's model names `refinedstorage:block/bottom`, which this
install does not have).

## 10. Coverage (family → evidence, per the parity rule)

| family | row |
|---|---|
| the band rule: start below `min`, fill to `max`, hold at `max` | **tested** — unit `decide_holds_at_the_minimum_while_idle` / `decide_crafts_below_the_minimum` / `decide_keeps_crafting_above_the_minimum_until_the_maximum` / `decide_finishes_at_the_maximum`; in game `stored=16 crafted=16` then 60 idle ticks unchanged, and the 64..128 band filling to 128 |
| one step: every cell out, the result in | **tested** — `a_step_consumes_every_cell_and_makes_the_result`, `a_step_prefers_the_first_candidate_the_stock_holds`, `apply_moves_the_stock_by_exactly_the_outcome`; in game 4 coal + 4 sticks → 16 torches, `stock=minecraft:coal:4,minecraft:stick:4,minecraft:torch:16` |
| `not-enough-ingredients` | **tested** — `a_missing_ingredient_refuses_the_step_and_names_the_cell`; in game `phase=not-enough-ingredients` at `crafted=16`, then the restock resumes to `crafted=32` |
| `network-full` | **tested** (unit only) — `a_full_inventory_refuses_the_step`; in game only by construction (1,728 items), recorded as such |
| the manual step | **tested** — `a_manual_step_crafts_even_inside_the_band`, `a_manual_step_still_needs_the_ingredients`; in game `result=crafted` then `result=not-enough-ingredients` |
| the pattern from the item's recipe | **tested** in game — `target=minecraft:torch cells=2 result=minecraft:torch:4`; refusals for a recipe-less item and an unknown id |
| the container (the stock) | **tested** in game — `ctl container` loads 8+8 and the readout agrees; empty for a fresh machine |
| the recipe | **tested** in game — `tools/craft_check.py` (one output, five ingredients consumed, wrong pattern crafts nothing) |
| the block and the item | **tested** in game — placed, given, read back; `stepcrafter:step_crafter_manager` refused by `setblock` and `give` |
| the commands | **tested** in game — every reply parsed, incl. `min 9 is above max 4` and the operator refusal |
| the `active` state | **tested** in game — `execute if block …[active=true]` → `Test passed` while filling, `[active=false]` once idle |
| loot, stock spill, `mineable/pickaxe` | **tested** in game — a survival break with a pickaxe drops `stepcrafter:step_crafter:1` + `minecraft:torch:8`; a bare hand drops no block |
| the step requester | **not ported** — `blocked(no-parent-addon-surface)`: needs the RS autocrafting component |
| the crafter/requester managers, the crafting monitor, the 48 colour variants | **not ported** — `blocked(no-parent-addon-surface)`: mixins into Refined Storage's grid screen |
| the slot upgrade, the FE buffer, the GUI, `direction`, advancements, JEI/EMI | **not ported** — upgrades/energy/GUI are the RS harness; `direction` is cosmetic here |
| crafting remainders | **not ported** — no recipe this slice proves has one |
