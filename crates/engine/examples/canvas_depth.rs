//! GPU regression fixture: intersecting triangles in opposite submission orders,
//! UI overlay, and a second camera whose farther geometry needs fresh depth.
use universe_engine::{glam::Vec2, Camera, Color, Config, Context, Frame, Game};
struct Fixture;
impl Game for Fixture {
    fn update(&mut self, _: &mut Context) {}
    fn camera(&self) -> Camera { Camera::default() }
    fn draw(&self, f: &mut Frame, _: &Context) {
        let size = f.size();
        f.hud_rect(Vec2::ZERO, size, Color::BLACK);
        for (offset, reverse) in [(0., false), (0.5, true)] {
            f.canvas_scene();
            let p = [[0.05+offset, 0.2], [0.45+offset, 0.2], [0.25+offset, 0.85]].map(|p| Vec2::from(p)*size);
            let triangles = [([2.,8.,8.], Color::rgb(1.,0.,0.)), ([8.,2.,2.], Color::rgb(0.,1.,0.))];
            for i in if reverse {[1,0]} else {[0,1]} {
                f.canvas_triangle(p, triangles[i].0, 0.1, [triangles[i].1;3]);
            }
            f.hud_rect(Vec2::new(0.24+offset,0.45)*size, Vec2::new(0.02,0.04)*size, Color::WHITE);
        }
        let p = [[0.8,0.8],[0.95,0.8],[0.9,0.95]].map(|p|Vec2::from(p)*size);
        f.canvas_triangle(p,[1.;3],0.1,[Color::rgb(1.,0.,0.);3]);
        f.canvas_scene();
        f.canvas_triangle(p,[100.;3],0.1,[Color::rgb(0.,0.,1.);3]);
    }
}
fn main() { universe_engine::run(Config { title: "Studio depth regression".into(), ..Default::default() }, Fixture); }
