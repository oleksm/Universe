# Art direction

Freefall is leaving the retro vector look. The aim now: **modern vector
graphics with good taste, leaning toward photoreality** — not there yet, but
every step goes that way. Space should feel vast, dark, lit by real light,
and the things in it solid, made and used.

## Pillars

1. **Light tells the truth.** One key light, the star; fill from what it
   lights (planetshine); deep shade where nothing reaches. Everything a
   viewer sees lit is lit for a reason (the sun, a lamp, a hot nozzle).
2. **Solid, not drawn.** Surfaces are filled and shaded; shapes read by
   their lighting and silhouette, not by outlines. Lines are detail on a
   surface, never its definition.
3. **Restraint.** Few colours, used with intent. Detail where it explains
   something (a hatch, a light, a seam), none for noise. Effects serve
   function: a plume shows thrust, a strobe shows heading, dust shows drift.
4. **Scale and quiet.** Space is mostly black. Contrast is high, the sky
   sparse. Small bright things (lights, stars, engines) against big dark ones.
5. **Readable first.** Immersion never costs the pilot what they need to
   know: silhouettes, lights and the HUD keep ships, ports and dangers clear.

## Rendering

- **Full resolution, antialiased.** No chunky low-res buffer, no pixel look.
- **HDR light, tone-mapped** (filmic): a lit hull and the sun live in one
  range; the eye adapts (exposure) slowly between sun and shade.
- **No contour outlines.** The green wireframe edges go. Edges survive only
  as *panel lines*: a few percent darker or lighter than the surface, thin,
  fading with distance.
- **Materials, coarse:** hull metal (matte or brushed, neutral greys and
  off-whites), dark composites, glass (dark, reflective), emissive (windows,
  lights, nozzles). A small specular highlight on metal where the sun glints.
- **Shadows** cast by everything near; soft-edged; planets eclipse.
- **Bloom on emissives only** (engines, lights, the sun): never a haze over
  the whole frame.
- **The sky:** near-black with the faintest blue, stars in their real colours
  and brightnesses, the galaxy's band faint. No grid lines in flight.

## Colour

- **World:** neutral hulls; one accent per livery (by role or maker), muted.
  Engines warm (amber to white-hot). Navigation lights red (port), green
  (starboard), white (strobe).
- **Stars and planets:** physical colours, never tinted for style.
- **UI:** its own palette, apart from the world's: white and soft cyan for
  information, amber for attention, red for danger, green only as a lamp
  meaning "go" (cleared, on). No green text, no green lines.

## Interface

- A clean typeface (vector, crisp at any size) instead of the 8-pixel font;
  hierarchy by size and weight, not by colour alone.
- Panels: dark, translucent, hairline borders, generous margins; headers on
  every table.
- The HUD over the world is sparse and light: thin lines, small type,
  nothing drawn without a job.

## Geometry and detail

- Hulls with chamfers and form, not slabs; detail (hatches, vents, seams,
  antennas) where it means something; lights on every ship.
- Stations and gates designed as built things: structure, lights, function
  you can read (where to land, which way the gate goes).
- Planets with atmospheres (rim glow, a warm terminator), clouds, night-side
  lights where people live; asteroids with craters and rubble.

## Not doing

- Outlined wireframes, scanlines, CRT effects, neon.
- Full-screen colour tints or haze; lens dirt; chromatic aberration.
- Decoration without function; saturated "sci-fi" colours for their own sake.

## Order of work

1. **Render core:** full resolution with antialiasing, HDR and tone mapping,
   no contour outlines (panel lines only), neutral hull materials, a clean sky.
2. **Interface:** the typeface, the UI palette, panels and HUD restyled.
3. **Lights and motion:** navigation lights, lit windows, bloom, space dust,
   thruster puffs.
4. **Bodies:** atmospheres, clouds, night lights, the sun's face and corona.
5. **Stations and gates** redesigned; **ship detail** and liveries.
