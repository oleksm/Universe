# Painted nozzles, a forge glow, two-tone hulls

- **Nozzles painted:** heat-tinted metal, bronze at the throat to blued steel at the lip, with a glint.
- **The forge glow:** a lit drive glows blue in its bells, white-hot at the heart, as bright as
  the drive's set (it flickers a little).
- **The plume:** a short blue cone of light fading and narrowing away from the bell, instead of
  long line spikes (exhaust is faint in vacuum). The thrusters' cold gas is pale.
- **Two-tone hulls:** wings and fins (flat parts) take a second tone per livery, for contrast
  with the body. Ours: graphite wings on a light body.
- `showship` takes `UNIVERSE_BURN` (mains held).

## Follow-up

- **The forge glow in the chase view:** glows made in the front layer (`Frame::in_front`) are now
  drawn with it (`front_glows`). Before, our own hull, drawn over everything, hid its bells' glow.
- The `manual` scenario takes `UNIVERSE_BURN` (mains held).
- **Small thrusters puff, don't glow:** lift and attitude thrusters give a white, dusty spray of cold gas,
  specks drifting out, widening and fading. Only the main drive has the blue forge glow and plume.
