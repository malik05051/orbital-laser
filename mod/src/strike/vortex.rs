//! Vortices: the funnels a storm lets down onto the land, as reusable pieces. The Supercell drops a few thin landspouts
//! out of its wall cloud (storm.rs); a full tornado strike is meant to be built from the same parts, scaled up.
//!
//! - `Funnel` and `wander`: a funnel's life (it comes down out of the cloud over VX_DOWN seconds, touches the land,
//!   wanders across it, lifts back up) and its track, a smooth random path sampled every VX_DT, deterministic per strike.
//! - `vx_words`: what the pass reads of a funnel (its times, its touch radius and its track, with the ground under it),
//!   VX_W words each: shaders/supercell.wgsl's vortex section (`vx_*`) draws the funnel from them. Keep VX_* in step.
//! - `scour`: the light terrain along its track, a stage a stretch of it: the grass and small plants stripped, the
//!   leaves torn off the trees it passes through, the grass scoured to dirt, a few blocks thrown up and out. No digging.
//! - `roar`: its roar played where it is, again every second as it moves, so the sound follows it.
//!
//! - `track`, `wreck`, `roar_as`: the Tornado's (storm.rs's `tornado_plan`): a straight track, what it wrecks by its EF
//!   rating, its roar louder and deeper; the pass's `vx_radius` and debris cloud scale with its touch radius.
use super::{Act, Blocks, Plan, TIDY_FLOWERS, TIDY_LOOSE, mh, vnoise};
use std::f64::consts::PI;

/// Seconds between a funnel's track samples, the most samples a funnel has, and the pass's words for one.
pub(super) const VX_DT: f64 = 0.5;
pub(super) const VX_S: usize = 96;
pub(super) const VX_W: usize = 4 + 3 * VX_S;
/// Seconds a funnel takes to come down out of the cloud to the land, and to lift back off it.
pub(super) const VX_DOWN: f64 = 3.5;
/// Seconds before it is gone that it lifts off the land (it ropes out over the 4 s before: supercell.wgsl's VX_ROPE).
pub(super) const VX_LIFT: f64 = 1.6;

#[derive(Clone, Debug)]
pub(super) struct Funnel {
    /// When it starts down out of the cloud, and when it is gone back up into it.
    pub t0: f64,
    pub t1: f64,
    /// Its foot every VX_DT from t0, off the storm's middle (x, z).
    pub path: Vec<[f64; 2]>,
    /// How far round its foot it scours the land while it touches.
    pub touch_r: f64,
}

impl Funnel {
    /// Where its foot is at `t` (off the storm's middle).
    pub fn at(&self, t: f64) -> [f64; 2] {
        let u = ((t - self.t0) / VX_DT).clamp(0.0, (self.path.len() - 1) as f64);
        let i = (u.floor() as usize).min(self.path.len().saturating_sub(2));
        let f = u - i as f64;
        let (a, b) = (self.path[i], self.path[(i + 1).min(self.path.len() - 1)]);
        [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f]
    }
    /// How much it is on the land at `t`: 0 aloft (coming down or lifting), 1 touching.
    pub fn touch(&self, t: f64) -> f64 {
        let s = |a: f64, b: f64, x: f64| {
            let u = ((x - a) / (b - a)).clamp(0.0, 1.0);
            u * u * (3.0 - 2.0 * u)
        };
        s(self.t0 + 0.8 * VX_DOWN, self.t0 + VX_DOWN, t) * (1.0 - s(self.t1 - VX_LIFT - 0.2, self.t1 - VX_LIFT + 0.4, t))
    }
}

/// A smooth random wander for funnel `k` of `seed`: from `start` (off the middle), heading `h0` (radians round from +x
/// towards +z), at about `speed` blocks a second, turning slowly back and forth; drawn back in towards the middle once it
/// is past `keep` blocks out, so it stays under its cloud.
pub(super) fn wander(seed: u32, k: u32, start: [f64; 2], h0: f64, speed: f64, t0: f64, t1: f64, keep: f64, touch_r: f64) -> Funnel {
    let n = (((t1 - t0) / VX_DT).ceil() as usize + 1).min(VX_S);
    let r = |j: u32| mh(k * 31 + j, seed, 0x7a11);
    let (w1, w2) = (0.25 + 0.2 * r(1), 0.6 + 0.4 * r(2));
    let (p1, p2) = (2.0 * PI * r(3), 2.0 * PI * r(4));
    let (a1, a2) = (0.9 + 0.5 * r(5), 0.35 * r(6));
    let mut p = start;
    let mut path = Vec::with_capacity(n);
    for i in 0..n {
        path.push(p);
        let t = i as f64 * VX_DT;
        let mut h = h0 + a1 * (w1 * t + p1).sin() + a2 * (w2 * t + p2).sin();
        // (past `keep`, steered back in: towards the middle, more the further out)
        let rr = p[0].hypot(p[1]);
        if rr > keep {
            let home = (-p[1]).atan2(-p[0]);
            let pull = ((rr - keep) / 30.0).min(1.0);
            let d = (home - h + PI).rem_euclid(2.0 * PI) - PI;
            h += d * pull;
        }
        let v = speed * (1.0 + 0.3 * (0.7 * t + p2).sin());
        p = [p[0] + h.cos() * v * VX_DT, p[1] + h.sin() * v * VX_DT];
    }
    Funnel { t0, t1: t0 + (n - 1) as f64 * VX_DT, path, touch_r }
}

/// Where the land is asked round each sample of a track (`ground_track`): the sample's own column and eight round it,
/// three and four blocks out.
pub(super) const VX_PROBE: [(i32, i32); 9] = [(0, 0), (3, 0), (-3, 0), (0, 3), (0, -3), (3, 3), (-3, -3), (3, -3), (-3, 3)];
/// The land under a funnel's track, as its foot glides over it: at each sample the lowest of the land round it
/// (`ground`: the world's ground, which counts a tree's trunk), so it never climbs a tree or a wall it passes, then
/// smoothed along the track (twice over two seconds or so) so it never hops on a step of the land. None where nothing
/// round a sample is known: the known ones round it carry over, 0 if there are none.
pub(super) fn ground_track(path: &[[f64; 2]], ground: impl Fn(f64, f64) -> Option<f64>) -> Vec<f64> {
    crate::meter::spend(2_000 * path.len() as u64);
    let raw: Vec<Option<f64>> = path
        .iter()
        .map(|p| VX_PROBE.iter().filter_map(|&(dx, dz)| ground(p[0] + f64::from(dx), p[1] + f64::from(dz))).reduce(f64::min))
        .collect();
    let known: Vec<(usize, f64)> = raw.iter().enumerate().filter_map(|(i, v)| v.map(|v| (i, v))).collect();
    let mut h: Vec<f64> = (0..path.len())
        .map(|i| raw[i].unwrap_or_else(|| known.iter().min_by_key(|(j, _)| j.abs_diff(i)).map_or(0.0, |k| k.1)))
        .collect();
    for _ in 0..2 {
        h = (0..h.len())
            .map(|i| {
                let (a, b) = (i.saturating_sub(2), (i + 2).min(h.len() - 1));
                h[a..=b].iter().sum::<f64>() / (b - a + 1) as f64
            })
            .collect();
    }
    h
}

/// The pass's words for a funnel (VX_W of them): t0, t1, how many samples, its touch radius, then each sample's x, z
/// (off the middle) and the land's height there over the middle's (`dy`, from `ground_track`: one a sample).
pub(super) fn vx_words(f: &Funnel, dy: &[f64]) -> Vec<u32> {
    let mut w = vec![0u32; VX_W];
    w[0] = (f.t0 as f32).to_bits();
    w[1] = (f.t1 as f32).to_bits();
    w[2] = (f.path.len() as f32).to_bits();
    w[3] = (f.touch_r as f32).to_bits();
    for (i, p) in f.path.iter().enumerate() {
        w[4 + 3 * i] = (p[0] as f32).to_bits();
        w[5 + 3 * i] = (p[1] as f32).to_bits();
        w[6 + 3 * i] = (dy.get(i).copied().unwrap_or(0.0) as f32).to_bits();
    }
    w
}

/// What the funnel scours where it touches, over its track's samples k0..k1: each column within its touch radius (a
/// little ragged) once, when the funnel first reaches it: the loose plants and flowers gone, the leaves over it torn off
/// up to 24 high, the grass scoured to dirt in streaks; and now and then a block of the ground thrown up and out, spun
/// round it. `seen` holds the columns already scoured (the track crosses itself). (cx, cz): the storm's middle.
pub(super) fn scour(q: &mut Plan, f: &Funnel, k0: usize, k1: usize, cx: i32, cz: i32, seed: u32, seen: &mut crate::ground::FxSet<(i32, i32)>) {
    let mut b = Blocks::default();
    let mut when: crate::ground::FxMap<(i32, i32), f32> = Default::default();
    let r = f.touch_r;
    let ri = r.ceil() as i32 + 1;
    for k in k0..k1.min(f.path.len()) {
        let t = f.t0 + k as f64 * VX_DT;
        if f.touch(t) < 0.5 {
            continue;
        }
        let [px, pz] = f.path[k];
        let (fx, fz) = (f64::from(cx) + 0.5 + px, f64::from(cz) + 0.5 + pz);
        let (ix, iz) = (fx.floor() as i32, fz.floor() as i32);
        for dz in -ri..=ri {
            for dx in -ri..=ri {
                let (x, z) = (ix + dx, iz + dz);
                let d = (f64::from(x) + 0.5 - fx).hypot(f64::from(z) + 0.5 - fz);
                crate::meter::spend(40);
                if d > r * (0.8 + 0.35 * vnoise(f64::from(x), f64::from(z), 3.0, seed ^ 0x51)) || !seen.insert((x, z)) {
                    continue;
                }
                when.insert((x, z), t as f32);
                b.put(x, z, 0, 3, "minecraft:air", Some(TIDY_LOOSE), true);
                b.put(x, z, 0, 2, "minecraft:air", Some(TIDY_FLOWERS), true);
                b.put(x, z, 0, 24, "minecraft:air", Some("#minecraft:leaves"), true);
                if mh((x as u32).wrapping_mul(73_856_093) ^ (z as u32).wrapping_mul(19_349_663), seed, 0x5c) < 0.55 + 0.4 * (1.0 - d / r) {
                    let to = if mh(x as u32 ^ (z as u32).rotate_left(13), seed, 0x5d) < 0.6 { "minecraft:coarse_dirt" } else { "minecraft:dirt" };
                    b.put(x, z, -1, -1, to, Some("minecraft:grass_block"), true);
                }
            }
        }
        // (a block of the ground thrown up and out every so often, spun round the funnel: falling blocks, which come
        // down a few seconds later off the track, breaking up where they land)
        if k % 3 == 0 {
            let a = 2.0 * PI * mh(k as u32, seed, 0x5e);
            let (ox, oz) = (a.cos() * r * 0.8, a.sin() * r * 0.8);
            let (vx, vz) = (-a.sin() * 0.55 + a.cos() * 0.25, a.cos() * 0.55 + a.sin() * 0.25);
            let vy = 0.7 + 0.4 * mh(k as u32, seed, 0x5f);
            let blk = ["minecraft:dirt", "minecraft:coarse_dirt", "minecraft:oak_leaves", "minecraft:gravel"][(mh(k as u32, seed, 0x60) * 4.0) as usize % 4];
            let (x, z) = ((fx + ox).floor() as i32, (fz + oz).floor() as i32);
            q.ev.push((
                t as f32,
                Act::Cmd(format!(
                    "execute positioned {x} 0 {z} positioned over motion_blocking_no_leaves run summon minecraft:falling_block ~ ~0.5 ~ {{BlockState:{{Name:\"{blk}\"}},Time:1,DropItem:0b,CancelDrop:1b,Motion:[{vx:.3}d,{vy:.3}d,{vz:.3}d]}}"
                )),
            ));
        }
    }
    for (t, e) in b.split_by(|x, z| when.get(&(x, z)).copied().unwrap_or(0.0)) {
        q.ev.push((t, Act::Blocks(Box::new(e))));
    }
}

/// What a tornado of EF rating `ef` wrecks where it touches, over its track's samples k0..k1 (`scour`'s light work done
/// harder): each column within its touch radius once, the core (inside 55% of it) at its rating and the edge one less,
/// each rating doing what the ones under it do and more, as the Enhanced Fujita scale's damage goes:
/// - EF0: the loose plants gone, leaves torn off in patches, the grass scoured in streaks.
/// - EF1: every leaf, the windows (glass panes, glass) broken, doors, trapdoors and fences torn away.
/// - EF2: wooden roofs off (their stairs and slabs, the planks under), the trees' crowns snapped.
/// - EF3: wooden walls down, trees uprooted whole, stone roofs and walls off, beds and wool gone.
/// - EF4: cobblestone, brick, stone brick and terracotta houses half levelled, all roofs.
/// - EF5: every house swept away to its floor, the ground itself scoured (its top to coarse dirt and gravel).
/// Blocks are thrown up and out round it (`throw`), more and heavier the stronger it is. (cx, cz): the storm's middle.
/// A tornado's wrecking under way (`wreck_stage`): its funnel, the storm's middle, its rating, how far along its track
/// the columns it reaches have been found, those found, and those still to wreck (x, z, when, at what rating).
pub(super) struct Wreck {
    pub f: std::rc::Rc<Funnel>,
    pub cx: i32,
    pub cz: i32,
    pub seed: u32,
    pub ef: u8,
    next_k: usize,
    seen: crate::ground::FxSet<(i32, i32)>,
    queue: std::collections::VecDeque<(i32, i32, f32, u8)>,
}
impl Wreck {
    pub fn new(f: std::rc::Rc<Funnel>, cx: i32, cz: i32, seed: u32, ef: u8) -> Wreck {
        Wreck { f, cx, cz, seed, ef, next_k: 0, seen: Default::default(), queue: Default::default() }
    }
    /// The columns sample k reaches first (a little ragged), queued: the core (inside 55% of its touch radius) at its
    /// rating, the edge one less; and the blocks it throws.
    fn find(&mut self, q: &mut Plan, k: usize) {
        let f = self.f.clone();
        let t = f.t0 + k as f64 * VX_DT;
        if f.touch(t) < 0.5 {
            return;
        }
        let r = f.touch_r;
        let ri = (r * 1.15).ceil() as i32 + 1;
        let [px, pz] = f.path[k];
        let (fx, fz) = (f64::from(self.cx) + 0.5 + px, f64::from(self.cz) + 0.5 + pz);
        let (ix, iz) = (fx.floor() as i32, fz.floor() as i32);
        crate::meter::spend(30 * ((2 * ri + 1) * (2 * ri + 1)) as u64);
        for dz in -ri..=ri {
            for dx in -ri..=ri {
                let (x, z) = (ix + dx, iz + dz);
                let d = (f64::from(x) + 0.5 - fx).hypot(f64::from(z) + 0.5 - fz);
                if d > r * 1.15 || self.seen.contains(&(x, z)) {
                    continue;
                }
                crate::meter::spend(250);
                if d > r * (0.8 + 0.35 * vnoise(f64::from(x), f64::from(z), 3.0, self.seed ^ 0x51)) {
                    continue;
                }
                self.seen.insert((x, z));
                let level = if d < 0.55 * r { self.ef } else { self.ef.saturating_sub(1) };
                self.queue.push_back((x, z, t as f32, level));
            }
        }
        if k % 2 == 0 {
            for j in 0..1 + u32::from(self.ef) / 2 {
                throw(q, &f, k, j, fx, fz, t, self.seed, self.ef);
            }
        }
    }
}
/// The edits that wreck column (x, z) at `level`, as the Enhanced Fujita scale's damage goes, each rating doing what
/// the ones under it do and more:
/// - EF0: the loose plants gone, leaves torn off in patches, the grass scoured in streaks.
/// - EF1: every leaf, the windows (glass panes, glass) broken, doors, trapdoors and fences torn away.
/// - EF2: wooden roofs off (their stairs and slabs, the planks under), the trees' crowns snapped.
/// - EF3: wooden walls down, trees uprooted whole, stone roofs and walls off, beds and wool gone.
/// - EF4: cobblestone, brick, stone brick and terracotta houses half levelled, all roofs.
/// - EF5: every house swept away to its floor, the ground itself scoured (its top to coarse dirt and gravel).
/// How many edits it made.
fn wreck_column(b: &mut Blocks, x: i32, z: i32, level: u8, seed: u32) -> u64 {
    const WOOD_OPEN: [&str; 4] = ["#minecraft:wooden_doors", "#minecraft:wooden_trapdoors", "#minecraft:wooden_fences", "#minecraft:fence_gates"];
    const GLASS: [&str; 2] = ["minecraft:glass_pane", "#minecraft:impermeable"];
    const STONE: [&str; 6] = ["minecraft:cobblestone", "minecraft:mossy_cobblestone", "minecraft:bricks", "#minecraft:stone_bricks", "#minecraft:terracotta", "minecraft:hay_block"];
    let n0 = b.runs.len();
    let h = (x as u32).wrapping_mul(73_856_093) ^ (z as u32).wrapping_mul(19_349_663);
    let roll = |kk: u32| mh(h, seed, kk);
    b.put(x, z, 0, 3, "minecraft:air", Some(TIDY_LOOSE), true);
    b.put(x, z, 0, 2, "minecraft:air", Some(TIDY_FLOWERS), true);
    if level >= 1 || roll(0x70) < 0.5 {
        b.put(x, z, 0, 28, "minecraft:air", Some("#minecraft:leaves"), true);
    }
    if roll(0x5c) < 0.3 + 0.12 * f64::from(level) {
        let to = if level >= 5 && roll(0x71) < 0.4 { "minecraft:gravel" } else if roll(0x5d) < 0.6 { "minecraft:coarse_dirt" } else { "minecraft:dirt" };
        b.put(x, z, -1, -1, to, Some("minecraft:grass_block"), true);
    }
    if level >= 1 {
        for g in GLASS {
            b.put(x, z, -12, -1, "minecraft:air", Some(g), true);
        }
        for w in WOOD_OPEN {
            b.put(x, z, -12, -1, "minecraft:air", Some(w), true);
        }
    }
    if level >= 2 {
        b.put(x, z, -8, -1, "minecraft:air", Some("#minecraft:wooden_stairs"), true);
        b.put(x, z, -8, -1, "minecraft:air", Some("#minecraft:wooden_slabs"), true);
        b.put(x, z, -3, -1, "minecraft:air", Some("#minecraft:planks"), true);
        b.put(x, z, -6, -1, "minecraft:air", Some("#minecraft:logs"), true);
    }
    if level >= 3 {
        b.put(x, z, -14, -1, "minecraft:air", Some("#minecraft:planks"), true);
        b.put(x, z, -32, -1, "minecraft:air", Some("#minecraft:logs"), true);
        b.put(x, z, -6, -1, "minecraft:air", Some("#minecraft:stairs"), true);
        b.put(x, z, -6, -1, "minecraft:air", Some("#minecraft:slabs"), true);
        b.put(x, z, -6, -1, "minecraft:air", Some("#minecraft:walls"), true);
        b.put(x, z, -14, -1, "minecraft:air", Some("#minecraft:beds"), true);
        b.put(x, z, -14, -1, "minecraft:air", Some("#minecraft:wool"), true);
    }
    if level >= 5 || (level >= 4 && roll(0x72) < 0.6) {
        let depth = if level >= 5 { -16 } else { -10 };
        for s in STONE {
            b.put(x, z, depth, -1, "minecraft:air", Some(s), true);
        }
        b.put(x, z, -12, -1, "minecraft:air", Some("#minecraft:stairs"), true);
        b.put(x, z, -12, -1, "minecraft:air", Some("#minecraft:slabs"), true);
    }
    if level >= 5 && roll(0x73) < 0.45 {
        b.put(x, z, -1, -1, "minecraft:air", Some("minecraft:dirt"), true);
    }
    (b.runs.len() - n0) as u64
}
/// How much a stage of `wreck_stage` works at most (the meter's count: an edit's work counted at what it costs).
const WRECK_BUDGET: u64 = 5_000_000;
const WRECK_EDIT: u64 = 900;
/// One stage of a tornado's wrecking at strike second `now`: the columns its track reaches up to a second ahead found
/// and queued, then as many of the queued wrecked (`wreck_column`) as the stage's budget allows, each due as the funnel
/// reaches it; again a little later while any are left. So however wide it is, no tick does more than its share.
pub(super) fn wreck_stage(q: &mut Plan, w: std::rc::Rc<std::cell::RefCell<Wreck>>, now: f64) {
    let more = {
        let mut wr = w.borrow_mut();
        let f = wr.f.clone();
        while wr.next_k < f.path.len() && f.t0 + wr.next_k as f64 * VX_DT <= now + 1.0 && crate::meter::spent() < WRECK_BUDGET / 2 {
            let k = wr.next_k;
            wr.find(q, k);
            wr.next_k += 1;
        }
        let mut b = Blocks::default();
        let mut when: crate::ground::FxMap<(i32, i32), f32> = Default::default();
        while crate::meter::spent() < WRECK_BUDGET {
            let Some((x, z, t, level)) = wr.queue.pop_front() else { break };
            let seed = wr.seed;
            let n = wreck_column(&mut b, x, z, level, seed);
            crate::meter::spend(WRECK_EDIT * n);
            when.insert((x, z), t);
        }
        for (t, e) in b.split_by(|x, z| when.get(&(x, z)).copied().unwrap_or(0.0)) {
            q.ev.push((t, Act::Blocks(Box::new(e))));
        }
        wr.next_k < f.path.len() || !wr.queue.is_empty()
    };
    if more {
        let next = now + 0.1;
        q.later(next as f32, move |q| wreck_stage(q, w, next));
    }
}
/// A block thrown up off the land round a tornado's foot at sample k (the j-th then), spun round it and flung out: a
/// falling block, coming down a few seconds later off the track. What it is goes by the rating: soil and leaves,
/// then wood, then stone.
#[allow(clippy::too_many_arguments)]
fn throw(q: &mut Plan, f: &Funnel, k: usize, j: u32, fx: f64, fz: f64, t: f64, seed: u32, ef: u8) {
    let i = k as u32 * 7 + j;
    let r = f.touch_r;
    let a = 2.0 * PI * mh(i, seed, 0x5e);
    let (ox, oz) = (a.cos() * r * 0.8, a.sin() * r * 0.8);
    let w = 0.45 + 0.1 * f64::from(ef);
    let (vx, vz) = (-a.sin() * w + a.cos() * 0.25, a.cos() * w + a.sin() * 0.25);
    let vy = 0.7 + 0.12 * f64::from(ef) + 0.4 * mh(i, seed, 0x5f);
    let pool: &[&str] = match ef {
        0 | 1 => &["minecraft:dirt", "minecraft:coarse_dirt", "minecraft:oak_leaves", "minecraft:gravel"],
        2 | 3 => &["minecraft:oak_planks", "minecraft:spruce_log", "minecraft:oak_leaves", "minecraft:dirt", "minecraft:glass"],
        _ => &["minecraft:cobblestone", "minecraft:bricks", "minecraft:oak_planks", "minecraft:oak_log", "minecraft:stone_bricks", "minecraft:gravel"],
    };
    let blk = pool[(mh(i, seed, 0x60) * pool.len() as f64) as usize % pool.len()];
    let (x, z) = ((fx + ox).floor() as i32, (fz + oz).floor() as i32);
    q.ev.push((
        t as f32,
        Act::Cmd(format!(
            "execute positioned {x} 0 {z} positioned over motion_blocking_no_leaves run summon minecraft:falling_block ~ ~0.5 ~ {{BlockState:{{Name:\"{blk}\"}},Time:1,DropItem:0b,CancelDrop:1b,Motion:[{vx:.3}d,{vy:.3}d,{vz:.3}d]}}"
        )),
    ));
}
/// A straight track for a tornado: from `start` (off the storm's middle) heading `h` at `speed` blocks a second, its
/// foot weaving a little either side (`weave` blocks), from `t0` for as long as the table holds (VX_S samples).
pub(super) fn track(seed: u32, start: [f64; 2], h: f64, speed: f64, weave: f64, t0: f64, touch_r: f64) -> Funnel {
    let (w1, p1, w2, p2) = (0.18 + 0.1 * mh(1, seed, 0x7b), 2.0 * PI * mh(2, seed, 0x7b), 0.5 + 0.2 * mh(3, seed, 0x7b), 2.0 * PI * mh(4, seed, 0x7b));
    let (dir, side) = ([h.cos(), h.sin()], [-h.sin(), h.cos()]);
    let path: Vec<[f64; 2]> = (0..VX_S)
        .map(|i| {
            let t = i as f64 * VX_DT;
            let along = speed * t;
            let off = weave * ((w1 * t + p1).sin() + 0.3 * (w2 * t + p2).sin());
            [start[0] + dir[0] * along + side[0] * off, start[1] + dir[1] * along + side[1] * off]
        })
        .collect();
    Funnel { t0, t1: t0 + (VX_S - 1) as f64 * VX_DT, path, touch_r }
}

/// The funnel's roar, played where it is every second of its life (each a few seconds long, faded in and out, so they
/// overlap into one that moves with it): quieter while it is aloft. `track`: the land under its track over `c`'s
/// (`ground_track`).
pub(super) fn roar(q: &mut Plan, f: &Funnel, event: &str, c: [f64; 3], track: &[f64]) {
    roar_as(q, f, event, c, track, 1.0, 1.0);
}
/// `roar`, `gain` times as loud (past 1, heard further off) and at `pitch`: a tornado's, by its rating.
pub(super) fn roar_as(q: &mut Plan, f: &Funnel, event: &str, c: [f64; 3], track: &[f64], gain: f32, pitch: f32) {
    let mut t = f.t0;
    while t < f.t1 - 0.5 {
        let p = f.at(t + 0.8);
        let k = (((t + 0.8 - f.t0) / VX_DT).round() as usize).min(track.len().saturating_sub(1));
        let at = [c[0] + p[0], c[1] + track.get(k).copied().unwrap_or(0.0) + 6.0, c[2] + p[1]];
        let v = 0.45 + 0.55 * f.touch(t + 0.8);
        q.ev.push((t as f32, Act::Sound { event: event.to_string(), at: Some(at), volume: v as f32 * gain, pitch }));
        t += 1.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_funnel_wanders_smoothly_and_stays_under_its_cloud() {
        for k in 0..4 {
            let f = wander(37, k, [40.0, -10.0], 1.2, 5.0, 14.0, 30.0, 95.0, 4.0);
            assert!(f.path.len() > 20 && f.path.len() <= VX_S);
            for w in f.path.windows(2) {
                let step = (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]);
                assert!(step < 5.0 * 1.31 * VX_DT + 1e-6, "{step}");
            }
            assert!(f.path.iter().all(|p| p[0].hypot(p[1]) < 140.0), "{:?}", f.path);
            assert_eq!(f.touch(f.t0), 0.0);
            assert!(f.touch(0.5 * (f.t0 + f.t1)) > 0.99);
            assert!(f.touch(f.t1) < 1e-9);
        }
    }

    #[test]
    fn a_funnel_glides_over_the_land_under_the_trees() {
        let f = wander(37, 0, [40.0, -10.0], 1.2, 5.0, 14.0, 30.0, 95.0, 4.0);
        // (flat land at 0 with trunks 8 high every few blocks, and a step of 4 half way)
        let ground = |x: f64, z: f64| -> Option<f64> {
            let step = if x > 45.0 { 4.0 } else { 0.0 };
            let trunk = ((x.floor() as i64 * 7 + z.floor() as i64 * 13) % 9) == 0;
            Some(step + if trunk { 8.0 } else { 0.0 })
        };
        let h = ground_track(&f.path, ground);
        assert_eq!(h.len(), f.path.len());
        for w in h.windows(2) {
            assert!((w[1] - w[0]).abs() < 1.5, "a hop: {w:?}");
        }
        assert!(h.iter().all(|&v| v < 4.5), "it climbed a tree: {h:?}");
    }
}
