# Nova Weapons — example mod

Retired from the release (October 2026): kept as a reference example, never
loaded by the game nor packaged (`rewrite/mods/README.md`).

Three weapons written from scratch, a complete example of the `front[]` / `rear[]`
power levels. The `mod.json` is commented; read it alongside MODDING.md (§5).

| Weapon | Bay | Shows |
|------|------|--------|
| Nova Pulse Array | Front | 3 levels, spread through `speed_x`, original sprites referenced by name |
| Ion Piercer | Front | Piercing shot (`damage` 250+), mod PNG registered under its file name |
| Quark Seeker | Rear | Rear-only weapon, homing shot (`homing` > 5), smoke trail (`trail` 98) |

No entry carries a number: the engine assigns the slots, so this
mod cannot conflict with any other.
