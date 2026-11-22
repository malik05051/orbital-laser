//! The Earthquake's tsunami (the presets that raised one: `Quake::tsunami`). Not a wall of water: what a tsunami is at a
//! coast. A warning goes out as the shaking peaks, the JMA's (Major Tsunami Warning, the expected height, a countdown
//! to arrival, `/orbital tsunamitime`); the sea draws back from the coast, baring its bed; then a train of waves comes,
//! each a surge of the sea a few metres up to about twelve (a block a metre) that floods the low land inland and runs
//! back off it, the biggest not always the first. The water is real: water blocks put into the air over the land and
//! taken out of the sea, a chunk at a time (`sea_work`), so it can be swum in and it carries what floats; wooden houses
//! standing in two metres of it and more are washed away (as the Tohoku survey's fragility curves have it, sturdier
//! ones from six). Everything the sea did is put back as it ends, and so if the strike is stopped (`sea_restore`, an
//! `Act::Cleanup`): the sea at its level, the land dry.
//!
//! The target is the coast: the sea lies beyond it (the far side of the fault line, `QkGeo`'s d < 0) and the land
//! runs inland towards the player (d > 0). The sea's surface is taken as `SEA` (62, the top water block of vanilla's
//! sea level 63). Each column's water is the sea's level at the coast (`sea_s`) as it was `d / speed` seconds before,
//! shrinking inland to nothing at `reach` (`level`).
use super::quake::{QK_T0, QkGeo, Quake};
use super::{Act, Blocks, Plan};
use std::cell::RefCell;
use std::rc::Rc;

/// A preset's tsunami.
pub struct Tsu {
    /// The JMA's forecast height for its warning ("10 m+" is "10m超").
    pub expect: &'static str,
    /// How far the sea draws back before the first wave (blocks).
    pub draw: f64,
    /// The waves: (seconds after the first, its height over the sea at the coast in blocks, seconds it rises, holds and
    /// falls).
    pub waves: &'static [(f32, f32, f32, f32, f32)],
    /// How fast the water runs inland (blocks a second), and how far inland its height falls to nothing.
    pub speed: f64,
    pub reach: f64,
}

/// The sea's surface: the top water block of vanilla's sea level.
pub const SEA: i32 = 62;
/// How wide along the coast the sea's work reaches (each side), and how far out to sea.
pub const QT_HALF: f64 = 176.0;
pub const QT_OUT: f64 = 112.0;
/// The countdown to arrival the tests take, and the most one may be set to (seconds).
pub const QT_COUNTDOWN: f64 = 60.0;
pub const QT_COUNTDOWN_MAX: f64 = 900.0;
/// When none is set (`/orbital tsunamitime auto`, the default), the first wave comes between these many seconds after
/// the warning, a strike's own (`auto_countdown`): three to eight minutes, as the coast nearest a great quake has had.
pub const QT_AUTO: (f64, f64) = (180.0, 480.0);
/// The warning comes this long after the earthquake begins (the JMA's first Tsunami Warning for Tohoku was out three
/// minutes after its origin time).
pub const QT_WARN: f64 = 180.0;
/// A strike's own countdown when none is set: between `QT_AUTO`'s two, by its seed.
pub fn auto_countdown(seed: u32) -> f64 {
    let h = seed.wrapping_mul(0x9E37_79B1) ^ 0x5bd1_e995;
    let u = f64::from(h.wrapping_mul(0x85eb_ca6b) >> 8) / f64::from(1u32 << 24);
    (QT_AUTO.0 + u * (QT_AUTO.1 - QT_AUTO.0)).round()
}

fn smooth(a: f64, b: f64, x: f64) -> f64 {
    let u = ((x - a) / (b - a)).clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

/// When it all happens, strike seconds: the warning, the sea drawing back, the first wave at the coast, the end.
#[derive(Clone, Copy, Debug)]
pub struct SeaTime {
    pub warn: f64,
    pub draw: f64,
    pub arrive: f64,
    pub end: f64,
}

/// The tsunami's times for preset `q` with `countdown` seconds from the warning to arrival (none without a tsunami).
/// The first wave never comes before the strong shaking has been over for a while.
pub fn sea_time(q: &Quake, countdown: f64) -> Option<SeaTime> {
    let ts = q.tsunami.as_ref()?;
    let t0 = f64::from(QK_T0);
    let warn = t0 + QT_WARN;
    let shaken = t0 + q.pulses.iter().filter(|p| p.1 >= 0.4).map(|p| f64::from(p.0 + p.2)).fold(0.0, f64::max);
    let arrive = (warn + countdown.clamp(10.0, QT_COUNTDOWN_MAX)).max(shaken + 12.0);
    let draw = arrive - (0.5 * (arrive - warn)).clamp(14.0, 26.0);
    let _ = ts;
    // (the quake's strike ends here and the Tsunami's takes over, its wave reaching the coast TS_T later: `HandOff`)
    Some(SeaTime { warn, draw, arrive, end: arrive - super::tsunami::TS_T })
}
/// The strike's whole length for preset `q` and the countdown: its shaking, or with a tsunami until it hands over to the
/// Tsunami's strike (`HandOff`).
pub fn qk_length(q: &Quake, countdown: f64) -> f32 {
    sea_time(q, countdown).map_or(q.length, |s| (s.end + 0.5) as f32)
}

/// How big the Tsunami's wave is drawn for this tsunami: its highest wave over the Tsunami's own ~145 at the coast.
pub fn wave_k(ts: &Tsu) -> f64 {
    let h = ts.waves.iter().map(|w| f64::from(w.1)).fold(0.0, f64::max);
    (h / 145.0).clamp(0.02, 1.0)
}

/// The sea's level at the coast over its own (blocks; below 0 drawn back) at strike second `t`.
pub fn sea_s(ts: &Tsu, st: &SeaTime, t: f64) -> f64 {
    if t <= st.draw {
        return 0.0;
    }
    let last = ts.waves.iter().map(|w| f64::from(w.0 + w.2 + w.3 + w.4)).fold(0.0, f64::max) + st.arrive;
    let rise0 = ts.waves.first().map_or(8.0, |w| f64::from(w.2));
    // (drawn back before the first wave, half as far between them, back to its own level after the last)
    let base = -ts.draw
        * smooth(st.draw, st.draw + 0.6 * (st.arrive - st.draw), t)
        * (1.0 - 0.5 * smooth(st.arrive, st.arrive + rise0, t))
        * (1.0 - smooth(last, last + 12.0, t));
    let mut s = base;
    for &(t0, h, rise, hold, fall) in ts.waves {
        let a = st.arrive + f64::from(t0);
        let (r, hd, f) = (f64::from(rise), f64::from(hold), f64::from(fall));
        let env = smooth(a, a + r, t) * (1.0 - smooth(a + r + hd, a + r + hd + f, t));
        s += env * (f64::from(h) - base);
    }
    s
}

/// The water's level over the sea's own at `x` blocks inland of the coast (negative: out to sea) at second `t`.
pub fn level(ts: &Tsu, st: &SeaTime, x: f64, t: f64) -> f64 {
    let s = sea_s(ts, st, t - x / ts.speed);
    if x <= 0.0 {
        return s;
    }
    if s >= 0.0 { s * (1.0 - x / ts.reach).max(0.0).powf(0.7) } else { s * (1.0 - x / (0.4 * ts.reach)).max(0.0) }
}

/// The highest the water gets `x` blocks inland, over the sea's own (which columns can ever be reached).
fn reach_at(ts: &Tsu, x: f64) -> f64 {
    let h = ts.waves.iter().map(|w| f64::from(w.1)).fold(0.0, f64::max);
    if x <= 0.0 { h } else { h * (1.0 - x / ts.reach).max(0.0).powf(0.7) }
}

/// What the sea does to a column: left alone, land (`orig` its ground), or water at the sea's level (`orig` its
/// surface, `base` its bed).
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum Kind {
    #[default]
    Alone,
    Land,
    Sea,
}
#[derive(Clone, Copy, Default, Debug)]
pub struct FCol {
    pub kind: Kind,
    pub base: i16,
    pub orig: i16,
    /// The water's top now (as the edits sent have it), and the highest it has been put.
    pub cur: i16,
    pub hi: i16,
    /// The top of what stands on it, and whether that is a house (1 wooden, 2 sturdier), washed away yet.
    pub top: i16,
    pub house: u8,
    pub washed: bool,
}
pub struct FChunk {
    pub cx: i32,
    pub cz: i32,
    pub cols: Box<[FCol; 256]>,
    /// The water has reached it (its splash heard once).
    pub reached: bool,
}
/// The sea's work: the chunks it may reach, as the quake's work leaves them (`quake::qk_chunk` adds them).
pub struct Flood {
    pub geo: Rc<QkGeo>,
    pub st: SeaTime,
    pub chunks: Vec<FChunk>,
    pub next: usize,
    pub washed: u32,
    pub peak: i32,
    /// Moved to the coast the player is near (`coast_flood`): what the quake's land work finds at its own target is
    /// no longer the sea's.
    pub moved: bool,
}

impl Flood {
    pub fn new(geo: Rc<QkGeo>, st: SeaTime) -> Flood {
        Flood { geo, st, chunks: Vec::new(), next: 0, washed: 0, peak: 0, moved: false }
    }
    fn ts(&self) -> &'static Tsu {
        self.geo.q.tsunami.as_ref().expect("a tsunami")
    }
    /// Whether the chunk (cx, cz) lies where the sea works.
    pub fn covers(&self, cx: i32, cz: i32) -> bool {
        sea_covers(&self.geo, self.ts(), cx, cz)
    }
    /// A chunk as the quake left it: kept if the sea can reach any of it.
    pub fn add(&mut self, cx: i32, cz: i32, cols: Box<[FCol; 256]>) {
        if self.moved {
            return;
        }
        let ts = self.ts();
        let mut any = false;
        for (i, c) in cols.iter().enumerate() {
            crate::meter::spend(80);
            let (x, z) = (cx * 16 + (i % 16) as i32, cz * 16 + (i / 16) as i32);
            let (_, d) = self.geo.ud(f64::from(x) + 0.5, f64::from(z) + 0.5);
            any |= match c.kind {
                Kind::Alone => false,
                Kind::Sea => true,
                Kind::Land => f64::from(c.base) < f64::from(SEA) + reach_at(ts, d) - 0.5,
            };
        }
        if any {
            self.chunks.push(FChunk { cx, cz, cols, reached: false });
        }
    }
}

/// Whether the chunk (cx, cz) lies where tsunami `ts`'s sea works (it stays loaded until the sea is put back).
pub fn sea_covers(geo: &QkGeo, ts: &Tsu, cx: i32, cz: i32) -> bool {
    let (u, d) = geo.ud(f64::from(cx) * 16.0 + 8.0, f64::from(cz) * 16.0 + 8.0);
    u.abs() <= QT_HALF + 8.0 && d >= -QT_OUT - 8.0 && d <= ts.reach + 16.0
}

/// A column's water top at second `t`, `x` blocks inland.
fn target(ts: &Tsu, st: &SeaTime, c: &FCol, x: f64, t: f64) -> i32 {
    let l = level(ts, st, x, t);
    match c.kind {
        Kind::Alone => i32::from(c.cur),
        Kind::Sea => (i32::from(c.orig) + l.round() as i32).clamp(i32::from(c.base), i32::from(c.orig) + 40),
        Kind::Land => {
            let top = (f64::from(SEA) + l).floor() as i32;
            if top > i32::from(c.base) { top.min(i32::from(c.base) + 40) } else { i32::from(c.base) }
        }
    }
}

fn wash_chance(house: u8, depth: i32) -> f64 {
    let t: &[(f64, f64)] = if house == 1 { &[(2.0, 0.25), (3.0, 0.55), (4.0, 0.8), (6.0, 1.0)] } else { &[(4.0, 0.0), (6.0, 0.3), (8.0, 0.6), (10.0, 0.9)] };
    let d = f64::from(depth);
    if d <= t[0].0 {
        return if d < t[0].0 { 0.0 } else { t[0].1 };
    }
    for w in t.windows(2) {
        if d <= w[1].0 {
            return w[0].1 + (w[1].1 - w[0].1) * (d - w[0].0) / (w[1].0 - w[0].0);
        }
    }
    t[t.len() - 1].1
}

/// One visit of a chunk at second `t`: each column's water brought to where the sea has it then (only air is filled,
/// only water taken out), the houses it is deep enough to take washed away. False if nothing changed.
fn visit(f: &mut Flood, k: usize, t: f64, q: &mut Plan) -> bool {
    let ts = f.ts();
    let st = f.st;
    let geo = f.geo.clone();
    let seed = geo.seed;
    let ch = &mut f.chunks[k];
    let (x0, z0) = (ch.cx * 16, ch.cz * 16);
    let mut b = Blocks { clear_of_player: false, ..Default::default() };
    let mut washed = Vec::new();
    let mut deepest = 0;
    crate::meter::spend(2_000 + 260 * 256);
    for i in 0..256 {
        let c = &mut ch.cols[i];
        if c.kind == Kind::Alone {
            continue;
        }
        let (x, z) = (x0 + (i % 16) as i32, z0 + (i / 16) as i32);
        let (_, d) = geo.ud(f64::from(x) + 0.5, f64::from(z) + 0.5);
        let tt = target(ts, &st, c, d, t);
        let cur = i32::from(c.cur);
        if c.kind == Kind::Land && c.house > 0 && !c.washed && tt - i32::from(c.base) >= 2 {
            let p = wash_chance(c.house, tt - i32::from(c.base));
            if super::mh((x.div_euclid(6) as u32).wrapping_mul(73_856_093) ^ (z.div_euclid(6) as u32).wrapping_mul(19_349_663), seed, 0x5ea) < p {
                b.put(x, z, i32::from(c.base) + 1, i32::from(c.top), "minecraft:air", None, false);
                c.washed = true;
                washed.push([f64::from(x) + 0.5, f64::from(c.base) + 1.0, f64::from(z) + 0.5]);
            }
        }
        if tt > cur {
            b.put(x, z, cur + 1, tt, "minecraft:water", Some("minecraft:air"), false);
        } else if tt < cur {
            b.put(x, z, tt + 1, cur, "minecraft:air", Some("minecraft:water"), false);
        }
        c.cur = tt as i16;
        c.hi = c.hi.max(c.cur);
        if c.kind == Kind::Land {
            deepest = deepest.max(tt - i32::from(c.base));
        }
    }
    f.washed += washed.len() as u32;
    let first = !ch.reached && deepest >= 1;
    if first {
        ch.reached = true;
    }
    f.peak = f.peak.max(deepest);
    if b.runs.is_empty() {
        return false;
    }
    let at = t as f32;
    q.ev.push((at, Act::Blocks(Box::new(b))));
    // (heard where it happens: the water coming in over the land, houses going)
    if first {
        let [x, z] = [f64::from(x0) + 8.0, f64::from(z0) + 8.0];
        q.ev.push((at, Act::Sound { event: "minecraft:entity.generic.splash".into(), at: Some([x, f64::from(SEA) + 1.0, z]), volume: 3.0, pitch: 0.6 }));
        q.ev.push((at, Act::Cmd(format!("particle minecraft:splash {x:.1} {} {z:.1} 6 0.5 6 0 120 force", SEA + 1))));
    }
    if let Some(&[x, y, z]) = washed.first() {
        let n = 1 + (x as i64).rem_euclid(4) as u32;
        q.ev.push((at, Act::Sound { event: format!("orbital:tsunami.crack{n}"), at: Some([x, y, z]), volume: 3.0, pitch: 0.9 }));
    }
    true
}

/// One stage of the sea's work: the next chunks in turn visited (each about once a second), until it is over.
pub fn sea_work(q: &mut Plan, f: Rc<RefCell<Flood>>, now: f64) {
    let more = {
        let mut f = f.borrow_mut();
        let n = f.chunks.len();
        if n > 0 {
            let per = n.div_ceil(16).max(4).min(n);
            for _ in 0..per {
                if crate::meter::spent() > 6_000_000 {
                    break;
                }
                let k = f.next % n;
                f.next = (f.next + 1) % n;
                // (a little ahead: the edits wait their turn behind the frame's pace)
                visit(&mut f, k, now + 0.2, q);
            }
        }
        if now >= f.st.end {
            crate::sdk::log::write(crate::sdk::log::Level::Info, &format!("earthquake: its tsunami flooded {} chunks, up to {} deep, washed away {} columns of houses", n, f.peak, f.washed));
        }
        now < f.st.end
    };
    if more {
        q.later((now + 0.05) as f32, move |q| sea_work(q, f, now + 0.05));
    }
}

/// Everything the sea did put back, part `part` of `parts` of its chunks: the water put on the land taken off, the sea
/// filled back to its own level (only water taken out, only air filled: the same whatever was sent before). Made even
/// if the strike is stopped.
pub fn sea_restore(q: &mut Plan, f: &Rc<RefCell<Flood>>, part: usize, parts: usize, at: f32) {
    let mut f = f.borrow_mut();
    let n = f.chunks.len();
    for k in (part..n).step_by(parts) {
        let ch = &mut f.chunks[k];
        let (x0, z0) = (ch.cx * 16, ch.cz * 16);
        let mut b = Blocks { cleanup: true, ..Default::default() };
        crate::meter::spend(1_000 + 80 * 256);
        for i in 0..256 {
            let c = &mut ch.cols[i];
            if c.kind == Kind::Alone {
                continue;
            }
            let (x, z) = (x0 + (i % 16) as i32, z0 + (i / 16) as i32);
            let (orig, hi) = (i32::from(c.orig), i32::from(c.hi));
            if hi > orig {
                b.put(x, z, orig + 1, hi, "minecraft:air", Some("minecraft:water"), false);
            }
            if c.kind == Kind::Sea && i32::from(c.base) < orig {
                b.put(x, z, i32::from(c.base) + 1, orig, "minecraft:water", Some("minecraft:air"), false);
            }
            c.cur = c.orig;
        }
        if !b.runs.is_empty() {
            q.ev.push((at, Act::Blocks(Box::new(b))));
        }
    }
}

/// The JMA's warning for it: its kind (by the expected height) and its colour on the JMA's map.
pub fn warning(ts: &Tsu) -> (&'static str, u32) {
    let h = ts.waves.iter().map(|w| f64::from(w.1)).fold(0.0, f64::max);
    if h > 3.0 {
        ("MAJOR TSUNAMI WARNING", 0xC800FF)
    } else if h > 1.0 {
        ("TSUNAMI WARNING", 0xFF2800)
    } else {
        ("TSUNAMI ADVISORY", 0xFAF500)
    }
}

/// The coast the tsunami comes to, worked out from strike second `at` (some twenty seconds before the sea draws back
/// at `st.draw`): the land round the player asked where the sea is (`find_coast`); with open sea near them, the
/// Tsunami's hand-over moved there (its coast, the way its wave comes in), the pass's drawn-back sea with it
/// (`table`, its `QK_TAB_COAST` set), and with the terrain on (`flood`) the sea's work too: the chunks there kept
/// loaded until `len`, their sea asked about, and the sea that draws back made of it, if it is in before the sea
/// starts to (if not, the target's as it was).
#[allow(clippy::too_many_arguments)]
pub fn relocate(p: &mut Plan, geo: Rc<QkGeo>, st: SeaTime, me0: [f64; 3], at: f32, flood: Option<Rc<RefCell<Flood>>>, table: Vec<u32>, len: f32) {
    p.later(at, move |q| {
        let me = crate::sdk::game::get_player().map_or(me0, |p| [p.x, p.y, p.z]);
        let tag = crate::ground::new_tag();
        let columns = coast_columns(me);
        crate::meter::spend(30 * columns.len() as u64);
        q.ev.push((at, Act::Survey { tag, columns }));
        coast_stage(q, geo, st, me, tag, at + 0.2, at + 4.0, flood, table, len);
    });
}
#[allow(clippy::too_many_arguments)]
fn coast_stage(q: &mut Plan, geo: Rc<QkGeo>, st: SeaTime, me: [f64; 3], tag: u32, at: f32, until: f32, flood: Option<Rc<RefCell<Flood>>>, table: Vec<u32>, len: f32) {
    if !crate::ground::ready(tag) {
        if at < until {
            q.later(at + 0.2, move |q| coast_stage(q, geo, st, me, tag, at + 0.2, until, flood, table, len));
        } else {
            crate::sdk::log::write(crate::sdk::log::Level::Info, "earthquake: the land round the player didn't answer; its tsunami comes to the target's coast");
        }
        return;
    }
    let Some((t, nm)) = find_coast(me) else {
        crate::sdk::log::write(crate::sdk::log::Level::Info, "earthquake: no open sea near the player; its tsunami comes to the target's coast");
        return;
    };
    crate::sdk::log::write(crate::sdk::log::Level::Info, &format!("earthquake: its tsunami comes to the coast at {:.0} {:.0}, inland towards {:.2} {:.2}", t[0], t[2], nm[0], nm[1]));
    let (bx, bz) = (t[0].floor() as i32, t[2].floor() as i32);
    with_pending(|h| {
        h.sea = (t, nm);
        h.coast = Some(([bx, SEA, bz], [t[0], f64::from(SEA + 1), t[2]]));
    });
    let mut table = table;
    let i = super::quake::QK_TAB_COAST;
    if table.len() < i + 4 {
        table.resize(i + 4, 0);
    }
    for (k, v) in [t[0], t[2], nm[0], nm[1]].iter().enumerate() {
        table[i + k] = (*v as f32).to_bits();
    }
    q.ev.push((at, Act::Table(table)));
    let Some(flood) = flood else { return };
    // (the chunks the sea's work reaches there kept loaded, their sea asked about once they are)
    let columns = coast_sea_columns(t, nm);
    let (mut x0, mut z0, mut x1, mut z1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for &(x, z) in &columns {
        (x0, z0, x1, z1) = (x0.min(x), z0.min(z), x1.max(x), z1.max(z));
    }
    if columns.is_empty() {
        return;
    }
    q.ev.push((at, Act::Cmd(format!("forceload add {x0} {z0} {x1} {z1}"))));
    q.ev.push(((len - 1.0).max(at + 1.0), Act::Spare(format!("forceload remove {x0} {z0} {x1} {z1}"))));
    let ask = at + 2.5;
    q.later(ask, move |q| {
        let tag = crate::ground::new_tag();
        q.ev.push((ask, Act::Survey { tag, columns: columns.clone() }));
        sea_stage(q, geo, st, t, nm, tag, columns, ask + 0.2, flood);
    });
}
#[allow(clippy::too_many_arguments)]
fn sea_stage(q: &mut Plan, geo: Rc<QkGeo>, st: SeaTime, t: [f64; 3], nm: [f64; 2], tag: u32, columns: Vec<(i32, i32)>, at: f32, flood: Rc<RefCell<Flood>>) {
    let until = (st.draw - 1.0) as f32;
    if !crate::ground::ready(tag) {
        if at < until {
            q.later(at + 0.2, move |q| sea_stage(q, geo, st, t, nm, tag, columns, at + 0.2, flood));
        } else {
            crate::sdk::log::write(crate::sdk::log::Level::Info, "earthquake: the coast's sea didn't answer in time; the target's draws back");
        }
        return;
    }
    build_stage(q, geo, st, t, nm, Rc::new(columns), 0, SeaChunks::new(), at, flood);
}
/// The coast's sea made of the survey's answer a share of its columns a stage (the meter's), and set as the sea's work
/// once it is all in, if before the sea starts to draw back.
#[allow(clippy::too_many_arguments)]
fn build_stage(q: &mut Plan, geo: Rc<QkGeo>, st: SeaTime, t: [f64; 3], nm: [f64; 2], columns: Rc<Vec<(i32, i32)>>, from: usize, mut out: SeaChunks, at: f32, flood: Rc<RefCell<Flood>>) {
    let mut i = from;
    while i < columns.len() && crate::meter::spent() < 6_000_000 {
        let j = (i + 2_000).min(columns.len());
        coast_sea_cols(&mut out, &columns[i..j]);
        i = j;
    }
    if i < columns.len() {
        if f64::from(at) < st.draw - 0.6 {
            q.later(at + 0.05, move |q| build_stage(q, geo, st, t, nm, columns, i, out, at + 0.05, flood));
        }
        return;
    }
    let f = coast_flood(&geo, st, t, nm, out);
    crate::sdk::log::write(crate::sdk::log::Level::Info, &format!("earthquake: the sea draws back from {} chunks of that coast", f.chunks.len()));
    *flood.borrow_mut() = f;
}

// ---- the sirens: a J-Alert pole on every stretch of land -------------------------------------------------------------
/// Japan's towns all have their disaster-prevention radio's poles, not just its coast. So it is here: the land is cut
/// into squares `NET_G` blocks across, and each square near the player has its pole, put up as they come near it
/// (wherever they have gone, teleported or flown) on the dry land nearest the square's middle; a square that is all sea
/// has none. What the poles broadcast is heard from the nearest that is up (`net_heard`), so there are sirens
/// wherever there is land, and none at sea.
pub const NET_G: i32 = 160;
/// Squares whose middle is within this of the player get their pole (the game keeps 12 chunks round them loaded).
const NET_R: f64 = 190.0;
/// How far round its square's middle a pole looks for dry land, and the step it looks in.
const NET_LOOK: i32 = 72;
const SITE_STEP: i32 = 8;
/// How often the network looks where the player is (strike seconds), how many of its rounds a square asked about waits
/// for the world's answer before it is asked again, and how many poles go up a round at most.
const NET_EVERY: f32 = 1.5;
const NET_WAIT: u32 = 4;
const NET_BUILD: usize = 4;
/// The poles' station groups: each group sets its broadcast off on its own (`sirens`), so near one pole it wails while
/// the next answers from further off. `NET_ALL`: every pole.
pub const NET_GROUPS: u8 = 3;
pub const NET_ALL: u8 = u8::MAX;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Square {
    /// The world asked where its land is (`round`: the network's round it was asked in).
    Asked { tag: u32, round: u32 },
    /// Its pole, (x, its ground, z).
    Pole([f64; 3]),
    Sea,
}
#[derive(Default)]
struct Net {
    on: bool,
    /// Whether its poles are built as blocks (the terrain on); off, they are only where their sounds come from.
    build: bool,
    seed: u32,
    round: u32,
    squares: std::collections::HashMap<(i32, i32), Square>,
}
thread_local! {
    static NET: RefCell<Net> = RefCell::new(Net::default());
}
/// A new network for a strike that sounds sirens (the Earthquake's, the Tsunami's own); its poles built as blocks if
/// `build`. The Tsunami an Earthquake hands over to keeps the quake's.
pub fn net_start(build: bool, seed: u32) {
    NET.with(|n| *n.borrow_mut() = Net { on: true, build, seed, ..Net::default() });
}
/// No sirens: none heard (a strike with its sirens off).
pub fn net_stop() {
    NET.with(|n| *n.borrow_mut() = Net::default());
}
/// Square (i, j)'s middle, a little off its true middle (each square its own).
fn square_mid(i: i32, j: i32, seed: u32) -> (i32, i32) {
    let h = (i as u32).wrapping_mul(73_856_093) ^ (j as u32).wrapping_mul(19_349_663);
    let off = |k: u32| ((super::mh(h, seed, k) - 0.5) * 48.0) as i32;
    (i * NET_G + NET_G / 2 + off(0x5e1), j * NET_G + NET_G / 2 + off(0x5e2))
}
fn square_group(i: i32, j: i32) -> u8 {
    ((i.rem_euclid(3) + 2 * j.rem_euclid(3)) % i32::from(NET_GROUPS)) as u8
}
/// The places round a square's middle its pole may go, nearest first.
fn look_offsets() -> Vec<(i32, i32)> {
    let n = NET_LOOK / SITE_STEP;
    let mut v: Vec<(i32, i32)> = (-n..=n).flat_map(|i| (-n..=n).map(move |j| (i * SITE_STEP, j * SITE_STEP))).filter(|&(a, b)| a * a + b * b <= NET_LOOK * NET_LOOK).collect();
    v.sort_by_key(|&(a, b)| (a * a + b * b, a, b));
    v
}
/// Where the network's poles of `group` (`NET_ALL`: all of them) sound from: their horns, nine up.
pub fn net_heard(group: u8) -> Vec<[f64; 3]> {
    NET.with(|n| {
        let n = n.borrow();
        n.squares
            .iter()
            .filter(|&(&(i, j), _)| group == NET_ALL || square_group(i, j) == group)
            .filter_map(|(_, s)| match *s {
                Square::Pole([x, g, z]) => Some([x, g + 9.0, z]),
                _ => None,
            })
            .collect()
    })
}
/// One round of the network from strike second `at`: the squares round the player (where they are now; `me0` where
/// nothing says) asked about, those answered given their pole (or none, all sea), and again every `NET_EVERY` until
/// `until`.
pub fn net_watch(q: &mut Plan, me0: [f64; 3], at: f32, until: f32) {
    let me = crate::sdk::game::get_player().map_or(me0, |p| [p.x, p.y, p.z]);
    let on = NET.with(|n| {
        let mut n = n.borrow_mut();
        if !n.on {
            return false;
        }
        n.round += 1;
        let (round, seed, build) = (n.round, n.seed, n.build);
        let offs = look_offsets();
        let (ci, cj) = ((me[0] / f64::from(NET_G)).floor() as i32, (me[2] / f64::from(NET_G)).floor() as i32);
        let mut up = 0;
        for i in ci - 2..=ci + 2 {
            for j in cj - 2..=cj + 2 {
                crate::meter::spend(400);
                let (mx, mz) = square_mid(i, j, seed);
                if (f64::from(mx) + 0.5 - me[0]).hypot(f64::from(mz) + 0.5 - me[2]) > NET_R {
                    continue;
                }
                match n.squares.get(&(i, j)).copied() {
                    None => {
                        let tag = crate::ground::new_tag();
                        let columns: Vec<(i32, i32)> = offs.iter().map(|&(a, b)| (mx + a, mz + b)).collect();
                        crate::meter::spend(30 * columns.len() as u64);
                        q.ev.push((at, Act::Survey { tag, columns }));
                        n.squares.insert((i, j), Square::Asked { tag, round });
                    }
                    Some(Square::Asked { tag, .. }) if crate::ground::ready(tag) && up < NET_BUILD => {
                        up += 1;
                        match dry_spot(mx, mz, &offs) {
                            Some((x, z, g)) => {
                                n.squares.insert((i, j), Square::Pole([f64::from(x) + 0.5, f64::from(g), f64::from(z) + 0.5]));
                                if build {
                                    q.ev.push((at + 0.05 * up as f32, Act::Blocks(Box::new(siren_pole_at(x, z, Some(g))))));
                                }
                            }
                            // (all of it sea: no pole; none of it loaded: asked again when the player is nearer)
                            None if offs.iter().any(|&(a, b)| crate::ground::at(mx + a, mz + b).is_some()) => {
                                n.squares.insert((i, j), Square::Sea);
                            }
                            None => {
                                n.squares.remove(&(i, j));
                            }
                        }
                    }
                    // (the answer lost, a new strike's survey having cleared it: asked again)
                    Some(Square::Asked { round: r, .. }) if round.saturating_sub(r) > NET_WAIT => {
                        n.squares.remove(&(i, j));
                    }
                    _ => {}
                }
            }
        }
        true
    });
    if on && at + NET_EVERY < until {
        q.later(at + NET_EVERY, move |q| net_watch(q, me0, at + NET_EVERY, until));
    }
}
/// The dry land nearest (x0, z0) among `offs` round it, as the survey has it: firm ground (dry the steps round it too)
/// if there is some, else any dry column; (x, z, its ground).
fn dry_spot(x0: i32, z0: i32, offs: &[(i32, i32)]) -> Option<(i32, i32, i32)> {
    let dry = |x: i32, z: i32| {
        crate::meter::spend(150);
        crate::ground::at(x, z).filter(|_| crate::ground::water(x, z).is_none())
    };
    let round = [(SITE_STEP, 0), (-SITE_STEP, 0), (0, SITE_STEP), (0, -SITE_STEP)];
    let firm = offs.iter().find(|&&(a, b)| dry(x0 + a, z0 + b).is_some() && round.iter().all(|&(c, d)| dry(x0 + a + c, z0 + b + d).is_some()));
    let &(a, b) = firm.or_else(|| offs.iter().find(|&&(a, b)| dry(x0 + a, z0 + b).is_some()))?;
    Some((x0 + a, z0 + b, dry(x0 + a, z0 + b)?))
}

// ---- where the sea is: the tsunami comes to the coast the player is near -----------------------------------------------
/// A tsunami raised by a great quake reaches every coast for hundreds of kilometres, so it comes where the player is:
/// as the sea is about to draw back, the land round them is asked where the sea is, and the nearest open sea (not a
/// river or a pond) is the sea that draws back and the wave comes in from. None near them: the target's coast, as fired.
/// How far round the player it looks (the land the game keeps loaded), and the step.
const COAST_R: i32 = 176;
const COAST_STEP: i32 = 8;
/// How many of the steps must be open sea for it to be a sea.
const COAST_OPEN: usize = 6;
/// The columns asked about round the player at `me`.
pub fn coast_columns(me: [f64; 3]) -> Vec<(i32, i32)> {
    let (px, pz) = (me[0].floor() as i32, me[2].floor() as i32);
    let n = COAST_R / COAST_STEP;
    (-n..=n).flat_map(|a| (-n..=n).map(move |b| (a, b))).filter(|&(a, b)| a * a + b * b <= n * n).map(|(a, b)| (px + a * COAST_STEP, pz + b * COAST_STEP)).collect()
}
/// Whether column (x, z) is the sea's (water two deep or more, its top at the sea's level), as the survey has it; none
/// where it said nothing.
fn sea_at(x: i32, z: i32) -> Option<bool> {
    crate::meter::spend(300);
    let g = crate::ground::at(x, z)?;
    Some(crate::ground::water(x, z).is_some_and(|w| w >= 2 && (SEA - 3..=SEA + 1).contains(&(g + w - 1))))
}
/// The coast nearest `me` as the survey of `coast_columns(me)` has it: the shore between the open sea nearest them and
/// the land (its x, the sea's surface over it, z) and the way inland from it. None if no open sea is near.
pub fn find_coast(me: [f64; 3]) -> Option<([f64; 3], [f64; 2])> {
    let (px, pz) = (me[0].floor() as i32, me[2].floor() as i32);
    let n = COAST_R / COAST_STEP;
    let s = |a: i32, b: i32| if a * a + b * b <= n * n { sea_at(px + a * COAST_STEP, pz + b * COAST_STEP) } else { None };
    let mut open = Vec::new();
    for a in -n..=n {
        for b in -n..=n {
            if s(a, b) == Some(true) && (-1..=1).all(|i| (-1..=1).all(|j| s(a + i, b + j) == Some(true))) {
                open.push((a * a + b * b, a, b));
            }
        }
    }
    if open.len() < COAST_OPEN {
        return None;
    }
    let &(_, oa, ob) = open.iter().min()?;
    // (the way inland: from the sea's steps near it to the land's)
    let (mut sea, mut land, mut ns, mut nl) = ([0.0f64; 2], [0.0f64; 2], 0.0, 0.0);
    for a in oa - 8..=oa + 8 {
        for b in ob - 8..=ob + 8 {
            match s(a, b) {
                Some(true) => {
                    sea[0] += f64::from(a);
                    sea[1] += f64::from(b);
                    ns += 1.0;
                }
                Some(false) => {
                    land[0] += f64::from(a);
                    land[1] += f64::from(b);
                    nl += 1.0;
                }
                None => {}
            }
        }
    }
    let norm = |v: [f64; 2]| {
        let l = v[0].hypot(v[1]);
        (l > 1e-6).then(|| [v[0] / l, v[1] / l])
    };
    let towards_me = norm([-f64::from(oa), -f64::from(ob)]).unwrap_or([0.0, 1.0]);
    let nm = if ns > 0.0 && nl > 0.0 { norm([land[0] / nl - sea[0] / ns, land[1] / nl - sea[1] / ns]).unwrap_or(towards_me) } else { towards_me };
    // (the shore: from the open sea along the way inland to the last of the sea)
    let (ox, oz) = (f64::from(px + oa * COAST_STEP), f64::from(pz + ob * COAST_STEP));
    let mut last = (ox, oz);
    for k in 1..=2 * n {
        let (x, z) = (ox + nm[0] * f64::from(k * COAST_STEP), oz + nm[1] * f64::from(k * COAST_STEP));
        let (a, b) = (((x - f64::from(px)) / f64::from(COAST_STEP)).round() as i32, ((z - f64::from(pz)) / f64::from(COAST_STEP)).round() as i32);
        match s(a, b) {
            Some(true) => last = (x, z),
            Some(false) => {
                last = (0.5 * (last.0 + x), 0.5 * (last.1 + z));
                break;
            }
            None => break,
        }
    }
    Some(([last.0.floor() + 0.5, f64::from(SEA + 1), last.1.floor() + 0.5], nm))
}
/// The sea's columns along the coast at `t` (inland `nm`) that the sea's work reaches (`QT_HALF` each side, `QT_OUT` out to
/// sea and a little inland), the world to be asked about.
pub fn coast_sea_columns(t: [f64; 3], nm: [f64; 2]) -> Vec<(i32, i32)> {
    let st = [nm[1], -nm[0]];
    let r = (QT_HALF.hypot(QT_OUT) + 2.0).ceil() as i32;
    let (cx, cz) = (t[0].floor() as i32, t[2].floor() as i32);
    let mut out = Vec::new();
    crate::meter::spend(10 * (2 * r as u64 + 1).pow(2));
    for x in cx - r..=cx + r {
        for z in cz - r..=cz + r {
            let (dx, dz) = (f64::from(x) + 0.5 - t[0], f64::from(z) + 0.5 - t[2]);
            let (u, d) = (dx * st[0] + dz * st[1], dx * nm[0] + dz * nm[1]);
            if u.abs() <= QT_HALF && (-QT_OUT..=24.0).contains(&d) {
                out.push((x, z));
            }
        }
    }
    crate::meter::spend(20 * out.len() as u64);
    out
}
/// The sea's columns found among `columns` (from the survey of `coast_sea_columns`), put in the chunks they are in.
pub type SeaChunks = std::collections::BTreeMap<(i32, i32), Box<[FCol; 256]>>;
pub fn coast_sea_cols(out: &mut SeaChunks, columns: &[(i32, i32)]) {
    for &(x, z) in columns {
        crate::meter::spend(450);
        let (Some(gr), Some(w)) = (crate::ground::at(x, z), crate::ground::water(x, z)) else { continue };
        let top = gr + w - 1;
        if w < 2 || !(SEA - 3..=SEA + 1).contains(&top) {
            continue;
        }
        let (base, orig) = ((gr - 1) as i16, top as i16);
        let cols = out.entry((x.div_euclid(16), z.div_euclid(16))).or_insert_with(|| Box::new([FCol::default(); 256]));
        cols[(z.rem_euclid(16) * 16 + x.rem_euclid(16)) as usize] = FCol { kind: Kind::Sea, base, orig, cur: orig, hi: orig, top: orig, house: 0, washed: false };
    }
}
/// The sea's work at the coast found (`find_coast`): its chunks with the sea in them, their sea's columns (the land left
/// alone: before the wave it does not change), in the coast's own axes.
pub fn coast_flood(geo: &QkGeo, st: SeaTime, t: [f64; 3], nm: [f64; 2], chunks: SeaChunks) -> Flood {
    let mut g = QkGeo { t, st: [nm[1], -nm[0]], nm, ..geo.clone() };
    g.t[1] = f64::from(SEA + 1);
    let mut f = Flood::new(Rc::new(g), st);
    f.moved = true;
    f.chunks = chunks.into_iter().map(|((cx, cz), cols)| FChunk { cx, cz, cols, reached: false }).collect();
    f
}

/// A J-Alert siren pole at (x, z), as real blocks standing on `ground` (the first air over the land; none: on each
/// column's own top): a grey metal post (iron bars, which the quake's damage leaves standing) eleven high, loudspeaker
/// horns facing out four ways near its top and two more under them, a lightning rod on its crown.
pub fn siren_pole_at(x: i32, z: i32, ground: Option<i32>) -> Blocks {
    let mut b = Blocks { clear_of_player: true, ..Default::default() };
    let (g, surface) = ground.map_or((0, true), |g| (g, false));
    b.put(x, z, g, g + 10, "minecraft:iron_bars", None, surface);
    b.put(x, z, g + 11, g + 11, "minecraft:waxed_lightning_rod[facing=up]", None, surface);
    let horns: [(i32, i32, &'static str); 4] = [
        (0, -1, "minecraft:waxed_lightning_rod[facing=north]"),
        (1, 0, "minecraft:waxed_lightning_rod[facing=east]"),
        (0, 1, "minecraft:waxed_lightning_rod[facing=south]"),
        (-1, 0, "minecraft:waxed_lightning_rod[facing=west]"),
    ];
    for (k, (dx, dz, horn)) in horns.iter().enumerate() {
        b.put(x + dx, z + dz, g + 9, g + 9, horn, None, surface);
        if k % 2 == 1 {
            b.put(x + dx, z + dz, g + 7, g + 7, horn, None, surface);
        }
    }
    b
}

/// What a quake that raised a tsunami hands over to the Tsunami strike lib.rs fires at its target as it ends: the sea it
/// drew back (put back as the wave reaches the coast, or if the Tsunami is stopped), the wave's size, the warning, the
/// siren poles and their volume.
pub struct HandOff {
    pub flood: Option<Rc<RefCell<Flood>>>,
    pub k: f64,
    pub kind: &'static str,
    pub color: u32,
    pub expect: &'static str,
    pub major: bool,
    pub siren: f32,
    pub seed: u32,
    /// The coast's middle and the way inland from it (the sea the other way): the wave comes from the side the sea drew
    /// back from, wherever the player has gone since (`on_sea_axis`).
    pub sea: ([f64; 3], [f64; 2]),
    /// Where the Tsunami is fired (its block and point): the coast the player was near as the sea drew back
    /// (`relocate`), none the quake's own target.
    pub coast: Option<([i32; 3], [f64; 3])>,
}
thread_local! {
    static PENDING: RefCell<Option<HandOff>> = const { RefCell::new(None) };
    static ACTIVE: RefCell<Option<HandOff>> = const { RefCell::new(None) };
}
/// The quake's plan leaves its hand-over here; lib.rs takes it as the quake ends (and drops it on a stop).
pub fn set_pending(h: HandOff) {
    PENDING.with(|p| *p.borrow_mut() = Some(h));
}
/// The hand-over waiting, changed (the run-up worked out as the quake ends).
pub fn with_pending(f: impl FnOnce(&mut HandOff)) {
    PENDING.with(|p| {
        if let Some(h) = p.borrow_mut().as_mut() {
            f(h);
        }
    });
}
/// How many times its height at the coast a tsunami may run up the land at most.
pub const QT_RUNUP: f64 = 3.0;
/// The Tsunami's size (`TsGeo::k`) for a wave of size `k0` that runs up to the player's ground (`ground`, the first air
/// over it) a couple of blocks deep as it holds there: `k0` if that is enough, up to `QT_RUNUP` times it.
pub fn run_up_k(ts: &super::tsunami::TsGeo, ground: f64, k0: f64) -> f64 {
    use super::tsunami::{TS_A, TS_LOW, TS_SEA};
    // (the level the water holds at the player's place, over the target's ground, at the Tsunami's own size)
    let hold = TS_A * ts.env(ts.dp, 0.0) * (1.0 - TS_LOW) - TS_SEA;
    if hold <= 1.0 {
        return k0;
    }
    let want = (ground - ts.t[1] + 2.0) / hold;
    want.clamp(k0, (QT_RUNUP * k0).min(1.0))
}
pub fn take_pending() -> Option<HandOff> {
    PENDING.with(|p| p.borrow_mut().take())
}
/// lib.rs makes it the next Tsunami's (`tsunami_plan` takes it).
pub fn activate(h: HandOff) {
    ACTIVE.with(|p| *p.borrow_mut() = Some(h));
}
/// The coast of the Tsunami about to be fired for a quake (lib.rs's `fire_at`), if it is one.
pub fn active_sea() -> Option<([f64; 3], [f64; 2])> {
    ACTIVE.with(|p| p.borrow().as_ref().map(|h| h.sea))
}
/// Where the Tsunami takes the player to be, so that its wave comes in from the sea the quake drew back (`HandOff::sea`):
/// on the line inland from the coast's middle, as far inland as they are (at least a little), at their height.
pub fn on_sea_axis(t: [f64; 3], nm: [f64; 2], me: [f64; 3]) -> [f64; 3] {
    let d = ((me[0] - t[0]) * nm[0] + (me[2] - t[2]) * nm[1]).max(24.0);
    [t[0] + d * nm[0], me[1], t[2] + d * nm[1]]
}
pub fn take_active() -> Option<HandOff> {
    ACTIVE.with(|p| p.borrow_mut().take())
}

/// How loud a pole's sounds are where they are made, at the player's 100% (the game takes a volume over 1 as more
/// reach, not more loudness: a pole's sounds fade out linearly over their `attenuation_distance`, 400, in sounds.json).
pub const QT_POLE_VOL: f32 = 0.55;
/// A broadcast's whole cycle (seconds): the siren twice, then its announcement spoken, then again (Oiso's poles on 30
/// July 2025, the Kamchatka M8.8's Tsunami Warning: blasts 11 s apart, some 12 s of voice, the next at 31 s).
pub const QT_CYCLE: f64 = 31.0;
/// How far from a pole its sounds carry (the game's fade over the sounds' `attenuation_distance`, 400, ends there too),
/// and past how far they are its far take (muffled, echoing): both from where the player is as each plays (`Act::Heard`),
/// so walking away from the coast the sirens fade and stop, and coming back to it they come back.
pub const QT_HEAR: f64 = 400.0;
pub const QT_NEAR: f64 = 140.0;

/// The disaster-prevention radio, from the network's poles (`net_watch`), as a town's sounds in life: no chime, the
/// siren twice in the JMA's pattern (a Major Tsunami Warning's 3 s on and 2 s off: `quake.major`, two blasts, twice; a
/// Tsunami Warning's 5 s on and 6 s off: `siren5`, twice), then the pause its announcement is spoken in, and again every
/// `QT_CYCLE`, from `from` to `until`. Each of the poles' groups is set off on its own, some seconds apart, each heard
/// from its nearest pole that is up, fainter the further off and past `QT_NEAR` muffled and echoing (`_far`), so near
/// one pole it wails while the next answers from further off; none where no pole is near. `vol` the player's siren
/// volume (1 as made); none at all at 0.
pub fn sirens(p: &mut Plan, from: f64, until: f64, major: bool, vol: f32, seed: u32) {
    if vol <= 0.0 {
        return;
    }
    // (every tsunami's warning on the Major Tsunami Warning's siren, the player's recording where they put one in: a
    // smaller one's synthesised `siren5` is no longer played)
    let _ = major;
    let (blast, every) = ("major", 10.0);
    let v = QT_POLE_VOL * vol;
    for g in 0..NET_GROUPS {
        let late = super::mh(u32::from(g), seed, 0x5a1) * 8.0 + 3.0 * f64::from(g);
        let push = |p: &mut Plan, t: f64, ev: &str| {
            if t >= from && t < until + 6.0 {
                let event = format!("orbital:quake.{ev}");
                p.ev.push((t as f32, Act::Heard { far: format!("{event}_far"), event, at: Vec::new(), volume: v, near: QT_NEAR, reach: QT_HEAR, here: false, net: Some(g) }));
            }
        };
        let mut c = from + late;
        while c < until {
            push(p, c, blast);
            push(p, c + every, blast);
            c += QT_CYCLE;
        }
    }
    // (and further off still, past the poles: the player's own recording of a coast's sirens, if they put one in, faint,
    // and only within the poles' reach, fainter the further off the nearest is)
    let mut t = from + 6.0;
    while t < until {
        let event = "orbital:quake.siren".to_string();
        p.ev.push((t as f32, Act::Heard { far: event.clone(), event, at: Vec::new(), volume: (0.12 * vol).min(1.0), near: QT_HEAR, reach: QT_HEAR, here: true, net: Some(NET_ALL) }));
        t += 16.0;
    }
}

/// The J-Alert's general alert for every laser but the Earthquake and the Tsunami (which have their own warnings): on
/// or off (`/orbital alertsirens`, the wheel's SIRENS for them; kept) and its volume (the player's siren volume).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AlertConfig {
    pub on: bool,
    pub vol: f32,
}
thread_local! {
    static ALERT: std::cell::Cell<AlertConfig> = const { std::cell::Cell::new(AlertConfig { on: true, vol: 1.0 }) };
}
pub fn alert_config() -> AlertConfig {
    ALERT.with(std::cell::Cell::get)
}
pub fn alert_configure(c: AlertConfig) {
    ALERT.with(|a| a.set(c));
}
/// The general alert's whole cycle (seconds): the recording (some 14 s) and a pause before the next.
pub const ALERT_CYCLE: f64 = 24.0;
/// The J-Alert's general alert as a laser strikes, until `until`: the network's poles put up round the player
/// (`net_watch`) and each group of them sounding `quake.alert` (the player's recording where they put one in) every
/// `ALERT_CYCLE`, a few seconds apart, heard from the nearest pole up as the Earthquake's sirens are; none with it off.
pub fn alert_sirens(p: &mut Plan, me: [f64; 3], terrain: bool, seed: u32, until: f64) {
    let c = alert_config();
    if !c.on || c.vol <= 0.0 {
        net_stop();
        return;
    }
    net_start(terrain, seed);
    let end = until as f32;
    p.later(0.1, move |q| net_watch(q, me, 0.1, end));
    for g in 0..NET_GROUPS {
        let mut t = 0.6 + super::mh(u32::from(g), seed, 0x5a3) * 4.0 + 2.5 * f64::from(g);
        while t < until {
            let event = "orbital:quake.alert".to_string();
            p.ev.push((t as f32, Act::Heard { far: format!("{event}_far"), event, at: Vec::new(), volume: QT_POLE_VOL * c.vol, near: QT_NEAR, reach: QT_HEAR, here: false, net: Some(g) }));
            t += ALERT_CYCLE;
        }
    }
}

/// The disaster-prevention radio's earthquake alert from the poles at `t`, a broadcast a pole as `sirens` has them.
pub fn eew_alert(p: &mut Plan, t: f64, vol: f32, seed: u32) {
    if vol <= 0.0 {
        return;
    }
    for g in 0..NET_GROUPS {
        let late = super::mh(u32::from(g), seed, 0x5a2) * 1.5 + 0.6 * f64::from(g);
        let event = "orbital:quake.eew".to_string();
        p.ev.push(((t + late) as f32, Act::Heard { far: format!("{event}_far"), event, at: Vec::new(), volume: 0.8 * QT_POLE_VOL * vol, near: QT_NEAR, reach: QT_HEAR, here: false, net: Some(g) }));
    }
}

/// The Tsunami's strike for a quake's tsunami (`tsunami_plan` calls it with what the quake handed over): the sea the
/// quake drew back put back as the wave reaches the coast (and if it is stopped); the sirens going on until the wave
/// has come and gone; a title as it arrives; over the hotbar the warning and the coast's gauge, the water's height at
/// the target over its ground as the wave's model has it.
pub fn qk_tsunami_extras(p: &mut Plan, geo: &super::tsunami::TsGeo, h: HandOff, me: [f64; 3]) {
    let tt = super::tsunami::TS_T;
    if let Some(f) = &h.flood {
        const PARTS: usize = 24;
        for part in 0..PARTS {
            let f2 = f.clone();
            let at = tt as f32 + 0.05 * part as f32;
            p.later(at, move |q| sea_restore(q, &f2, part, PARTS, at));
            if let Some((t, Act::Later(job))) = p.ev.pop() {
                p.ev.push((t, Act::Cleanup(job)));
            }
        }
    }
    // (the quake's network of poles kept, still put up round the player as they go)
    sirens(p, 0.0, tt + 24.0, h.major, h.siren, h.seed);
    if h.siren > 0.0 {
        p.later(0.1, move |q| net_watch(q, me, 0.1, (tt + 24.0) as f32));
    }
    let t0 = (tt - 1.5) as f32;
    p.at(t0, "title @s times 5 60 20");
    p.at(t0, format!(r##"title @s subtitle {{"text":"{}  ·  get to high ground","color":"#{:06X}"}}"##, h.expect, h.color));
    p.at(t0, format!(r##"title @s title {{"text":"TSUNAMI","color":"#{:06X}"}}"##, h.color));
    let mut top = 0.0f64;
    for k in 0..40 {
        let t = f64::from(k);
        let obs = geo.level(0.0, 0.0, t).unwrap_or(-1.0);
        top = top.max(obs);
        let text = if t < tt {
            format!("{}  ·  {}  ·  wave in {}", h.kind, h.expect, mmss(tt - t))
        } else if obs > 0.0 {
            format!("TSUNAMI  ·  {obs:+.1} m  ·  max {top:.1} m")
        } else {
            format!("TSUNAMI  ·  max {top:.1} m  ·  stay on high ground")
        };
        p.ev.push((t as f32, Act::Cmd(format!(r##"title @s actionbar {{"text":"{text}","color":"#{:06X}"}}"##, h.color))));
    }
}

/// Seconds as m:ss.
pub fn mmss(s: f64) -> String {
    let s = s.max(0.0).ceil() as i64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// The highest the sea has been at the coast from the first wave to second `t` (the gauge's maximum so far).
pub fn observed_max(ts: &Tsu, st: &SeaTime, t: f64) -> f64 {
    let mut m = 0.0f64;
    let mut u = st.arrive;
    while u <= t {
        m = m.max(sea_s(ts, st, u));
        u += 0.5;
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strike::quake::QK_PRESETS;

    /// The plan played out against a world whose ground is `ground` and water over it `water` (its depth), each stage
    /// worked out as it falls due and each survey answered at once, until `until`: what it gave, stages left out.
    fn drive(mut plan: Vec<(f32, Act)>, until: f32, ground: fn(i32, i32) -> Option<i32>, water: fn(i32, i32) -> Option<i32>) -> Vec<(f32, Act)> {
        crate::ground::set_test(Some(ground));
        let mut out = Vec::new();
        while !plan.is_empty() {
            plan.sort_by(|a, b| a.0.total_cmp(&b.0));
            let (t, act) = plan.remove(0);
            match act {
                // (past `until`, what is given kept, no more worked out)
                Act::Later(_) | Act::Soon(_) | Act::Survey { .. } if t > until => {}
                Act::Later(job) | Act::Soon(job) => plan.extend(job()),
                Act::Survey { tag, columns } => {
                    crate::ground::test_answer(tag, &columns);
                    let wet: Vec<((i32, i32), i32)> = columns.iter().filter_map(|&(x, z)| water(x, z).map(|w| ((x, z), w))).collect();
                    crate::ground::set_test_water(&wet);
                }
                a => out.push((t, a)),
            }
        }
        out
    }
    fn plan() -> Plan {
        Plan { laser: &crate::lasers::EARTHQUAKE, ev: Vec::new(), rng: super::super::Rng::new(1), bx: 0, by: 64, bz: 0 }
    }

    /// The sea west of x = 37 (its bed at 50, twelve of water over it to the sea's level), the land east of it at 66.
    fn west_sea_ground(x: i32, _: i32) -> Option<i32> {
        Some(if x < 37 { 50 } else { 66 })
    }
    fn west_sea_water(x: i32, _: i32) -> Option<i32> {
        (x < 37).then_some(12)
    }

    #[test]
    fn sirens_go_up_on_every_stretch_of_land_round_the_player_wherever_they_go_and_none_at_sea() {
        crate::meter::reset();
        net_start(true, 7);
        let mut p = plan();
        let me = [60.5, 67.0, 0.5];
        p.later(0.0, move |q| net_watch(q, me, 0.0, 12.0));
        let out = drive(p.ev, 12.0, west_sea_ground, west_sea_water);
        let poles = net_heard(NET_ALL);
        // (a pole in each square near the player with land in it, on that land, its horns nine up)
        assert!(poles.len() >= 3, "{poles:?}");
        let nearest = poles.iter().map(|p| (p[0] - me[0]).hypot(p[2] - me[2])).fold(f64::MAX, f64::min);
        assert!(nearest < 160.0, "one near the player: {nearest}");
        assert!(poles.iter().all(|p| p[0] >= 37.0 && p[1] == 75.0), "{poles:?}");
        let built: Vec<&Blocks> = out.iter().filter_map(|(_, a)| if let Act::Blocks(b) = a { Some(&**b) } else { None }).collect();
        assert_eq!(built.len(), poles.len(), "each pole put up as blocks, once");
        assert!(built.iter().all(|b| b.runs.iter().all(|r| r.x >= 36)), "none in the sea");
        // (and every one near enough the player: none more than a square and a half off)
        assert!(poles.iter().all(|p| (p[0] - me[0]).hypot(p[2] - me[2]) < NET_R + f64::from(NET_LOOK) + 40.0));
        // (the groups between them: each pole in one)
        let by_group: usize = (0..NET_GROUPS).map(|g| net_heard(g).len()).sum();
        assert_eq!(by_group, poles.len());
        // a long way off (teleported): poles there too, the first kept
        let mut p = plan();
        let far = [2000.5, 67.0, 3000.5];
        p.later(0.0, move |q| net_watch(q, far, 0.0, 12.0));
        drive(p.ev, 12.0, west_sea_ground, west_sea_water);
        let now = net_heard(NET_ALL);
        assert!(now.iter().any(|p| (p[0] - far[0]).hypot(p[2] - far[2]) < 150.0), "a pole near where they went");
        assert!(now.len() > poles.len() && poles.iter().all(|p| now.contains(p)));
        // out at sea, far from any land: none
        net_start(true, 7);
        let mut p = plan();
        let sea = [-900.5, 63.0, 0.5];
        p.later(0.0, move |q| net_watch(q, sea, 0.0, 12.0));
        drive(p.ev, 12.0, west_sea_ground, west_sea_water);
        assert!(net_heard(NET_ALL).is_empty());
        // with the terrain off: where they would stand, but none built
        net_start(false, 7);
        let mut p = plan();
        p.later(0.0, move |q| net_watch(q, me, 0.0, 12.0));
        let out = drive(p.ev, 12.0, west_sea_ground, west_sea_water);
        assert!(!net_heard(NET_ALL).is_empty() && !out.iter().any(|(_, a)| matches!(a, Act::Blocks(_))));
        // the sirens heard from them, none with them off
        let mut p = plan();
        sirens(&mut p, 0.0, 100.0, true, 1.0, 3);
        eew_alert(&mut p, 1.0, 1.0, 3);
        assert!(p.ev.iter().all(|(_, a)| matches!(a, Act::Heard { net: Some(_), .. })));
        assert!(p.ev.len() > 12);
        net_stop();
        assert!(net_heard(NET_ALL).is_empty());
        crate::ground::set_test(None);
    }

    /// The sea south of z = -60, a river eight wide at x 20 to 27 (at the sea's level), a pond up on the hill at x 100 to
    /// 140 (above it), the land at 66.
    fn coast_ground(x: i32, z: i32) -> Option<i32> {
        Some(if z < -60 { 50 } else if (20..28).contains(&x) { 59 } else if (100..140).contains(&x) && (0..40).contains(&z) { 64 } else { 66 })
    }
    fn coast_water(x: i32, z: i32) -> Option<i32> {
        if z < -60 {
            Some(12)
        } else if (20..28).contains(&x) {
            Some(4)
        } else if (100..140).contains(&x) && (0..40).contains(&z) {
            Some(8)
        } else {
            None
        }
    }

    #[test]
    fn the_tsunami_comes_to_the_open_sea_nearest_the_player_not_a_river_or_a_pond() {
        crate::meter::reset();
        let me = [0.5, 67.0, 0.5];
        crate::ground::set_test(Some(coast_ground));
        let cols = coast_columns(me);
        let tag = crate::ground::new_tag();
        crate::ground::test_answer(tag, &cols);
        crate::ground::set_test_water(&cols.iter().filter_map(|&(x, z)| coast_water(x, z).map(|w| ((x, z), w))).collect::<Vec<_>>());
        let (t, nm) = find_coast(me).expect("the sea to the south");
        assert!((t[2] + 60.0).abs() <= 9.0 && t[0].abs() <= 9.0 && t[1] == f64::from(SEA + 1), "the shore south of the player: {t:?}");
        assert!(nm[1] > 0.9, "inland: north, towards the player: {nm:?}");
        // (the sea's columns there, for the sea to draw back from: water at its level over its bed)
        let sea_cols = coast_sea_columns(t, nm);
        let tag = crate::ground::new_tag();
        crate::ground::test_answer(tag, &sea_cols);
        crate::ground::set_test_water(&sea_cols.iter().filter_map(|&(x, z)| coast_water(x, z).map(|w| ((x, z), w))).collect::<Vec<_>>());
        let mut chunks = SeaChunks::new();
        coast_sea_cols(&mut chunks, &sea_cols);
        let q = &QK_PRESETS[0];
        let st = sea_time(q, 60.0).unwrap();
        let target = super::super::Target { block: [400, 70, 400], point: [400.5, 71.0, 400.5] };
        let geo = QkGeo::new(0, &target, [400.5, 71.0, 600.5], 17.0);
        let f = coast_flood(&geo, st, t, nm, chunks);
        assert!(f.moved && f.chunks.len() > 20, "{}", f.chunks.len());
        // (the sea's surface at 61, its bed at 49; the river's mouth with it, at its own; only what was asked about)
        let asked: std::collections::HashSet<(i32, i32)> = sea_cols.iter().copied().collect();
        for c in &f.chunks {
            for (i, col) in c.cols.iter().enumerate() {
                let (x, z) = (c.cx * 16 + (i % 16) as i32, c.cz * 16 + (i / 16) as i32);
                match coast_water(x, z).filter(|_| asked.contains(&(x, z))) {
                    Some(12) => assert!(col.kind == Kind::Sea && col.base == 49 && col.orig == 61),
                    Some(4) => assert!(col.kind == Kind::Sea && col.base == 58 && col.orig == 62),
                    _ => assert!(col.kind == Kind::Alone),
                }
            }
        }
        // (and the drawing back works from there: along its own way inland)
        let (_, d) = f.geo.ud(t[0], t[2] - 40.0);
        assert!(d < -35.0, "out to sea is d < 0: {d}");
        // no open sea near: none (a river and a pond are not one)
        let me2 = [0.5, 67.0, 300.5];
        let cols = coast_columns(me2);
        let tag = crate::ground::new_tag();
        crate::ground::test_answer(tag, &cols);
        crate::ground::set_test_water(&cols.iter().filter_map(|&(x, z)| coast_water(x, z).map(|w| ((x, z), w))).collect::<Vec<_>>());
        assert_eq!(find_coast(me2), None);
        crate::ground::set_test(None);
    }

    #[test]
    fn a_quakes_tsunami_moves_to_the_coast_the_player_is_near_and_draws_its_sea_back() {
        crate::meter::reset();
        // (the quake fired a long way inland; the player by the sea at the origin as it is about to draw back)
        let target = super::super::Target { block: [900, 70, 900], point: [900.5, 71.0, 900.5] };
        let geo = Rc::new(QkGeo::new(0, &target, [900.5, 71.0, 950.5], 17.0));
        let st = sea_time(geo.q, 60.0).unwrap();
        let old = Flood::new(geo.clone(), st);
        let flood = Rc::new(RefCell::new(old));
        set_pending(HandOff { flood: Some(flood.clone()), k: 0.08, kind: "MAJOR TSUNAMI WARNING", color: 0, expect: "10 m+", major: true, siren: 1.0, seed: 1, sea: (geo.t, geo.nm), coast: None });
        let mut p = plan();
        let me = [0.5, 67.0, 0.5];
        let at = (st.draw - 22.0) as f32;
        relocate(&mut p, geo.clone(), st, me, at, Some(flood.clone()), vec![0; super::super::quake::QK_TAB_COAST + 4], 400.0);
        let out = drive(p.ev, st.draw as f32, coast_ground, coast_water);
        let h = take_pending().expect("still pending");
        let (block, point) = h.coast.expect("moved to the player's coast");
        assert!(block[1] == SEA && (block[2] + 60).abs() <= 9 && point[1] == f64::from(SEA + 1), "{block:?}");
        assert!(h.sea.1[1] > 0.9, "the wave comes in from the sea, south: {:?}", h.sea.1);
        // (the pass told where: its table's coast, and the chunks kept loaded)
        let i = super::super::quake::QK_TAB_COAST;
        assert!(out.iter().any(|(_, a)| matches!(a, Act::Table(w) if f32::from_bits(w[i + 3]) > 0.9)));
        assert!(out.iter().any(|(_, a)| matches!(a, Act::Cmd(c) if c.starts_with("forceload add"))));
        assert!(out.iter().any(|(_, a)| matches!(a, Act::Spare(c) if c.starts_with("forceload remove"))));
        let f = flood.borrow();
        assert!(f.moved && !f.chunks.is_empty(), "the coast's sea is the one that draws back");
        drop(f);
        // the sea there draws back before the wave
        let mut q = plan();
        let mut fb = flood.borrow_mut();
        for k in 0..fb.chunks.len() {
            crate::meter::reset();
            visit(&mut fb, k, st.end - 0.5, &mut q);
        }
        assert!(fb.chunks.iter().any(|c| c.cols.iter().any(|c| c.kind == Kind::Sea && c.cur < c.orig - 3)), "the sea drawn back");
        // (none near the player: left at the target's coast)
        drop(fb);
        set_pending(HandOff { flood: None, k: 0.08, kind: "", color: 0, expect: "", major: true, siren: 1.0, seed: 1, sea: (geo.t, geo.nm), coast: None });
        let mut p = plan();
        relocate(&mut p, geo.clone(), st, [0.5, 67.0, 300.5], at, None, vec![0; super::super::quake::QK_TAB_COAST + 4], 400.0);
        drive(p.ev, st.draw as f32, coast_ground, coast_water);
        assert!(take_pending().is_some_and(|h| h.coast.is_none()));
        crate::ground::set_test(None);
    }

    #[test]
    fn every_other_lasers_strike_sounds_the_general_alert_from_the_poles_and_every_tsunami_the_major_siren() {
        // (the general alert: each group of poles every ALERT_CYCLE, from the network, at the sirens' volume)
        alert_configure(AlertConfig { on: true, vol: 0.5 });
        let mut p = plan();
        alert_sirens(&mut p, [0.5, 70.0, 0.5], true, 9, 60.0);
        let alerts: Vec<&Act> = p.ev.iter().map(|(_, a)| a).filter(|a| matches!(a, Act::Heard { event, .. } if event == "orbital:quake.alert")).collect();
        assert!(alerts.len() >= 2 * usize::from(NET_GROUPS), "{}", alerts.len());
        assert!(alerts.iter().all(|a| matches!(a, Act::Heard { net: Some(g), volume, far, .. } if *g < NET_GROUPS && (*volume - 0.5 * QT_POLE_VOL).abs() < 1e-6 && far == "orbital:quake.alert_far")));
        assert!(p.ev.iter().any(|(_, a)| matches!(a, Act::Later(_))), "the network watched");
        assert!(p.ev.iter().all(|(t, _)| *t < 60.0));
        // (off: none, and no poles)
        alert_configure(AlertConfig { on: false, vol: 1.0 });
        let mut p = plan();
        alert_sirens(&mut p, [0.5, 70.0, 0.5], true, 9, 60.0);
        assert!(p.ev.is_empty() && net_heard(NET_ALL).is_empty());
        alert_configure(AlertConfig { on: true, vol: 1.0 });
        // a tsunami's warning, whatever its height, on the Major Tsunami Warning's siren (the player's recording)
        for major in [true, false] {
            let mut p = plan();
            sirens(&mut p, 0.0, 120.0, major, 1.0, 4);
            assert!(p.ev.iter().all(|(_, a)| matches!(a, Act::Heard { event, .. } if event == "orbital:quake.major" || event == "orbital:quake.siren")));
            assert!(p.ev.iter().any(|(_, a)| matches!(a, Act::Heard { event, .. } if event == "orbital:quake.major")));
        }
    }

    #[test]
    fn qk_sea_draws_back_then_comes_in_waves_and_settles() {
        for q in QK_PRESETS.iter().filter(|q| q.tsunami.is_some()) {
            let ts = q.tsunami.as_ref().unwrap();
            let st = sea_time(q, 60.0).unwrap();
            assert!(st.warn < st.draw && st.draw < st.end && st.end < st.arrive, "{}", q.key);
            assert_eq!(sea_s(ts, &st, st.draw - 0.1), 0.0);
            assert!(sea_s(ts, &st, st.arrive - 0.5) < -0.8 * ts.draw, "{}: the sea drawn back before the first wave", q.key);
            let top = ts.waves.iter().map(|w| f64::from(w.1)).fold(0.0, f64::max);
            let after = st.arrive + ts.waves.iter().map(|w| f64::from(w.0 + w.2 + w.3 + w.4)).fold(0.0, f64::max) + 14.0;
            let seen = observed_max(ts, &st, after);
            assert!((seen - top).abs() < 0.6, "{}: its highest wave at the coast ({seen} of {top})", q.key);
            assert!(sea_s(ts, &st, after).abs() < 0.05, "{}: back to its own level after its last wave", q.key);
            assert!(st.end < st.arrive && st.draw + 4.0 < st.end, "{}: the quake hands over with the sea drawn back", q.key);
            assert!(sea_s(ts, &st, st.end) < -0.5 * ts.draw, "{}", q.key);
            // (inland: later, lower, and nothing past its reach)
            let peak = |x: f64| (0..2000).map(|k| level(ts, &st, x, st.arrive + f64::from(k) * 0.1)).fold(f64::MIN, f64::max);
            assert!(peak(60.0) < peak(0.0) && peak(ts.reach + 5.0) <= 1e-9, "{}", q.key);
        }
    }

    #[test]
    fn qk_sea_countdown_is_the_players_but_waits_for_the_shaking() {
        let q = &QK_PRESETS[0];
        let a = sea_time(q, 60.0).unwrap();
        let b = sea_time(q, 180.0).unwrap();
        assert!((b.arrive - a.arrive - 120.0).abs() < 1e-9);
        let c = sea_time(q, 10.0).unwrap();
        assert!(c.arrive - c.warn > 10.0, "never before the strong shaking is over");
        assert!(qk_length(q, 300.0) > qk_length(q, 60.0));
        assert_eq!(qk_length(&QK_PRESETS[1], 60.0), QK_PRESETS[1].length, "Kobe raised no tsunami");
    }

    /// A strip of coast: sea out to the target, low land inland of it with a wooden house, a hill further in.
    #[test]
    fn qk_sea_floods_the_low_land_and_puts_it_all_back() {
        crate::meter::reset();
        let target = super::super::Target { block: [0, 61, 0], point: [0.5, 62.0, 0.5] };
        let geo = Rc::new(QkGeo::new(0, &target, [0.5, 70.0, 300.5], 17.0));
        let ts = geo.q.tsunami.as_ref().unwrap();
        let st = sea_time(geo.q, 60.0).unwrap();
        let mut f = Flood::new(geo.clone(), st);
        let mut grounds = Vec::new();
        for cz in -2..6 {
            let mut cols = Box::new([FCol::default(); 256]);
            for (i, c) in cols.iter_mut().enumerate() {
                let (x, z) = ((i % 16) as i32, cz * 16 + (i / 16) as i32);
                let (_, d) = geo.ud(f64::from(x) + 0.5, f64::from(z) + 0.5);
                *c = if d < 0.0 {
                    FCol { kind: Kind::Sea, base: 55, orig: 62, cur: 62, hi: 62, top: 62, house: 0, washed: false }
                } else {
                    let g = (63.0 + d / 12.0) as i16;
                    let house = u8::from((20.0..30.0).contains(&d));
                    FCol { kind: Kind::Land, base: g, orig: g, cur: g, hi: g, top: if house > 0 { g + 5 } else { g }, house, washed: false }
                };
                grounds.push((cz, i, c.base));
            }
            f.add(0, cz, cols);
        }
        assert!(f.chunks.len() >= 4, "the hill past its reach left out");
        let f = Rc::new(RefCell::new(f));
        let mut q = Plan { laser: &crate::lasers::EARTHQUAKE, ev: Vec::new(), rng: super::super::Rng::new(1), bx: 0, by: 61, bz: 0 };
        let mut t = st.draw - 1.0;
        let mut drawn = false;
        let mut flooded = false;
        let horizon = st.arrive + 130.0;
        while t < horizon {
            {
                let mut fb = f.borrow_mut();
                for k in 0..fb.chunks.len() {
                    crate::meter::reset();
                    visit(&mut fb, k, t, &mut q);
                }
                let fc = &fb.chunks;
                drawn |= fc.iter().any(|c| c.cols.iter().any(|c| c.kind == Kind::Sea && c.cur < c.orig - 1));
                flooded |= fc.iter().any(|c| c.cols.iter().any(|c| c.kind == Kind::Land && c.cur > c.base + 3));
            }
            t += 1.0;
        }
        assert!(drawn, "the sea drew back");
        assert!(flooded, "the low land flooded");
        assert!(f.borrow().washed > 0, "a wooden house in deep water washed away");
        assert!(sea_s(ts, &st, horizon).abs() < 0.05);
        let fb = f.borrow();
        assert!(fb.chunks.iter().all(|c| c.cols.iter().all(|c| c.cur == c.orig)), "all back where it was at the end");
        // (only air filled, only water taken out)
        for (_, a) in &q.ev {
            if let Act::Blocks(b) = a {
                for r in b.as_fills() {
                    assert!(r.contains("water replace minecraft:air") || r.contains("air replace minecraft:water") || r.contains("minecraft:air strict"), "{r}");
                }
            }
        }
        drop(fb);
        let mut q2 = Plan { laser: &crate::lasers::EARTHQUAKE, ev: Vec::new(), rng: super::super::Rng::new(1), bx: 0, by: 61, bz: 0 };
        sea_restore(&mut q2, &f, 0, 1, 1.0);
        assert!(q2.ev.iter().all(|(_, a)| matches!(a, Act::Blocks(b) if b.cleanup)));
    }
}
