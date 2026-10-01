# 2026-09-30 — Fight or flight, SAM aim, smooth guide, fainter sun rays

User: "guiding grid lines are kind of jerky, need it fixed. Make sol rays even more transparent
barely visible. Seems like sams are not hitting me. Also settlers should judge fight or flight and
sometimes try to collectively shoot the agressor. player should be in same rules as anyone else"

## SAMs weren't hitting you

- Turrets led their shots as if the target coasted. A ship burning its engine at 30 m/s² drifts
  about 60 m off over a 2 s slug flight, and its hull radius is 12 m. With the old aim, a player
  burning at full throttle 5.5 km out took no hits at all.
- Turret fire control now keeps a track on each aggressor, estimating its acceleration from how
  its velocity changes (`TurretTrack`). It leads on that acceleration less gravity, since gravity
  pulls the round the same way.
- New test: `turrets_shoot_down_an_aggressed_player_burning_hard_at_the_edge_of_their_reach`.

## Two aiming bugs that hit everyone

- **Gravity counted twice in the lead.** Fire control's track measures a target's total
  acceleration, gravity included, and the lead used all of it. The round falls the same way, so
  near a planet (10.8 m/s² in the test) the aim was off by about a hull's size. Gravity at the
  target is now taken off, in the hunter and in the player's fire control.
- **Ships saw others one frame ahead.**
  - The player steps first, then each craft in turn, every one from the same moment.
  - So a craft saw the ships stepped before it at the end of the frame, and itself at the start.
  - Everything moves at ~50 km/s with its orbit round the star, so that was ~835 m of error:
    hunters aimed at where the target would be.
  - Pirates only hit when they happened to be stepped before their prey.
  - **Fix:** every ship is snapshotted at the start of the frame (`Universe::snaps`), and radar
    pictures (`sightings`) and follow marks (`ship_mark`) come from that snapshot.

## Fight or flight

- **Judging an aggressor.** A lawful ship with an aggressor within 12 km judges it
  (`hunter::judge`):
  - **its side:** its own hull plus the hulls of other lawful ships within 10 km of the aggressor
    (they'll be judging the same);
  - **the aggressor's side:** its own hull plus any pirates or other aggressors near it;
  - **its nerve:** a temperament between 0.6 and 1.4, fixed per pair of ships, times 0.7 when
    carrying cargo;
  - **the rule:** it fights if its side times its nerve is at least twice the aggressor's side,
    and its hull is at least 50%.

  So a lone ship leaves an aggressor alone, and a handful nearby gang up on it.
- **The fight.** It drops its route and attacks with the hunter's code. Shooting the aggressed
  is no crime, and it doesn't fear the turrets.
  - It breaks off when the aggressor is no longer fair game, lands, jumps, gets 30 km away, or
    after 5 minutes.
  - Below 35% hull it breaks off and runs for the guns.
  - Afterwards it goes back to its route if it was flying one.
- **Standing down** (pirates and defenders alike), it first slows to the local traffic: the
  target's last known motion, falling as everything does since. It also keeps clear of the
  others. Before this, two defenders charging from opposite sides coasted into each other after
  the kill.
- **Separation is predictive.** A ship steers off the line of its closest approach to another
  within the next 10 s, not only once it's inside 400 m.
- **You are a ship like any other.** You're in everyone's radar picture now:
  - pirates hunt you;
  - aggressed, you're judged by every settler near you.
- Stats: `defences` and `aggressors_downed`, shown on the HUD traffic line as `POSSES n/m`.
- Tests:
  - `a_lone_settler_leaves_an_aggressor_alone`;
  - `settlers_gang_up_on_an_aggressor_and_shoot_it_down` (and no collisions after);
  - `an_aggressed_player_is_judged_like_anyone`;
  - `pirates_hunt_the_player_too`.

## Guide frames

- Flying by hand, each rebuild of the plan (up to 10 a second) starts from where you've drifted
  to. So the guide frames jumped about 8 m at every rebuild (measured; with the autopilot flying,
  0.1 m).
- The display now eases from the previous plan to the new one over the time until the next
  rebuild, at the same moments of absolute time, so the frames glide instead of stepping.

## Sun rays

- Opacity 0.35 → 0.12: barely there.
