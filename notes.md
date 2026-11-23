# Step Crafter — port notes

Ported from **Step Crafter 1.21.1-0.1.10** (`starforcraft/Step-Crafter`, **MIT**, author
Ultramega), a **Refined Storage** add-on. Upstream analysis against the public repository at
commit `963cd494187ea762b4d78cecd55a45f3cb479812` (2026-11-20, `minecraftVersion=26.1.2`,
`modVersion=26.1.2-1.0.5`) and the ATM10 jar `stepcrafter-neoforge-1.21.1-0.1.10.jar` (kept as
evidence under `ports/_shared/licence-cache/`). The port targets Horizon **26.3** at pin
`0a875da91dbb0fdcb7ba8c42c11341aa229af51a` + horizon-mixins `e59da9094f0c2fdda6c627fe9eff22dec6a906f0`,
`api = "0.2"`, package format `native-1`, launcher 0.2.92.

**Class + tier:** `content` / **machines** (`ports/ledger.csv`; provenance
`ports/_shared/source-map/atm10-sources.csv`). Every slice is a block entity with a container, a
saved band and a tick — the machines shape, no entities, renderers or dimensions.

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
  parent port *and* from the pin, and no Horizon mod can call another port's Rust — so the two
  managers, the monitor, the colour variants, the slot upgrade, the FE buffer and the managers'/
  monitor's GUIs are `blocked(no-parent-addon-surface)`, exactly as `mifa` was; the **requester**,
  its screen included, is the exception (0.2.1, 0.2.6): its rule needs no network, only a machine to
  ask, so it is re-rooted on a nearby Step Crafter (§2b) and configured on its own panel (§2g);
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
| `step_requester` block + entity (asks *other* autocrafters for a resource) | **ported differently (0.2.1, several entries in 0.2.5, its screen in 0.2.6)** — the ask goes to a **nearby Step Crafter** instead of the parent's autocrafting network: up to 45 filter entries (item, min, max, batch) edited on the requester's **own screen** (§2g) or by `/stepcrafter requester add|remove`, each linked to the crafter that maintains its item, whose inventory is the store it watches; the ask is a `Request` count in that crafter's saved compound |
| `step_crafter_manager`, `step_requester_manager` (+16 colours each), `step_crafting_monitor` (+16 colours) | `blocked(no-parent-addon-surface)` — each is a mixin into Refined Storage's grid screen, which the parent port does not ship (0 hits for `wireless_grid`) |
| `slot_upgrade` item (a slot upgrade, `Items.INSTANCE.getSlotUpgrade`) | `not ported` — upgrades are the RS `UpgradeContainer`; the machine has 27 slots and no upgrade slots, and the requester's 8 upgrade slots are the same container (§2g) |
| the 51 colouring recipes + the three colour tags | `not ported` — they recolour the managers/monitor, which are blocked |
| FE buffer (`energyUsage` per machine, `SimpleNetworkNode`) | `not ported` — energy is the RS network's; this machine's stock is its inventory |
| GUI (screens, side-button widgets, the preview screen, slots with min/max editors) | **ported for the requester (0.2.6)**: 45 filter cells, the selected entry's min/max/batch and the buttons that step them (§2g). The machine's 27-slot stock menu opens on a right click too (it never did before 0.2.6: the engine does not open a mod container's menu, §2g). The managers'/monitor's GUIs stay `blocked(no-parent-addon-surface)` |
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

## 2b. The slice: the Step Requester (version 0.2.1)

Upstream's requester (`StepRequesterNetworkNode.doWork`) watches the network's storage for a filtered
resource and asks the network's autocrafting component for batches of it:

```text
for each filter entry with batch != 0:
  stored = network.get(resource)
  if status != CRAFTING && stored >= min:   break          // nothing to do
  if status == CRAFTING && stored >= max:   FINISHED;      break
  if a failed task is timing out (20 ticks): break
  if a task is already running:              break
  needed = max - stored;  batch = min(needed, batchSize)
  startTask(resource, batch)                                 // empty => NOT_ENOUGH_INGREDIENTS + 20-tick timeout
```

This slice keeps that rule and replaces only what it reads and what it calls: `network.get(resource)`
is the stock of a **nearby Step Crafter**, and `startTask(resource, batch)` is a `Request` count
written into that crafter's saved compound, which the crafter's own tick spends one step at a time.
The decision to ask (when, how much) stays the requester's; the crafting stays the crafter's.

The two blocks therefore exchange **world state only** — the pin's sanctioned interop, not a Rust
call across ports:

| the requester does | the API |
|---|---|
| read a crafter's `Target`, `Phase`, `Request` and item slots | `Host::block_entity(level, pos)` → `BlockEntityRef::data` / `item_slot` |
| write one `Int` (`Request`) into that crafter's compound | `BlockEntityRef::merge_data` — the `/data merge block` path the API documents |

Each block still writes **only its own compound** while it ticks, except that one write, which the
`block_entity_tick` docs allow ("independent Host writes keep their own effects"). That is the
interop answer the brief asked for: **world-state interop is available and works**, and the
requester needed no fallback.

What the requester is:

- `/stepcrafter requester <pos> <item> <min> <max> <batch>` sets one filter entry (upstream's
  `ResourceMinMaxAmount`: resource, min, max, batch size; batch 0 disables the entry, as upstream
  skips it). 0.2.5 makes that entry one of the container's entries: `requester add` and
  `requester remove` edit several (§2f), and the legacy spelling still replaces the entry for its
  resource.
- It finds the **nearest** Step Crafter within **4 blocks** (a 9×9×9 cube, nearest first) whose
  `Target` is its filter item — upstream's reach is the network's, this is the documented re-root. The
  link is saved and re-validated every tick (block id and `Target`), so a moved or re-targeted
  machine is dropped and rescanned.
- The band is **hysteretic** and the batch caps one ask: it asks when the crafter's stock falls below
  `min`, keeps asking to `max`, `min(needed, batch)` at a time, and never adds a second ask while one
  is outstanding (`Request != 0`) — upstream's `isAlreadyRunningTask` guard.
- A crafter that refuses (its own phase is `not-enough-ingredients` or `network-full` while the stock
  is still short) is reported as the requester's own phase and stops it asking for **20 ticks**
  (upstream's `FAILED_TASK_TIMEOUT_TICKS`), then it retries.
- The `active` block state is true while it is asking.
- `/stepcrafter info <pos>` answers for either block; the requester's line is
  `stepcrafter <x> <y> <z> kind=requester item=… min=… max=… batch=… stored=… phase=… crafter=… backoff=… requests=…`,
  its `stored` being the filtered item's count in the linked crafter and its `running` being
  upstream's `CRAFTING` status (the crafter's own readout keeps `phase=crafting`). 0.2.5 appends
  `filters=<n>` and `entries=<item|min|max|batch|stored|phase|crafter|backoff;…>` (one field per
  entry), while the single-entry fields describe the first entry.

## 2c. The review slice (version 0.2.2): two defects the review found

Reviewer `levzzz5154`'s static review of `c8a6e83e` raised three points: two gameplay defects and a
listing/attribution blocker that the orchestrator fixes on the registry entry. This version fixes the
two defects and answers the third in the archive.

**1. The room check was the stock's budget, not the table's slots.** `runtime::craft` compared
`stock.total() + produced` against the whole inventory's 1,728 items. That is a budget for the stock,
not evidence that the result has anywhere to go: 26 slots of 64 coal and one of 60 sticks hold 1,724
items, so an aggregate 1,728 said four torches fit — and the machine consumed the coal and the stick
and put nothing in, while the requester's ask was counted as served. The fix models the inventory as
slots: `Slots`/`Slot` carry each stack's item, count and **stack limit** plus the room an empty slot
offers the result, and `craft` takes the ingredients from a copy of those slots and commits only when
`room_for(result)` in that post-consumption inventory holds the complete result. Otherwise it returns
`NetworkFull`, leaves the ingredients untouched and the ask is refused — not served.

**2. `take` destroyed item components.** It counted by item id and rebuilt `ItemStack::new`, so a
named, enchanted or damaged stack came out plain. It now hands over the **stored stacks themselves**,
split to the count asked for, and keeps each remainder in its slot; `put_into_slots` merges a step's
result only into stacks whose components match, so a plain result cannot take on a named one's
components.

**3. The listing and the archive's notice.** The registry entry is the orchestrator's and is not
touched here. The archive carries `LICENSE-NOTICE.md` (Ultramega, Starforcraft and
`https://github.com/starforcraft/Step-Crafter`, verbatim) beside upstream's `LICENSE` verbatim; §5
records both, and the packed `README.md` is the tree's current text (`cmp`-identical).

## 2e. The slice (version 0.2.4): a named stack is no room for a step's plain result

0.2.3's own gap line (§6) was the last silent-loss path in the step: `Slots` knew a slot's item,
count and limit, and `machine::put_into_slots` merged a step's plain result only into a stack whose
components match — but the room check (`Slots::room_for` / `Slots::put`) matched by item id alone.
A machine holding 60 **named** torches offered the 4 items of room their count left; the step consumed
a coal and a stick, the insert could not merge into the named stack, and the machine reported
`crafted=4` while storing nothing. The room check and the insert were two rules for one inventory.

What changed:

- `runtime::Slot` carries whether it accepts a **plain** insert (`plain`): a stack with a component
  patch or damage does not. `Slots::room_for` and `Slots::put` use only plain stacks of the item;
  `Slot::new` remains the plain constructor and `Slot::patched` names the other.
- `machine::slots_of` derives the flag from the **same predicate the insert uses**,
  `ItemStack::is_stackable_with(&ItemStack::of(key))`, so the room model and `put_into_slots` cannot
  disagree about what a slot will take.
- `machine::apply_craft` is now a commit-time dry run: it applies the whole outcome (consume, result,
  every remainder) to a **copy** of the slots through the real merge rule and commits only when every
  insert went in completely. A short insert returns `false`, the caller reports `network-full`, and
  nothing is consumed — a result the machine counted can never be silently lost again, even if a
  future rule change makes the model and the game diverge.
- A manual `/stepcrafter step` now records its refusal in `Phase` (`not-enough-ingredients` /
  `network-full`) exactly as a tick-driven attempt does, so the readout means "where the last step
  left the machine". The leg's 206 assertion moved from `phase=idle` to
  `phase=not-enough-ingredients` with it.
- The stock's **count** still counts a named torch as a torch: a name does not stop an ingredient
  matching a vanilla recipe, so consumption is unchanged; only the room for a plain *insert* changed.

## 2f. The slice (version 0.2.5): the requester's several filter entries

0.2.4's own gap line (§6) was *"The requester has one filter entry, set by command (upstream: 45
filter slots plus 8 upgrade slots edited in a screen)"* — the last mechanics gap before the GUI that
line defers.

### The artefact's saved shape, read before any code

Read from `starforcraft/Step-Crafter` at the port's analysis commit
`963cd494187ea762b4d78cecd55a45f3cb479812`:

| file | what it settles |
|---|---|
| `common/.../steprequester/StepRequesterBlockEntity.java` | `FILTERS = 9 * 5` (**45**); `TAG_RESOURCE_FILTER = "rf"` is written with `PatternResourceContainerContents.CODEC`; the 8 upgrade slots are a separate `upgr` container |
| `common/.../support/patternresource/PatternResourceContainerContents.java` | the saved value is `Codec.list(ResourceMinMaxAmount.OPTIONAL_CODEC)` — **45 optional entries, one per filter slot** |
| `common/.../support/ResourceMinMaxAmount.java` | one entry is `{resource, minAmount, maxAmount, batchSize, status}`, with `status` in `FINISHED, CRAFTING, NOT_ENOUGH_INGREDIENTS, NETWORK_FULL, EXTERNAL_CONTAINER_FULL` |
| `common/.../steprequester/StepRequesterNetworkNode.java` | a `runningTasks` and a `failedTaskTimeouts` map **per slot index**; `batchSize == 0` skips the entry; the five guards run for every entry each tick |

**The entry's target is a resource (an item), not a block position and not a crafter id.** The
resource is the entry's identity and what its band and batch belong to; its position in the saved
list is only the filter-slot index. The **block position** in this port is the *link*: which nearby
Step Crafter is asked for that resource (the network-less re-root of §2b), so the link is saved
beside each entry, not in place of its identity.

### What changed

- `runtime::Entry` is one `Filter` plus its **own** phase, link and backoff, saved as
  `item|min|max|batch|phase|link|backoff` (`|` appears in no id, phase name or position).
- `runtime::Filters` is upstream's 45-slot container: `set` adds an entry, or replaces the one that
  already maintains that resource; a 46th distinct resource is refused (`FiltersFull`); `remove`
  drops one; `encode`/`decode` are the `List` the block entity saves. A malformed saved entry is
  skipped rather than guessed at.
- `requester::State` is `{ filters, requests }`. Reading a 0.2.1–0.2.4 compound (the single
  `FilterItem/FilterMin/FilterMax/FilterBatch/Phase/Link/Backoff` keys) yields **one entry**, so an
  existing world keeps its filter; the next write uses the container and drops the old keys.
- `requester::tick` walks every entry: it resolves **that entry's** crafter and spends **that
  entry's** phase/backoff; the outstanding ask it reads is the linked crafter's own `Request`, so two
  entries on two crafters are independent, and one entry's refusal or backoff never stops another.
  `active` is true while any entry is asking.
- `commands`: `requester add <pos> <item> <min> <max> <batch>` and `requester remove <pos> <item>`
  join the legacy `requester <pos> <item> <min> <max> <batch>` (add-or-replace, so the four existing
  legs' replies are unchanged). The readout keeps the first entry's legacy fields and appends
  `filters=N entries=…` with every entry's own stored count, phase, crafter and backoff.

**A scenario correction, found in game:** the brief said "8 planks", but `minecraft:planks` is a
**tag**, not an item id at 26.3. `ctl container` stocked nothing for it (silently), so the stick
crafter could not step. The leg names `minecraft:oak_planks` and asserts the stock reads
`minecraft:oak_planks:8` before the filters, so the mistake can never pass unnoticed again.

## 2g. The slice (version 0.2.6): the Step Requester's filter screen

0.2.5's own gap line (§6) was *"the requester carries up to 45 filter entries, set by command
(upstream: the same 45 slots plus 8 upgrade slots edited in a screen)"* and §1 deferred the GUI —
*"screens are a later slice"*. This is that screen. The 8 upgrade slots stay
`blocked(no-parent-addon-surface)` (the RS `UpgradeContainer`) and are not part of it.

### The surface, measured before any code (`ports/_shared/sdk/0a875da/`)

| need | symbol | verdict |
|---|---|---|
| a menu type a content mod declares | `data/<id>/horizon/menu/<name>.json` — `slots` 1..256, `data_slots` 0..256, `results`; the type is `<id>:<name>` | **yes** |
| the menu on the block | `Registrar::block_container(&BlockKey, ModContainer { slots, faces, inputs, outputs, menu })`, where `menu` is a **menu type** id | **yes** (`minecraft:generic_9x3` for the stock, this package's own for the filters) |
| opening it | `Hooks::right_click_block` + `PlayerRef::open_menu(&mut ServerHost, BlockEntityRef)` → `Result<bool, BlockEntityError>` | **yes** |
| the panel | `BlockEntityPart::screen(&dyn Host, &mut ScreenEvent) -> Option<Screen>` (`width`, `height`, `slots`, `inventory_x/y`, `own_labels`) | **yes** |
| drawing | `screen_draw(host, &mut ScreenDrawEvent { block, data, draw, origin })`; `DrawContext::{fill, item, text}`; `ClientHost::screen_extra_data()` | **yes** |
| the press | `BlockEntityPart::screen_click(&dyn ClientHost, &mut ScreenClickEvent { block, data, at, right }) -> Option<i32>` → `ContainerButtonClick` → `BlockEntityPart::menu_button(host, &mut MenuButtonEvent { context, entity, carried })` | **yes** in game |
| `MenuClickedEvent` / `menu_clicked` | only for menus a mod **opens itself** (`native_open_menu`, a chest-shaped list of item stacks); a block's content menu gets `menu_button` | **not used**: this screen is a block menu, so its presses are `screen_click` → `menu_button` |
| config entries, not only item slots | the menu's **data slots** (`event.entity.data`, 16-bit on the wire, *not saved*) for the selected entry's numbers; `Hooks::menu_extra_data -> Option<Vec<u8>>` (≤ 32 KiB) + `ClientHost::screen_extra_data()` for the entries' item ids; the saved compound for the truth | **yes** — a chest menu that could only hold items would not express an entry, so the entries are drawn cells with a resource, a band and a batch |
| the entries on the client | — | the id list is the open payload (sent when the menu opens), the numbers are data slots (refreshed every tick and on each press) |

### Four traps, measured in game (each cost a run)

1. **The engine never opens a mod container's menu.** `data/<id>/horizon/menu/`, `block_container` and a
   `minecraft:stonecutter`/`barrel` template are not enough: a plain click on either block left
   `ctl screen` at `playing` (probe: `ports/_shared/evidence/stepcrafter/probe-menu.log`, empty hand
   and with an item). The campaign brief's trap — "a mod container's menu only opens when the block's
   template class is a container" — is a misdiagnosis: the class is irrelevant, **the call is the
   gate**. `bibliocraft` measured it first; the route `bibliocraft`, `rftoolsstorage` and
   `functionalstorage` have green in game is `Hooks::right_click_block` →
   `PlayerRef::open_menu(server, entity)` (the mod asking for its own menu, as a block's
   `getMenuProvider` would). Both blocks use it now (`machine::right_click`), which also repairs this
   port's 0.2.0–0.2.5 claim that a right click opens the stock: it never did.
2. **The container's `menu` is a menu type, not a block.** The stock's `menu: Some("minecraft:chest")`
   (0.2.0–0.2.5) and the requester's `stepcrafter:step_requester_menu` (the first 0.2.6 draft, whose
   content file was `menu/step_requester.json` ⇒ `stepcrafter:step_requester`) both name no menu type,
   so `open_menu` answered `Ok(false)` — *non-provider* — and nothing opened. They are
   `minecraft:generic_9x3` and `stepcrafter:step_requester_filters` now, and the content test
   `the_menu_declares_the_slots_the_screen_publishes` reads the menu's own file so the two cannot
   drift.
3. **A panel taller than the game's 240-pixel GUI is clipped and its clicks land elsewhere.** The
   first 0.2.6 layout was 176 × 258: the player's inventory was drawn 58 pixels below where the click
   mapping put it (`screen_click` reported `at [17, 85]` for a press at the first cell), so no press
   reached a cell. Vanilla's tallest container is 222 for the same reason. The panel is **176 × 238**
   (grid 18..108, two value rows 110/128, the engine's own "Inventory" label at 144, inventory at
   156) and the e2e proves the height from the game: the requester's panel top is 36 above the
   machine's 166-high chest panel, which is `(238 − 166) / 2`.
4. **`slot-0` in `ctl ui` is the hotbar's first slot**, not the main inventory's (vanilla's inventory
   order), so the panel's origin is `slot-0's bounds − (8, inventory_y + 58)`; the e2e derives it that
   way and checks it against the centred panel.

### What changed

- **`runtime.rs`**: `Press` (`Cell(u8)`, `MinDown/MinUp/MaxDown/MaxUp/BatchDown/BatchUp`, `Remove`,
  `Press::of(button)`/`button()` — ids `100 + cell` and `200..206`), `Editor { filters, selected }`
  with the screen's rule `apply(press, carried) -> bool`: a cell with an item makes or repoints an
  entry (a resource is an entry's identity, so a second entry for one item is refused; a repoint
  keeps the band and batch and drops the link), an empty hand only focuses a cell, and a step keeps
  the band valid (`min <= max`), never goes below zero and caps at `SCREEN_MAX = 4096`. Plus
  `view_data_slots` (`[count, selected, min, max, batch]`, 16-bit clamped) and `view_extra_data` /
  `extra_items` (the entries' ids, one per line, a malformed payload drawing nothing).
- **`requester.rs`**: `MENU = stepcrafter:step_requester_filters`; `declare` registers the container
  (one hidden slot, no faces, that menu); `State.selected` saved as `Selected`; the tick publishes the
  data slots every tick; `press` is `menu_button` on the server; `extra_data` is `menu_extra_data`.
- **`screen.rs`** (new): the 176 × 238 layout, `cell_at`/`button_at` hit tests and the drawing, with
  the row-fit as a compile-time assertion.
- **`machine.rs`**: `right_click` (the click route above) and the stock's menu type.
- **`lib.rs`**: the `right_click_block` and `menu_extra_data` hooks, and the requester's Part gains
  `screen`, `screen_draw`, `screen_click`, `menu_button`.
- **Content**: `data/stepcrafter/horizon/menu/step_requester_filters.json` (`{"slots": 1,
  "data_slots": 5}`).
- **Commands unchanged**: they write the same `Filters` the screen edits — one saved container.

The screen shows the selected entry's three values live (data slots) and every entry's icon (the open
payload, refreshed when the menu is reopened); a cell that is `count` is the next one a click fills.

## 2h. The slice (version 0.2.7): queued asks — one crafter, several requesters, each its own share

0.2.6's own gap line (§6) was *"the ask is one `Int` in the crafter's compound … A second requester
asking the same crafter is not queued — the ask is a count, and the stock both requesters watch is the
crafter's single inventory"*. That sentence named the limitation and the shared crafter it made
silent; this slice replaces the count with the queue.

### The artefact's task model, read before any code

Upstream never writes a count into a machine either. `StepRequesterNetworkNode.doWork` calls
`startTask(resource, batch)` on the network's autocrafting component, which creates a **task** in the
network's crafting monitor (`StepTaskImpl`/`StepTaskContainer`, with their own `amount` and
`craftedAmount`), and the monitor runs its tasks one at a time. Two requesters asking the same pattern
therefore get **two tasks**; each task's progress is its own, and a task that cannot start (missing
ingredients) answers empty — that *requester* records `NOT_ENOUGH_INGREDIENTS` and its 20-tick
failed-task timeout, not the network's other tasks.

The re-root keeps that shape in the crafter's saved compound, because the network's task queue is the
one part of the autocrafting surface that is a plain list of (owner, resource, amount):

| saved key | what it is |
|---|---|
| `Requests` | a `List` of `x,y,z\|item\|remaining` — one ask per (requester, item), the work the crafter owes |
| `Served` | a `List` of `x,y,z\|item\|count` — what each requester's asks have produced, its **share** of the stock |
| `Failed` | `x,y,z\|item` — the ask the crafter last could not serve (upstream's failed task, owned by its requester) |

0.2.6's `Request` `Int` is dropped on the next write. It was **operational** state, not configuration:
the asks in it are gone, and each requester sees its ask missing and re-asks on the next tick (unlike
the 0.2.1–0.2.4 filter, which was configuration and is still read as one entry, §2f).

### The rules

- **One ask per (requester, item).** Re-asking merges into the same ask (a larger count raises it), so
  upstream's `isAlreadyRunningTask` is still one guard, now per asker instead of per machine.
- **The queue's order is its identity**: sorted by requester position, then item. Two requesters asking
  in the same tick therefore cannot race for the head, and the same world always serves them in the
  same order. The head is spent one step per tick, and the step's product is credited to **its**
  requester (`Served`), never to whoever happens to be looking.
- **A failed ask keeps its place.** When the machine cannot take the head's step it names that ask in
  `Failed` and leaves it queued: the asks behind it are not lost with it. Only the named requester
  reports the refusal (`phase=not-enough-ingredients` / `network-full`) and holds the 20-tick timeout;
  its ask stays queued, so the restock serves it — the failed task is a timeout, not a dropped ask.
  An ask that *can* never be served (another item because the machine was re-targeted, or a requester
  that is no longer a block) is dropped, and a failure naming it is cleared with it.
- **A requester's `stored` is its own share** of the linked crafter's stock: what its asks produced,
  clamped to the stock that is there (`min(served, stock)`). With one requester it is the whole stock,
  exactly as before; with two, the first reads its four torches and the second reads zero instead of
  the first's torches under another name. `served` is the same number un-clamped (the cumulative
  share), and `queued` is its outstanding ask.
- **The machine writes only a state that moved.** A requester's ask is written into the crafter's
  compound with `merge_data` during that requester's own tick; if the crafter re-wrote the queue it
  read before the ask landed, the ask would vanish. It therefore writes when (and only when) its own
  read state changed, and a lost ask is self-healing anyway (the asker sees it missing and re-asks).

### What changed

- **`runtime.rs`**: `Queued` (one ask), `FailedAsk` (whose ask failed), `Queue` (the asks + the served
  counters + the failure) with `ask`/`outstanding`/`served_for`/`share`/`spend`/`fail`/`retain` and its
  saved forms; `Phase::is_refusal`; `settle` — the entry's phase before `ask` (the refusal while the
  crafter names this entry's ask, and the resumption of a run whose ask was served). `refused` (the
  0.2.1 heuristic "the crafter's phase says so and nothing is outstanding") is gone: the machine now
  names the ask, so the requester no longer has to infer it from a shared count.
- **`machine.rs`**: `State.queue` replaces `State.request`; `Shared` (`target`, `phase`, `queue`) is
  the part another block reads; the tick prunes unservable asks, serves the head, credits it, names a
  refusal, and writes only on change; the readout prints `queued=`, `asks=` and `served=`.
- **`requester.rs`**: `stored` = its share, plus `served`/`queued`; `settle` in place of `refused`;
  `ask_crafter` reads the crafter's queue and merges its ask into the `Requests` list.
- **`commands.rs`**: the requester's line gains `served=`/`queued=` (and the per-entry list gains
  `served|queued`); the machine's line prints the queue and its credits.
- **No new requester configuration**, so no world migrates: its filter container is untouched.

## 2d. The slice (version 0.2.3): crafting remainders

0.2.2's own next-slice line (§10) was *"crafting remainders are not produced (a bucket recipe leaves
its bucket upstream)"*. A step consumed every cell and put only the result in, so a cake cost three
milk buckets and gave none back — where a crafting grid returns one bucket per bucket.

The pin exposes the remainder twice, and the symbol check came before any code:

| symbol | where | verdict |
|---|---|---|
| `Host::item_crafting_remainder(&ItemStack) -> Option<ItemStack>` | `host/block_states.rs:44` (`#[access(Metadata)]`, side both) | **used**: one call per distinct candidate when `/stepcrafter target` resolves the pattern, never in the tick |
| `RecipeMatch.remainders: Vec<Option<ItemStack>>` via `LevelRef::recipe_for` | `item/crafting.rs:230`, `generated/mod_api_generated_host.rs:1678` | exists, but not usable here: `Recipe` (from `recipes_of_type`) carries no grid width/height (`item/crafting.rs:146`), so a `RecipeInput::Crafting` cannot be rebuilt without parsing the file's pattern. The item-level accessor is the same `getRemainingItems` value for a plain stack, and the step picks one candidate per cell, so a per-candidate answer is the precise one |

What changed:

- `Pattern` keeps `remainders` (consumed item → leftover item and count) **in its saved compound**
  (`Remainders`, one `consumed=left:count` per entry), so a restart keeps them; `/stepcrafter target`
  names them in its reply (`remainders=minecraft:milk_bucket=minecraft:bucket:1`).
- `runtime::craft` sums them per leftover item and carries them in `Outcome::Craft { remainders }`;
  `apply` and `machine::apply_craft` insert them after the result.
- The **0.2.2 room rule covers them**: the result and every remainder are simulated in the
  post-consumption slots through `Slots::put`, and a remainder with no room refuses the whole step
  (`NetworkFull`, nothing consumed). `Slots` learned each inserted item's own empty-slot room
  (`set_empty_room`), because a bucket's limit (16) is not a cake's (1).


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

Green at 0.2.1: **30 tests** (in the then-current `sdk-test.log`; 17 of them the 0.2.0 machine, 13 the requester).
The requester's red transcript is `red-unit-requester.log`: its eight rule tests ran against an
`ask()` stub answering `Hold` for every filter, so

```
assertion `left == right` failed: a stock below min must be asked for
  left: Hold
 right: Request { amount: 4 }
...
test result: FAILED. 19 passed; 6 failed
```

— and the two hold cases (`a_requester_holds_at_its_minimum_while_idle`,
`a_zero_batch_filter_is_disabled`) passed against the stub by construction, which is why they are the
control in the green run.

Green at 0.2.2 (the review slice): **34 tests** (the then-current `sdk-test.log`, which 0.2.3 has
since regenerated at 37). Its red transcript is
`red-unit-capacity.log`: the rule had been ported onto the new slot model but `craft` still made the
review's aggregate check, so the two tests that demand per-slot room failed for exactly that reason:

```
---- runtime::tests::a_result_that_fits_no_slot_is_refused_even_when_the_counts_add_up stdout ----

thread 'runtime::tests::a_result_that_fits_no_slot_is_refused_even_when_the_counts_add_up' (63033) panicked at src/runtime.rs:831:9:
assertion `left == right` failed: 1,724 items in 27 full-ish slots leave no room for 4 torches
  left: Craft { consume: [("minecraft:coal", 1), ("minecraft:stick", 1)], result: "minecraft:torch", result_count: 4 }
 right: NetworkFull { item: "minecraft:torch", room: 0, produced: 4 }

---- runtime::tests::a_slot_holding_the_result_offers_only_its_own_free_room stdout ----

thread 'runtime::tests::a_slot_holding_the_result_offers_only_its_own_free_room' (63034) panicked at src/runtime.rs:857:9:
assertion `left == right` failed
  left: Craft { consume: [("minecraft:coal", 1), ("minecraft:stick", 1)], result: "minecraft:torch", result_count: 4 }
 right: NetworkFull { item: "minecraft:torch", room: 1, produced: 4 }

test result: FAILED. 32 passed; 2 failed
```

The `take` fix has no unit-level red: handing stacks to a player needs the game's host, so its red is
the in-game counterfactual `SELFTEST=take` (§4), which asserts the defect (a rebuilt stack reads no
components) and fails on the fixed package.

Green at 0.2.3 (the remainder slice): **37 tests** (`sdk-test.log`). Its red transcript is
`red-unit-remainder.log`: `Pattern` carried the remainders and `Outcome::Craft` had the field, but
`craft` still returned none, so the two new tests failed for exactly that reason:

```
---- runtime::tests::a_step_returns_the_bucket_the_recipe_leaves stdout ----

thread 'runtime::tests::a_step_returns_the_bucket_the_recipe_leaves' (61217) panicked at src/runtime.rs:842:9:
assertion `left == right` failed: the three milk buckets leave three buckets in the machine
  left: Craft { consume: [("minecraft:egg", 1), ("minecraft:milk_bucket", 3), ("minecraft:sugar", 2), ("minecraft:wheat", 3)], result: "minecraft:cake", result_count: 1, remainders: [] }
 right: Craft { consume: [("minecraft:egg", 1), ("minecraft:milk_bucket", 3), ("minecraft:sugar", 2), ("minecraft:wheat", 3)], result: "minecraft:cake", result_count: 1, remainders: [("minecraft:bucket", 3)] }

---- runtime::tests::a_remainder_that_fits_no_slot_refuses_the_step stdout ----

thread 'runtime::tests::a_remainder_that_fits_no_slot_refuses_the_step' (61206) panicked at src/runtime.rs:876:9:
assertion `left == right` failed: no slot has room for the bucket the milk leaves
  left: Craft { consume: [("minecraft:milk_bucket", 1)], result: "minecraft:cake", result_count: 1, remainders: [] }
 right: NetworkFull { item: "minecraft:bucket", room: 0, produced: 1 }

test result: FAILED. 35 passed; 2 failed
```

The remainder's own red is that same pair; its in-game red is the `SELFTEST=remainder` run (§4),
which asserts today's defect (the buckets are gone) and ends non-zero on the fixed package.

Green at 0.2.4 (the component-room slice): **39 tests** (`sdk-test.log`). Its red transcript is
`red-unit-components.log`: `Slot::patched` existed but no rule used its flag yet, so both new tests
failed for exactly the defect (the two-line compile red that named the API first is
`red-unit-components-compile.log`):

```
---- runtime::tests::a_named_stack_of_the_result_is_no_room_for_a_plain_step stdout ----

thread 'runtime::tests::a_named_stack_of_the_result_is_no_room_for_a_plain_step' (78244) panicked at src/runtime.rs:1166:9:
assertion `left == right` failed: a named stack of the result is no room for a plain one
  left: 4
 right: 0

---- runtime::tests::a_plain_result_is_a_new_stack_beside_a_named_one stdout ----

thread 'runtime::tests::a_plain_result_is_a_new_stack_beside_a_named_one' (78246) panicked at src/runtime.rs:1205:9:
assertion `left == right` failed: the named stack keeps its name and count and the plain result gets its own slot
  left: [(64, false)]
 right: [(60, false), (4, true)]

test result: FAILED. 37 passed; 2 failed
```

The game half's red is `e2e-red-components-0.2.3.log`: the **previous** package, run with this
leg's new phase, answers `/stepcrafter step` with `result=crafted` and leaves the named stack at
`minecraft:torch:60` — the four torches exercised the room and were lost. The fixed package reads
`result=network-full`, `crafted=0`, stock unchanged (§4); `SELFTEST=components-room` asserts the
fixed behaviour and ends non-zero.

Green at 0.2.5 (the multi-entry slice): **46 tests** (`sdk-test.log`). Its red transcript is
`red-unit-filters.log`: `Filters::set` was written as the 0.2.4 shape (keep whichever entry was set
last) while the new tests demand the 45-slot container, so five tests failed for exactly that defect:

```
---- runtime::tests::a_second_filter_entry_is_kept_beside_the_first stdout ----

thread 'runtime::tests::a_second_filter_entry_is_kept_beside_the_first' (137363) panicked at src/runtime.rs:1705:9:
assertion `left == right` failed: a second resource gets its own entry, as upstream's next filter slot
  left: Ok(0)
 right: Ok(1)

---- runtime::tests::a_requester_carries_at_most_the_containers_45_entries stdout ----

thread 'runtime::tests::a_requester_carries_at_most_the_containers_45_entries' (137358) panicked at src/runtime.rs:1739:9:
assertion `left == right` failed
  left: 1
 right: 45

---- runtime::tests::a_removed_entry_leaves_the_others stdout ----

thread 'runtime::tests::a_removed_entry_leaves_the_others' (137355) panicked at src/runtime.rs:1756:9:
assertion `left == right` failed
  left: 0
 right: 1

test result: FAILED. 40 passed; 5 failed
```

(The other two reds are the same one-entry defect seen through the container's saved form:
`the_filter_container_round_trips_through_its_saved_form` saved 1 string where 2 entries were set,
and `a_filter_entry_round_trips_through_its_saved_form` caught the decoder accepting a malformed
`link` instead of refusing the entry.) The game half's red is the `SELFTEST=filters` run (§4), which
asserts today's `filters=1` reply and ends non-zero on the fixed package; the running leg's own red
was found in the first run — `minecraft:planks` is a tag, so the stick crafter was never stocked
(§2f's correction) — and is closed by the scenario's stock assertion.

Green at 0.2.6 (the screen slice): **64 tests** (`sdk-test.log`). Its red transcript is
`red-unit-screen.log`: `Editor`, `Press`, `view_data_slots` and `extra_items` existed as stubs (an
`apply` answering `false`, empty slots and an empty payload) while the twelve new tests demanded the
rule, so eleven failed for exactly that reason:

```
---- runtime::tests::a_click_on_an_empty_cell_with_a_carried_item_adds_the_entry stdout ----

thread 'runtime::tests::a_click_on_an_empty_cell_with_a_carried_item_adds_the_entry' (212593) panicked at src/runtime.rs:1997:9:
assertion failed: editor.apply(Press::Cell(0), Some("minecraft:torch"))

---- runtime::tests::a_step_keeps_the_band_valid stdout ----

thread 'runtime::tests::a_step_keeps_the_band_valid' (212622) panicked at src/runtime.rs:2061:9:
assertion failed: editor.apply(Press::MinUp, None)

---- runtime::tests::a_remove_press_drops_the_selected_entry_and_keeps_the_others stdout ----

thread 'runtime::tests::a_remove_press_drops_the_selected_entry_and_keeps_the_others' (212609) panicked at src/runtime.rs:2121:9:
assertion failed: editor.apply(Press::Remove, None)

---- runtime::tests::the_data_slots_are_the_selected_entry stdout ----

thread 'runtime::tests::the_data_slots_are_the_selected_entry' (212638) panicked at src/runtime.rs:2175:9:
assertion `left == right` failed
  left: [0, 0, 0, 0, 0]
 right: [2, 1, 8, 16, 8]

test result: FAILED. 49 passed; 11 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

The game half's red is `SELFTEST=screen` (§4): it asserts today's behaviour — a right click opens no
screen (`ctl screen` says `playing`) and no entry can be made from a menu — and ends non-zero on this
package. The route itself has its own measurement log, `probe-menu.log` (`notes.md` §2g trap 1).

Green at 0.2.7 (the queued-ask slice): **73 tests** (`sdk-test.log`, `Compiling stepcrafter v0.2.7`).
Its red transcript is `red-unit-queue.log`: `Queued`/`Queue`/`FailedAsk`/`settle` existed as stubs
(`ask` answering `false`, `outstanding`/`served_for`/`share` answering `0`, `spend`/`fail` answering
nothing, `settle` returning its input) while the eight new tests demanded the queue, so all eight
failed for exactly that reason:

```
---- runtime::tests::a_second_requester_is_queued_beside_the_first stdout ----

thread 'runtime::tests::a_second_requester_is_queued_beside_the_first' (337174) panicked at src/runtime.rs:2788:9:
the first ask is queued

---- runtime::tests::a_refusal_settles_the_entry_and_a_served_ask_resumes_it stdout ----

thread 'runtime::tests::a_refusal_settles_the_entry_and_a_served_ask_resumes_it' (337161) panicked at src/runtime.rs:2946:9:
assertion `left == right` failed: the failed task times out
  left: (Crafting, 0)
 right: (NotEnoughIngredients, 20)

---- runtime::tests::a_requester_holds_one_ask_per_resource stdout ----

thread 'runtime::tests::a_requester_holds_one_ask_per_resource' (337170) panicked at src/runtime.rs:2860:9:
assertion failed: queue.ask(ASKER_A, "minecraft:torch", 4)

test result: FAILED. 64 passed; 8 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

The one test the slice retired, `a_crafter_that_refused_is_the_requesters_failed_task`, is replaced by
`a_refusal_settles_the_entry_and_a_served_ask_resumes_it`: the machine names the refused ask now, so the
requester no longer infers a refusal from a shared count.

The game half's red is `e2e-red-queue-0.2.6.log`: the **previous** package (0.2.6, its export kept
before the rebuild) run with this leg reads the second requester at
`stored=4 phase=not-enough-ingredients` — the first's torches under its own name, `requests=3` of
asking-and-being-refused, and no queue anywhere — so the leg's own assertion
(`stored=0 served=0 queued=1 phase=running`) fails on it. That is the defect stated as a run, not as a
reading. `SELFTEST=queue` then asserts the fixed behaviour and ends non-zero (§4).

The machine's tests cover They cover the rule (`decide_holds_at_the_minimum_while_idle`,
`decide_crafts_below_the_minimum`, `decide_keeps_crafting_above_the_minimum_until_the_maximum`,
`decide_finishes_at_the_maximum`), the step (`a_step_consumes_every_cell_and_makes_the_result`,
`a_step_prefers_the_first_candidate_the_stock_holds`,
`a_missing_ingredient_refuses_the_step_and_names_the_cell`, `a_full_inventory_refuses_the_step`,
`apply_moves_the_stock_by_exactly_the_outcome`, `a_pattern_round_trips_through_its_saved_form`),
the manual step (`a_manual_step_crafts_even_inside_the_band`,
`a_manual_step_still_needs_the_ingredients`), and the saved compound. The requester's cover its rule
(`a_requester_holds_at_its_minimum_while_idle`, `a_requester_asks_below_its_minimum`,
`a_requester_keeps_asking_above_the_minimum_until_the_maximum`, `a_requester_finishes_at_the_maximum`,
`a_batch_caps_each_request`, `a_request_is_not_repeated_while_one_is_outstanding`,
`a_failed_task_backs_off_before_asking_again`, `a_zero_batch_filter_is_disabled`), the refusal it
reads from the crafter (`a_crafter_that_refused_is_the_requesters_failed_task`), its saved compound
(`the_requester_state_round_trips_through_the_saved_compound`,
`an_empty_compound_reads_as_an_unconfigured_requester`), its readout name
(`the_readout_calls_a_running_ask_running`) and its saved link (`a_saved_position_round_trips_and_foreign_text_is_refused`).

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

`ports/_shared/evidence/stepcrafter-e2e/` — twelve run logs plus the build log, all `--strict` green
(`logs=12 ok=12 missing-exit=0 bad-exit=0`):

| log | verdict | what it is |
|---|---|---|
| `e2e-launcher.log` | `PASS` + `EXIT:0` | the scenario below, on the game the launcher built with the package, plus an in-run control leg on the same Nightly built without it |
| `e2e-control.log` | `FAIL` + `EXIT:1` | the same script with `GAME` = the Nightly **without** the package: `setblock stepcrafter:step_crafter` leaves air |
| `e2e-red-components-0.2.3.log` | `FAIL` + `EXIT:1` | the **previous** package (0.2.3) run with 0.2.4's leg: `/stepcrafter step` answers `result=crafted` beside the named stack and leaves `torch:60` — the silent loss, reproduced |
| `e2e-red-queue-0.2.6.log` | `FAIL` + `EXIT:1` | the **previous** package (0.2.6) run with 0.2.7's leg: the second requester reads `stored=4 phase=not-enough-ingredients` — the first's torches as its own, and no queue anywhere — so the leg's `stored=0 served=0 queued=1 phase=running` fails on it |
| `e2e-selftest.log` | `FAIL` + `EXIT:1` | `SELFTEST=1` inverts the **requester's** headline assertion (`stored=8`) and the run ends non-zero; `SELFTEST=crafter` inverts the machine's (`stored=16`) instead |
| `e2e-selftest-capacity.log` | `FAIL` + `EXIT:1` | `SELFTEST=capacity` inverts the **no-room** assertion: “nothing fit, so nothing was served” (`crafted=0`) ends the run non-zero |
| `e2e-selftest-components-room.log` | `FAIL` + `EXIT:1` | `SELFTEST=components-room` inverts the **component-room** assertion: the named stack counted no room and the step was refused (`phase=network-full crafted=0`) ends the run non-zero |
| `e2e-selftest-take.log` | `FAIL` + `EXIT:1` | `SELFTEST=take` inverts the **component** assertion: the defect “a rebuilt stack reads no components” ends the run non-zero |
| `e2e-selftest-remainder.log` | `FAIL` + `EXIT:1` | `SELFTEST=remainder` inverts the **remainder** assertion: the defect “the step consumes the buckets and leaves none” ends the run non-zero |
| `e2e-selftest-filters.log` | `FAIL` + `EXIT:1` | `SELFTEST=filters` inverts the **multi-entry** assertion: today's defect is one entry (`filters=1`), so the run ends non-zero on the package that keeps `filters=2` |
| `e2e-selftest-screen.log` | `FAIL` + `EXIT:1` | `SELFTEST=screen` inverts the **screen** assertion: today's defect is a right click that opens no screen (`playing`) and a menu that can make no entry, so the run ends non-zero on the package that opens it |
| `e2e-selftest-queue.log` | `FAIL` + `EXIT:1` | `SELFTEST=queue` inverts the **queued-ask** assertion: today's defect is one `Int`, so the second requester's ask is lost and it reads the first's torches; this package serves it its own four (`served=4`), so the run ends non-zero |
| `game-build.log` | `EXIT:0` | the launcher's `native-build` at 0.2.7: `Compiling stepcrafter (1 of 1)` — the bumped manifest forced a real recompile, not a cached package |
| `screen-requester.png` | screenshot | the filter screen on a requester at `223 64 0`: the title, the 9 × 5 cells with the torch's icon, `min 4`, `max 8`, `batch 4` and the remove button |

The scenario, in order (each assertion is parsed from a reply, never "no error"):

1. the block places and reads back `stepcrafter:step_crafter`; the item registers; the unported
   `stepcrafter:step_crafter_manager` is refused by `setblock` and by `give`;
2. **both** carried recipes craft in a real table and their grids are consumed: one Step Crafter
   from 4 iron, 1 redstone, 2 gold, 1 crafter, 1 hopper, and one Step Requester from 4 iron,
   1 comparator, 2 gold, 1 crafter, 1 hopper — the same shape with another centre-top key must be a
   different recipe, not a shadowed one; the same items in another pattern craft nothing
   (`tools/craft_check.py`);
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
   with no ingredients answers `not-enough-ingredients` and records it (`phase=not-enough-ingredients`,
   0.2.4 — a manual step is a step);
10. band 64..128 with 32+32 loaded: `active=true` while it runs, `active=false` once idle, and it
    stops at **128** — past `min` (64), which is the hysteresis;
11. **the requester asks a nearby crafter**: a machine with `target minecraft:torch` and its own
    band `0..0` (so it never fires by itself) with 4 coal + 4 sticks in its container, a requester
    beside it filtered `minecraft:torch 4 8 4` → the requester links it (`crafter=211,64,0`), asks
    (`requests=2`), and 40 ticks later reads `stored=8` while the crafter reads `stored=8
    crafted=8` with exactly 2 coal and 2 sticks consumed;
12. **the differential**: the same world with the requester removed leaves a second machine, loaded
    with the same 4 + 4 and configured the same way, at `stored=0 crafted=0` — the ask is the only
    difference between the two, so the assertion above can fail;
13. the crafter's own refusal reaches it: with the sticks drained the requester reads
    `phase=not-enough-ingredients queued=1` — the machine names the ask it could not serve, only its
    asker reports the refusal, and the ask keeps its place — and the crafter `crafted=0`; the restock
    serves the queued ask and resumes both (`stored=8`, `crafted=8`);
14. a 32-step run (filter `4 128 4`, 32 + 32 loaded) reads `phase=running` while it works,
    `active=true` on the requester, then `active=false` and `stored=128` / `crafted=128` at rest;
15. **the review's room defect (0.2.2)**: a crafter at 216 with `target minecraft:torch`, band `0..0`
    and 26 slots of 64 coal + 60 sticks (`items=1724`, an aggregate budget of 1,728) is asked for
    torches by the requester at 217; after 60 ticks the crafter reads `stored=0 phase=network-full
    crafted=0` with its stock **unchanged** (`coal:1664,stick:60`) and the requester reads
    `stored=0 phase=network-full` — nothing was consumed and no ask was served;
16. **the review's component defect (0.2.2)**: `/data merge block` plants two named torch stacks
    (Alpha ×2, Beta ×3) and one damaged, enchanted sword (Blade, `damage 7`, `sharpness 3`) in the
    machine at 218; `/stepcrafter take … minecraft:torch 5` and `… minecraft:diamond_sword 1` hand
    them over and `ctl inventory` reads two torch stacks with their own names and counts and a sword
    with `components {"custom_name":"Blade","damage":7,"enchantments":{"sharpness":3}}`, `damage 7`
    and `durability_left 1554`;
17. **crafting remainders (0.2.3)**: `/stepcrafter target … minecraft:cake` resolves the cake's
    9-cell recipe and names what it leaves (`remainders=minecraft:milk_bucket=minecraft:bucket:1`);
    with 3 milk buckets, 2 sugar, an egg and 3 wheat loaded and the band at `0..0`, one
    `/stepcrafter step` reads `result=crafted` and the machine then holds
    `stock=minecraft:bucket:3,minecraft:cake:1` (`stored=1 crafted=1 items=4`) — the cake **and** the
    three buckets, with every ingredient consumed. `SELFTEST=remainder` asserts the old behaviour
    (no bucket) and ends the run non-zero;
18. `/stepcrafter requester` refuses an inverted band, an unknown item and a negative batch, and
    `/stepcrafter target` on a requester answers "that is a Step Requester";
19. a survival break with a pickaxe drops the requester's own item, and the machine's break drops
    its item **and spills the 8 torches** (`drops: minecraft:torch:8, stepcrafter:step_crafter:1`);
    placing the machine's item again gives an unconfigured machine (the recorded limit, §6);
20. **the component-room defect (0.2.4)**: `/data merge block` plants a **named** torch stack of 60
    (4 items of nominal room) in slot 0 and the torch recipe's ingredients in every other slot
    (25 coal stacks + a stick; `items=1724`, none empty) in the machine at 221; with `target
    minecraft:torch` and band `0..0`, one `/stepcrafter step` answers `result=network-full` and the
    machine reads `stored=60 phase=network-full crafted=0` with its stock **unchanged**
    (`coal:1600,stick:64,torch:60`). The **differential** at 222 is the same machine with a **plain**
    torch stack of 60: the same step answers `result=crafted` and the stock becomes
    `coal:1599,stick:63,torch:64` (`stored=64 crafted=4`) — only the name differs, so the assertion
    can fail. `SELFTEST=components-room` asserts the fixed refusal and ends the run non-zero (the
    previous package's `result=crafted` is `e2e-red-components-0.2.3.log`);
21. a non-operator's `/stepcrafter info` is refused.
22. **the requester's several filter entries (0.2.5)**: after the component-room leg, one requester
    at `222 64 0` carries `minecraft:torch 4 8 4` and `minecraft:stick 8 16 8` (both added with
    `requester add`, reading `filters=1` then `filters=2`); the torch crafter at `221 64 0` is stocked
    with 4 coal + 4 sticks and the stick crafter at `223 64 0` with 8 `minecraft:oak_planks`. After 80
    ticks the requester reads `filters=2` and
    `entries=minecraft:torch|4|8|4|8|8|0|idle|221,64,0|0;minecraft:stick|8|16|8|16|16|0|idle|223,64,0|0`
    (`item|min|max|batch|stored|served|queued|phase|crafter|backoff` per entry),
    with `crafted=8` on 221 and `crafted=16` on 223 — each entry served by the crafter that maintains
    its item. The **differential**: a second requester at `222 64 4` with only the torch entry (the
    0.2.1–0.2.4 spelling) beside its own fresh pair reads `filters=1`, leaves the torch crafter at
    `crafted=8` and the stick crafter at `crafted=0` — the removed entry is the only difference.
    `SELFTEST=filters` asserts the old one-entry reply and ends the run non-zero.
23. **the requester's filter screen (0.2.6)**: the machine's own stock menu opens on a right click —
    `ctl screen` `container` with **27** slots at 300..326 (it never opened before this version: the
    engine does not open a mod container's menu, §2g) — and so does the requester's screen at
    `223 64 0`: `container`, a 36-slot player inventory and no visible container slot (the one it
    needs is hidden by the layout). The panel is checked from the game: its top is 36 pixels above the
    166-high chest panel's, i.e. the declared `238`. Picking the torch up onto the cursor and
    clicking the first cell makes filter entry 0 (`filters=1 item=minecraft:torch min=0 max=0
    batch=0`); the drawn buttons then step it: at 0..0 a `min +` is **refused** (the band cannot
    invert), eight `max +`, four `min +` and four `batch +` read `min=4 max=8 batch=4` — the brief's
    assertion. A carried stick clicked onto that cell **repoints** it to `minecraft:stick` while
    keeping `min=4 max=8 batch=4`; `/stepcrafter requester add … minecraft:torch 4 8 4` then reads
    `filters=2` (the command's entry is a cell of the same container) and the screen's remove button
    on the second cell brings it back to `filters=1 item=minecraft:stick`. The **differential**: the
    requester at `223 64 4`, never opened, reads `filters=0 item=none`. The screenshot
    `screen-requester.png` is the open screen. `SELFTEST=screen` asserts today's `screen: playing` /
    `filters=0` and ends the run non-zero.
24. **queued asks (0.2.7)**: one crafter at `221 64 0` (`target minecraft:torch`, band `0..0`, **1 coal
    + 1 stick** = exactly one step) and two requesters, `222 64 0` and `222 64 2`, both filtered
    `minecraft:torch 8 8 4`. After 40 ticks the first reads `stored=4 served=4 queued=1`, the second
    `stored=0 served=0 queued=1 phase=running`, and the crafter `stored=4 crafted=4 queued=2
    asks=222,64,0|minecraft:torch|4;222,64,2|minecraft:torch|4 served=222,64,0|minecraft:torch|4`: the
    first ask was served and credited to its own asker, the second is queued behind it and has its own
    share of zero. A restock of two steps then serves the first's kept ask (it reaches its maximum:
    `stored=8 served=8 phase=idle`) and the second's (`stored=4 served=4`), with the crafter at
    `stored=12 crafted=12`. `SELFTEST=queue` asserts the fixed behaviour and ends the run non-zero; the
    previous package run with this leg is `e2e-red-queue-0.2.6.log`.

## 5. Packaging

| item | value |
|---|---|
| file | `ports/stepcrafter/stepcrafter.hmod` |
| size | **84,810 B** |
| sha256 | `968321fd7ef5c64b9825fbe4eb87a1b0c4910512cab89f1d5ebe27f225c24bbc` |
| entries | 39 (`302,942 B` uncompressed), **no `Cargo.lock`** |
| version | `0.2.7` (`horizon-mod.toml` + `Cargo.toml`) |
| build | `cargo horizon-mod build`, twice, `rm -f Cargo.lock` before each → `cmp` identical (`sdk-build-twice.log`) |
| pin | `0a875da  made for` / `0b14f8a  fits` (`sdk-fits-0a875da.log`, `sdk-fits-0b14f8a.log`, each printing the sha) |
| notice | packed `LICENSE` and `LICENSE-NOTICE.md` are `cmp`-identical to the tree's: upstream's MIT text (Starforcraft, 2025) and the notice naming **Ultramega**, **Starforcraft** and `https://github.com/starforcraft/Step-Crafter`; the packed `README.md` is `cmp`-identical to the tree's |
| gate | `python3 ports/tools/verify-sdk-package.py stepcrafter` → **1/1** (`verify-sdk-package.log`; also `package matches the tree`) |

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
- ~~**The room model counts item ids, not components.**~~ **Closed in 0.2.4** (§2e): a patched stack is
  no room for a plain insert (`Slot::patched`), the flag comes from the same
  `ItemStack::is_stackable_with` the insert uses, and `machine::apply_craft` re-runs the whole
  outcome on a copy through the real merge rule and refuses the step if any insert is short. The
  0.2.4 leg proves both sides: the named stack refuses (`network-full`, nothing consumed) and the
  plain twin merges its four torches.
- **Crafting remainders are carried from the *item*, not from a matched grid.** A step's pattern
  keeps what each candidate leaves (`Host::item_crafting_remainder`, one call per candidate at
  `/stepcrafter target`) and inserts it beside the result through the same per-slot room rule
  (0.2.3). `RecipeMatch::remainders` (`LevelRef::recipe_for`) is the same data for a real grid but is
  not reachable here: `Recipe` from `recipes_of_type` carries no grid width/height, so the input grid
  cannot be rebuilt from it. A recipe that overrides `getRemainingItems` with something other than the
  item's own remainder (a custom recipe class) is therefore not expressed; vanilla crafting recipes,
  which is what this pattern resolves, are. A remainder with no room refuses the step rather than
  dropping the leftover on the floor.
- **`direction` state, the GUI, the colours, the managers, the monitor, the slot upgrade, the FE
  buffer** — see §1's table, each with its reason.
- **The requester's reach is 4 blocks, not the network's.** Upstream's requester asks the network's
  autocrafting component from anywhere on the network; this one asks the **nearest** Step Crafter
  inside a 9×9×9 cube whose `Target` is its filtered item (`LINK_RADIUS = 4`, saved and re-validated
  every tick). Two crafters that both maintain the same item are interchangeable to it; a crafter
  that maintains something else is never asked, so a filter cannot spend another pattern's items.
- **The requester carries up to 45 filter entries, edited on its screen or by command** (upstream:
  the same 45 slots plus 8 upgrade slots edited in a screen). A resource names one entry: a cell click
  or `requester add` repoints/replaces the entry for that item rather than adding a second, so two
  bands for one item are not expressible (upstream's slots would allow it; this port's entry identity
  is the resource). `requester add` keeps the command path and writes the same container the screen
  edits. The **8 upgrade slots stay unported** (the RS `UpgradeContainer`), which is this screen's
  one missing column. Batch 0 disables an entry exactly as upstream skips it. A compound written by
  0.2.1–0.2.4 reads as one entry and is migrated to the container on the next write.
- **The screen edits one entry's values at a time.** The 45 cells are drawn (a resource, a band and a
  batch are not an item stack, and a container slot cannot hold them); clicking a cell focuses it and
  a click with a carried item makes or repoints its entry there, as upstream's ghost filter slot does.
  The `min`/`max`/`batch` buttons then step the focused entry by one, refusing a step that would
  invert the band (`min <= max`, the command's own rule) or go below zero, and capping a screen edit
  at 4096. Upstream draws a min/max editor beside *every* slot; this is one focused row, which is the
  recorded deviation, and the entry's numbers are read from `/stepcrafter info` as well.
- **The cells' icons are the menu's open payload.** The item ids travel in `menu_extra_data` (32 KiB)
  and the numbers in the data slots, which are a view and not saved. A resource set *from* the screen
  shows its icon when the menu is reopened (the payload is sent when the menu opens); the numbers
  update at every press.
- ~~**A stale sentence in the crate's module summary.**~~ **Closed in 0.2.7** (§2h): `src/lib.rs`'s
  module doc listed the requester and the GUIs among the families blocked on the parent's surface,
  and `machine`/`requester`'s summaries still described the ask as one `Int`. They now name the
  requester, its screen and the ask queue, and this slice's build and legs are the ones recorded
  below.
- **A panel taller than the game's 240-pixel GUI is unreachable** (§2g trap 3): the requester's is
  176 × 238, which fits. A future row must come out of the layout, not be added below it.
- **The machine's stock menu and the requester's screen both open from the mod's own click handler**
  (§2g trap 1), not from the engine: a click is consumed on both sides, so a held item is not used
  behind the menu. Before 0.2.6 the stock menu did not open at all, though §2 claimed it did.
- **Each entry links its own crafter.** A Step Crafter maintains one target, so two entries with
  different resources cannot link the same crafter; a re-targeted crafter drops the entry's saved link
  and the next tick rescans (the §2b rule), entry by entry.
- **The asks are a list in the crafter's compound** (`Requests`, one `x,y,z|item|remaining` per
  (requester, item); 0.2.6's single `Request` `Int` is dropped on the next write, and its asker
  re-asks), served head first, one step per tick. The queue's order is fixed — requester position,
  then item — so two askers that write in the same tick cannot race for the head. A requester holds
  **one** ask per resource (re-asking raises its count), which is upstream's `isAlreadyRunningTask`
  per asker instead of per machine.
- **A share is a share.** `Served` (`x,y,z|item|count`) credits a step's product to the ask it served,
  and a requester's `stored` is `min(its served, the stock that is there)`. With one requester that is
  the whole stock; with two, each reads its own torches. The stock itself is still the crafter's one
  27-slot inventory (no network), so a player emptying it lowers every share with it.
- **A refusal names one ask.** The machine records the ask it could not serve in `Failed` and keeps it
  queued; only that asker reports `not-enough-ingredients`/`network-full` and holds the 20-tick
  timeout, and the restock serves the kept ask. An ask that can never be served is dropped: another
  item (the machine was re-targeted) or a requester block that is no longer there (a requester whose
  chunk is merely unloaded keeps its place). A machine with no pattern leaves its asks waiting rather
  than refusing them — an unconfigured machine is not a failed task.
- **A requester can lose its credit by moving.** `Served` lives in the crafter's compound, because the
  crafter is what crafts; a re-linked requester reads its share of the crafter it now asks (0 for a
  crafter that never served it), not a lifetime total.
- **Upstream's `FINISHED` status is not saved as a phase**: reaching `max` returns the requester to
  `idle`, which is the state a player reads. `running` is upstream's `CRAFTING`; `not-enough-ingredients`
  and `network-full` are the crafter's refusals, reported with the stock unchanged.
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
| one step = resolve every cell, take, insert | `StepCraftingAlgorithm.prepareCraft/extractResolvedInputs/insertIntoNetwork` | `runtime::craft` (candidate resolution, reservation by counting), `machine::apply_craft` |
| what the ingredients leave | `getRemainingItems` on the pattern's recipe | `Pattern.remainders` (per candidate, from `Host::item_crafting_remainder` at configuration), carried in `Outcome::Craft`, inserted beside the result under the same room rule |
| `NETWORK_FULL` | `canInsertIntoNetwork` before extracting | `runtime::craft`'s room check **after** the ingredients are taken: the free room in the **plain** slots that hold the result plus one empty slot's worth per empty slot (the result's stack limit) — the stock's item count is not the room, and a stack carrying components is no room for a plain insert. `machine::apply_craft` then re-runs the whole outcome on a copy through the real merge rule and commits only if every insert went in, so a counted result cannot be lost |
| the manual step (`/stepcrafter step`) | a click on a step-crafting request | `runtime::step_now` + `machine::step_once`; band bypassed, and the refusal is saved as `Phase` exactly as a tick-driven attempt saves it |
| `NOT_ENOUGH_INGREDIENTS` | `prepareCraft` returns null | `Outcome::NotEnoughIngredients` naming the short cell |
| status per pattern slot | `ResourceStatus` (`CRAFTING`, `FINISHED`, `NOT_ENOUGH_INGREDIENTS`, `NETWORK_FULL`) | `runtime::Phase`, saved as `Phase` |
| `active` block state | the RS network node's `active` property | true while the machine is crafting |
| one step per tick without speed upgrades | `for j < speed + 1` with `speed = 0` | `runtime::STEPS_PER_TICK = 1` |
| the click | `getMenuProvider`/`ExtendedMenuProvider` opens the block's menu on use | the mod asks for its own menu on `right_click_block` → `PlayerRef::open_menu` (the engine does not open a mod container's menu at this pin, §2g); the stock opens `minecraft:generic_9x3`, the requester `stepcrafter:step_requester_filters` |
| the requester's filter screen | `StepRequesterScreen`: 45 filter slots with a resource, a min/max editor and a batch each, plus the 8 upgrade slots | a 176 × 238 panel: 45 drawn cells (the entries' icons from the menu's open payload), the focused entry's `min`/`max`/`batch` from the data slots, `-`/`+` buttons per value and a `remove` button; a cell click with a carried item makes or repoints that entry (upstream's ghost slot). The 8 upgrade slots are not ported |
| a screen press | a slot click or a side button in the mod's screen | `screen_click` (the client's hit test) → the menu's button id → `menu_button` on the server (`runtime::Editor::apply`), which writes the same saved container the command writes |
| taking the resource out | `network.extract(resource, count)` | `/stepcrafter take` hands over the **stored stacks themselves**, split to the count asked for: names, enchantments and damage stay on them |
| the requester's rule (start below min, ask to max, `min(needed, batch)` per task) | `StepRequesterNetworkNode.doWork`, five guards | `runtime::ask`, `requester::tick_entry` |
| several filter entries, each with its own status and failed-task timeout | the `rf` container (45 `ResourceMinMaxAmount`) and the node's per-index `runningTasks`/`failedTaskTimeouts` | `runtime::Filters`/`Entry` (add-or-replace by resource, 45 cap, saved list), one `tick_entry` per entry; the readout's `filters=N entries=…` |
| "a task is already running" | `isAlreadyRunningTask` on the task id map | the ask this requester already holds for that resource (`Queue::outstanding`) — other askers' asks are theirs |
| a task that cannot start | `startTask` answers empty: that requester's `NOT_ENOUGH_INGREDIENTS` + 20 ticks | the machine names the ask in `Failed` and keeps it queued; only its asker reports the refusal and backs off, the restock serves it |
| the queued tasks' progress | `StepTaskImpl`/`StepTaskContainer` (`amount`, `craftedAmount`) per task | `Queue`'s head ask + `Served` per (requester, item) |
| a failed task (20-tick timeout) | `failedTaskTimeouts`, `FAILED_TASK_TIMEOUT_TICKS` | `runtime::refused` + `State::backoff` (`FAILED_TASK_TIMEOUT_TICKS = 20`) |
| the store the requester watches | `StorageNetworkComponent.get(resource)` | the linked crafter's own 27 slots (`requester::stock_count`), of which the requester's `stored` is its own share (`Queue::share`) |
| `startTask(resource, batch)` | the autocrafting component creates a task in the crafting monitor | one ask (`x,y,z\|item\|remaining`) merged into the crafter's `Requests` list (`merge_data`); the step's product is credited to it in `Served` |
| the requester's status per filter slot | `ResourceStatus` again | `Phase`, printed as `idle` / `running` / `not-enough-ingredients` / `network-full` |
| the requester's `active` | the network node's property | true while it is asking |

## 8. Upstream defects fixed / adaptations

None claimed: nothing in the jar's data contradicted its code. The recipe re-rooting in §6 is a
**port adaptation** (the parent's items are not representable in an isolated install), recorded as
a deviation, not as an upstream defect.

## 9. Files

| file | what it is |
|---|---|
| `src/runtime.rs` | the pure rules: `Band`, `Phase`, `Step`, `decide`, `Stock`, `Slots`/`Slot` (per-slot room, per-item empty-slot room), `Pattern` (cells, remainders, saved form), `plan`, `step_now`, `craft`, `apply`, and the requester's `Filter`, `Entry`, `Filters` (the 45-entry container), `Ask`, `ask`, `settle`, and the crafter's `Queued`/`FailedAsk`/`Queue` (the ask list, its credits and its failed ask, with their saved forms) — no `mod_api` types |
| `src/pattern.rs` | the target item's crafting recipe resolved into a `Pattern` (one host crossing, at configuration), including what each candidate leaves (`item_crafting_remainder`) |
| `src/machine.rs` | the crafter's block entity: saved compound (pattern, remainders, band, phase, the ask `Queue` with its `Served` credits and `Failed` ask), the 27-slot inventory as a stock, `Shared` (the part a requester reads), the tick that prunes, serves and credits the head ask, the manual step, `apply_craft`, and `right_click` (the mod's own menu open for both blocks) |
| `src/requester.rs` | the requester's block entity: its filter entries (the container's saved `Filters` list, with the 0.2.4 single-entry keys read as a fallback), each entry's link to a nearby crafter, its ask merged into that crafter's queue, the per-entry tick (`settle` → `ask`) and the readout's views (its own `stored`/`served`/`queued`), the selected cell, the menu's data slots and open payload, and `press` (the server half of a screen press) |
| `src/screen.rs` | the filter screen: the 176 × 238 layout, `cell_at`/`button_at` hit tests and the drawing (cells, icons, the selected entry's values, the buttons) |
| `src/nbt.rs` | the saved-compound vocabulary both block entities share (`put`, `string`, `int`, `list`, …) |
| `src/commands.rs` | `/stepcrafter info|target|configure|requester|step|take` |
| `src/lib.rs` | the one `impl Hooks` and the two `Part`s the game ticks |
| `tools/e2e.sh`, `tools/craft_check.py` | the in-game leg of §4 |
| `LICENSE` | upstream's MIT text (`starforcraft/Step-Crafter`) |
| `assets/`, `data/` | the carried content: two blocks (states `active`), their models/textures, loot, recipes, tag, lang, creative tab |

Textures are upstream's own (MIT) — `step_crafter/{top,side_active,side_inactive}.png` and
`step_requester/{active,inactive}.png`, byte-copied from the jar; the crafter's two models are
re-authored for this package (a cube with those three textures and a vanilla bottom, since
upstream's model names `refinedstorage:block/bottom`, which this install does not have), and the
requester's two are upstream's own `cube_all` shape unchanged.

## 10. Coverage (family → evidence, per the parity rule)

| family | row |
|---|---|
| the band rule: start below `min`, fill to `max`, hold at `max` | **tested** — unit `decide_holds_at_the_minimum_while_idle` / `decide_crafts_below_the_minimum` / `decide_keeps_crafting_above_the_minimum_until_the_maximum` / `decide_finishes_at_the_maximum`; in game `stored=16 crafted=16` then 60 idle ticks unchanged, and the 64..128 band filling to 128 |
| one step: every cell out, the result in | **tested** — `a_step_consumes_every_cell_and_makes_the_result`, `a_step_prefers_the_first_candidate_the_stock_holds`, `apply_moves_the_stock_by_exactly_the_outcome`; in game 4 coal + 4 sticks → 16 torches, `stock=minecraft:coal:4,minecraft:stick:4,minecraft:torch:16` |
| `not-enough-ingredients` | **tested** — `a_missing_ingredient_refuses_the_step_and_names_the_cell`; in game `phase=not-enough-ingredients` at `crafted=16`, then the restock resumes to `crafted=32` |
| `network-full` | **tested** — `a_full_inventory_refuses_the_step`, `a_result_that_fits_no_slot_is_refused_even_when_the_counts_add_up`, `a_slot_holding_the_result_offers_only_its_own_free_room`, `consuming_the_last_item_of_a_slot_frees_that_slot_for_the_result`; in game a 26 × 64 coal + 60 sticks machine refuses every ask with nothing consumed or crafted (`SELFTEST=capacity` inverts it) |
| the manual step | **tested** — `a_manual_step_crafts_even_inside_the_band`, `a_manual_step_still_needs_the_ingredients`; in game `result=crafted` then `result=not-enough-ingredients` |
| the pattern from the item's recipe | **tested** in game — `target=minecraft:torch cells=2 result=minecraft:torch:4`; refusals for a recipe-less item and an unknown id |
| the container (the stock) | **tested** in game — `ctl container` loads 8+8 and the readout agrees; empty for a fresh machine |
| the recipes (both) | **tested** in game — `tools/craft_check.py`: one Step Crafter and one Step Requester, five ingredients consumed each, another pattern crafts nothing |
| the blocks and the items (both) | **tested** in game — placed, given, read back; `stepcrafter:step_crafter_manager` refused by `setblock` and `give` |
| the commands | **tested** in game — every reply parsed, incl. `min 9 is above max 4`, the requester's three refusals, the wrong-block refusal and the operator refusal |
| `/stepcrafter take` (the stored stacks themselves) | **tested** in game — two named torch stacks (Alpha ×2, Beta ×3) and a damaged, enchanted sword come out with both names, both counts, `damage 7` and `sharpness 3` (`durability_left 1554`); `SELFTEST=take` asserts the defect (no components) and the run ends non-zero |
| the slots' room for the result (per-slot limits, post-consumption) | **tested** — `a_result_that_fits_no_slot_…`, `a_slot_holding_the_result_offers_only_its_own_free_room`, `consuming_the_last_item_of_a_slot_frees_that_slot_for_the_result`, `an_empty_slot_offers_each_inserted_item_its_own_room`; in game the 1,724-item machine (`items=1724` of 1,728) refuses |
| the room for a **plain** insert beside a stack with components | **tested** — `a_named_stack_of_the_result_is_no_room_for_a_plain_step` (room 0, `NetworkFull`), `a_plain_result_is_a_new_stack_beside_a_named_one` (the named stack stays 60, the result is its own plain stack); in game the 60-named-torch machine refuses with `phase=network-full crafted=0` and the stock unchanged, while the plain-stack twin merges its four torches (`stock=coal:1599,stick:63,torch:64`); `SELFTEST=components-room` inverts the fixed behaviour and the 0.2.3 package's `result=crafted` is `e2e-red-components-0.2.3.log` |
| the `active` states (both) | **tested** in game — `execute if block …[active=true]` → `Test passed` while filling, `[active=false]` once idle, on the machine and on the requester |
| loot, stock spill, `mineable/pickaxe` | **tested** in game — survival breaks with a pickaxe drop `stepcrafter:step_crafter:1` + `minecraft:torch:8` and `stepcrafter:step_requester:1`; a bare hand drops no block |
| the requester's rule: ask below `min`, batch to `max`, never twice at once, 20-tick backoff after a refusal | **tested** — `a_requester_asks_below_its_minimum` / `a_requester_keeps_asking_above_the_minimum_until_the_maximum` / `a_requester_finishes_at_the_maximum` / `a_batch_caps_each_request` / `a_request_is_not_repeated_while_one_is_outstanding` / `a_failed_task_backs_off_before_asking_again` / `a_zero_batch_filter_is_disabled` / `a_crafter_that_refused_is_the_requesters_failed_task`; in game the 4..8 run asked twice (`requests=2`) and stopped at `stored=8`, and the 4..128 run read `phase=running` at `stored=44` |
| **the interop: the requester asks a nearby Step Crafter** | **tested** in game — the pair reaches `stored=8` / `crafted=8` with 2 coal and 2 sticks consumed, and the *same world with the requester removed* leaves an identically configured machine at `stored=0 crafted=0` |
| **several filter entries, each served by the crafter that maintains its item** | **tested** — `a_second_filter_entry_is_kept_beside_the_first`, `a_filter_entry_is_replaced_by_its_own_resource`, `a_requester_carries_at_most_the_containers_45_entries`, `a_removed_entry_leaves_the_others`, `a_filter_entry_round_trips_through_its_saved_form`, `the_filter_container_round_trips_through_its_saved_form`, `a_legacy_single_filter_compound_still_reads_as_one_entry`; in game one requester with a torch entry and a stick entry reads `filters=2` with both entries and their stored counts, `crafted=8` on the torch crafter and `crafted=16` on the stick crafter, while the same world with only the torch entry leaves the stick crafter at `crafted=0`; `SELFTEST=filters` ends non-zero |
| the refusal path across the two blocks | **tested** in game — the crafter with no sticks answers `not-enough-ingredients`, the requester whose ask it names shows `phase=not-enough-ingredients queued=1` (the ask keeps its place), the restock serves it and resumes both to `stored=8 crafted=8` |
| **queued asks: two requesters on one crafter, each its own share** | **tested** — `a_second_requester_is_queued_beside_the_first`, `the_queue_serves_in_a_fixed_order_whatever_order_it_was_written`, `a_step_is_credited_to_the_ask_it_served`, `a_requester_holds_one_ask_per_resource`, `a_failed_ask_keeps_its_place_and_names_its_requester`, `the_queue_round_trips_through_its_saved_form`, `a_refusal_settles_the_entry_and_a_served_ask_resumes_it`, `the_queue_drops_asks_that_can_never_be_served`, with the machine's `the_shared_read_is_the_queue_and_the_phase` and `a_legacy_single_ask_is_not_a_queued_ask`; in game: after 40 ticks the first reads `stored=4 served=4`, the second `stored=0 served=0 queued=1 phase=running`, the crafter `queued=2 asks=222,64,0\|minecraft:torch\|4;222,64,2\|minecraft:torch\|4`; a restock of two steps serves both (`served=8` / `served=4`, `crafted=12`); `SELFTEST=queue` inverts it and ends non-zero, and the 0.2.6 export fails the same leg (`e2e-red-queue-0.2.6.log`) |
| the requester's own loot | **tested** in game — a survival break with a pickaxe drops `stepcrafter:step_requester:1` |
| **the requester's filter screen** (open, cells, values, remove) | **tested** — `a_press_round_trips_through_its_button_id`, `a_click_on_an_empty_cell_with_a_carried_item_adds_the_entry`, `a_click_on_a_filled_cell_selects_it`, `a_click_with_another_item_repoints_the_entry_and_keeps_its_band`, `a_click_that_would_give_two_entries_one_resource_is_refused`, `a_step_keeps_the_band_valid`, `a_step_never_goes_below_zero_and_a_zero_batch_stays_disabled`, `a_batch_step_enables_an_entry`, `a_remove_press_drops_the_selected_entry_and_keeps_the_others`, `a_cell_beyond_the_next_free_one_changes_nothing`, `a_step_on_an_empty_requester_changes_nothing`, `the_data_slots_are_the_selected_entry`, `the_screen_payload_lists_every_entrys_resource`, `the_editor_and_the_command_are_one_container`, `the_grid_is_nine_by_five_and_the_cells_do_not_overlap`, `the_buttons_are_the_presses_the_server_applies`, `the_panel_holds_its_rows_and_the_inventory` (compile-time), and `the_menu_declares_the_slots_the_screen_publishes`; in game the screen opens on a right click, a carried torch makes entry 0, the buttons set `min=4 max=8 batch=4`, a carried stick repoints the cell, the command's entry is a cell the remove button drops, and an unopened requester has none; `SELFTEST=screen` ends non-zero |
| the screen's panel and its origin | **tested** in game — the panel's top is 36 above the 166-high chest panel's (the declared 238), its left is the centred 176, and `slot-0` (the hotbar's first) is where the layout puts it |
| the machine's stock menu | **tested** in game — a right click opens `container` with 27 slots at 300..326 |
| the 8 upgrade slots | `blocked(no-parent-addon-surface)` — the RS `UpgradeContainer`, which the parent port does not ship |
| the crafter/requester managers, the crafting monitor, the 48 colour variants | **not ported** — `blocked(no-parent-addon-surface)`: mixins into Refined Storage's grid screen |
| the slot upgrade, the FE buffer, the GUI, `direction`, advancements, JEI/EMI | **not ported** — upgrades/energy/GUI are the RS harness; `direction` is cosmetic here |
| crafting remainders (a bucket recipe leaves its bucket) | **tested** — `a_step_returns_the_bucket_the_recipe_leaves` (3 milk buckets → cake + 3 buckets), `a_remainder_that_fits_no_slot_refuses_the_step`, `a_pattern_round_trips_through_its_saved_form` (the saved `Remainders`); in game a cake step reads `stock=minecraft:bucket:3,minecraft:cake:1` with every ingredient consumed, and `SELFTEST=remainder` asserts the old defect (no bucket) and ends non-zero |
