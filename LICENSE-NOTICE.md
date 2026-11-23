# Provenance and attribution — Step Crafter for Horizon

- **Code:** a clean-room Rust rewrite of
  [Step Crafter](https://github.com/starforcraft/Step-Crafter) by **Ultramega**
  (`starforcraft/Step-Crafter`), read at commit `963cd494187ea762b4d78cecd55a45f3cb479812`
  (2026-11-20) and from the ATM10 jar `stepcrafter-neoforge-1.21.1-0.1.10.jar`
  (`META-INF/neoforge.mods.toml`: `license = "MIT"`, `authors = "Ultramega"`). No Java code was
  translated and no jar was decompiled.
- **Licence:** MIT. **Ultramega**'s notice — the MIT licence text of the mod, whose copyright holder is
  **Starforcraft** — is the package's `LICENSE`, byte-identical to upstream's `LICENSE.txt`
  (`Copyright (c) 2025 Starforcraft`). The mod's home is
  <https://github.com/starforcraft/Step-Crafter>; `README.md` and `horizon-mod.toml` name both the
  author and the upstream repository.
- **Textures carried from the jar (MIT):** `assets/stepcrafter/textures/block/step_crafter/`
  `top.png`, `side_active.png`, `side_inactive.png` — byte-identical to the jar's own, whose art is
  the mod's (MIT covers it; the mod is not an `assets-arr` row).
- **Re-authored for this package:** the two block models and the item model. Upstream's models
  name `refinedstorage:block/bottom`, a texture this install does not have; the port's models use
  the same three carried textures on a vanilla cube and a vanilla bottom face.
- **Adapted data:** `data/stepcrafter/recipe/step_crafter.json` keeps upstream's `ECE/AMA/EDE`
  shape and re-roots its five keys on vanilla items, because upstream's keys are Refined Storage
  items (`notes.md` §6).
