//! GPU fixture: explicit alpha modes, cross-object ordering, UI and camera scopes.
use universe_engine::{glam::Vec2, Camera, CanvasAlpha, Color, Config, Context, Frame, Game};
struct Fixture;
impl Game for Fixture {
    fn update(&mut self, _: &mut Context) {}
    fn camera(&self) -> Camera { Camera::default() }
    fn draw(&self, f: &mut Frame, _: &Context) {
        let size = f.size();
        for panel in 0..3 {
            f.canvas_scene();
            let x = panel as f32 / 3.;
            let p = [[x+0.02,0.1],[x+0.31,0.1],[x+0.16,0.9]].map(|p|Vec2::from(p)*size);
            let red = Color([1.,0.,0.,0.5]);
            let green = Color([0.,1.,0.,0.5]);
            if panel == 0 {
                f.canvas_triangle_alpha(p,[2.;3],0.1,[Color([1.,0.,0.,0.01]);3],CanvasAlpha::Opaque);
            } else if panel == 1 {
                f.canvas_triangle_alpha(p,[2.;3],0.1,[Color([0.,1.,0.,0.]);3],CanvasAlpha::Mask(0.5));
                let q=p.map(|v|v+Vec2::new(0.,size.y*0.15));
                f.canvas_triangle_alpha(q,[2.;3],0.1,[green;3],CanvasAlpha::Mask(0.5));
            } else {
                // Near first, far later, UI between: transparent depth must not
                // suppress the far layer or the still-later opaque background.
                f.canvas_triangle_alpha(p,[2.;3],0.1,[red;3],CanvasAlpha::Blend);
                f.hud_rect(Vec2::new(x+0.14,0.4)*size,Vec2::new(0.04,0.04)*size,Color::WHITE);
                f.canvas_triangle_alpha(p,[3.;3],0.1,[green;3],CanvasAlpha::Blend);
            }
            f.canvas_triangle_alpha(p,[8.;3],0.1,[Color::rgb(0.,0.,1.);3],CanvasAlpha::Opaque);
        }
        // New camera clears earlier depth but preserves its color outside this triangle.
        f.canvas_scene();
        let p=[[0.02,0.75],[0.12,0.75],[0.07,0.9]].map(|p|Vec2::from(p)*size);
        f.canvas_triangle_alpha(p,[100.;3],0.1,[Color::WHITE;3],CanvasAlpha::Opaque);
    }
}
fn main(){universe_engine::run(Config{title:"Canvas alpha regression".into(),..Default::default()},Fixture);}
