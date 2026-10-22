//! The planet view: a world's globe, its full maps near the eye, its ground as patches and the lines
//! on it, its air, drawn for any system in view (`docs/miniapps-plan.md` step 4). It holds its own
//! caches and reads only what `Sight` gives it, so the game and a planet app draw worlds the same way.

use std::sync::Arc;
use universe_engine::glam::DVec3;
use universe_engine::{Camera, Color, Frame, Transform};
use universe_sim::{BodyKind, StarSystem};

use crate::planet_studio::{Look, WorldFrame};
use crate::terrain_view;

/// A globe this big on screen (radius, px) gets its full mesh.
pub const GLOBE_FULL_PX: f32 = 90.0;

/// What's built for drawing worlds and kept.
#[derive(Default)]
pub struct PlanetView {
    /// The ground near worlds, as patches (see `terrain_lod`); made while drawing.
    pub terrain_lod: std::cell::RefCell<crate::terrain_lod::Lod>,
    /// Ready globe leases for the current view. Missing entries draw base geometry.
    pub globes: std::collections::HashMap<(usize, usize), Arc<crate::globe_assets::Globe>>,
    /// The lines laid on the ground round the eye, near a world drawn from its lines (see `lines_near`).
    pub lines_near: std::cell::RefCell<crate::lines_near::Near>,
    /// Worlds grown by the planet simulation: their full-resolution maps, made in the
    /// background (None till ready), by (system, body).
    pub world_maps: std::collections::HashMap<
        (usize, usize),
        Arc<std::sync::Mutex<Option<Arc<universe_engine::WorldMaps>>>>,
    >,
    /// The world near the eye whose ground is a land package, drawn by the planet LOD renderer.
    pub land: std::cell::RefCell<Option<crate::land_view::LandView>>,
}

/// What the view is drawn from this frame.
pub struct Sight<'a> {
    pub system: &'a Arc<StarSystem>,
    /// What this system's globes are kept under (see `View::cache`).
    pub cache: usize,
    /// Body positions now, in the frame drawn.
    pub positions: &'a [DVec3],
    /// World time (s).
    pub t: f64,
    pub show_grid: bool,
    /// The eye is on foot outside (the ground's patches finer underfoot).
    pub outside: bool,
    /// The body the eye stands on, in this system (its grid underfoot).
    pub underfoot: Option<usize>,
}

impl PlanetView {
    /// Terrain globes for a system in view (built once, on first sight), and the full maps of
    /// a baked world near the eye. `cache`: what this system's globes are kept under.
    pub fn prepare(
        &mut self,
        system: &Arc<StarSystem>,
        cache: usize,
        positions: &[DVec3],
        camera: &Camera,
        look: Look,
        then: Option<WorldFrame>,
    ) {
        let origin = cache;
        // Drop inactive leases; the shared scheduler retains recently used data for return.
        self.globes.retain(|&(o, _), _| o == origin);
        self.world_maps.retain(|&(o, _), _| o == origin);
        crate::globe_assets::maintain();
        crate::land_view::LandView::maintain(self.land.get_mut(), system, cache, positions, camera.position);
        for (i, b) in system.bodies.iter().enumerate() {
            let position = positions.get(i).copied().unwrap_or_default();
            if let Some(globe) = crate::globe_assets::request(system.clone(), i, &camera, position)
            {
                self.globes.insert((origin, i), globe);
            } else {
                self.globes.remove(&(origin, i));
            }
            // A world with a bake: its full-resolution maps, read and encoded on a thread of their
            // own once the eye comes within `crate::WORLD_MAPS_NEAR` of it, let go past `crate::WORLD_MAPS_FAR`
            // (a few hundred megabytes each while held).
            let away = positions
                .get(i)
                .map_or(f64::INFINITY, |c| c.distance(camera.position))
                / b.rail.radius;
            if away > crate::WORLD_MAPS_FAR {
                self.world_maps.remove(&(origin, i));
            }
            if let Some(t) = b.terrain.as_ref().filter(|t| t.baked())
                && away < crate::WORLD_MAPS_NEAR
                && self.globes.contains_key(&(origin, i))
                && !self.world_maps.contains_key(&(origin, i))
            {
                let slot = std::sync::Arc::new(std::sync::Mutex::new(None));
                self.world_maps.insert((origin, i), slot.clone());
                let (t, look) = (t.clone(), look);
                // (A frame of its history in place of today's colour: its sea and clouds then
                // aren't today's, so none.)
                let then = then.clone().filter(|f| f.key == b.key);
                let key = b.key.clone();
                let lines = self
                    .globes
                    .get(&(origin, i))
                    .is_some_and(|g| g.lines.is_some());
                std::thread::spawn(move || {
                    let started = std::time::Instant::now();
                    // (Each read and encoded before the next is read: one image held at a time.)
                    let names = [
                        look.file(),
                        "globe_ground.jpg",
                        "globe_normal.jpg",
                        "climate.png",
                        "rockid.png",
                        "globe_spec.png",
                    ];
                    let mut k = 0;
                    let encoded = names.map(|name| {
                        let image = match &then {
                            Some(f) if k == 0 => f.history.image(f.frame),
                            Some(_) if name == "globe_spec.png" => None,
                            // (A world drawn from its lines: their flat ground in place of its colour, and
                            // none of its bake's other maps: ground, normals, climate, rock, shine.)
                            None if lines => {
                                if k == 0 {
                                    universe_sim::world::worlds::lines_colour_rgba(&key)
                                } else {
                                    None
                                }
                            }
                            _ => t.bake_image(name),
                        };
                        let e = image.and_then(|(w, h, rgba)| {
                            universe_engine::WorldMaps::encode(
                                k,
                                universe_engine::pbr::Image {
                                    width: w as u32,
                                    height: h as u32,
                                    rgba,
                                },
                            )
                        });
                        k += 1;
                        e
                    });
                    let maps = universe_engine::WorldMaps::encoded(encoded, t.bake_air())
                        .with_air_luts(
                            t.bake_air_luts().map(|l| [l.transmittance, l.multiscatter]),
                        );
                    let mut maps = maps;
                    if let Some(c) = t.bake_clouds().filter(|_| then.is_none()) {
                        let img =
                            |(w, h, rgba): (usize, usize, Vec<u8>)| universe_engine::pbr::Image {
                                width: w as u32,
                                height: h as u32,
                                rgba,
                            };
                        let [a, b, c3] = c.maps;
                        maps.clouds_year = Some((c.year_days, c.enso));
                        maps.clouds_format = c.format;
                        maps = maps.with_clouds(Some([img(a), img(b), img(c3)]));
                    }
                    log::info!(
                        "world maps read and encoded in {:.1} s",
                        started.elapsed().as_secs_f64()
                    );
                    if let Ok(mut s) = slot.lock() {
                        *s = Some(std::sync::Arc::new(maps));
                    }
                });
            }
        }
    }

    /// The world nearest the eye with its full maps ready: drawn with them (and its clouds now).
    pub fn draw_maps(&self, frame: &mut Frame, s: &Sight) {
        let sys = s.system;
        let t = s.t;
        let cam = frame.camera.position;
        // The world nearest the eye with its full-resolution maps ready (within a few radii): drawn with them.
        let near = self
            .world_maps
            .iter()
            .filter(|((o, _), _)| *o == s.cache)
            .filter_map(|((_, i), m)| Some((*i, m.lock().ok()?.clone()?)))
            .filter(|(i, _)| s.positions[*i].distance(cam) < sys.bodies[*i].rail.radius * 6.0)
            .min_by(|a, b| {
                s.positions[a.0]
                    .distance(cam)
                    .total_cmp(&s.positions[b.0].distance(cam))
            });
        if let Some((i, maps)) = near
            && let Some(globe) = self.globes.get(&(s.cache, i))
        {
            if crate::devenv::var_os("UNIVERSE_NO_WORLD_MAPS").is_none() {
                frame.keep_alive(globe.clone());
                frame.world_maps(&maps, &globe.map, s.positions[i], sys.bodies[i].rail.radius);
                // Its clouds now: the year's phase and El Niño's index at world time (one epoch for
                // every player), the time wrapped as the clouds' shader wants it.
                if let Some((year_days, enso)) = &maps.clouds_year {
                    let (month, index) =
                        universe_sim::world::worlds::clouds_at(t, *year_days, enso.as_ref());
                    frame.world_clouds(
                        sys.bodies[i].rotation(t).as_quat().as_dquat(),
                        month,
                        index,
                        t.rem_euclid(universe_engine::shaders::CLOUD_WRAP_S) as f32,
                    );
                }
            }
        }
    }

    /// World `i`'s globe or ground, when its globe is ready (false: not ready, draw it plain).
    pub fn draw_world(
        &self,
        frame: &mut Frame,
        s: &Sight,
        i: usize,
        center: DVec3,
        px: f32,
        c: Color,
    ) -> bool {
        let b = &s.system.bodies[i];
        let (t, cam) = (s.t, frame.camera.position);
        let rotation = b.rotation(t).as_quat();
        // A world whose ground is a land package: the terrain subsystem's ground, in the world's air (casting no
        // shadow maps, as the planet LOD app draws it).
        if let Some(v) = self.land.borrow_mut().as_mut().filter(|v| v.at == (s.cache, i)) {
            let (depth, shell) = terrain_view::air(b).unwrap_or_default();
            let mut drawn = false;
            universe_prof::time("draw/scene/bodies/land", || frame.no_shadow(|frame| frame.with_air(depth, shell, |frame| drawn = v.draw(frame, center, b.rotation(t), |d| b.terrain.as_ref().map_or(0.0, |t| t.surface(d))))));
            if drawn {
                return true;
            }
        }
        let land = b.terrain.as_ref().is_some_and(|t| t.land().is_some());
        let Some(ready) = self.globes.get(&(s.cache, i)) else {
            return false;
        };
        frame.keep_alive(ready.clone());
        let (full, coarse, map) = (&ready.full, &ready.coarse, &ready.map);
        // Small on screen: the coarse mesh does (a sixteenth of the triangles).
        let globe = if px > GLOBE_FULL_PX { full } else { coarse };
        // Terrain world: from afar its globe; near, its ground as patches
        // finer toward the eye (see `terrain_lod`).
        // (A land package's world is never drawn as patches: its globe till its renderer is open.)
        let near = !land && cam.distance(center) - b.rail.radius < terrain_view::near_altitude(b);
        let patches_ready = near
            && universe_prof::time("draw/scene/bodies/ground", || {
                self.terrain_lod.borrow_mut().draw(
                    frame,
                    s.cache,
                    &s.system,
                    i,
                    map,
                    center,
                    b.rotation(t),
                    cam,
                    c,
                    t,
                    s.outside,
                )
            });
        if patches_ready {
            // A world drawn from its lines: the lines laid on the ground round the eye.
            if let Some((_, lines)) = ready.lines.as_ref() {
                universe_prof::time("draw/scene/bodies/lines near", || {
                    crate::lines_near::draw(
                        &mut self.lines_near.borrow_mut(),
                        frame,
                        (s.cache, i),
                        &s.system,
                        lines,
                        center,
                        b.rotation(t),
                        cam,
                    );
                });
            }
        } else {
            let relief = b.terrain.as_ref().map_or(0.0, |t| t.amplitude) as f32;
            universe_prof::time("draw/scene/bodies/globe mesh", || {
                let (depth, shell) = terrain_view::air(b).unwrap_or_default();
                frame.no_shadow(|frame| {
                    frame.with_air(depth, shell, |frame| {
                        frame.with_globe(
                            map,
                            terrain_view::globe_kind(b),
                            relief,
                            terrain_view::globe_bright(b),
                            [0.0, 0.0, 0.0, 1.0],
                            DVec3::ZERO,
                            |frame| {
                                frame.model_shaded_faded(
                                    globe,
                                    &Transform {
                                        position: center,
                                        rotation,
                                        scale: b.rail.radius,
                                    },
                                    c,
                                    c,
                                    if s.show_grid { grid_detail(px) } else { 0.0 },
                                );
                            },
                        )
                    })
                });
                // A world drawn from its vector lines: the lines over it, in their own colours.
                if let Some((lines, _)) = ready.lines.as_ref() {
                    frame.no_shadow(|frame| {
                        frame.model_colored(
                            lines,
                            &Transform {
                                position: center,
                                rotation,
                                scale: b.rail.radius,
                            },
                            1.0,
                            0.0,
                        )
                    });
                }
            });
        }
        // The grid on the ground (F4), and underfoot on foot.
        if near && s.show_grid {
            universe_prof::time("draw/scene/bodies/surface grid", || {
                terrain_view::surface_grid(frame, b, center, t, None, true)
            });
            if s.underfoot == Some(i) {
                terrain_view::surface_grid(frame, b, center, t, Some(4.0), true);
            }
        }
        if frame.projected_radius(center, b.rail.radius) > 150.0 {
            universe_prof::time("draw/scene/bodies/crater rims", || {
                terrain_view::crater_rims(frame, b, center, t)
            });
        }
        true
    }

    pub fn atmosphere(&self, frame: &mut Frame, s: &Sight, i: usize, center: DVec3) {
        let b = &s.system.bodies[i];
        let cam = frame.camera.position;
        let to = center - cam;
        let d = to.length();
        let r = b.rail.radius;
        if d <= r * 1.02 {
            return;
        }
        let Some(star) = s
            .system
            .bodies
            .iter()
            .position(|b| b.kind == BodyKind::Star)
        else {
            return;
        };
        let sun = (s.positions[star] - center).normalize();
        let dir = to / d;
        // The limb: the circle where the line of sight grazes the surface.
        let limb_c = center - dir * (r * r / d);
        let limb_r = r * (1.0 - (r * r) / (d * d)).sqrt();
        let thick = 1.0 + 0.035_f64.max(100_000.0 / r);
        let (u, v) = (
            dir.any_orthonormal_vector(),
            dir.cross(dir.any_orthonormal_vector()),
        );
        let sky = terrain_view::sky_color(b);
        let n = 96;
        let point = |k: usize, s: f64| {
            let a = k as f64 / n as f64 * std::f64::consts::TAU;
            let out = u * a.cos() + v * a.sin();
            (limb_c + out * limb_r * s, out)
        };
        let light = |out: DVec3| {
            // Lit by how far round to the sun this edge is; warm at the terminator.
            let day = out.dot(sun);
            let lit = ((day + 0.25) / 1.25).clamp(0.0, 1.0) as f32;
            let warm = (1.0 - (day.abs() / 0.3)).clamp(0.0, 1.0) as f32;
            let k = 2.4 * lit;
            [
                k * (sky[0] + 0.8 * warm),
                k * (sky[1] + 0.25 * warm),
                k * sky[2] * (1.0 - 0.5 * warm),
            ]
        };
        for k in 0..n {
            let ((a0, o0), (a1, o1)) = (point(k, 1.0), point(k + 1, 1.0));
            let ((b0, _), (b1, _)) = (point(k, thick), point(k + 1, thick));
            let (l0, l1) = (light(o0), light(o1));
            frame.glow_triangle([a0, a1, b1], [l0, l1, [0.0; 3]]);
            frame.glow_triangle([a0, b1, b0], [l0, [0.0; 3], [0.0; 3]]);
        }
    }
}

/// How much of a body's latitude/longitude grid to show at `px` pixels of
/// radius on screen: none when small (a lit disc and its outline circle are
/// enough), fading in as it grows, and never more than a light touch (the
/// shading carries the shape).
pub fn grid_detail(px: f32) -> f32 {
    0.5 * ((px - 80.0) / 420.0).clamp(0.0, 1.0)
}
