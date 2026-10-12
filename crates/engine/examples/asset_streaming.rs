//! End-to-end asynchronous loading and warm GPU reuse fixture.
//! ASSET_FIXTURE_GLB names a model; standard UNIVERSE_SCREENSHOT controls capture.
use std::{sync::Arc, time::Duration};
use universe_engine::{
    Camera, Config, Context, Frame, Game, PbrModel, Transform,
    assets::{AssetCache, Demand, Loaded},
    glam::{DVec3, Vec3},
};
struct Fixture {
    cache: AssetCache<PbrModel>,
    model: Option<Arc<PbrModel>>,
    camera: Camera,
    path: String,
    frame: u64,
}
impl Game for Fixture {
    fn update(&mut self, ctx: &mut Context) {
        self.frame += 1;
        let path = self.path.clone();
        self.model = self.cache.request(
            path.clone(),
            Demand {
                estimated_bytes: 128 * 1024 * 1024,
                ..Default::default()
            },
            move || {
                let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
                let model = PbrModel::load_gltf(&bytes)?;
                let n = model
                    .data
                    .images
                    .iter()
                    .map(|i| i.rgba.len())
                    .sum::<usize>()
                    + model
                        .data
                        .primitives
                        .iter()
                        .map(|p| {
                            p.vertices.len()
                                * std::mem::size_of::<universe_engine::pbr::PbrVertex>()
                                + p.indices.len() * 4
                        })
                        .sum::<usize>();
                Ok(Loaded {
                    value: model,
                    bytes: n,
                })
            },
        );
        if self.frame == 200 {
            let cpu = self.cache.snapshot();
            assert_eq!(cpu.loads, 1);
            assert!(cpu.hits > 0);
            assert_eq!(
                ctx.perf.resources.asset_uploads, 1,
                "warm return must not upload again"
            );
            println!("ASSET_FIXTURE cpu={cpu:?} gpu={:?}", ctx.perf.resources);
        }
    }
    fn camera(&self) -> Camera {
        self.camera
    }
    fn draw(&self, f: &mut Frame, _: &Context) {
        // Brief absence exercises retention without rebuilding the immutable model.
        if !(100..110).contains(&self.frame) {
            if let Some(m) = &self.model {
                f.model_pbr(&m.with_lease(m.clone()), &Transform::default());
            }
        }
    }
}
fn main() {
    let path = std::env::var("ASSET_FIXTURE_GLB").expect("ASSET_FIXTURE_GLB");
    let mut camera = Camera {
        position: DVec3::new(2., 3., 4.),
        ..Default::default()
    };
    camera.look_at(DVec3::ZERO, Vec3::Y);
    universe_engine::run(
        Config {
            title: "Asset streaming regression".into(),
            ..Default::default()
        },
        Fixture {
            cache: AssetCache::new(universe_engine::assets::Config {
                idle_for: Duration::from_secs(60),
                ..Default::default()
            }),
            model: None,
            camera,
            path,
            frame: 0,
        },
    );
}
