# Sounds from inside the ship

What's heard is what carries through the hull and the cabin air (there's no sound in space).
The synth is still procedural (no sample files, nothing to license), now driven by what the
simulation does.

- **Stereo, in a small metal room** (a Schroeder reverb, darker with each echo).
- **Thrusters:**
  - each jet hisses through its own band while it fires: a valve's knock as it opens, a short
    "pssht" as it closes (12 ms attack, 50 ms release);
  - panned to the side the jet sits on, brighter and louder nearer the cockpit;
  - lift jets bigger and lower;
  - every jet's real level each frame, so manual flight sounds like what you fire.
- **The main drive:** a deep rumble through the frame with a sub and a slow throb, from the
  main nozzles' real levels.
- **Life support:** the plant's hum and the fans' air, while the ship is powered and you're
  aboard. Powering up: relays clicking, then a rising whine; down, a falling one.
- **Struck metal:**
  - a crack, the plating ringing in a plate's inharmonic modes (damped by its frame and
    lining, not a bell), and a thud through the frame;
  - hits panned to the side the shooter is on;
  - a round that breaches: the air venting, debris rattling;
  - collisions and rock strikes by speed; a crash adds a blast's boom and crackle.
- **More:**
  - the gun's kick through the frame;
  - the gear's thud and hydraulics on landing;
  - the deck clamps letting go on launch;
  - the hatch's seals hissing and the door thudding home;
  - footsteps on the deck plating aboard, softer outside;
  - the fuel hose; the hull being mended;
  - the vending machine's works and the can dropping;
  - the gate's swell and rush.
- **The computer's chirps:** soft sine tones instead of square-wave bleeps.
- Test: every sound rendered offline (`Audio::offline`, `render`) is heard and none clips.
  `UNIVERSE_SOUND_WAVS=dir` writes each as a WAV to listen to.
