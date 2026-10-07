//! The client's colours, once: the text and its dimmer kind, the warning amber and the alarm red,
//! and the dark of a panel behind them (at the alpha a panel wants).

use universe_engine::Color;

/// Text, and the HUD's lines.
pub const TEXT: Color = Color::hex(0xdcebf2);
/// Text that matters less: labels, column heads, hints.
pub const DIM: Color = Color::hex(0x7d93a0);
/// A warning, or what's picked.
pub const AMBER: Color = Color::hex(0xffb040);
/// An alarm: danger, a refusal.
pub const RED: Color = Color::hex(0xff4040);

/// A panel's backing at `alpha`.
pub const fn panel(alpha: f32) -> Color {
    Color([0.012, 0.018, 0.026, alpha])
}

/// The usual panel's backing.
pub const PANEL: Color = panel(0.85);
