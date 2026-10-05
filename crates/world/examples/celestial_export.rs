//! Writes what the seed makes of the settled systems (home and the stars its
//! gates reach) as JSON on stdout, for the celestial registry
//! (`standards/Celestial`, written out by `tools/standards/celestial_export.py`).
//! Run: `cargo run -q -p universe-world --example celestial_export [seed]`.

use universe_world::system::BodyKind;
use universe_world::World;

fn s(t: &str) -> String {
    format!("\"{}\"", t.replace('\\', "\\\\").replace('"', "\\\""))
}

fn main() {
    let seed: u64 = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(1984);
    let w = World::new(seed);
    let home = w.home_system;
    let mut systems = vec![home];
    for &(a, b) in &w.gate_links {
        for i in [a, b] {
            if !systems.contains(&i) {
                systems.push(i);
            }
        }
    }
    let name_of = |i: usize| w.system(i).name.clone();
    let mut out = vec![format!("{{\"seed\": {seed}, \"home\": {}, \"systems\": [", s(&name_of(home)))];
    for (n, &i) in systems.iter().enumerate() {
        let sys = w.system(i);
        let star = &w.galaxy.stars[i];
        let off = star.position - w.galaxy.stars[home].position;      // (a star's position is in light years already)
        let mut positions = Vec::new();
        sys.positions(0.0, &mut positions);
        out.push(format!(
            "  {{\"name\": {}, \"index\": {i}, \"class\": {}, \"mass_suns\": {}, \"luminosity_suns\": {}, \"from_home_ly\": [{:.3}, {:.3}, {:.3}],",
            s(&sys.name), s(&sys.class.letter().to_string()), star.mass_suns(), sys.luminosity, off.x, off.y, off.z
        ));
        out.push("   \"bodies\": [".into());
        let mut rows = Vec::new();
        for (bi, b) in sys.bodies.iter().enumerate() {
            let r = &b.rail;
            let mut f = vec![format!("\"name\": {}", s(&b.name)), format!("\"kind\": {}", s(b.kind.label())), format!("\"mass\": {:e}", b.mass), format!("\"radius\": {}", r.radius)];
            if let Some(p) = r.parent {
                f.push(format!("\"parent\": {}", s(&sys.bodies[p].name)));
            }
            if let Some(o) = &r.orbit {
                let incl = o.normal().y.clamp(-1.0, 1.0).acos().to_degrees();
                f.push(format!("\"orbit\": {{\"semi_major_axis\": {}, \"eccentricity\": {}, \"period\": {}, \"inclination\": {:.4}}}", o.semi_major_axis, o.eccentricity, o.period(), incl));
            }
            if !b.kind.artificial() && b.kind != BodyKind::Star {
                f.push(format!("\"gravity\": {:.4}", r.mu / (r.radius * r.radius)));
            }
            f.push(format!("\"day\": {}", r.day));
            let axis = r.tilt * glam::DVec3::Y;
            f.push(format!("\"tilt\": {:.4}", axis.y.clamp(-1.0, 1.0).acos().to_degrees()));
            f.push(format!("\"colour\": [{:.3}, {:.3}, {:.3}]", b.color[0], b.color[1], b.color[2]));
            if let Some((a, c)) = b.rings {
                f.push(format!("\"rings\": [{a}, {c}]"));
            }
            if let Some(t) = &b.terrain {
                f.push(format!("\"terrain\": {}", s(&format!("{:?}", t.kind).to_lowercase())));
                f.push(format!("\"relief\": {}", t.amplitude));
            }
            if let Some(a) = &r.atmosphere {
                f.push(format!("\"atmosphere\": {{\"surface_density\": {}, \"scale_height\": {}, \"top\": {}}}", a.surface_density, a.scale_height, a.top));
            }
            if b.kind.is_planet() || b.kind == BodyKind::Moon {
                f.push(format!("\"albedo\": {}", universe_world::climate::albedo(&sys, bi)));
                f.push(format!("\"mean_temperature\": {:.1}", universe_world::climate::mean_temperature(&sys, bi, &positions)));
            }
            if let Some(to) = b.link {
                f.push(format!("\"gate_to\": {}", s(&name_of(to))));
                f.push(format!("\"gate_distance_ly\": {:.3}", w.galaxy.stars[i].position.distance(w.galaxy.stars[to].position)));
                f.push(format!("\"ring\": {}", s(&universe_world::hypernet::gate_ring(&w.galaxy, &sys, bi).key)));
            }
            if let Some(rock) = &b.rock {
                f.push(format!("\"rock\": {{\"class\": {}, \"structure\": {}, \"density\": {}}}", s(rock.class.label()), s(rock.structure.label()), rock.density));
            }
            rows.push(format!("    {{{}}}", f.join(", ")));
        }
        out.push(rows.join(",\n"));
        out.push("   ],\n   \"ports\": [".into());
        let ports: Vec<String> = sys.spaceports.iter().map(|p| {
            let d = p.direction;
            format!("    {{\"name\": {}, \"body\": {}, \"latitude\": {:.3}, \"longitude\": {:.3}}}", s(&p.name), s(&sys.bodies[p.body].name), d.y.clamp(-1.0, 1.0).asin().to_degrees(), (-d.z).atan2(d.x).to_degrees())
        }).collect();
        out.push(ports.join(",\n"));
        out.push("   ],\n   \"fields\": [".into());
        let fields: Vec<String> = sys.fields.iter().map(|fl| {
            format!("    {{\"name\": {}, \"kind\": {}, \"anchor\": {}, \"extent\": {}, \"count\": {}, \"class\": {}}}", s(&fl.name), s(&format!("{:?}", fl.kind)), s(&sys.bodies[fl.body].name), fl.extent, fl.count, s(fl.class(&sys).label()))
        }).collect();
        out.push(fields.join(",\n"));
        out.push(format!("   ]\n  }}{}", if n + 1 < systems.len() { "," } else { "" }));
    }
    out.push("]}".into());
    println!("{}", out.join("\n"));
}
