//! The passengers panel (docked, the passengers key): the passage booked
//! from here (where to, how many, what each pays, what the seats free would
//! earn), and landing those aboard if this is where they're bound. ↑/↓
//! pick, ENTER board (or land), the key or ESC close.

use universe_engine::glam::Vec2;
use universe_engine::{Color, Context, Frame, KeyCode};
use universe_sim::Command;

use crate::App;

const TEXT: Color = Color::hex(0x40ff70);
const DIM: Color = Color::hex(0x208838);
const SELECT: Color = Color::hex(0xffc040);

/// Its rows: landing those aboard (if they're bound here), then each booking.
fn rows(app: &App) -> usize {
    usize::from(lands_here(app)) + app.v.bookings.len()
}

fn lands_here(app: &App) -> bool {
    let s = &app.v.ship;
    s.passengers > 0 && app.v.docked_market.is_some_and(|m| s.bound_for == Some((app.v.ship_system, m)))
}

/// Keys while open. False when it should close.
pub fn input(app: &mut App, ctx: &Context) -> bool {
    let input = &ctx.input;
    if crate::keys::pressed(input, crate::keys::Act::Passengers) || input.pressed(KeyCode::Escape) || app.v.docked_market.is_none() {
        return false;
    }
    let n = rows(app).max(1);
    let Some(pick) = app.passengers.as_mut() else { return false };
    if input.pressed(KeyCode::ArrowDown) {
        *pick = (*pick + 1) % n;
    } else if input.pressed(KeyCode::ArrowUp) {
        *pick = (*pick + n - 1) % n;
    }
    let pick = (*pick).min(n - 1);
    if input.pressed(KeyCode::Enter) {
        let land = lands_here(app);
        if land && pick == 0 {
            app.engine.send(Command::Passengers(None));
        } else if let Some(b) = app.v.bookings.get(pick - usize::from(land)) {
            app.engine.send(Command::Passengers(Some((b.system, b.to))));
        }
    }
    true
}

pub fn draw(frame: &mut Frame, app: &App, pick: usize) {
    let s = &app.v.ship;
    let spec = s.spec();
    let free = s.passenger_room();
    let mut lines: Vec<(String, Color)> = Vec::new();
    let here = app.v.docked_market.map(|m| m.name(&app.view.system).to_uppercase()).unwrap_or_default();
    lines.push((format!("PASSENGERS - {here}"), TEXT));
    let aboard = match s.bound_for {
        Some((system, to)) if s.passengers > 0 => format!("{} ABOARD, BOUND FOR {} ({:.0} CR EACH)   ", s.passengers, to.name(&app.charts.system(system)).to_uppercase(), s.fare),
        _ => String::new(),
    };
    lines.push((if spec.seats == 0 { "NO PASSENGER CABIN: FIT ONE IN A CARGO SLOT AT A SHIPYARD".to_string() } else { format!("{aboard}{free} OF {} SEATS FREE", spec.seats) }, DIM));
    lines.push((String::new(), DIM));
    let mut row = 0;
    if lands_here(app) {
        let here = row == pick;
        lines.push((format!("{}LAND THEM HERE FOR {:.0} CR", if here { ">" } else { " " }, s.fare * s.passengers as f64), if here { SELECT } else { TEXT }));
        row += 1;
    }
    lines.push((format!(" {:<34} {:>6} {:>6} {:>8}", "BOOKED FOR", "PEOPLE", "FARE", "FOR YOU"), DIM));
    if app.v.bookings.is_empty() {
        lines.push(("  NOBODY HERE WANTS TO LEAVE".into(), DIM));
    }
    for b in &app.v.bookings {
        let here = row == pick;
        let sys = app.charts.system(b.system);
        let to = b.to.name(&sys).to_uppercase();
        let earn = b.fare * b.people.min(free) as f64;
        lines.push((format!("{}{:<34} {:>6} {:>6.0} {:>8.0}", if here { ">" } else { " " }, to.chars().take(34).collect::<String>(), b.people, b.fare, earn), if here { SELECT } else { TEXT }));
        row += 1;
    }
    lines.push((String::new(), DIM));
    lines.push((format!("UP/DOWN PICK  ENTER {}  {} CLOSE", if lands_here(app) && pick == 0 { "LAND" } else { "BOARD" }, crate::keys::key(crate::keys::Act::Passengers)), DIM));
    // A screen of its own (as the market's).
    let size = frame.size();
    frame.hud_rect(Vec2::ZERO, size, Color([0.0, 0.015, 0.01, 1.0]));
    for (k, (t, c)) in lines.iter().enumerate() {
        frame.text(Vec2::new(12.0, 12.0 + k as f32 * 12.0), t, *c);
    }
}
