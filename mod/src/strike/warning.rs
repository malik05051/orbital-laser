//! The warning before a strike (`/orbital strikedelay`, the wheel's DELAY; kept): the time before it strikes, which the
//! player chooses. Fired with a delay, a laser first warns: its name as a title, "strikes in m:ss" counting down over
//! the hotbar, and the J-Alert's general alert from the poles round the player (the Earthquake's countdown has no
//! sirens: an earthquake can't be foreseen, its Early Warning comes as it strikes). Then it strikes at the target, as
//! fired then (lib.rs's `fire_plain` and its `then`). The Tsunami has its own (`TsConfig::arrival`) and the false alert
//! none.
use super::{Act, Plan, Rng, Target};
use crate::lasers::Laser;

/// The steps DELAY goes through (seconds; 0 now, as fired), and the most it may be set to.
pub const DELAYS: [u32; 12] = [0, 10, 20, 30, 60, 120, 180, 300, 600, 900, 1200, 1800];
pub const DELAY_MAX: u32 = 1800;

thread_local! {
    static DELAY: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}
pub fn delay() -> u32 {
    DELAY.with(std::cell::Cell::get)
}
pub fn set_delay(s: u32) {
    DELAY.with(|d| d.set(if s == 0 { 0 } else { s.clamp(10, DELAY_MAX) }));
}
/// DELAY a step shorter or longer.
pub fn delay_step(dir: i8) {
    let d = delay();
    let at = DELAYS.iter().enumerate().min_by_key(|(_, s)| s.abs_diff(d)).map_or(0, |x| x.0);
    set_delay(DELAYS[(at as i64 + i64::from(dir.signum())).clamp(0, DELAYS.len() as i64 - 1) as usize]);
}
/// The delay as the wheel shows it.
pub fn delay_text(s: u32) -> String {
    match s {
        0 => "NOW".to_string(),
        s if s < 60 => format!("{s}s"),
        s if s % 60 == 0 => format!("{}m", s / 60),
        s => format!("{}m{:02}", s / 60, s % 60),
    }
}
/// Whether laser `key` warns first with a delay set: all but the Tsunami (its ARRIVAL) .
pub fn warns(key: &str) -> bool {
    key != "tsunami"
}

/// The warning's plan for laser `laser` fired at `target` (the player at `me`), `delay` seconds before it strikes: the
/// title, the countdown, and (all but the Earthquake's) the general alert from the poles, at the volume
/// `qk_sea::alert_config` has.
pub fn warning_plan(laser: &'static Laser, target: &Target, terrain: bool, shader_seed: f32, me: [f64; 3], delay: u32) -> Vec<(f32, Act)> {
    let [bx, by, bz] = target.block;
    let mut p = Plan { laser, ev: Vec::new(), rng: Rng::new(u64::from(shader_seed.to_bits()) ^ 0x3a7e), bx, by, bz };
    let len = f64::from(delay.max(1));
    let rgb = laser.rgb;
    let mmss = |s: f64| super::qk_sea::mmss(s);
    // (a title is drawn four times the size: the laser's name, the rest under it)
    p.at(0.0, "title @s times 10 70 20");
    p.at(0.0, format!(r##"title @s subtitle {{"text":"strikes in {}  ·  take cover","color":"#{rgb:06X}"}}"##, mmss(len)));
    p.at(0.0, format!(r##"title @s title {{"text":"{}","color":"#{rgb:06X}"}}"##, laser.title));
    let mut t = 0.0;
    while t < len {
        p.ev.push((t as f32, Act::Cmd(format!(r##"title @s actionbar {{"text":"WARNING  ·  {}  ·  strikes in {}","color":"#{rgb:06X}"}}"##, laser.title, mmss(len - t)))));
        t += 1.0;
    }
    if laser.key != "earthquake" {
        super::qk_sea::alert_sirens(&mut p, me, terrain, shader_seed as u32 ^ 0x3a7e, len);
    }
    p.ev.sort_by(|a, b| a.0.total_cmp(&b.0));
    p.ev
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_delayed_strike_warns_counting_down_with_the_general_alert() {
        let target = Target { block: [0, 64, 0], point: [0.5, 65.0, 0.5] };
        super::super::qk_sea::alert_configure(super::super::qk_sea::AlertConfig { on: true, vol: 1.0 });
        let plan = warning_plan(&crate::lasers::NUKE, &target, true, 3.0, [0.5, 66.0, 200.5], 120);
        let bars: Vec<&String> = plan.iter().filter_map(|(_, a)| if let Act::Cmd(c) = a { c.contains("strikes in").then_some(c) } else { None }).collect();
        assert!(bars.iter().any(|c| c.contains("2:00")) && bars.len() >= 120, "{}", bars.len());
        assert!(plan.iter().any(|(_, a)| matches!(a, Act::Heard { event, .. } if event == "orbital:quake.alert")));
        assert!(plan.iter().all(|(t, a)| *t <= 120.0 && !matches!(a, Act::Blocks(_) | Act::Read { .. } | Act::Table(_))));
        assert!(plan.windows(2).all(|w| w[0].0 <= w[1].0));
        // (an earthquake can't be foreseen: its countdown, no sirens)
        let quake = warning_plan(&crate::lasers::EARTHQUAKE, &target, true, 3.0, [0.5, 66.0, 200.5], 60);
        assert!(quake.iter().all(|(_, a)| !matches!(a, Act::Heard { .. })));
        assert!(quake.iter().any(|(_, a)| matches!(a, Act::Cmd(c) if c.contains("strikes in 1:00"))));
        // (the steps: now, then 10 s up to half an hour)
        set_delay(0);
        delay_step(-1);
        assert_eq!(delay(), 0);
        delay_step(1);
        assert_eq!(delay(), 10);
        for _ in 0..20 {
            delay_step(1);
        }
        assert_eq!(delay(), DELAY_MAX);
        set_delay(5);
        assert_eq!(delay(), 10);
        assert_eq!(delay_text(1800), "30m");
        set_delay(0);
        assert!(warns("nuke") && warns("earthquake") && !warns("tsunami"));
    }
}
