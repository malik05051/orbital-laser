//! The Earthquake (laser 19): a historic earthquake played on the land round the target, one of `QK_PRESETS` (the
//! wheel's QUAKE row, `/orbital quake`). No beam and nothing from the sky: the ground itself.
//!
//! **Everything goes by the JMA Shindo scale** (気象庁震度階級: 0, 1, 2, 3, 4, 5-, 5+, 6-, 6+, 7). Each place's
//! instrumental intensity comes from its distance to the rupture (`QkGeo::ii`): Si and Midorikawa's (1999) attenuation of
//! peak ground velocity with distance, `lpgv`, turned into intensity as Midorikawa et al. (1999) do (I = 2.68 + 1.72 log
//! PGV), anchored so that right over the rupture it is the preset's own highest Shindo. That intensity decides how hard
//! the camera shakes where the player stands, what the panel and the actionbar say, and what the land suffers, column by
//! column, as the JMA's own table of what each class does: glass breaks from 5-, unreinforced block walls fall from 5+,
//! wooden houses lean and fall from 6- (many at 6+, most at 7), reinforced ones from 6+, the ground cracks and slopes slide
//! from 6-, soft wet ground liquefies from 5+.
//!
//! The rupture: a fault through the target, across the player's view (`QkGeo`). A strike-slip one (Kobe, Kumamoto)
//! breaks the surface along its trace as the rupture runs out from its hypocentre: the land each side shifted past the
//! other (right-lateral), roads, rivers and fences offset, an en-echelon fissure, mole tracks, the far side a block
//! higher; a reverse one (Noto) heaves the far side up in a scarp. A megathrust (Tohoku, Kanto, Valdivia) breaks far
//! offshore beyond the target, nothing on land but its shaking: long, rolling and in pulses; and those that raised a
//! tsunami raise it here (strike/qk_sea.rs: the JMA's warning, the sirens and its countdown, the sea drawing back, a
//! train of waves flooding the low land in real water, then all of it put back).
//!
//! The shaking at a place (`QkGeo::env`): the P wave's rattle from its arrival, then the S waves' strong shaking in the
//! preset's pulses (its subevents, `Quake::pulses`), each arriving as the rupture front reaches the nearest part of the
//! fault and the waves travel on from there (`QkGeo::arrive`). Real times are compressed by `QK_COMPRESS` and real
//! distances by each preset's `kpb` (kilometres a block); slips are real: a block a metre.
//!
//! Keep `QK_PRESETS`' kind, mw, shindo, depth, kpb, half, hypo, slip_v and pulses, `QK_T0`, `QK_COMPRESS`, `QK_VS`,
//! `QK_VP`, `QK_VR`, `QK_DIP_W` and `QK_BAND` in step with shaders/earthquake.wgsl's.
use super::hu_blocks::HU_BLOCKS;
use super::hurricane::hu_block;
use super::qk_sea::{self, Tsu};
use super::{Act, Blocks, Col, Plan, Rng, Target, mh, tidy, wing_angle};
use crate::lasers::Laser;
use std::cell::RefCell;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::rc::Rc;

/// How a preset's fault breaks.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fault {
    /// Vertical, the land each side sliding past the other (right-lateral), breaking the surface along its trace.
    StrikeSlip,
    /// Dipping under the far side, which rides up in a scarp.
    Reverse,
    /// A subduction zone's: far offshore beyond the target, nothing broken on land, a tsunami after.
    Megathrust,
}

/// A historic earthquake.
pub struct Quake {
    /// `/orbital quake KEY`, kept in the settings.
    pub key: &'static str,
    /// The wheel's button.
    pub button: &'static str,
    /// What the panel and the wheel call it.
    pub name: &'static str,
    /// The day it struck, and the name it is known by, if any (Great Hanshin).
    pub date: &'static str,
    pub aka: Option<&'static str>,
    /// Moment magnitude.
    pub mw: f64,
    pub fault: Fault,
    /// The highest instrumental Shindo, right over the rupture (6.5 and up is a 7).
    pub shindo: f64,
    /// Its highest Shindo is estimated, not measured (before the instruments, or outside Japan).
    pub estimated: bool,
    /// Kilometres from the land right over the rupture to the rupture itself (the closest it comes).
    pub depth: f64,
    /// Kilometres a block, for the distances the shaking weakens over.
    pub kpb: f64,
    /// Half the rupture's length, in blocks, along its strike; and where along it it starts (-1 one end, 1 the other).
    pub half: f64,
    pub hypo: f64,
    /// At the trace, in blocks (metres): the right-lateral offset, and how far the far side rises.
    pub slip_h: i32,
    pub slip_v: i32,
    /// The subevents: (seconds after the first, share of the strongest, seconds it lasts), the strongest's 1.0.
    pub pulses: &'static [(f32, f32, f32)],
    /// The tsunami it raised (strike/qk_sea.rs): the warning, the sea drawing back, the waves flooding the land.
    pub tsunami: Option<Tsu>,
    /// An Earthquake Early Warning went out for it (from 2007): the actionbar warns before the S waves come.
    pub eew: bool,
    /// Fires that break out in the houses it brings down, at most.
    pub fires: u32,
    /// How prone its land is to slide, and to liquefy (1 typical).
    pub slides: f64,
    pub liquefy: f64,
    /// The shaking's seconds (with a tsunami the strike runs on: `qk_length`).
    pub length: f32,
    /// The wheel's line for it.
    pub blurb: &'static str,
}

impl Quake {
    /// The strongest subevent's time.
    pub fn t_main(&self) -> f32 {
        self.pulses.iter().fold((0.0f32, 0.0f32), |m, p| if p.1 > m.1 { (p.0, p.1) } else { m }).0
    }
    pub fn main_dur(&self) -> f32 {
        self.pulses.iter().fold((0.0f32, 0.0f32), |m, p| if p.1 > m.1 { (p.2, p.1) } else { m }).0
    }
    /// Its highest Shindo as the JMA writes it.
    pub fn top_class(&self) -> &'static str {
        shindo_class(self.shindo).0
    }
    /// The surface breaks along the trace.
    pub fn ruptures(&self) -> bool {
        self.fault != Fault::Megathrust
    }
}

pub const QK_PRESETS: [Quake; 6] = [
    Quake {
        key: "tohoku",
        button: "TOHOKU",
        name: "Tohoku 2011",
        date: "11 March 2011",
        aka: Some("Great East Japan"),
        mw: 9.1,
        fault: Fault::Megathrust,
        shindo: 6.7,
        estimated: false,
        depth: 30.0,
        kpb: 0.55,
        half: 420.0,
        hypo: -0.1,
        slip_h: 0,
        slip_v: 0,
        // (two great packets of shaking some 50 s apart in the real records, compressed; an aftershock near the end)
        pulses: &[(0.0, 0.45, 13.0), (18.0, 1.0, 18.0), (36.0, 0.6, 14.0), (62.0, 0.22, 5.0)],
        tsunami: Some(Tsu {
            expect: "10 m+",
            draw: 7.0,
            // (a first rise, the great second wave the coast saw half an hour on, a third)
            waves: &[(0.0, 3.5, 10.0, 3.0, 12.0), (26.0, 12.0, 14.0, 6.0, 20.0), (68.0, 6.0, 10.0, 4.0, 14.0)],
            speed: 12.0,
            reach: 240.0,
        }),
        eew: true,
        fires: 6,
        slides: 0.8,
        liquefy: 1.4,
        length: 80.0,
        blurb: "Mw 9.1 megathrust, Shindo 7: minutes of rolling shaking, then the sea.",
    },
    Quake {
        key: "kobe",
        button: "KOBE",
        name: "Kobe 1995",
        date: "17 January 1995",
        aka: Some("Great Hanshin"),
        mw: 6.9,
        fault: Fault::StrikeSlip,
        shindo: 6.6,
        estimated: false,
        depth: 3.0,
        kpb: 0.15,
        half: 170.0,
        hypo: -0.2,
        slip_h: 2,
        slip_v: 1,
        // (some fifteen violent seconds, right under the city)
        pulses: &[(0.0, 1.0, 8.0), (3.5, 0.7, 6.0), (24.0, 0.18, 3.0)],
        tsunami: None,
        eew: false,
        fires: 14,
        slides: 0.6,
        liquefy: 1.5,
        length: 40.0,
        blurb: "Mw 6.9 strike-slip under a city, Shindo 7: houses down, the Nojima fault torn open.",
    },
    Quake {
        key: "kanto",
        button: "KANTO",
        name: "Kanto 1923",
        date: "1 September 1923",
        aka: Some("Great Kanto"),
        mw: 7.9,
        fault: Fault::Megathrust,
        shindo: 6.6,
        estimated: true,
        depth: 15.0,
        kpb: 0.35,
        half: 260.0,
        hypo: -0.6,
        slip_h: 0,
        slip_v: 0,
        // (the main shock, then a great aftershock within minutes)
        pulses: &[(0.0, 0.8, 11.0), (9.0, 1.0, 13.0), (34.0, 0.55, 7.0)],
        tsunami: Some(Tsu {
            expect: "10 m",
            draw: 5.0,
            // (Atami's 12 m came within minutes, the bay's waves after it)
            waves: &[(0.0, 6.0, 8.0, 3.0, 12.0), (24.0, 8.0, 10.0, 4.0, 14.0), (56.0, 3.0, 8.0, 2.0, 10.0)],
            speed: 10.0,
            reach: 140.0,
        }),
        eew: false,
        fires: 48,
        slides: 1.2,
        liquefy: 1.0,
        length: 62.0,
        blurb: "Mw 7.9 under Sagami Bay at lunchtime: Shindo 7 (est.), the fires spread, then a tsunami.",
    },
    Quake {
        key: "kumamoto",
        button: "KUMAMOTO",
        name: "Kumamoto 2016",
        date: "14 and 16 April 2016",
        aka: None,
        mw: 7.0,
        fault: Fault::StrikeSlip,
        shindo: 6.6,
        estimated: false,
        depth: 3.0,
        kpb: 0.15,
        half: 180.0,
        hypo: -0.7,
        slip_h: 2,
        slip_v: 1,
        // (a Shindo 7 foreshock, a lull, then the main shock: two 7s, 28 hours apart in life)
        pulses: &[(0.0, 0.75, 6.0), (16.0, 1.0, 11.0), (34.0, 0.25, 4.0)],
        tsunami: None,
        eew: true,
        fires: 2,
        slides: 1.5,
        liquefy: 0.8,
        length: 46.0,
        blurb: "Two Shindo 7s: a foreshock, then Mw 7.0 on the Futagawa fault. Slopes give way.",
    },
    Quake {
        key: "noto",
        button: "NOTO",
        name: "Noto 2024",
        date: "1 January 2024",
        aka: None,
        mw: 7.5,
        fault: Fault::Reverse,
        shindo: 6.7,
        estimated: false,
        depth: 5.0,
        kpb: 0.25,
        half: 220.0,
        hypo: -0.1,
        slip_h: 0,
        slip_v: 4,
        pulses: &[(0.0, 0.7, 6.0), (6.0, 1.0, 14.0), (36.0, 0.2, 4.0)],
        tsunami: Some(Tsu {
            expect: "5 m",
            draw: 3.0,
            // (Suzu's 4 to 5 m, the first wave within minutes)
            waves: &[(0.0, 3.0, 7.0, 3.0, 10.0), (20.0, 4.5, 8.0, 3.0, 12.0), (44.0, 2.0, 6.0, 2.0, 8.0)],
            speed: 9.0,
            reach: 100.0,
        }),
        eew: true,
        fires: 10,
        slides: 1.3,
        liquefy: 1.2,
        length: 48.0,
        blurb: "Mw 7.5 reverse fault, Shindo 7: the far side heaves up 4 m, fires, then a tsunami.",
    },
    Quake {
        key: "valdivia",
        button: "VALDIVIA",
        name: "Valdivia 1960",
        date: "22 May 1960",
        aka: Some("Great Chilean"),
        mw: 9.5,
        fault: Fault::Megathrust,
        shindo: 6.8,
        estimated: true,
        depth: 25.0,
        kpb: 0.8,
        half: 520.0,
        hypo: -0.9,
        slip_h: 0,
        slip_v: 0,
        // (the strongest ever measured: some ten minutes of rupture, compressed into four great surges)
        pulses: &[(0.0, 0.6, 18.0), (14.0, 0.9, 20.0), (34.0, 1.0, 20.0), (54.0, 0.6, 15.0), (74.0, 0.2, 5.0)],
        tsunami: Some(Tsu {
            expect: "10 m+",
            draw: 8.0,
            waves: &[(0.0, 4.0, 12.0, 4.0, 14.0), (30.0, 10.0, 14.0, 6.0, 20.0), (72.0, 7.0, 12.0, 4.0, 16.0)],
            speed: 11.0,
            reach: 220.0,
        }),
        eew: false,
        fires: 3,
        slides: 1.4,
        liquefy: 1.2,
        length: 100.0,
        blurb: "Mw 9.5, the strongest ever recorded. Shindo 7 (est.), the land rolls for minutes, then the sea.",
    },
];

/// The preset for `/orbital quake KEY` (its key or its button, any case).
pub fn qk_preset(word: &str) -> Option<usize> {
    let w = word.to_lowercase();
    QK_PRESETS.iter().position(|q| q.key == w || q.button.to_lowercase() == w)
}

/// The JMA class of instrumental intensity `i`: as written (5-, 6+ ...), in words, and the JMA's own map colour.
pub fn shindo_class(i: f64) -> (&'static str, &'static str, u32) {
    match i {
        i if i < 0.5 => ("0", "0", 0xF2F2FF),
        i if i < 1.5 => ("1", "1", 0xF2F2FF),
        i if i < 2.5 => ("2", "2", 0x00AAFF),
        i if i < 3.5 => ("3", "3", 0x0041FF),
        i if i < 4.5 => ("4", "4", 0xFAE696),
        i if i < 5.0 => ("5-", "5 Lower", 0xFFE600),
        i if i < 5.5 => ("5+", "5 Upper", 0xFF9900),
        i if i < 6.0 => ("6-", "6 Lower", 0xFF2800),
        i if i < 6.5 => ("6+", "6 Upper", 0xA50021),
        _ => ("7", "7", 0xB40068),
    }
}

/// When the earthquake starts (its first subevent), seconds into the strike.
pub const QK_T0: f32 = 2.0;
/// Real seconds to the strike's: the waves, the rupture and the pulses run this much faster than in life.
const QK_COMPRESS: f64 = 3.0;
/// The S and P waves' and the rupture's real speeds, km/s (a block's worth with `kpb` and `QK_COMPRESS`).
const QK_VS: f64 = 3.5;
const QK_VP: f64 = 6.0;
const QK_VR: f64 = 2.5;
/// How far under the far side a reverse fault's rupture reaches, in blocks (its plane's width seen from above).
const QK_DIP_W: f64 = 40.0;
/// Blocks each side of the trace the land is dragged along with the slip (and heaved, for a reverse fault).
const QK_BAND: f64 = 12.0;
/// How far round the target the land is read and damaged (blocks), and the most chunks of it.
const QK_REACH: f64 = 208.0;
const QK_MAX_CHUNKS: usize = 900;
/// The columns read round each chunk's own (for the slip's sources, the slopes and the water near), and each column's
/// blocks read under its top.
const QK_M: i32 = 3;
const QK_W: i32 = 16 + 2 * QK_M;
const QK_BELOW: u32 = 40;
const QK_ABOVE: u32 = 1;
/// Chunks a read, and what one stage of the land's work may spend (fuel).
const QK_PART: usize = 8;
const QK_STAGE: u64 = 5_000_000;

/// The shaking's strength on the game's camera (0 to 1, which the game squares) for instrumental intensity `i`.
pub fn shake_of(i: f64) -> f64 {
    ((i - 2.5) / 4.5).clamp(0.0, 1.0)
}
/// How violent it is at instrumental intensity `i`, past `shake_of`: the JMA's 6+ and 7 aren't a little more than a 6-
/// (from 6+ no one can stand or move but by crawling, at 7 people and furniture are thrown), so the picture's sway and
/// the camera's jolts grow steeply over them, to 2.6 times at 7 (earthquake.wgsl's qk_violence: keep in step).
pub fn qk_violence(i: f64) -> f64 {
    shake_of(i) * (1.0 + 1.6 * smooth(5.5, 7.0, i))
}

/// Peak ground velocity's log at `x` km from the rupture (Si and Midorikawa 1999, less the magnitude's own terms, which
/// the preset's top Shindo stands for); magnitudes over 8.3 saturate.
fn lpgv(mw: f64, x: f64) -> f64 {
    let m = mw.min(8.3);
    -(x + 0.0028 * 10f64.powf(0.5 * m)).log10() - 0.002 * x
}

fn smooth(a: f64, b: f64, x: f64) -> f64 {
    let u = ((x - a) / (b - a)).clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}
/// Linear between the table's points (intensity, value), flat past its ends.
fn ramp(t: &[(f64, f64)], i: f64) -> f64 {
    if i <= t[0].0 {
        return t[0].1;
    }
    for w in t.windows(2) {
        if i <= w[1].0 {
            return w[0].1 + (w[1].1 - w[0].1) * (i - w[0].0) / (w[1].0 - w[0].0);
        }
    }
    t[t.len() - 1].1
}
/// One subevent's shaking at `x` of its length after it arrives: a quick rise, a long decay.
pub fn pulse(x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    smooth(0.0, 0.12, x) * if x < 0.12 { 1.0 } else { (-2.6 * (x - 0.12)).exp() }
}
fn chance(x: i32, z: i32, seed: u32, k: u32) -> f64 {
    mh((x as u32).wrapping_mul(73_856_093) ^ (z as u32).wrapping_mul(19_349_663), seed, k)
}

/// Where the fault lies, and the shaking anywhere.
pub struct QkGeo {
    pub q: &'static Quake,
    pub preset: usize,
    /// The target's middle, on its ground.
    pub t: [f64; 3],
    /// Along the fault's strike, and across it towards the player's side.
    pub st: [f64; 2],
    pub nm: [f64; 2],
    /// The strike's angle (radians, the pass's).
    pub angle: f64,
    /// Along the strike, the way a right-lateral slip moves the player's side (+1 or -1).
    pub sense: f64,
    pub seed: u32,
}

impl QkGeo {
    pub fn new(preset: usize, target: &Target, me: [f64; 3], shader_seed: f32) -> QkGeo {
        let q = qk_quake(preset);
        let [bx, by, bz] = target.block;
        let t = [f64::from(bx) + 0.5, f64::from(by) + 1.0, f64::from(bz) + 0.5];
        let seed = shader_seed as u32;
        // (across the player's view, give or take 20 degrees: the trace runs across in front of them)
        let toward = f64::from(wing_angle(t, me));
        let angle = toward + PI / 2.0 + (mh(seed, 0x9a4e, 3) - 0.5) * 0.7;
        let st = [angle.cos(), angle.sin()];
        let mut nm = [-st[1], st[0]];
        if nm[0] * toward.cos() + nm[1] * toward.sin() < 0.0 {
            nm = [-nm[0], -nm[1]];
        }
        // (right-lateral: from the far side, looking at the player's, it moves to the right: (-n.z, n.x) facing -n)
        let right = [nm[1], -nm[0]];
        let sense = if right[0] * st[0] + right[1] * st[1] >= 0.0 { 1.0 } else { -1.0 };
        QkGeo { q, preset: preset.min(QK_CUSTOM), t, st, nm, angle: angle.rem_euclid(2.0 * PI), sense, seed }
    }
    /// (along the strike, across it towards the player) of the world point (x, z).
    pub fn ud(&self, x: f64, z: f64) -> (f64, f64) {
        let (dx, dz) = (x - self.t[0], z - self.t[2]);
        (dx * self.st[0] + dz * self.st[1], dx * self.nm[0] + dz * self.nm[1])
    }
    /// How far (blocks, over the land) from the rupture seen from above.
    pub fn rh(&self, u: f64, d: f64) -> f64 {
        let du = (u.abs() - self.q.half).max(0.0);
        let dd = match self.q.fault {
            Fault::StrikeSlip => d.abs(),
            Fault::Reverse => {
                if d > 0.0 {
                    d
                } else {
                    (-d - QK_DIP_W).max(0.0)
                }
            }
            Fault::Megathrust => d.max(0.0),
        };
        du.hypot(dd)
    }
    /// The instrumental Shindo at (u, d), 0 to 7.
    pub fn ii_ud(&self, u: f64, d: f64) -> f64 {
        let q = self.q;
        let x = (self.rh(u, d) * q.kpb).hypot(q.depth);
        (q.shindo + 1.72 * (lpgv(q.mw, x) - lpgv(q.mw, q.depth))).clamp(0.0, 7.0)
    }
    pub fn ii(&self, x: f64, z: f64) -> f64 {
        let (u, d) = self.ud(x, z);
        self.ii_ud(u, d)
    }
    /// Blocks a second, in the strike's time, of a real speed of `v` km/s.
    fn speed(&self, v: f64) -> f64 {
        v * QK_COMPRESS / self.q.kpb
    }
    /// The rupture front's time to `u` along the strike from its start (seconds after its main subevent).
    pub fn front(&self, u: f64) -> f64 {
        // (a megathrust's subevents are its rupture's progress already: `pulses`)
        if !self.q.ruptures() {
            return 0.0;
        }
        let u = u.clamp(-self.q.half, self.q.half);
        (u - self.q.hypo * self.q.half).abs() / self.speed(QK_VR)
    }
    /// When the S waves (and the P waves) arrive at (u, d), seconds after a subevent sets out: the rupture front to the
    /// nearest part of the fault, then the waves on from there.
    pub fn arrive(&self, u: f64, d: f64) -> (f64, f64) {
        let f = self.front(u);
        let r = self.rh(u, d) + self.q.depth / self.q.kpb * 0.5;
        (f + r / self.speed(QK_VS), f * 0.7 + r / self.speed(QK_VP))
    }
    /// The trace breaks at `u` at this strike second.
    pub fn trace_at(&self, u: f64) -> f64 {
        f64::from(QK_T0 + self.q.t_main()) + self.front(u)
    }
    /// How hard it shakes at (u, d) at strike second `t`, 0 to about 1 (times `shake_of` its Shindo).
    pub fn env(&self, u: f64, d: f64, t: f64) -> f64 {
        let (s, p) = self.arrive(u, d);
        let tau = t - f64::from(QK_T0);
        // (later subevents arrive from further along the fault: a little later the further it is)
        let mut e = 0.0f64;
        for &(tk, a, dur) in self.q.pulses {
            let x = (tau - s - f64::from(tk)) / f64::from(dur);
            e = e.max(f64::from(a) * pulse(x));
        }
        // (the P waves' rattle, from their arrival to the S waves')
        let a0 = f64::from(self.q.pulses[0].1);
        let pw = 0.2 * a0 * smooth(p, p + 0.4, tau) * (1.0 - smooth(s - 0.2, s + 0.3, tau));
        e.max(pw)
    }
    /// When the strongest shaking reaches (u, d), strike seconds.
    pub fn strong_at(&self, u: f64, d: f64) -> f64 {
        let (s, _) = self.arrive(u, d);
        f64::from(QK_T0 + self.q.t_main()) + s + 0.15 * f64::from(self.q.main_dur())
    }
    /// The slip at (u, d) (blocks): along the strike (the land there came from `u - h`), and up.
    fn slip(&self, u: f64, d: f64) -> (f64, f64) {
        let q = self.q;
        if !q.ruptures() || d.abs() > QK_BAND || u.abs() > q.half {
            return (0.0, 0.0);
        }
        // (dying away towards the rupture's ends)
        let taper = 1.0 - smooth(0.7, 1.0, u.abs() / q.half);
        let drag = 1.0 - smooth(0.0, QK_BAND, d.abs());
        let h = f64::from(q.slip_h) * 0.5 * d.signum() * drag * taper * self.sense;
        let v = if d < 0.0 { f64::from(q.slip_v) * taper * (1.0 - smooth(0.35 * QK_BAND, QK_BAND, -d)) } else { 0.0 };
        (h, v)
    }
}

// ---- the blocks: what each is to the shaking ----------------------------------------------------------------------------

/// What a block is (`classify`): its class as the Hurricane's table has it (0 air, 1 water and lava, 2 the land's own
/// ground, 3 logs, 4 leaves, 5 plants, 6 light things people put up, 7 buildings, 8 what is left alone: what can't be
/// broken, what holds things, what the table doesn't know), and what else matters here.
#[derive(Clone, Copy, Default)]
struct Pal {
    name: &'static str,
    class: u8,
    glass: bool,
    /// An unreinforced block wall (a cobblestone, brick or stone wall).
    wall: bool,
    /// Built of wood (a wooden house's).
    wood: bool,
    /// Ground that liquefies when wet (sand, silt, loose soil).
    soft: bool,
}

/// The blocks the quake puts down itself, as ids past the read's palette.
const X0: u16 = 0xF000;
const AIR: u16 = X0;
const EXTRA: [&str; 9] = [
    "minecraft:air",
    "minecraft:coarse_dirt",
    "minecraft:dirt",
    "minecraft:gravel",
    "minecraft:mud",
    "minecraft:sand",
    "minecraft:fire",
    "minecraft:cobblestone",
    "minecraft:rooted_dirt",
];
const COARSE: u16 = X0 + 1;
const DIRT: u16 = X0 + 2;
const GRAVEL: u16 = X0 + 3;
const MUD: u16 = X0 + 4;
const SAND: u16 = X0 + 5;
const FIRE: u16 = X0 + 6;
const COBBLE: u16 = X0 + 7;
const ROOTED: u16 = X0 + 8;

fn interned(name: &str) -> &'static str {
    thread_local! {
        static NAMES: RefCell<HashMap<String, &'static str>> = RefCell::new(HashMap::new());
    }
    NAMES.with(|n| {
        let mut n = n.borrow_mut();
        if let Some(s) = n.get(name) {
            return *s;
        }
        let s: &'static str = Box::leak(name.to_string().into_boxed_str());
        n.insert(name.to_string(), s);
        s
    })
}

fn classify(name: &str) -> Pal {
    crate::meter::spend(1_500);
    let plain = name.split('[').next().unwrap_or(name);
    let n = plain.trim_start_matches("minecraft:");
    let class = if n.ends_with("air") {
        0
    } else if matches!(n, "water" | "lava" | "bubble_column" | "seagrass" | "tall_seagrass" | "kelp" | "kelp_plant") {
        1
    } else if super::hurricane::stays(plain) || n.ends_with("lightning_rod") {
        // (and lightning rods: the J-Alert siren poles' horns and crowns, qk_sea::siren_pole, stand through it all)
        8
    } else {
        hu_block(plain).map_or(8, |b| HU_BLOCKS[b as usize].class)
    };
    let woods = ["oak", "spruce", "birch", "jungle", "acacia", "dark_oak", "mangrove", "cherry", "pale_oak", "bamboo", "crimson", "warped"];
    Pal {
        name: interned(name),
        class,
        glass: n.contains("glass"),
        wall: n.ends_with("_wall") && class == 7,
        wood: n.contains("planks") || n.ends_with("_log") || n.ends_with("_wood") || (class == 7 && woods.iter().any(|w| n.starts_with(w))),
        soft: matches!(n, "sand" | "red_sand" | "gravel" | "dirt" | "grass_block" | "coarse_dirt" | "mud" | "clay" | "podzol" | "rooted_dirt" | "farmland" | "dirt_path" | "suspicious_sand" | "suspicious_gravel"),
    }
}
fn extra_pal(i: usize) -> Pal {
    let class = match i {
        0 => 0,
        6 => 5,
        _ => 2,
    };
    Pal { name: EXTRA[i], class, glass: false, wall: false, wood: false, soft: matches!(i, 1 | 2 | 3 | 4 | 5 | 8) }
}

/// A read's answer, its palette sorted out, and the chunks it holds.
struct Cur {
    a: Rc<crate::columns::Answer>,
    pal: Vec<Pal>,
    chunks: Vec<(i32, i32, f64)>,
    k: usize,
}
impl Cur {
    fn pal(&self, id: u16) -> Pal {
        if id >= X0 { extra_pal(usize::from(id - X0).min(EXTRA.len() - 1)) } else { self.pal.get(usize::from(id)).copied().unwrap_or_default() }
    }
}

/// The land's work: the chunks in the order the strong shaking reaches them, their reads, and what came of it.
struct Work {
    geo: Rc<QkGeo>,
    chunks: Vec<(i32, i32, f64)>,
    tags: Vec<u32>,
    asked: usize,
    taken: usize,
    cur: Option<Cur>,
    retry: Vec<(i32, i32, f64)>,
    retry_q: Option<(u32, Vec<(i32, i32, f64)>)>,
    next_retry: f64,
    land_by: f64,
    fires: u32,
    collapsed: u32,
    walls: u32,
    slid: u32,
    cracked: u32,
    liquefied: u32,
    torn: u32,
    changed: u32,
    done: bool,
    /// The sea's work, for a quake that raised a tsunami: each chunk where it may reach, as this work leaves it.
    flood: Option<Rc<RefCell<qk_sea::Flood>>>,
}

/// The columns a chunk's read asks for: QK_W by QK_W round it, row by row.
fn chunk_columns(cx: i32, cz: i32, out: &mut Vec<(i32, i32)>) {
    for gz in 0..QK_W {
        for gx in 0..QK_W {
            out.push((cx * 16 - QK_M + gx, cz * 16 - QK_M + gz));
        }
    }
}

/// A column of the grid as read: where its blocks start, and them (bottom first), the air all one id.
struct Grid {
    y0: Vec<i32>,
    per: usize,
    /// QK_W * QK_W columns' blocks, `per` each; none read: `y0` is i32::MIN.
    blocks: Vec<u16>,
}
impl Grid {
    fn at(&self, j: usize, y: i32) -> Option<u16> {
        let y0 = self.y0[j];
        if y0 == i32::MIN || y < y0 {
            return None;
        }
        let k = (y - y0) as usize;
        Some(if k < self.per { self.blocks[j * self.per + k] } else { AIR })
    }
}

/// What a column holds, from its top down: its ground (the highest of the land's own blocks), its top, whether water
/// stands on it, and whether anything on it is to be left alone.
#[derive(Clone, Copy)]
struct Look {
    g: i32,
    top: i32,
    wet: bool,
    locked: bool,
    /// Blocks people built standing on it.
    built: u8,
}
fn look(stack: &[u16], y0: i32, cur: &Cur) -> Look {
    let mut l = Look { g: i32::MIN, top: i32::MIN, wet: false, locked: false, built: 0 };
    for k in (0..stack.len()).rev() {
        let p = cur.pal(stack[k]);
        let y = y0 + k as i32;
        match p.class {
            0 => continue,
            1 => l.wet = true,
            2 => {
                l.g = y;
                if l.top == i32::MIN {
                    l.top = y;
                }
                return l;
            }
            7 => l.built = l.built.saturating_add(1),
            8 => l.locked = true,
            _ => {}
        }
        if l.top == i32::MIN {
            l.top = y;
        }
    }
    l
}

/// What the sea makes of a column as the quake left it (its blocks bottom first from `lo`): water at the sea's level
/// over a bed (`Sea`), land with its ground and what stands on it (`Land`; a lake or river above the sea's level is its
/// ground), or nothing it may touch (what holds things, lava, nothing read).
fn sea_col(stack: &[u16], lo: i32, cur: &Cur) -> qk_sea::FCol {
    use qk_sea::{FCol, Kind, SEA};
    crate::meter::spend(30 * stack.len() as u64);
    let mut wtop = None;
    let mut ground = None;
    let mut top = None;
    let mut built = 0;
    let mut wood = 0;
    for k in (0..stack.len()).rev() {
        let p = cur.pal(stack[k]);
        let y = lo + k as i32;
        match p.class {
            0 => continue,
            1 if p.name.contains("lava") => return FCol::default(),
            1 => {
                wtop.get_or_insert(y);
            }
            2 => {
                ground = Some(y);
                break;
            }
            8 => return FCol::default(),
            c => {
                if wtop.is_none() {
                    top.get_or_insert(y);
                    built += i32::from(c == 7);
                    wood += i32::from(p.wood);
                }
            }
        }
    }
    let i16of = |y: i32| y.clamp(-2000, 2000) as i16;
    match (wtop, ground) {
        (Some(w), g) if (SEA - 3..=SEA + 1).contains(&w) => {
            let b = g.unwrap_or(lo - 1);
            FCol { kind: Kind::Sea, base: i16of(b), orig: i16of(w), cur: i16of(w), hi: i16of(w), top: i16of(w), house: 0, washed: false }
        }
        (Some(w), _) => FCol { kind: Kind::Land, base: i16of(w), orig: i16of(w), cur: i16of(w), hi: i16of(w), top: i16of(w), house: 0, washed: false },
        (None, Some(g)) => {
            let house = if built >= 2 { if wood * 2 >= built { 1 } else { 2 } } else { 0 };
            let t = top.unwrap_or(g).max(g);
            FCol { kind: Kind::Land, base: i16of(g), orig: i16of(g), cur: i16of(g), hi: i16of(g), top: i16of(t), house, washed: false }
        }
        (None, None) => FCol::default(),
    }
}

/// One chunk's land: each column's new blocks as the slip moved it and the shaking left it, set against what was
/// there; false if too little of it was loaded (asked again).
fn qk_chunk(w: &mut Work, cur: &Cur, k: usize, q: &mut Plan) -> bool {
    let (cx, cz, due) = cur.chunks[k];
    let geo = w.geo.clone();
    let quake = geo.q;
    let a = &cur.a;
    let n = (QK_W * QK_W) as usize;
    let per = a.per;
    let base = k * n;
    // the grid as read (air all one id)
    let mut grid = Grid { y0: vec![i32::MIN; n], per, blocks: vec![AIR; n * per] };
    let mut missing = 0;
    let air_ids: Vec<bool> = cur.pal.iter().map(|p| p.class == 0).collect();
    crate::meter::spend(4_000 + (n * per) as u64 * 3);
    for j in 0..n {
        let Some((surface, bl)) = a.column(base + j) else {
            let (gx, gz) = (j as i32 % QK_W, j as i32 / QK_W);
            if (QK_M..QK_M + 16).contains(&gx) && (QK_M..QK_M + 16).contains(&gz) {
                missing += 1;
            }
            continue;
        };
        grid.y0[j] = surface - a.below;
        for (i, &b) in bl.iter().enumerate() {
            grid.blocks[j * per + i] = if air_ids.get(usize::from(b)).copied().unwrap_or(false) { AIR } else { b };
        }
    }
    if missing > 128 {
        return false;
    }
    // what each column holds as read
    let looks: Vec<Option<Look>> = (0..n).map(|j| (grid.y0[j] != i32::MIN).then(|| look(&grid.blocks[j * per..(j + 1) * per], grid.y0[j], cur))).collect();
    crate::meter::spend(60 * (n * per) as u64 / 4);
    // how far each column is from standing water (in columns, up to 7): what liquefies
    let mut wet = vec![99u8; n];
    for j in 0..n {
        if let Some(l) = looks[j]
            && l.wet
        {
            wet[j] = 0;
        }
    }
    for _ in 0..2 {
        for gz in 0..QK_W {
            for gx in 0..QK_W {
                let j = (gz * QK_W + gx) as usize;
                let mut m = wet[j];
                if gx > 0 {
                    m = m.min(wet[j - 1].saturating_add(1));
                }
                if gz > 0 {
                    m = m.min(wet[j - QK_W as usize].saturating_add(1));
                }
                wet[j] = m;
            }
        }
        for gz in (0..QK_W).rev() {
            for gx in (0..QK_W).rev() {
                let j = (gz * QK_W + gx) as usize;
                let mut m = wet[j];
                if gx + 1 < QK_W {
                    m = m.min(wet[j + 1].saturating_add(1));
                }
                if gz + 1 < QK_W {
                    m = m.min(wet[j + QK_W as usize].saturating_add(1));
                }
                wet[j] = m;
            }
        }
    }
    let gi = |gx: i32, gz: i32| (gz * QK_W + gx) as usize;
    let ground = |j: usize| looks[j].map_or(i32::MIN, |l| l.g);

    let (x0, z0) = (cx * 16, cz * 16);
    let ic = geo.ii(f64::from(x0) + 8.0, f64::from(z0) + 8.0);
    let seed = geo.seed ^ (geo.preset as u32).wrapping_mul(0x9E37_79B9);

    // ground cracks: a few lines across the chunk, mostly along the fault's strike, more the harder it shook
    let mut crack = [0u8; 256];
    let rate = ramp(&[(5.5, 0.0), (6.0, 0.2), (6.5, 0.8), (7.0, 1.6)], ic);
    let ncr = (rate + chance(cx, cz, seed, 1)).floor() as i32;
    for c in 0..ncr.max(0) as u32 {
        let (mut px, mut pz) = (chance(cx, cz, seed, 10 + c) * 16.0, chance(cx, cz, seed, 20 + c) * 16.0);
        let along = chance(cx, cz, seed, 30 + c) < 0.7;
        let a0 = geo.angle + if along { 0.0 } else { PI / 2.0 } + (chance(cx, cz, seed, 40 + c) - 0.5) * 0.8;
        let len = 7.0 + 14.0 * chance(cx, cz, seed, 50 + c);
        let mut s = 0.0;
        while s < len {
            let a = a0 + 0.5 * ((s * 0.3 + f64::from(c)).sin());
            px += a.cos();
            pz += a.sin();
            s += 1.0;
            let (lx, lz) = (px.floor() as i32, pz.floor() as i32);
            if !(0..16).contains(&lx) || !(0..16).contains(&lz) {
                break;
            }
            let deep = 1 + (3.0 * (1.0 - (2.0 * s / len - 1.0).abs())).round() as u8;
            let i = (lz * 16 + lx) as usize;
            crack[i] = crack[i].max(deep);
        }
    }

    // each of the chunk's columns: its new blocks, from `lo` up
    let mut out: Vec<Option<(i32, Vec<u16>, i32)>> = vec![None; 256];
    let mut deposit = [0u8; 256];
    let mut slid_any = false;
    let mut slid = [false; 256];
    let mut events: Vec<(u8, [f64; 3])> = Vec::new();
    for lz in 0..16 {
        for lx in 0..16 {
            let (gx, gz) = (lx + QK_M, lz + QK_M);
            let j = gi(gx, gz);
            let Some(lk) = looks[j] else { continue };
            if lk.g == i32::MIN {
                continue;
            }
            let (x, z) = (x0 + lx, z0 + lz);
            let (u, d) = geo.ud(f64::from(x) + 0.5, f64::from(z) + 0.5);
            let ii = geo.ii_ud(u, d);
            // the slip: where this column's land came from, and how far up
            let (h, v) = geo.slip(u, d);
            let (sx, sz) = ((h * geo.st[0]).round() as i32, (h * geo.st[1]).round() as i32);
            let (vx, vy) = (gx - sx, gz - sz);
            let src = if (0..QK_W).contains(&vx) && (0..QK_W).contains(&vy) && looks[gi(vx, vy)].is_some() { gi(vx, vy) } else { j };
            let mut v = v.round() as i32;
            let on_trace = quake.ruptures() && d.abs() < 0.8 && u.abs() < quake.half * 0.95;
            // (the scarp's crest tumbles: a block lower on its edge, and that earth heaped at its foot)
            if quake.fault == Fault::Reverse && v > 0 && d > -1.6 {
                v -= 1;
            }
            let sl = looks[src].unwrap_or(lk);
            if lk.locked || sl.locked || (src != j && sl.g == i32::MIN) {
                continue;
            }
            let lo = grid.y0[j];
            let hi = (lk.top.max(if sl.top == i32::MIN { lk.top } else { sl.top + v }) + 2).max(lo);
            crate::meter::spend(1_200 + 60 * (hi - lo + 1) as u64);
            let mut col: Vec<u16> = (lo..=hi).map(|y| grid.at(src, y - v).or_else(|| grid.at(j, y)).unwrap_or(AIR)).collect();
            let at = |y: i32| (y - lo) as usize;
            // the column's ground and what stands on it, now
            let nl = look(&col, lo, cur);
            if nl.g == i32::MIN {
                continue;
            }
            let g = nl.g;
            let r = |k: u32| chance(x, z, seed, k);
            // the trace itself: an en-echelon fissure (strike-slip), a heap of earth at the scarp's foot (reverse)
            if on_trace && quake.fault == Fault::StrikeSlip {
                let seg = ((u + 3.0 * (geo.seed % 7) as f64) / 7.0).floor() as i32;
                if chance(seg, 0, seed, 60) < 0.78 {
                    let deep = 2 + (chance(seg, 1, seed, 61) * 4.0) as i32;
                    for y in (g - deep + 1).max(lo)..=g {
                        if cur.pal(col[at(y)]).class == 2 {
                            col[at(y)] = AIR;
                        }
                    }
                    for y in g + 1..=hi {
                        if matches!(cur.pal(col[at(y)]).class, 5 | 6) {
                            col[at(y)] = AIR;
                        }
                    }
                    w.torn += 1;
                    if (u.round() as i64).rem_euclid(24) == 0 {
                        events.push((4, [f64::from(x) + 0.5, f64::from(g), f64::from(z) + 0.5]));
                    }
                }
            } else if quake.ruptures() && d.abs() < 2.2 && quake.fault == Fault::StrikeSlip && r(62) < 0.35 && u.abs() < quake.half * 0.9 {
                // (mole tracks: the ground rucked up beside it)
                if g + 1 <= hi && col[at(g + 1)] == AIR {
                    col[at(g + 1)] = if r(63) < 0.5 { COARSE } else { DIRT };
                }
            } else if quake.fault == Fault::Reverse && (0.0..1.8).contains(&d) && u.abs() < quake.half * 0.9 && quake.slip_v > 0 {
                let heap = if d < 0.9 { 2 } else { 1 };
                for y in g + 1..=(g + heap).min(hi) {
                    if col[at(y)] == AIR {
                        col[at(y)] = if r(64) < 0.4 { GRAVEL } else { COARSE };
                    }
                }
                if d < 0.9 && (u.round() as i64).rem_euclid(24) == 0 {
                    events.push((4, [f64::from(x) + 0.5, f64::from(g), f64::from(z) + 0.5]));
                }
            }
            // ground cracks
            let c = crack[(lz * 16 + lx) as usize];
            if c > 0 && nl.built == 0 && !nl.wet {
                for y in (g - i32::from(c) + 1).max(lo)..=g {
                    if cur.pal(col[at(y)]).class == 2 {
                        col[at(y)] = AIR;
                    }
                }
                if g + 1 <= hi && matches!(cur.pal(col[at(g + 1)]).class, 5) {
                    col[at(g + 1)] = AIR;
                }
                w.cracked += 1;
            }
            // what stands on it: the buildings by their house (a cell of 6 by 6), the walls, the glass
            let cell = chance(x.div_euclid(6), z.div_euclid(6), seed, 70);
            let wood = (g + 1..=hi).filter(|&y| cur.pal(col[at(y)]).wood).count();
            let built = (g + 1..=hi).filter(|&y| cur.pal(col[at(y)]).class == 7).count();
            let house = built >= 2;
            let wooden = wood * 2 >= built.max(1);
            let p_fall = if wooden { ramp(&[(5.5, 0.0), (6.0, 0.15), (6.5, 0.6), (6.8, 0.85), (7.0, 0.95)], ii) } else { ramp(&[(6.0, 0.0), (6.5, 0.25), (6.8, 0.45), (7.0, 0.6)], ii) };
            let p_roof = ramp(&[(5.0, 0.0), (5.5, 0.2), (6.0, 0.5), (6.5, 0.8), (7.0, 1.0)], ii);
            let p_glass = ramp(&[(4.5, 0.0), (5.0, 0.15), (5.5, 0.35), (6.0, 0.6), (6.5, 0.85), (7.0, 1.0)], ii);
            let p_wall = ramp(&[(5.0, 0.0), (5.5, 0.3), (6.0, 0.6), (6.5, 0.85), (7.0, 1.0)], ii);
            if house && cell < p_fall {
                // collapsed: its floors pancaked onto its ground floor, a third of what it was, the glass and the light
                // things gone; now and then a fire breaks out in the wreck
                let kept: Vec<u16> = (g + 1..=hi).map(|y| col[at(y)]).filter(|&b| {
                    let p = cur.pal(b);
                    p.class >= 3 && p.class != 4 && p.class != 5 && p.class != 6 && !p.glass
                }).collect();
                for y in g + 1..=hi {
                    col[at(y)] = AIR;
                }
                let mut y = g + 1;
                for (i, &b) in kept.iter().enumerate() {
                    if i % 3 == 0 && y <= hi {
                        col[at(y)] = b;
                        y += 1;
                    }
                }
                if wooden && w.fires < quake.fires && y <= hi && r(71) < 0.035 {
                    col[at(y)] = FIRE;
                    w.fires += 1;
                    events.push((3, [f64::from(x) + 0.5, f64::from(y), f64::from(z) + 0.5]));
                }
                w.collapsed += 1;
                if r(72) < 0.05 {
                    events.push((1, [f64::from(x) + 0.5, f64::from(g + 2), f64::from(z) + 0.5]));
                }
            } else {
                // standing: the roof's top blocks shaken off, glass broken, block walls down to their footing
                if house && cell < p_fall + p_roof * 0.5 && r(73) < p_roof {
                    let mut gone = 0;
                    for y in (g + 1..=hi).rev() {
                        let p = cur.pal(col[at(y)]);
                        if p.class == 0 {
                            continue;
                        }
                        if p.class != 7 || gone >= 1 + (r(74) * 2.0) as i32 {
                            break;
                        }
                        col[at(y)] = AIR;
                        gone += 1;
                    }
                }
                let mut wall = false;
                for y in g + 1..=hi {
                    let p = cur.pal(col[at(y)]);
                    if p.glass && chance(x, z.wrapping_add(y * 31), seed, 75) < p_glass {
                        col[at(y)] = AIR;
                        if r(76) < 0.03 {
                            events.push((2, [f64::from(x) + 0.5, f64::from(y), f64::from(z) + 0.5]));
                        }
                    } else if p.wall && y > g + 1 && cell < p_wall {
                        col[at(y)] = AIR;
                        wall = true;
                    }
                }
                if wall {
                    w.walls += 1;
                    if g + 2 <= hi && col[at(g + 2)] == AIR && r(77) < 0.5 {
                        col[at(g + 2)] = COBBLE;
                    }
                }
            }
            // liquefaction: soft ground near water boils up sand and mud, and what stands on it sinks a block
            let soft = cur.pal(col[at(g)]).soft;
            let p_liq = ramp(&[(5.0, 0.0), (5.5, 0.3), (6.0, 0.6), (7.0, 0.9)], ii) * quake.liquefy;
            if soft && !nl.wet && wet[j] <= 4 && chance(x.div_euclid(4), z.div_euclid(4), seed, 80) < p_liq * 0.2 {
                let boil = r(81) < 0.3;
                if house {
                    // (sunk and tilted a little: the ground floor down a block into the mud)
                    for y in g..hi {
                        col[at(y)] = col[at(y + 1)];
                    }
                    col[at(hi)] = AIR;
                    if g - 1 >= lo {
                        col[at(g - 1)] = MUD;
                    }
                } else if boil {
                    col[at(g)] = if r(82) < 0.6 { SAND } else { MUD };
                    if g + 1 <= hi && cur.pal(col[at(g + 1)]).class == 5 {
                        col[at(g + 1)] = AIR;
                    }
                }
                w.liquefied += 1;
            }
            // landslides: a steep slope's skin let go in patches (8 by 8), what stood on it with it
            let p_slide = ramp(&[(5.5, 0.0), (6.0, 0.25), (6.5, 0.6), (7.0, 0.9)], ii) * quake.slides;
            let g0 = ground(j);
            let lowest = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .filter_map(|&(ox, oz)| {
                    let jj = gi(gx + ox, gz + oz);
                    let gg = ground(jj);
                    (gg != i32::MIN).then_some((gg, ox, oz))
                })
                .min();
            if let Some((gl, _, _)) = lowest
                && g0 - gl >= 3
                && !house
                && !nl.wet
                && chance(x.div_euclid(8), z.div_euclid(8), seed, 90) < p_slide
            {
                let strip = (g0 - gl - 1).clamp(2, 5);
                for y in (g - strip + 1).max(lo)..=hi {
                    let p = cur.pal(col[at(y)]);
                    if y > g || p.class == 2 {
                        col[at(y)] = AIR;
                    }
                }
                slid[(lz * 16 + lx) as usize] = true;
                slid_any = true;
                w.slid += 1;
                // (its earth runs down the slope, at most six columns, and heaps where it stops)
                let (mut px, mut pz) = (gx, gz);
                for _ in 0..6 {
                    let here = ground(gi(px, pz));
                    let next = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                        .iter()
                        .map(|&(ox, oz)| (px + ox, pz + oz))
                        .filter(|&(ax, az)| (QK_M..QK_M + 16).contains(&ax) && (QK_M..QK_M + 16).contains(&az))
                        .map(|(ax, az)| (ground(gi(ax, az)), ax, az))
                        .filter(|t| t.0 != i32::MIN)
                        .min();
                    match next {
                        Some((gg, ax, az)) if gg < here => {
                            px = ax;
                            pz = az;
                        }
                        _ => break,
                    }
                }
                let i = ((pz - QK_M) * 16 + px - QK_M) as usize;
                deposit[i] = deposit[i].saturating_add((strip as u8 + 1) / 2);
                if r(91) < 0.04 {
                    events.push((5, [f64::from(x) + 0.5, f64::from(g), f64::from(z) + 0.5]));
                }
            }
            out[(lz * 16 + lx) as usize] = Some((lo, col, g));
        }
    }
    // the slides' earth heaped where it came to rest
    for i in 0..256 {
        let n = deposit[i];
        if n == 0 || slid[i] {
            continue;
        }
        if let Some((lo, col, _)) = &mut out[i] {
            let lo = *lo;
            let nl = look(col, lo, cur);
            if nl.g == i32::MIN || nl.built > 0 {
                continue;
            }
            let (x, z) = (x0 + (i % 16) as i32, z0 + (i / 16) as i32);
            for k in 1..=i32::from(n.min(4)) {
                let y = nl.g + k;
                let idx = (y - lo) as usize;
                if idx < col.len() && matches!(cur.pal(col[idx]).class, 0 | 5) {
                    col[idx] = *[COARSE, DIRT, GRAVEL, ROOTED].get((chance(x, z, seed, 92 + k as u32) * 4.0) as usize).unwrap_or(&COARSE);
                }
            }
        }
    }
    // what the sea will find here, for a quake that raised a tsunami (strike/qk_sea.rs)
    if let Some(f) = &w.flood
        && f.borrow().covers(cx, cz)
    {
        let mut cols = Box::new([qk_sea::FCol::default(); 256]);
        for (i, fc) in cols.iter_mut().enumerate() {
            let j = gi((i % 16) as i32 + QK_M, (i / 16) as i32 + QK_M);
            let (lo, stack): (i32, &[u16]) = match &out[i] {
                Some((lo, col, _)) => (*lo, col),
                None if grid.y0[j] != i32::MIN => (grid.y0[j], &grid.blocks[j * per..(j + 1) * per]),
                None => continue,
            };
            *fc = sea_col(stack, lo, cur);
        }
        f.borrow_mut().add(cx, cz, cols);
    }
    // set against what was there: only what changed goes in
    let mut b = Blocks { clear_of_player: true, ..Default::default() };
    for i in 0..256 {
        let Some((lo, col, _)) = &out[i] else { continue };
        let (lx, lz) = ((i % 16) as i32, (i / 16) as i32);
        let j = gi(lx + QK_M, lz + QK_M);
        let (x, z) = (x0 + lx, z0 + lz);
        crate::meter::spend(40 * col.len() as u64);
        let mut y = 0usize;
        while y < col.len() {
            let yy = *lo + y as i32;
            let old = grid.at(j, yy).unwrap_or(AIR);
            if old == col[y] || cur.pal(old).name == cur.pal(col[y]).name {
                y += 1;
                continue;
            }
            let id = col[y];
            let differs = |k: usize| {
                let o = grid.at(j, *lo + k as i32).unwrap_or(AIR);
                o != id && cur.pal(o).name != cur.pal(id).name
            };
            let mut e = y;
            while e + 1 < col.len() && col[e + 1] == id && differs(e + 1) {
                e += 1;
            }
            b.put(x, z, yy, *lo + e as i32, cur.pal(id).name, None, false);
            y = e + 1;
        }
    }
    let t = (due.max(0.2)) as f32;
    if !b.runs.is_empty() {
        w.changed += 1;
        q.ev.push((t, Act::Blocks(Box::new(b))));
    }
    // what the slides left hanging (leaves whose trunk went, plants whose ground went)
    if slid_any {
        let s = slid;
        let tb = tidy(x0, z0, x0 + 15, z0 + 15, 14, 2, move |x, z| {
            let (lx, lz) = (x - x0, z - z0);
            if (0..16).contains(&lx) && (0..16).contains(&lz) && s[(lz * 16 + lx) as usize] { Col::Dug } else { Col::Kept }
        });
        if !tb.runs.is_empty() {
            q.ev.push((t + 0.15, Act::Blocks(Box::new(tb))));
        }
    }
    // the sounds and dust of it, where it happened (a few a chunk)
    events.truncate(4);
    for (kind, [x, y, z]) in events {
        let (event, vol) = match kind {
            1 => (format!("orbital:supervolcano.splinter{}", 1 + (chance(x as i32, z as i32, seed, 100) * 3.0) as u32 % 3), 2.5),
            2 => ("minecraft:block.glass.break".to_string(), 1.6),
            3 => ("minecraft:block.fire.ambient".to_string(), 1.5),
            4 => (format!("orbital:sinkhole.crack{}", 1 + (chance(x as i32, z as i32, seed, 101) * 3.0) as u32 % 3), 3.0),
            _ => (format!("orbital:supervolcano.rockfall{}", 1 + (chance(x as i32, z as i32, seed, 102) * 3.0) as u32 % 3), 3.0),
        };
        q.ev.push((t, Act::Sound { event, at: Some([x, y, z]), volume: vol, pitch: 0.9 + 0.2 * chance(x as i32, z as i32, seed, 103) as f32 }));
        if kind == 1 || kind == 5 || kind == 4 {
            let dust = if kind == 1 { "minecraft:campfire_cosy_smoke" } else { "minecraft:poof" };
            q.ev.push((t + 0.05, Act::Cmd(format!("particle {dust} {x:.1} {:.1} {z:.1} 2 1 2 0.02 12 force", y + 1.0))));
        }
    }
    true
}

/// The next answer in, its palette sorted out: a retry's if one is out and in, else the next part's.
fn next_answer(wk: &mut Work) -> Option<Cur> {
    if let Some((tag, _)) = &wk.retry_q
        && let Some(a) = crate::columns::take(*tag)
    {
        let (_, chunks) = wk.retry_q.take()?;
        let pal = a.palette.iter().map(|n| classify(n)).collect();
        return Some(Cur { a, pal, chunks, k: 0 });
    }
    if wk.taken < wk.asked
        && let Some(a) = crate::columns::take(wk.tags[wk.taken])
    {
        let part = wk.taken;
        wk.taken += 1;
        let pal = a.palette.iter().map(|n| classify(n)).collect();
        let (c0, c1) = (part * QK_PART, ((part + 1) * QK_PART).min(wk.chunks.len()));
        return Some(Cur { a, pal, chunks: wk.chunks[c0..c1].to_vec(), k: 0 });
    }
    None
}

/// One stage of the land's work: the reads asked a few at a time in the order the shaking reaches the chunks, their
/// answers taken in as they come and worked chunk by chunk, the chunks not loaded asked again.
fn qk_work(q: &mut Plan, w: Rc<RefCell<Work>>, now: f64) {
    let more = {
        let mut guard = w.borrow_mut();
        let wk = &mut *guard;
        while wk.asked < wk.tags.len() && wk.asked < wk.taken + 3 {
            let part = wk.asked;
            let (c0, c1) = (part * QK_PART, ((part + 1) * QK_PART).min(wk.chunks.len()));
            let mut columns = Vec::with_capacity((c1 - c0) * (QK_W * QK_W) as usize);
            for &(cx, cz, _) in &wk.chunks[c0..c1] {
                chunk_columns(cx, cz, &mut columns);
            }
            crate::meter::spend(40 * columns.len() as u64);
            q.ev.push((now as f32, Act::Read { tag: wk.tags[part], columns, below: QK_BELOW, above: QK_ABOVE }));
            wk.asked += 1;
        }
        while crate::meter::spent() < QK_STAGE {
            if wk.cur.is_none() {
                wk.cur = next_answer(wk);
            }
            let Some(mut cur) = wk.cur.take() else { break };
            while cur.k < cur.chunks.len() && crate::meter::spent() < QK_STAGE {
                let k = cur.k;
                cur.k += 1;
                if !qk_chunk(wk, &cur, k, q) {
                    wk.retry.push(cur.chunks[k]);
                }
            }
            if cur.k < cur.chunks.len() {
                wk.cur = Some(cur);
            }
        }
        if wk.retry_q.is_none() && !wk.retry.is_empty() && now >= wk.next_retry {
            if now > wk.land_by {
                crate::sdk::log::write(crate::sdk::log::Level::Info, &format!("earthquake: {} chunks of the land never loaded; left alone there", wk.retry.len()));
                wk.retry.clear();
            } else {
                let take = wk.retry.len().min(QK_PART * 2);
                let chunks: Vec<(i32, i32, f64)> = wk.retry.drain(..take).map(|(x, z, t)| (x, z, t.max(now + 0.3))).collect();
                let mut columns = Vec::new();
                for &(cx, cz, _) in &chunks {
                    chunk_columns(cx, cz, &mut columns);
                }
                let tag = crate::columns::new_tag();
                q.ev.push((now as f32, Act::Read { tag, columns, below: QK_BELOW, above: QK_ABOVE }));
                wk.retry_q = Some((tag, chunks));
                wk.next_retry = now + 0.6;
            }
        }
        let read = wk.taken == wk.tags.len() && wk.cur.is_none() && wk.retry.is_empty() && wk.retry_q.is_none();
        if read && !wk.done {
            wk.done = true;
            crate::sdk::log::write(
                crate::sdk::log::Level::Info,
                &format!(
                    "earthquake [{}]: {} chunks changed, {} columns of houses down, {} of walls, {} slid, {} cracked, {} liquefied, {} torn along the trace, {} fires",
                    wk.geo.q.key, wk.changed, wk.collapsed, wk.walls, wk.slid, wk.cracked, wk.liquefied, wk.torn, wk.fires
                ),
            );
        }
        !wk.done && now < wk.land_by + 4.0
    };
    if more {
        q.later((now + 0.05) as f32, move |q| qk_work(q, w, now + 0.05));
    }
}

/// The chunks the quake works on: those round the target where it shakes at 5- or more, in the order the strong
/// shaking reaches them (each with that time, a little spread).
fn qk_chunks(geo: &QkGeo) -> Vec<(i32, i32, f64)> {
    // (and with a tsunami, the coast it may reach: QT_HALF each way along it, QT_OUT out to sea, its reach inland)
    let sea = geo.q.tsunami.as_ref().map(|ts| ts.reach + 16.0);
    let r = sea.map_or(QK_REACH, |reach| QK_REACH.max(qk_sea::QT_HALF.hypot(reach.max(qk_sea::QT_OUT))));
    let in_sea = |u: f64, d: f64| sea.is_some_and(|reach| u.abs() <= qk_sea::QT_HALF + 8.0 && d >= -qk_sea::QT_OUT - 8.0 && d <= reach);
    let (tx, tz) = (geo.t[0], geo.t[2]);
    let (c0x, c1x) = (((tx - r) / 16.0).floor() as i32, ((tx + r) / 16.0).floor() as i32);
    let (c0z, c1z) = (((tz - r) / 16.0).floor() as i32, ((tz + r) / 16.0).floor() as i32);
    let mut out = Vec::new();
    for cz in c0z..=c1z {
        for cx in c0x..=c1x {
            let (mx, mz) = (f64::from(cx) * 16.0 + 8.0, f64::from(cz) * 16.0 + 8.0);
            let (u, d) = geo.ud(mx, mz);
            let shaken = (mx - tx).hypot(mz - tz) <= QK_REACH && (geo.ii_ud(u, d) >= 4.5 || (geo.q.ruptures() && d.abs() < 24.0 && u.abs() < geo.q.half));
            if !shaken && !in_sea(u, d) {
                continue;
            }
            let mut due = geo.strong_at(u, d) + 0.25 * f64::from(geo.q.main_dur()) * chance(cx, cz, geo.seed, 5);
            if geo.q.ruptures() && d.abs() < 24.0 {
                due = due.min(geo.trace_at(u) + 0.15);
            }
            out.push((cx, cz, due));
        }
    }
    out.sort_by(|a, b| a.2.total_cmp(&b.2));
    if out.len() > QK_MAX_CHUNKS {
        // (the furthest let go: those it shakes least)
        out.sort_by(|a, b| {
            let da = (f64::from(a.0) * 16.0 + 8.0 - tx).hypot(f64::from(a.1) * 16.0 + 8.0 - tz);
            let db = (f64::from(b.0) * 16.0 + 8.0 - tx).hypot(f64::from(b.1) * 16.0 + 8.0 - tz);
            da.total_cmp(&db)
        });
        out.truncate(QK_MAX_CHUNKS);
        out.sort_by(|a, b| a.2.total_cmp(&b.2));
    }
    out
}

/// The chunks as forceload boxes: a run of a row's chunks each.
fn qk_load_boxes(chunks: &[(i32, i32, f64)]) -> Vec<[i32; 4]> {
    let mut by_row: std::collections::BTreeMap<i32, Vec<i32>> = Default::default();
    for &(cx, cz, _) in chunks {
        by_row.entry(cz).or_default().push(cx);
    }
    let mut out = Vec::new();
    for (cz, mut xs) in by_row {
        xs.sort_unstable();
        let mut start = xs[0];
        let mut last = xs[0];
        for &x in xs.iter().skip(1).chain(std::iter::once(&i32::MAX)) {
            if x == last + 1 {
                last = x;
                continue;
            }
            out.push([start * 16, cz * 16, last * 16 + 15, cz * 16 + 15]);
            start = x;
            last = x;
        }
    }
    out
}

// ---- the plan ----------------------------------------------------------------------------------------------------------

/// The actionbar line, in a colour.
/// (short and not bold: the actionbar is one line across the middle of the screen, and a long one runs off its sides)
fn bar(text: &str, rgb: u32) -> String {
    format!(r##"title @s actionbar {{"text":"{text}","color":"#{rgb:06X}"}}"##)
}

/// An aftershock: when it sets out (strike seconds; the waves reach a place `QkGeo::arrive` later), its magnitude, how
/// much weaker its shaking is than the main shock's anywhere (Shindo), and how long it shakes (strike seconds).
#[derive(Clone, Copy, Debug)]
pub struct After {
    pub t: f64,
    pub mw: f64,
    pub drop: f64,
    pub dur: f64,
}
/// Most aftershocks the pass's table holds.
pub const QK_AFTER_MAX: usize = 16;

/// The aftershocks, from when the main shock's strong shaking is over to the strike's end `len`: as many as its size
/// brings, their times by Omori's law (the rate falling as 1 / (t + c), so most come early), their magnitudes by
/// Gutenberg and Richter's (b = 1, from 3 under the main shock), the largest about 1.2 under it (Båth's law) and among
/// the first few. Each one's Shindo anywhere is the main shock's less what its smaller size takes (the intensity's own
/// 1.1 a magnitude, both saturating past 8.3 as `lpgv` has it), give or take where along the fault it broke.
pub fn qk_aftershocks(q: &Quake, seed: u32, len: f64) -> Vec<After> {
    let t0 = f64::from(QK_T0) + q.pulses.iter().filter(|p| p.1 >= 0.4).map(|p| f64::from(p.0 + p.2)).fold(0.0, f64::max) + 3.0;
    let span = len - 4.0 - t0;
    if span < 4.0 || q.mw < 5.5 {
        return Vec::new();
    }
    // (a shallow M7 a handful in the strike's compressed time, a great M9 a dozen and more)
    let n = (((q.mw - 5.5) * 3.2) * (span / 40.0).clamp(0.6, 1.6)).round().clamp(2.0, QK_AFTER_MAX as f64) as usize;
    let h = |k: u32| mh(k, seed, 0xaf7e);
    let c = 3.0;
    let mut out: Vec<After> = (0..n as u32)
        .map(|k| {
            let t = t0 + c * ((1.0 + span / c).powf(h(k)) - 1.0);
            let mw = (q.mw - 3.0 - (1.0 - h(100 + k)).max(1e-6).log10()).min(q.mw - 1.0);
            After { t, mw, drop: 0.0, dur: 0.0 }
        })
        .collect();
    out.sort_by(|a, b| a.t.total_cmp(&b.t));
    // (the largest: Båth's 1.2 under the main shock, one of the first three)
    let big = (h(200) * 3.0) as usize % out.len().min(3);
    out[big].mw = q.mw - 1.2 + (h(201) - 0.5) * 0.4;
    for (k, a) in out.iter_mut().enumerate() {
        let m = |x: f64| x.min(8.3);
        a.drop = (1.1 * (m(q.mw) - m(a.mw)) + (h(300 + k as u32) - 0.35) * 0.6).max(0.1);
        a.dur = (1.5 + (a.mw - 5.0) * 1.8).clamp(1.5, 10.0);
    }
    out
}
/// How hard aftershock `a` shakes at (u, d) at strike second `t`, 0 to 1 (times `shake_of` its Shindo there).
fn after_env(a: &After, s_arr: f64, t: f64) -> f64 {
    pulse((t - a.t - s_arr) / a.dur)
}

/// A second of the shaking where the player is now: the jolts, being thrown about at 6+ and 7 (the knockback's switch),
/// and the actionbar's Shindo.
#[allow(clippy::too_many_arguments)]
fn qk_second(q: &mut Plan, geo: Rc<QkGeo>, t0: f32, me0: [f64; 3], sea: Option<qk_sea::SeaTime>, afters: Rc<Vec<After>>) {
    let me = crate::sdk::game::get_player().map_or(me0, |p| [p.x, p.y, p.z]);
    let quake = geo.q;
    let (u, d) = geo.ud(me[0], me[2]);
    let ii = geo.ii_ud(u, d);
    let (cls, words, rgb) = shindo_class(ii);
    let (arr_s, arr_p) = geo.arrive(u, d);
    let tp = f64::from(QK_T0) + arr_p;
    let ts = f64::from(QK_T0) + arr_s;
    // (the long rolling of a great quake in slower, longer heaves; a shallow one's in quick jolts; quicker still at 6+
    // and 7, where it throws things about)
    let (step, hold) = if quake.mw >= 8.0 { (0.42, 0.85) } else { (0.26, 0.5) };
    let step = step * (1.0 - 0.4 * smooth(5.8, 6.8, ii));
    // the main shock's shaking, and each aftershock's at its own (weaker) Shindo here
    let felt = |t: f64| -> (f64, f64) {
        let mut best = (geo.env(u, d, t), ii);
        for a in afters.iter() {
            let ia = ii - a.drop;
            let e = after_env(a, arr_s, t);
            if e * qk_violence(ia) > best.0 * qk_violence(best.1) {
                best = (e, ia);
            }
        }
        best
    };
    let mut t = f64::from(t0) + (q.rng.next() * step);
    let mut peak = 0.0f64;
    let mut thrown = false;
    while t < f64::from(t0) + 1.0 {
        let (e, i_now) = felt(t);
        let v = qk_violence(i_now);
        peak = peak.max(e);
        if e > 0.015 && v > 0.0 {
            let (j, h) = (q.rng.uniform(0.7, 1.15), q.rng.uniform(0.8, 1.2));
            q.shake(t as f32, (0.75 * v * e * j).min(1.0) as f32, (hold * h) as f32);
            // (from 6+ no one stays on their feet: knocked a step or two any way, at 7 off the ground)
            let hard = smooth(5.8, 7.0, i_now) * e;
            if !thrown && hard > 0.25 && q.rng.next() < 0.7 {
                thrown = true;
                let a = q.rng.next() * 2.0 * PI;
                let from = [me[0] - a.cos(), me[1], me[2] - a.sin()];
                q.ev.push((t as f32, Act::Shove { from, out: (0.42 * hard * q.rng.uniform(0.6, 1.1)) as f32, up: (0.32 * hard * smooth(6.3, 7.0, i_now) * q.rng.uniform(0.6, 1.1)) as f32, reach: 8.0 }));
            }
        }
        t += step * q.rng.uniform(0.75, 1.25);
    }
    // the actionbar: the warning before the S waves (if one went out that day), the Shindo while it shakes; with a
    // tsunami, once the shaking is over, the JMA's warning: its countdown, the sea as the coast's gauge sees it
    let tf = f64::from(t0);
    let last = quake.pulses.iter().map(|p| f64::from(p.0) + f64::from(p.2)).fold(0.0f64, f64::max) + f64::from(QK_T0) + arr_s;
    let title = format!("M{:.1}", quake.mw);
    let shaking = tf + 1.0 >= tp && tf <= last && (peak > 0.04 || tf + 1.0 >= ts) && ii >= 0.5;
    let shindo = || {
        let est = if quake.estimated { " (est.)" } else { "" };
        let w = if words == cls { String::new() } else { format!(" ({words})") };
        bar(&format!("SHINDO {cls}{w}{est}  ·  {title}"), rgb)
    };
    // (the JMA's warning and its countdown, nothing else: the sea drawing back is seen, not announced)
    let tsunami = quake.tsunami.as_ref().zip(sea).filter(|(_, st)| tf >= st.warn - 0.01 && tf < st.end).map(|(tsu, st)| {
        let (kind, col) = qk_sea::warning(tsu);
        bar(&format!("{kind}  ·  {}  ·  wave in {}", tsu.expect, qk_sea::mmss(st.arrive - tf)), col)
    });
    // an aftershock shaking here now (felt from Shindo 1), with its warning first if it is strong
    let after = afters.iter().find_map(|a| {
        let ia = ii - a.drop;
        let (s0, s1) = (a.t + arr_s, a.t + arr_s + a.dur * 0.9);
        if ia < 0.5 || tf + 1.0 < a.t + arr_p || tf > s1 {
            return None;
        }
        let (c, _, rgb) = shindo_class(ia);
        Some(if quake.eew && ia >= 4.5 && tf + 1.0 < s0 {
            bar(&format!("EARTHQUAKE EARLY WARNING  ·  SHINDO {c}"), 0xFF2800)
        } else {
            bar(&format!("AFTERSHOCK  ·  SHINDO {c}  ·  M{:.1}", a.mw), rgb)
        })
    });
    let line = if quake.eew && tf >= f64::from(QK_T0) + 1.0 && tf < ts && ii >= 3.5 {
        Some(bar(&format!("EARTHQUAKE EARLY WARNING  ·  SHINDO {cls}"), 0xFF2800))
    } else if let Some(a) = after.filter(|_| !shaking || peak < 0.2) {
        Some(a)
    } else if shaking && (tsunami.is_none() || (t0 as i32) % 3 != 2) {
        // (while it still shakes after the tsunami warning went out, the two take turns)
        Some(shindo())
    } else {
        tsunami.or_else(|| shaking.then(shindo))
    };
    if let Some(l) = line {
        q.ev.push((t0, Act::Cmd(l)));
    }
}

/// The sounds where the player stands: the Earthquake Early Warning's chime (`quake.eew`) as a phone and a television
/// sound it, the P waves' rumble, each subevent's roar through the ground, the creak of timber while it shakes hard;
/// the rupture's cracks along the trace where it breaks; each aftershock's rumble, the strong ones warned of again.
fn qk_sounds(p: &mut Plan, geo: &QkGeo, me: [f64; 3], afters: &[After], poles: &[[f64; 2]], siren: f32) {
    let quake = geo.q;
    let (u, d) = geo.ud(me[0], me[2]);
    let ii = geo.ii_ud(u, d);
    let loud = ((ii - 1.0) / 5.5).clamp(0.08, 1.0) as f32;
    let (s, pw) = geo.arrive(u, d);
    let deep = (1.05 - (quake.mw - 6.8) * 0.1).clamp(0.72, 1.0) as f32;
    let t0 = f64::from(QK_T0);
    // (the P waves: a deep rumble like a train coming)
    p.ev.push(((t0 + pw) as f32, Act::Sound { event: "orbital:sinkhole.rumble".into(), at: None, volume: 0.45 * loud, pitch: deep }));
    if quake.eew {
        p.ev.push(((t0 + 1.0) as f32, Act::Sound { event: "orbital:quake.eew".into(), at: None, volume: 0.85, pitch: 1.0 }));
    }
    // the aftershocks
    for a in afters {
        let ia = ii - a.drop;
        if ia < 0.5 {
            continue;
        }
        let l = ((ia - 1.0) / 5.5).clamp(0.06, 1.0) as f32;
        let pitch = (1.05 - (a.mw - 6.8) * 0.1).clamp(0.72, 1.1) as f32;
        if quake.eew && ia >= 4.5 {
            p.ev.push(((a.t + 0.6) as f32, Act::Sound { event: "orbital:quake.eew".into(), at: None, volume: 0.85, pitch: 1.0 }));
            if ia >= 5.0 {
                qk_sea::eew_alert(p, poles, geo.t[1], me, a.t + 0.8, siren, geo.seed ^ (a.t as u32));
            }
        }
        p.ev.push(((a.t + pw) as f32, Act::Sound { event: "orbital:sinkhole.rumble".into(), at: None, volume: 0.4 * l, pitch }));
        p.ev.push(((a.t + s) as f32, Act::Sound { event: "orbital:supervolcano.tremor".into(), at: None, volume: (0.9 * l).min(1.0), pitch }));
        if ia >= 5.0 {
            p.ev.push(((a.t + s + 0.1) as f32, Act::Sound { event: "orbital:supervolcano.seism".into(), at: None, volume: (0.5 * l).min(1.0), pitch }));
        }
        if ia >= 4.0 {
            let k = 1 + (p.rng.next() * 4.0) as u32 % 4;
            p.ev.push(((a.t + s + 0.4) as f32, Act::Sound { event: format!("orbital:supervolcano.creak{k}"), at: None, volume: (0.45 * l).min(1.0), pitch: 0.9 }));
        }
    }
    for (k, &(tk, a, dur)) in quake.pulses.iter().enumerate() {
        let start = t0 + s + f64::from(tk);
        let v = (a * loud).min(1.0);
        p.ev.push((start as f32, Act::Sound { event: "orbital:supervolcano.tremor".into(), at: None, volume: v, pitch: deep }));
        // (a long one's roar kept up)
        let mut tt = 0.0;
        let mut n = 0;
        while tt < f64::from(dur) * 0.9 {
            let ev = ["orbital:sinkhole.rumble", "orbital:skybreak.rumble", "orbital:supervolcano.tremor"][(k + n) % 3];
            p.ev.push(((start + tt + 0.3) as f32, Act::Sound { event: ev.into(), at: None, volume: (v * if n == 0 { 0.8 } else { 0.6 }).min(1.0), pitch: deep * (0.95 + 0.1 * p.rng.next() as f32) }));
            tt += 11.0;
            n += 1;
        }
        if a > 0.5 {
            p.ev.push(((start + 0.1) as f32, Act::Sound { event: "orbital:supervolcano.seism".into(), at: None, volume: (0.6 * v).min(1.0), pitch: deep }));
        }
    }
    // (timber creaking and things rattling while it shakes hard where they are)
    if ii >= 4.0 {
        let mut t = t0 + s;
        let end = t0 + s + quake.pulses.iter().map(|p| f64::from(p.0 + p.2)).fold(0.0, f64::max);
        while t < end {
            let e = geo.env(u, d, t);
            if e > 0.3 {
                let k = 1 + (p.rng.next() * 4.0) as u32 % 4;
                p.ev.push((t as f32, Act::Sound { event: format!("orbital:supervolcano.creak{k}"), at: None, volume: (0.5 * e as f32 * loud).min(1.0), pitch: 0.85 + 0.3 * p.rng.next() as f32 }));
                if ii >= 5.5 && p.rng.next() < 0.35 {
                    let g = 1 + (p.rng.next() * 2.0) as u32 % 2;
                    p.ev.push(((t + 0.3) as f32, Act::Sound { event: format!("orbital:supervolcano.grind{g}"), at: None, volume: (0.4 * e as f32 * loud).min(1.0), pitch: 0.9 }));
                }
            }
            t += p.rng.uniform(0.6, 1.6);
        }
    }
    // (the trace breaking, heard where it breaks)
    if quake.ruptures() {
        let mut u = -quake.half * 0.95;
        while u <= quake.half * 0.95 {
            let x = geo.t[0] + u * geo.st[0];
            let z = geo.t[2] + u * geo.st[1];
            let k = 1 + (p.rng.next() * 3.0) as u32 % 3;
            p.ev.push((geo.trace_at(u) as f32, Act::Sound { event: format!("orbital:sinkhole.crack{k}"), at: Some([x, geo.t[1], z]), volume: 4.0, pitch: 0.8 + 0.3 * p.rng.next() as f32 }));
            u += 20.0;
        }
    }
}

/// The tsunami's warnings and sounds where the player is, until the quake hands over to the Tsunami: the JMA's warning
/// as a title as it goes out (only then: the sea drawing back is seen, not announced), NHK's alert tone (`quake.nhk_alert`, the player's own
/// recording, git-ignored: none without it) with a soft chime, the coast's sirens from the poles (`qk_sea::sirens`), the
/// sea's hiss as it withdraws.
#[allow(clippy::too_many_arguments)]
fn qk_sea_warnings(p: &mut Plan, geo: &QkGeo, tsu: &qk_sea::Tsu, st: &qk_sea::SeaTime, me: [f64; 3], poles: &[[f64; 2]], cfg: &QkConfig) {
    let (kind, col) = qk_sea::warning(tsu);
    let title = |p: &mut Plan, t: f64, head: &str, sub: &str, rgb: u32| {
        let t = t as f32;
        p.at(t, "title @s times 10 70 20");
        p.at(t, format!(r##"title @s subtitle {{"text":"{sub}","color":"#{rgb:06X}"}}"##));
        p.at(t, format!(r##"title @s title {{"text":"{head}","color":"#{rgb:06X}"}}"##));
    };
    // (a title is drawn four times the size: its line short, the rest in the subtitle under it)
    let major = if kind.starts_with("MAJOR") { "Major warning" } else { "Warning" };
    title(p, st.warn, "TSUNAMI", &format!("{major}  ·  {}  ·  get to high ground", tsu.expect), col);
    let alert = |p: &mut Plan, t: f64, vol: f32| p.ev.push((t as f32, Act::Sound { event: "orbital:quake.nhk_alert".into(), at: None, volume: vol, pitch: 1.0 }));
    alert(p, st.warn, 0.9);
    for (n, pitch) in [1.5, 1.26, 1.0].into_iter().enumerate() {
        p.sound((st.warn + 0.22 * n as f64) as f32, "block.note_block.chime", 3.0, pitch);
    }
    let major = kind.starts_with("MAJOR");
    qk_sea::sirens(p, poles, geo.t[1], me, st.warn + 1.5, st.end + 0.4, major, cfg.siren, geo.seed);
    let (_, d) = geo.ud(me[0], me[2]);
    let near = (1.0 - d.max(0.0) / (1.5 * tsu.reach)).clamp(0.15, 1.0) as f32;
    p.ev.push((st.draw as f32, Act::Sound { event: "orbital:tsunami.rumble_after".into(), at: None, volume: 0.35 * near, pitch: 0.8 }));
}

/// Where the pass's table puts the aftershocks (a count, then each one's start, Shindo drop and length) and the sea's
/// drawing back (its start, the first wave's arrival, how far down, how fast it runs, the hand-over); earthquake.wgsl's
/// `QK_TAB_*`: keep in step.
pub const QK_TAB_AFTER: usize = 0;
pub const QK_TAB_SEA: usize = 52;
fn qk_table(q: &Quake, afters: &[After], sea: Option<qk_sea::SeaTime>) -> Vec<u32> {
    let mut w = vec![0f32; QK_TAB_SEA + 6];
    let n = afters.len().min(QK_AFTER_MAX);
    w[QK_TAB_AFTER] = n as f32;
    for (k, a) in afters.iter().take(n).enumerate() {
        let i = QK_TAB_AFTER + 1 + 3 * k;
        w[i] = a.t as f32;
        w[i + 1] = a.drop as f32;
        w[i + 2] = a.dur as f32;
    }
    if let (Some(ts), Some(st)) = (q.tsunami.as_ref(), sea) {
        w[QK_TAB_SEA..QK_TAB_SEA + 6].copy_from_slice(&[st.draw as f32, st.arrive as f32, ts.draw as f32, ts.speed as f32, st.end as f32, qk_sea::SEA as f32]);
    }
    w.iter().map(|f| f.to_bits()).collect()
}

/// What the player has set for the next Earthquake (lib.rs's `fire_at`, through `qk_configure`): the tsunami's
/// countdown and the sirens' volume.
#[derive(Clone, Copy, Debug)]
pub struct QkConfig {
    pub countdown: f64,
    pub siren: f32,
}
thread_local! {
    static CONFIG: std::cell::Cell<QkConfig> = const { std::cell::Cell::new(QkConfig { countdown: qk_sea::QT_COUNTDOWN, siren: 1.0 }) };
    static CUSTOM: std::cell::Cell<Option<&'static Quake>> = const { std::cell::Cell::new(None) };
}
pub fn qk_configure(c: QkConfig) {
    CONFIG.with(|x| x.set(c));
}
fn config() -> QkConfig {
    CONFIG.with(std::cell::Cell::get)
}

/// The custom earthquake's index, after the presets.
pub const QK_CUSTOM: usize = QK_PRESETS.len();
/// The earthquake `i`: a preset, or the custom one (`qk_set_custom`).
pub fn qk_quake(i: usize) -> &'static Quake {
    if i >= QK_CUSTOM {
        // (a shallow M7.0 until the player sets one)
        return CUSTOM.with(std::cell::Cell::get).unwrap_or_else(|| qk_set_custom(7.0, 10.0, None));
    }
    &QK_PRESETS[i]
}

/// A custom earthquake from a moment magnitude, its depth (km) and the highest Shindo over it (instrumental, 0 to 7; none:
/// worked out from the other two), its other traits as such a quake has them: a megathrust (offshore, a tsunami) from
/// Mw 7.6 at 60 km or shallower, a fault breaking the surface when shallower than 15 km, otherwise buried; its rupture as
/// long as Wells and Coppersmith's (1994) regression has it (log L = 0.59 Mw - 2.44), as many subevents as its size,
/// its tsunami's height by its magnitude and depth. Made once for each set of numbers and kept.
pub fn qk_set_custom(mw: f64, depth: f64, shindo: Option<f64>) -> &'static Quake {
    let mw = mw.clamp(5.0, 9.6);
    let depth = depth.clamp(1.0, 700.0);
    let shindo = shindo.unwrap_or_else(|| {
        // (the near-field intensity a quake like it brings, close over a shallow one, far less over a deep one)
        let x = depth.max(3.0);
        // (with soft ground's amplification over stiff ground's, about 3.5 times, where people live)
        (2.68 + 1.72 * (0.58 * mw.min(8.3) - 1.29 - (x + 0.0028 * 10f64.powf(0.5 * mw.min(8.3))).log10() - 0.002 * x + 0.55)).clamp(1.0, 7.0)
    });
    let shindo = shindo.clamp(0.5, 7.0);
    let mega = mw >= 7.6 && depth <= 60.0;
    let fault = if mega {
        Fault::Megathrust
    } else if depth < 15.0 {
        Fault::StrikeSlip
    } else {
        Fault::Megathrust
    };
    let kpb = (0.15 * 10f64.powf((mw - 6.9) * 0.33)).clamp(0.08, 0.9);
    let len_km = 10f64.powf(0.59 * mw - 2.44);
    let half = (len_km / 2.0 / kpb).clamp(60.0, 520.0);
    // (its rupture's real seconds, sped up as the presets are)
    let dur = ((len_km / 2.5) / 3.0).clamp(5.0, 90.0) as f32;
    let n = (1.0 + (mw - 6.5) * 1.5).clamp(1.0, 4.0) as usize;
    let shares = [0.7f32, 1.0, 0.65, 0.5];
    let mut pulses: Vec<(f32, f32, f32)> = (0..n).map(|i| (dur * i as f32 / n as f32, if n == 1 { 1.0 } else { shares[i] }, (dur / n as f32).max(5.0) * 1.3)).collect();
    pulses.push((dur + 10.0, 0.2, 4.0));
    let length = 2.0 + dur + 26.0;
    let slip = if fault == Fault::StrikeSlip { 10f64.powf(0.5 * mw - 3.2).round().clamp(0.0, 5.0) as i32 } else { 0 };
    let tsunami = mega.then(|| {
        let h = (2.0 * 10f64.powf(0.5 * (mw - 7.5)) * (1.0 - depth / 80.0)).clamp(0.5, 30.0);
        let expect: &'static str = match h {
            h if h > 10.0 => "10 m+",
            h if h > 5.0 => "10 m",
            h if h > 3.0 => "5 m",
            h if h > 1.0 => "3 m",
            _ => "1 m",
        };
        let hf = h as f32;
        let waves: &'static [(f32, f32, f32, f32, f32)] = Box::leak(vec![(0.0, 0.35 * hf, 10.0, 3.0, 12.0), (26.0, hf, 14.0, 6.0, 20.0), (64.0, 0.55 * hf, 10.0, 4.0, 14.0)].into_boxed_slice());
        Tsu { expect, draw: (0.6 * h).clamp(1.0, 8.0), waves, speed: 11.0, reach: (20.0 * h).clamp(60.0, 260.0) }
    });
    let name: &'static str = Box::leak(format!("Custom M{mw:.1}").into_boxed_str());
    let date: &'static str = Box::leak(format!("{depth:.0} km deep").into_boxed_str());
    let q = Box::leak(Box::new(Quake {
        key: "custom",
        button: "CUSTOM",
        name,
        date,
        aka: None,
        mw,
        fault,
        shindo,
        estimated: false,
        depth: if fault == Fault::StrikeSlip { depth.min(15.0) } else { depth },
        kpb,
        half,
        hypo: -0.3,
        slip_h: slip,
        slip_v: slip / 2,
        pulses: Box::leak(pulses.into_boxed_slice()),
        tsunami,
        eew: mw >= 6.0,
        fires: ((mw - 6.0) * 6.0).clamp(0.0, 30.0) as u32,
        slides: 1.0,
        liquefy: 1.0,
        length,
        blurb: "Your own earthquake: /orbital quake custom MAGNITUDE DEPTH [SHINDO].",
    }));
    let q: &'static Quake = q;
    CUSTOM.with(|c| c.set(Some(q)));
    q
}

/// The preset the pass draws earthquake `i` as (it knows the presets only): itself, or for the custom one the preset
/// most like it (the same kind of fault, the nearest magnitude).
pub fn qk_pass_preset(i: usize) -> usize {
    if i < QK_CUSTOM {
        return i;
    }
    let q = qk_quake(i);
    (0..QK_CUSTOM)
        .min_by(|&a, &b| {
            let cost = |k: usize| (QK_PRESETS[k].mw - q.mw).abs() + if QK_PRESETS[k].fault == q.fault { 0.0 } else { 10.0 };
            cost(a).total_cmp(&cost(b))
        })
        .unwrap_or(0)
}

/// The strike's whole length for earthquake `preset` with a tsunami countdown of `countdown` seconds.
pub fn qk_length(preset: usize, countdown: f64) -> f32 {
    qk_sea::qk_length(qk_quake(preset), countdown)
}

pub(super) fn quake_plan(laser: &'static Laser, target: &Target, terrain: bool, _dimension: &str, shader_seed: f32, me: [f64; 3], preset: usize) -> Vec<(f32, Act)> {
    let cfg = config();
    let countdown = cfg.countdown;
    let [bx, by, bz] = target.block;
    let seed = (bx as i64).wrapping_mul(73_856_093) ^ (bz as i64).wrapping_mul(19_349_663) ^ by as i64;
    let mut p = Plan { laser, ev: Vec::new(), rng: Rng::new(seed as u64), bx, by, bz };
    let geo = Rc::new(QkGeo::new(preset, target, me, shader_seed));
    let quake = geo.q;
    let sea = qk_sea::sea_time(quake, countdown);
    let len = qk_sea::qk_length(quake, countdown);

    let afters = Rc::new(qk_aftershocks(quake, geo.seed, f64::from(len)));
    // the shaking where the player is, worked out a second at a time from where they are then
    let mut t = 0.0f32;
    while t < len {
        let g = geo.clone();
        let a = afters.clone();
        let t0 = t;
        p.soon(t0, move |q| qk_second(q, g, t0, me, sea, a));
        t += 1.0;
    }
    let poles = qk_sea::siren_sites(&geo, me);
    qk_sounds(&mut p, &geo, me, &afters, &poles, cfg.siren);
    // what the pass reads: the aftershocks (its sway) and the sea drawing back (its wet bed)
    p.ev.push((0.0, Act::Table(qk_table(quake, &afters, sea))));
    // the coast's siren poles (built with the terrain on; heard either way), its earthquake alert from them as the warning
    // goes out, and with a tsunami its warnings and sirens
    if terrain {
        for (k, [x, z]) in poles.iter().enumerate() {
            p.ev.push((0.5 + 0.05 * k as f32, Act::Blocks(Box::new(qk_sea::siren_pole(x.floor() as i32, z.floor() as i32)))));
        }
    }
    if quake.eew {
        qk_sea::eew_alert(&mut p, &poles, geo.t[1], me, f64::from(QK_T0) + 1.2, cfg.siren, geo.seed);
    }
    if let (Some(tsu), Some(st)) = (quake.tsunami.as_ref(), sea) {
        qk_sea_warnings(&mut p, &geo, tsu, &st, me, &poles, &cfg);
    }
    let mut flood_out = None;

    if terrain {
        let chunks = qk_chunks(&geo);
        for [x0, z0, x1, z1] in qk_load_boxes(&chunks) {
            p.at(0.0, format!("forceload add {x0} {z0} {x1} {z1}"));
            p.ev.push((len - 1.0, Act::Spare(format!("forceload remove {x0} {z0} {x1} {z1}"))));
        }
        let parts = chunks.len().div_ceil(QK_PART);
        let w = Rc::new(RefCell::new(Work {
            geo: geo.clone(),
            chunks,
            tags: (0..parts).map(|_| crate::columns::new_tag()).collect(),
            asked: 0,
            taken: 0,
            cur: None,
            retry: Vec::new(),
            retry_q: None,
            next_retry: 0.0,
            // (the land worked out before the sea draws back: the sea works on it as the quake leaves it)
            land_by: sea.map_or(f64::from(len) - 8.0, |st| st.draw - 3.0),
            fires: 0,
            collapsed: 0,
            walls: 0,
            slid: 0,
            cracked: 0,
            liquefied: 0,
            torn: 0,
            changed: 0,
            done: false,
            flood: sea.map(|st| Rc::new(RefCell::new(qk_sea::Flood::new(geo.clone(), st)))),
        }));
        let flood = w.borrow().flood.clone();
        p.later(0.1, move |q| qk_work(q, w, 0.1));
        // the sea drawn back from as it withdraws until the quake hands over; put back by the Tsunami as its wave
        // reaches the coast, or here if this strike is stopped first (`Act::Cleanup` past the end: never made if it ends)
        if let (Some(f), Some(st)) = (flood, sea) {
            let t = (st.draw - 0.5) as f32;
            let f1 = f.clone();
            p.later(t, move |q| qk_sea::sea_work(q, f1, f64::from(t)));
            const PARTS: usize = 24;
            for part in 0..PARTS {
                let f2 = f.clone();
                let at = len + 1.0 + 0.05 * part as f32;
                p.later(at, move |q| qk_sea::sea_restore(q, &f2, part, PARTS, at));
                if let Some((t, Act::Later(job))) = p.ev.pop() {
                    p.ev.push((t, Act::Cleanup(job)));
                }
            }
            flood_out = Some(f);
        }
    }
    // the hand-over to the Tsunami (lib.rs fires it at the same target as this strike ends on its own)
    if let Some(tsu) = quake.tsunami.as_ref() {
        let (kind, color) = qk_sea::warning(tsu);
        let k0 = qk_sea::wave_k(tsu);
        qk_sea::set_pending(qk_sea::HandOff { flood: flood_out, k: k0, kind, color, expect: tsu.expect, major: kind.starts_with("MAJOR"), poles, siren: cfg.siren, seed: geo.seed });
        // the run-up: a tsunami runs on inland and up the land well over its height at the coast (Tohoku's 10 to 15
        // m ran up to 40), so the wave is made as big as it takes to reach the ground where the player stands as it
        // comes, up to `QT_RUNUP` times its own; higher ground than that is safe, as it is in life
        let tag = crate::ground::new_tag();
        let t_ask = (len - 3.0).max(0.2);
        p.later(t_ask, move |q| {
            let [x, _, z] = crate::sdk::game::get_player().map_or(me, |p| [p.x, p.y, p.z]);
            q.ev.push((t_ask, Act::Survey { tag, columns: vec![(x.floor() as i32, z.floor() as i32)] }));
        });
        let (block, point) = (target.block, target.point);
        p.later((len - 1.0).max(0.3), move |_| {
            let now = crate::sdk::game::get_player().map_or(me, |p| [p.x, p.y, p.z]);
            let ground = crate::ground::at(now[0].floor() as i32, now[2].floor() as i32).map_or(now[1].floor(), f64::from);
            let ts = super::tsunami::TsGeo::new(&Target { block, point }, now, 0.0);
            let k = qk_sea::run_up_k(&ts, ground, k0);
            qk_sea::with_pending(|h| h.k = k);
        });
    }
    // (what would come after its end left out: the far player's last aftershock; but not what a stop must still do)
    p.ev.retain(|e| e.0 < len - 0.05 || matches!(e.1, Act::Cleanup(_)));
    p.ev.sort_by(|a, b| a.0.total_cmp(&b.0));
    p.ev
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geo(preset: usize) -> QkGeo {
        let target = Target { block: [0, 64, 0], point: [0.5, 65.0, 0.5] };
        QkGeo::new(preset, &target, [0.5, 66.0, 200.5], 17.0)
    }

    #[test]
    fn quake_tops_out_at_its_shindo_over_the_rupture_and_weakens_away() {
        for (i, q) in QK_PRESETS.iter().enumerate() {
            let g = geo(i);
            let top = g.ii_ud(0.0, 0.0);
            assert!((top - q.shindo).abs() < 1e-9, "{}", q.key);
            assert_eq!(shindo_class(top).0, "7", "{}: every preset is a Shindo 7", q.key);
            let mut last = top;
            for d in [20.0, 60.0, 150.0, 300.0, 600.0] {
                let i = g.ii_ud(0.0, d);
                assert!(i <= last + 1e-9, "{} weakens with distance", q.key);
                last = i;
            }
            assert!(last < top - 0.4, "{}: weaker far off ({last})", q.key);
        }
    }

    #[test]
    fn quake_custom_takes_the_players_numbers_and_the_traits_of_such_a_quake() {
        let q = qk_set_custom(9.0, 20.0, None);
        assert!(q.fault == Fault::Megathrust && q.tsunami.is_some(), "a great shallow quake: a megathrust with a tsunami");
        assert!(q.shindo >= 6.0, "{}", q.shindo);
        let q = qk_set_custom(6.5, 8.0, Some(6.25));
        assert!(q.fault == Fault::StrikeSlip && q.tsunami.is_none() && q.ruptures());
        assert_eq!(q.top_class(), "6+");
        let q = qk_set_custom(7.2, 300.0, None);
        assert!(!q.ruptures() && q.tsunami.is_none(), "a deep one breaks nothing and raises no tsunami");
        assert!(q.shindo < 5.5, "a deep one shakes less: {}", q.shindo);
        assert!(std::ptr::eq(qk_quake(QK_CUSTOM), q));
        for (mw, d) in [(9.6, 10.0), (5.0, 1.0), (8.0, 60.0)] {
            qk_set_custom(mw, d, None);
            assert!(qk_length(QK_CUSTOM, qk_sea::QT_COUNTDOWN_MAX) <= crate::lasers::EARTHQUAKE.length);
            assert!(qk_pass_preset(QK_CUSTOM) < QK_CUSTOM);
        }
    }

    #[test]
    fn quake_shindo_classes_follow_the_jma_table() {
        let c = |i: f64| shindo_class(i).0;
        assert_eq!([c(0.2), c(1.0), c(2.0), c(3.0), c(4.0), c(4.7), c(5.2), c(5.7), c(6.2), c(6.8)], ["0", "1", "2", "3", "4", "5-", "5+", "6-", "6+", "7"]);
    }

    #[test]
    fn quake_kobe_at_osaka_distance_is_about_five_or_six() {
        // (Kobe 1995: Shindo 4 to 5 in Osaka some 30 km off; at 0.15 km a block that is 200 blocks)
        let g = geo(1);
        let i = g.ii_ud(0.0, 200.0);
        assert!((4.3..=5.8).contains(&i), "{i}");
        // (Tohoku 2011: Shindo 6- to 6+ some 150 km inland of the rupture's deep end)
        let g = geo(0);
        let i = g.ii_ud(0.0, 280.0);
        assert!((5.4..=6.5).contains(&i), "{i}");
    }

    #[test]
    fn quake_waves_arrive_p_before_s_and_shake_most_in_the_main_pulse() {
        for i in 0..QK_PRESETS.len() {
            let g = geo(i);
            let (s, p) = g.arrive(0.0, 120.0);
            assert!(p < s && p > 0.0);
            let q = g.q;
            let t_peak = f64::from(QK_T0 + q.t_main()) + s + 0.12 * f64::from(q.main_dur());
            assert!(g.env(0.0, 120.0, t_peak) > 0.95, "{}", q.key);
            assert!(g.env(0.0, 120.0, f64::from(QK_T0) + p - 0.5) < 1e-6);
            assert!(f64::from(q.length) > f64::from(QK_T0) + s + q.pulses.iter().map(|p| f64::from(p.0)).fold(0.0, f64::max));
        }
    }

    #[test]
    fn quake_slip_jumps_across_the_trace_and_dies_out_of_the_band() {
        let g = geo(1);
        let (h1, v1) = g.slip(0.0, 0.3);
        let (h2, v2) = g.slip(0.0, -0.3);
        assert!((h1 - h2).abs() > 1.5, "a jump of the slip across the trace");
        assert!(v2 > v1, "the far side up");
        assert_eq!(g.slip(0.0, QK_BAND + 1.0), (0.0, 0.0));
        let g = geo(0);
        assert_eq!(g.slip(0.0, 0.3), (0.0, 0.0), "a megathrust breaks nothing on land");
    }

    #[test]
    fn quake_plan_is_in_order_and_ends_in_time() {
        let target = Target { block: [0, 64, 0], point: [0.5, 65.0, 0.5] };
        for (i, q) in QK_PRESETS.iter().enumerate() {
            for cd in [0u32, 30, 300] {
                qk_configure(QkConfig { countdown: if cd == 0 { qk_sea::QT_COUNTDOWN } else { f64::from(cd) }, siren: 1.0 });
                let plan = quake_plan(&crate::lasers::EARTHQUAKE, &target, true, "minecraft:overworld", 17.0, [0.5, 66.0, 150.5], i);
                let _ = qk_sea::take_pending();
                let len = qk_length(i, if cd == 0 { qk_sea::QT_COUNTDOWN } else { f64::from(cd) });
                assert!(plan.windows(2).all(|w| w[0].0 <= w[1].0), "{}", q.key);
                // (but the cleanups a stop makes, which the end leaves to the Tsunami)
                assert!(plan.iter().all(|e| e.0 < len || matches!(e.1, Act::Cleanup(_))), "{}", q.key);
                assert!(len <= crate::lasers::EARTHQUAKE.length, "{} runs {len}", q.key);
                // (a quake that raised a tsunami puts the sea back even if it is stopped)
                assert_eq!(q.tsunami.is_some(), plan.iter().any(|(_, a)| matches!(a, Act::Cleanup(_))), "{}", q.key);
            }
        }
    }

    #[test]
    fn quake_shindo_seven_is_far_more_violent_than_six_lower() {
        assert!(qk_violence(7.0) > 2.4 * qk_violence(5.7) && qk_violence(7.0) > 2.5);
        assert!(qk_violence(4.0) < 0.4);
    }

    #[test]
    fn quake_aftershocks_follow_omori_and_bath() {
        for (i, q) in QK_PRESETS.iter().enumerate() {
            let len = f64::from(qk_length(i, qk_sea::QT_COUNTDOWN));
            let a = qk_aftershocks(q, 17 + i as u32, len);
            assert!(a.len() >= 2 && a.len() <= QK_AFTER_MAX, "{}: {}", q.key, a.len());
            assert!(a.windows(2).all(|w| w[0].t <= w[1].t), "{}", q.key);
            assert!(a.iter().all(|x| x.t + 4.0 <= len && x.mw <= q.mw - 0.95 && x.drop > 0.0), "{}", q.key);
            let top = a.iter().map(|x| x.mw).fold(0.0, f64::max);
            assert!((top - (q.mw - 1.2)).abs() <= 0.25, "{}: the largest {top} (Båth: {})", q.key, q.mw - 1.2);
            // (Omori: more in the first half of the window than in the second)
            let mid = 0.5 * (a[0].t + len - 4.0);
            assert!(a.iter().filter(|x| x.t < mid).count() * 2 >= a.len(), "{}", q.key);
        }
    }

    #[test]
    fn quake_tsunami_runs_up_to_the_player_but_not_past_its_limit() {
        let target = Target { block: [0, 62, 0], point: [0.5, 63.0, 0.5] };
        let ts = super::super::tsunami::TsGeo::new(&target, [0.5, 70.0, 300.5], 0.0);
        let k0 = qk_sea::wave_k(QK_PRESETS[0].tsunami.as_ref().unwrap());
        assert_eq!(qk_sea::run_up_k(&ts, 64.0, k0), k0, "low land: its own size");
        let k = qk_sea::run_up_k(&ts, 80.0, k0);
        assert!(k > k0 && k < qk_sea::QT_RUNUP * k0, "{k}");
        assert!((qk_sea::run_up_k(&ts, 250.0, k0) - qk_sea::QT_RUNUP * k0).abs() < 1e-9, "a mountain: safe");
    }

    #[test]
    fn quake_tsunami_warns_without_announcing_the_sea_drawing_back() {
        let target = Target { block: [0, 64, 0], point: [0.5, 65.0, 0.5] };
        qk_configure(QkConfig { countdown: qk_sea::QT_COUNTDOWN, siren: 1.0 });
        let plan = quake_plan(&crate::lasers::EARTHQUAKE, &target, false, "minecraft:overworld", 17.0, [0.5, 66.0, 150.5], 0);
        let _ = qk_sea::take_pending();
        assert!(plan.iter().all(|(_, a)| !matches!(a, Act::Cmd(c) if c.contains("WITHDRAW"))));
        assert!(plan.iter().any(|(_, a)| matches!(a, Act::Table(w) if f32::from_bits(w[QK_TAB_AFTER]) >= 2.0)));
    }

    /// A chunk's work over a made-up village by a river: houses fall, glass breaks, the trace tears through.
    #[test]
    fn quake_chunk_work_damages_the_land_by_its_shindo() {
        crate::meter::reset();
        let target = Target { block: [0, 64, 0], point: [0.5, 65.0, 0.5] };
        let geo = Rc::new(QkGeo::new(1, &target, [0.5, 66.0, 60.5], 17.0));
        let chunks = vec![(0, 0, 3.0), (-1, -1, 3.0)];
        let tag = crate::columns::new_tag();
        let mut columns = Vec::new();
        for &(cx, cz, _) in &chunks {
            chunk_columns(cx, cz, &mut columns);
        }
        let per = (QK_BELOW + QK_ABOVE) as usize;
        crate::columns::test_answer(tag, &columns, QK_BELOW, QK_ABOVE, &|x, z| {
            // ground at 64; a house of planks 5 high with glass windows on every 3rd column; water at x == 9
            let mut v: Vec<&'static str> = Vec::with_capacity(per);
            let house = x.rem_euclid(8) < 5 && z.rem_euclid(8) < 5;
            let top = if house { 69 } else { 64 };
            let surface = top + 1;
            for k in 0..per as i32 {
                let y = surface - QK_BELOW as i32 + k;
                v.push(if x == 9 && y == 64 {
                    "minecraft:water"
                } else if y <= 63 {
                    "minecraft:stone"
                } else if y == 64 {
                    "minecraft:grass_block"
                } else if house && y <= top {
                    if (x + z) % 3 == 0 && y == 66 { "minecraft:glass" } else { "minecraft:oak_planks" }
                } else {
                    "minecraft:air"
                });
            }
            Some((surface, v))
        });
        let a = crate::columns::take(tag).expect("answered");
        let pal = a.palette.iter().map(|n| classify(n)).collect();
        let cur = Cur { a, pal, chunks: chunks.clone(), k: 0 };
        let mut w = Work {
            geo,
            chunks,
            tags: vec![],
            asked: 0,
            taken: 0,
            cur: None,
            retry: vec![],
            retry_q: None,
            next_retry: 0.0,
            land_by: 30.0,
            fires: 0,
            collapsed: 0,
            walls: 0,
            slid: 0,
            cracked: 0,
            liquefied: 0,
            torn: 0,
            changed: 0,
            done: false,
            flood: None,
        };
        let mut q = Plan { laser: &crate::lasers::EARTHQUAKE, ev: Vec::new(), rng: Rng::new(1), bx: 0, by: 64, bz: 0 };
        assert!(qk_chunk(&mut w, &cur, 0, &mut q));
        assert!(qk_chunk(&mut w, &cur, 1, &mut q));
        assert!(w.collapsed > 0, "houses down at Shindo 7");
        assert!(w.torn > 0, "the trace torn open");
        assert!(w.changed == 2);
        let edits: Vec<String> = q.ev.iter().filter_map(|(_, a)| if let Act::Blocks(b) = a { Some(b.as_fills()) } else { None }).flatten().collect();
        assert!(edits.iter().any(|f| f.contains("minecraft:air")));
    }
}
#[cfg(test)]
mod print {
    use super::*;
    #[test]
    #[ignore]
    fn qk_print_table() {
        let target = Target { block: [0, 64, 0], point: [0.5, 65.0, 0.5] };
        for (i, q) in QK_PRESETS.iter().enumerate() {
            let g = QkGeo::new(i, &target, [0.5, 66.0, 150.5], 17.0);
            for d in [0.0, 50.0, 100.0, 150.0, 200.0, 300.0] {
                let (s, p) = g.arrive(0.0, d);
                println!("{:9} d {d:5} I {:.2} ({:2}) P {p:5.1} S {s:5.1} strong {:5.1}", q.key, g.ii_ud(0.0, d), shindo_class(g.ii_ud(0.0, d)).0, g.strong_at(0.0, d));
            }
            println!("{:9} chunks {}", q.key, qk_chunks(&g).len());
        }
    }
}
