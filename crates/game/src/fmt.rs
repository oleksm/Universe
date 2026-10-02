use universe_sim::units::{AU, DAY, LIGHT_YEAR};

const C: f64 = 299_792_458.0;

pub fn distance(m: f64) -> String {
    let a = m.abs();
    if a < 10_000.0 {
        format!("{m:.0} M")
    } else if a < 1.0e9 {
        format!("{:.0} KM", m / 1e3)
    } else if a < 0.05 * LIGHT_YEAR {
        format!("{:.2} AU", m / AU)
    } else {
        format!("{:.1} LY", m / LIGHT_YEAR)
    }
}

/// A mass in tonnes, kilotonnes, megatonnes or gigatonnes.
pub fn tonnes(kg: f64) -> String {
    let t = kg / 1000.0;
    match t {
        t if t < 1.0e3 => format!("{t:.1} T"),
        t if t < 1.0e6 => format!("{:.1} KT", t / 1.0e3),
        t if t < 1.0e9 => format!("{:.1} MT", t / 1.0e6),
        t => format!("{:.1} GT", t / 1.0e9),
    }
}

pub fn speed(v: f64) -> String {
    let a = v.abs();
    if a < 1000.0 {
        format!("{v:.1} M/S")
    } else if a < 0.01 * C {
        format!("{:.2} KM/S", v / 1e3)
    } else if a < 1.0e5 * C {
        format!("{:.1} C", v / C)
    } else {
        format!("{:.1e} C", v / C)
    }
}

/// A delay: seconds (to the hundredth under ten), minutes, then hours and on.
pub fn lag(s: f64) -> String {
    if s < 10.0 {
        format!("{s:.2} S")
    } else if s < 120.0 {
        format!("{s:.0} S")
    } else if s < 7200.0 {
        format!("{:.0} MIN", s / 60.0)
    } else {
        duration(s)
    }
}

pub fn duration(s: f64) -> String {
    if s < 2.0 * DAY {
        format!("{:.1} H", s / 3600.0)
    } else if s < 1000.0 * DAY {
        format!("{:.1} D", s / DAY)
    } else {
        format!("{:.1} Y", s / (365.25 * DAY))
    }
}

pub fn clock(t: f64) -> String {
    let day = (t / DAY).floor();
    let rem = t - day * DAY;
    let (h, m, s) = ((rem / 3600.0) as u32, (rem / 60.0) as u32 % 60, rem as u32 % 60);
    format!("DAY {day:.0} {h:02}:{m:02}:{s:02}")
}

pub fn warp(w: f64) -> String {
    if w >= 1e4 {
        format!("X{:.0}K", w / 1e3)
    } else {
        format!("X{w:.0}")
    }
}

/// Countdown style: "0:45", "12:03", "2:05:00".
pub fn countdown(s: f64) -> String {
    let s = s.max(0.0) as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// A temperature (K) as people read it: degrees Celsius.
pub fn temperature(kelvin: f64) -> String {
    format!("{:.0} °C", kelvin - 273.15)
}
