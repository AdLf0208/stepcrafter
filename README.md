# Step Crafter for Horizon

A port of [Step Crafter](https://github.com/starforcraft/Step-Crafter) (MIT, Ultramega), a Refined
Storage add-on, to Horizon's native mod SDK (`api = "0.2"`, game 26.3).

The **Step Crafter** keeps a resource in stock: it is told **what** to keep (`/stepcrafter target`)
and **between which amounts** (`/stepcrafter configure <min> <max>`), resolves the item's own
crafting recipe into a step, and crafts one step at a time out of the ingredients in its own 27-slot
inventory — starting when the stock falls below the minimum and filling up to the maximum, as
upstream's `StepCrafterNetworkNode.doWork` does. Right-click the block to open its inventory.

The **Step Requester** keeps a filtered item in stock *through* the Step Crafter beside it: when that
crafter's stock of the filter falls below the requester's minimum, it asks the crafter for batches of
the item until the maximum is reached (upstream's `StepRequesterNetworkNode.doWork`), and reports a
refusal the crafter cannot serve. Upstream's version asks a Refined Storage autocrafting network;
this one asks the nearest Step Crafter within 4 blocks that maintains the same item.

```
/stepcrafter target <pos> <item>        the item a Step Crafter maintains (its crafting recipe becomes the step)
/stepcrafter configure <pos> <min> <max>  the machine's band: start below min, fill to max
/stepcrafter requester <pos> <item> <min> <max> <batch>
                                        the requester's filter: what to keep, its band, and the most
                                        one request asks for (0 disables the entry)
/stepcrafter info <pos>                  either block's whole state, one line
/stepcrafter step <pos>                  take one step now
/stepcrafter take <pos> <item> <count>   take the target back
```

**Licence:** MIT (upstream's `LICENSE` travels inside the package). **Pin:** Horizon
`0a875da91dbb0fdcb7ba8c42c11341aa229af51a` + mixins `e59da9094f0c2fdda6c627fe9eff22dec6a906f0`.

**Scope:** the Step Crafter machine and the Step Requester. The Step Crafter/Requester Managers, the
Step Crafting Monitor, their sixteen colour variants, the slot upgrade, the energy buffer and the
GUIs need Refined Storage's grid screens and upgrade/energy harness, which neither the parent port
nor the pin ships; they are recorded as `blocked(no-parent-addon-surface)` in `notes.md`.
