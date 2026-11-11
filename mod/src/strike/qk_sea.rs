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
/// The countdown to arrival when none is set, and the most it may be (seconds).
pub const QT_COUNTDOWN: f64 = 60.0;
pub const QT_COUNTDOWN_MAX: f64 = 300.0;

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
    let warn = t0 + f64::from(q.t_main()) + 0.6 * f64::from(q.main_dur());
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
}

impl Flood {
    pub fn new(geo: Rc<QkGeo>, st: SeaTime) -> Flood {
        Flood { geo, st, chunks: Vec::new(), next: 0, washed: 0, peak: 0 }
    }
    fn ts(&self) -> &'static Tsu {
        self.geo.q.tsunami.as_ref().expect("a tsunami")
    }
    /// Whether the chunk (cx, cz) lies where the sea works.
    pub fn covers(&self, cx: i32, cz: i32) -> bool {
        let (u, d) = self.geo.ud(f64::from(cx) * 16.0 + 8.0, f64::from(cz) * 16.0 + 8.0);
        u.abs() <= QT_HALF + 8.0 && d >= -QT_OUT - 8.0 && d <= self.ts().reach + 16.0
    }
    /// A chunk as the quake left it: kept if the sea can reach any of it.
    pub fn add(&mut self, cx: i32, cz: i32, cols: Box<[FCol; 256]>) {
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

/// Where the coast's J-Alert siren poles stand (x, z): four along it a little inland of the target's line, two far off
/// along it, the one nearest the player first.
pub fn siren_sites(geo: &QkGeo, me: [f64; 3]) -> Vec<[f64; 2]> {
    // (four along the coast near the target, and two more a long way off along it, inland: the next towns')
    let mut out: Vec<[f64; 2]> = [(-120.0, 0.0), (-40.0, 0.0), (40.0, 0.0), (120.0, 0.0), (-300.0, 120.0), (330.0, 160.0)]
        .iter()
        .enumerate()
        .map(|(k, &(u, inland))| {
            let u = u + (super::mh(k as u32, geo.seed, 0x51e) - 0.5) * 30.0;
            let d = 26.0 + inland + super::mh(k as u32, geo.seed, 0x51f) * 14.0;
            [(geo.t[0] + u * geo.st[0] + d * geo.nm[0]).floor() + 0.5, (geo.t[2] + u * geo.st[1] + d * geo.nm[1]).floor() + 0.5]
        })
        .collect();
    out.sort_by(|a, b| (a[0] - me[0]).hypot(a[1] - me[2]).total_cmp(&(b[0] - me[0]).hypot(b[1] - me[2])));
    out
}

/// A J-Alert siren pole at (x, z), as real blocks on its ground: a grey metal post (iron bars, which the quake's damage
/// leaves standing) eleven high, loudspeaker horns facing out
/// four ways near its top and two more under them, a lightning rod on its crown.
pub fn siren_pole(x: i32, z: i32) -> Blocks {
    let mut b = Blocks { clear_of_player: true, ..Default::default() };
    b.put(x, z, 0, 10, "minecraft:iron_bars", None, true);
    b.put(x, z, 11, 11, "minecraft:waxed_lightning_rod[facing=up]", None, true);
    let horns: [(i32, i32, &'static str); 4] = [
        (0, -1, "minecraft:waxed_lightning_rod[facing=north]"),
        (1, 0, "minecraft:waxed_lightning_rod[facing=east]"),
        (0, 1, "minecraft:waxed_lightning_rod[facing=south]"),
        (-1, 0, "minecraft:waxed_lightning_rod[facing=west]"),
    ];
    for (k, (dx, dz, horn)) in horns.iter().enumerate() {
        b.put(x + dx, z + dz, 9, 9, horn, None, true);
        if k % 2 == 1 {
            b.put(x + dx, z + dz, 7, 7, horn, None, true);
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
    pub poles: Vec<[f64; 2]>,
    pub siren: f32,
    pub seed: u32,
    /// The coast's middle and the way inland from it (the sea the other way): the wave comes from the side the sea drew
    /// back from, wherever the player has gone since (`on_sea_axis`).
    pub sea: ([f64; 3], [f64; 2]),
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

/// The coast's disaster-prevention radio, from its poles, as a town's sounds in life: no chime, the siren twice in the
/// JMA's pattern (a Major Tsunami Warning's 3 s on and 2 s off: `quake.major`, two blasts, twice; a Tsunami Warning's
/// 5 s on and 6 s off: `siren5`, twice), then the pause its announcement is spoken in, and again every `QT_CYCLE`,
/// from `from` to `until`. Every pole is its own station: started some seconds apart from the next (each town's
/// set off on its own), heard late by its distance (at 343 a second) and fainter, the far ones muffled and echoing
/// (`_far`), so near the coast one wails while others answer from further off. `vol` the player's siren volume (1 as
/// made); none at all at 0.
#[allow(clippy::too_many_arguments)]
pub fn sirens(p: &mut Plan, poles: &[[f64; 2]], y: f64, me: [f64; 3], from: f64, until: f64, major: bool, vol: f32, seed: u32) {
    if vol <= 0.0 {
        return;
    }
    let (blast, every) = if major { ("major", 10.0) } else { ("siren5", 11.0) };
    for (k, &[x, z]) in poles.iter().enumerate() {
        let d = (x - me[0]).hypot(z - me[2]);
        let far = if d > 140.0 { "_far" } else { "" };
        let late = super::mh(k as u32, seed, 0x5a1) * 8.0 + d / 343.0;
        let at = Some([x, y + 9.0, z]);
        let v = QT_POLE_VOL * vol;
        let push = |p: &mut Plan, t: f64, ev: &str| {
            if t >= from && t < until + 6.0 {
                p.ev.push((t as f32, Act::Sound { event: format!("orbital:quake.{ev}{far}"), at, volume: v, pitch: 1.0 }));
            }
        };
        let mut c = from + late;
        while c < until {
            push(p, c, blast);
            push(p, c + every, blast);
            c += QT_CYCLE;
        }
    }
    // (and further off still, past the poles: the player's own recording of a coast's sirens, if they put one in, faint)
    let mut t = from + 6.0;
    while t < until {
        p.ev.push((t as f32, Act::Sound { event: "orbital:quake.siren".into(), at: None, volume: (0.12 * vol).min(1.0), pitch: 1.0 }));
        t += 16.0;
    }
}

/// The disaster-prevention radio's earthquake alert from the poles at `t`, a broadcast a pole as `sirens` has them.
pub fn eew_alert(p: &mut Plan, poles: &[[f64; 2]], y: f64, me: [f64; 3], t: f64, vol: f32, seed: u32) {
    if vol <= 0.0 {
        return;
    }
    for (k, &[x, z]) in poles.iter().enumerate() {
        let d = (x - me[0]).hypot(z - me[2]);
        let far = if d > 140.0 { "_far" } else { "" };
        let late = super::mh(k as u32, seed, 0x5a2) * 1.5 + d / 343.0;
        p.ev.push(((t + late) as f32, Act::Sound { event: format!("orbital:quake.eew{far}"), at: Some([x, y + 9.0, z]), volume: 0.8 * QT_POLE_VOL * vol, pitch: 1.0 }));
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
    sirens(p, &h.poles, geo.t[1], me, 0.0, tt + 24.0, h.major, h.siren, h.seed);
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
