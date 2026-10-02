# The engine as a kernel, the world as a distro

- **Two sheets.**
  - `config/physics.ron` is the kernel's laws only: nature's constants, the medium, the field's
    draw, the gate throat. It generates `universe_physics::laws`.
  - The world's fixed design numbers moved to `content/base/sheet.ron`: drive exhaust, field
    efficiency, ring sizes and classes, capacitor technology, the heat model's ship numbers. It
    generates `universe_world::sheet`.
  - One generator for both (`crates/physics/build/sheetgen.rs`).
- **The hyper layer's laws live in the kernel** (`universe_physics::hyper`: `slack`, `field_draw`,
  `field_speed`, `throat_upkeep`, `transit_energy`). The world's hyperdrive is a device built on them.
- **Materials are the world's:** `content/base/materials.ron`.
  - Deuterium, helium-3, the D–He3 and D–T blends, enriched uranium, methalox, kerolox, hydrolox and
    hydrogen, at real densities and energies.
  - A **tank says what it holds**, a **plant says what it burns and how well**. Content checks a tank
    against its material's density and that a plant burns something that burns.
  - A ship holds one fuel; its reactor's fuel burn comes from the material's energy and its plants'
    efficiency.
- **The code names no fuel:** ships' traded fuel is whatever the starting hull's tanks hold.
- **Dogma checks:** the kernel's laws name no material or device. The report shows the laws, the
  world's numbers and its materials.
- `docs/architecture.md`: the kernel/distro split. `docs/physics.md` points at it.
