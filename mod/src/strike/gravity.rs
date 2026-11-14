//! Gravity after a strike (`/orbital gravity on|off`, the wheel's GRAVITY switch; kept): once a strike with the terrain on
//! is over and every world change it left waiting has gone in, the land round its target is read again and whatever it
//! left with nothing holding it up comes down, as a gravity mod does (Realistic Block Physics and the like, which run
//! on Java editions Horizon can't load).
//!
//! What holds what (`Kind`):
//! - **The land's own ground, logs and leaves** (`Mass`: stone, dirt, sand, ores, logs, leaves) hold if they are joined, face to face,
//!   to the land under them, however they are joined: a cliff's overhang, a cave's roof and a tree's branches stay; a
//!   lump of a crater's rim the blast cut free, a crown over a trunk blown away, falls (its leaves with it).
//! - **What is built** (`Built`: planks, bricks, stone, concrete, glass, slabs, stairs, fences) holds when it stands on
//!   something that holds, or reaches across to it: no further than its span (`span`: glass and wool 3, wood 5, stone 7,
//!   iron, obsidian and the like 12) from a block that stands on something or from the land's own ground beside it. A
//!   wall cut through at its foot comes down; a floor that has lost its walls comes down past its span.
//! - **What hangs on another** (`Loose`: plants, torches, doors, beds, carpets, lanterns, dripstone) goes with the block
//!   under it (it is taken out where that block falls); it holds nothing up.
//! - **What is left alone** (`Fixed`: chests and what holds things, bedrock, barriers, blocks the table doesn't know, a
//!   mod's) never moves and holds everything joined to it.
//! - Air, water and lava are nothing.
//!
//! The land is read a tile at a time (`GV_TILE` square, read `GV_MARGIN` wider each way so that what holds a block just
//! over the tile's edge is seen; the read's own edge counts as holding), `GV_BELOW` deep under each column's top, and
//! worked out a piece at a time under the meter (`Tile::work`: only what stands over a gap is looked at), the tiles nearest the player
//! first. What falls near the player falls as falling blocks (`setblock ... air`, then a falling block summoned where it
//! stood, its whole state kept: up to `GV_THROWN` of them a strike), further off or past that it is moved straight
//! down onto what is under it in one edit (`world.edit-blocks`), stacked in the order it stood.
use super::Blocks;
use super::hu_blocks::HU_BLOCKS;
use super::hurricane::{hu_block, stays};
use crate::columns;
use std::cell::RefCell;
use std::rc::Rc;
use std::collections::{HashMap, VecDeque};

/// A tile's side (the columns it decides), and how much wider each way it is read.
pub const GV_TILE: i32 = 24;
pub const GV_MARGIN: i32 = 8;
const GV_W: i32 = GV_TILE + 2 * GV_MARGIN;
/// How deep under each column's top it reads, and the most of the land's height a tile works on (its top down).
pub const GV_BELOW: u32 = 48;
const GV_SPAN_Y: i32 = 96;
/// Tiles asked of the world at once, and the ticks an answer is waited for before the tile is given up.
const GV_FLIGHT: usize = 3;
const GV_TIMEOUT: u32 = 200;
/// Ticks after the strike's last change has gone in before the land is read (its thrown blocks have landed by then).
pub const GV_WAIT: u32 = 40;
/// Falling blocks a strike at most (the rest moved straight down), and how near the player one falls as one.
pub const GV_THROWN: u32 = 480;
const GV_NEAR: f64 = 112.0;

/// How far round its target each laser's land is settled (none: the Supervolcano, whose vent is far from the target).
pub fn reach(key: &str) -> Option<f64> {
    Some(match key {
        "plasma" | "godly" | "mothership" => 64.0,
        "blackhole" => 72.0,
        "meteor" => 150.0,
        "downfall" => 170.0,
        "planetkiller" => 360.0,
        "crystalbloom" => 150.0,
        "sinkhole" => 170.0,
        "payload" => 100.0,
        "nuke" => 300.0,
        "rod" => 250.0,
        "supercell" => 120.0,
        "hurricane" => 170.0,
        "tsunami" => 360.0,
        "fold" => 360.0,
        "skybreak" => 160.0,
        "carpetbomb" => 220.0,
        "earthquake" => 210.0,
        _ => return None,
    })
}

/// What a block is to gravity (see the module's notes).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Kind {
    Empty,
    Loose,
    Mass,
    Built(u8),
    Fixed,
}

pub(super) fn kind_of(name: &str) -> Kind {
    crate::meter::spend(1_200);
    let plain = name.split('[').next().unwrap_or(name);
    let n = plain.trim_start_matches("minecraft:");
    if n.ends_with("air") {
        return Kind::Empty;
    }
    if stays(plain) {
        return Kind::Fixed;
    }
    let Some(b) = hu_block(plain) else { return Kind::Fixed };
    let loose = || {
        ["door", "_bed", "pointed_dripstone", "_bud", "amethyst_cluster", "lantern", "chain", "bell", "_button", "lever", "torch", "ladder", "_head", "_skull", "banner", "carpet", "candle", "pressure_plate", "rail", "scaffolding", "sign", "frame", "_hook", "vine", "lichen", "roots", "snow", "redstone_wire", "repeater", "comparator", "flower_pot", "potted_", "coral_fan"]
            .iter()
            .any(|w| n.contains(w))
    };
    match HU_BLOCKS[b as usize].class {
        0 | 1 => Kind::Empty,
        2..=4 => Kind::Mass,
        5 => Kind::Loose,
        6 if n.contains("fence") || n.ends_with("_bars") || n.ends_with("_pane") => Kind::Built(span(n)),
        6 => Kind::Loose,
        _ if loose() => Kind::Loose,
        _ => Kind::Built(span(n)),
    }
}

/// How far a built block reaches across from what holds it, in blocks.
fn span(n: &str) -> u8 {
    let has = |ws: &[&str]| ws.iter().any(|w| n.contains(w));
    if has(&["glass", "_pane", "wool", "ice", "leaves", "sponge", "hay", "slime", "honey", "mushroom_block", "_wart_block", "dried_kelp"]) {
        3
    } else if has(&["obsidian", "iron_block", "gold_block", "diamond_block", "emerald_block", "netherite_block", "copper", "_bars", "reinforced", "anvil"]) {
        12
    } else if has(&["planks", "_wood", "_log", "bamboo", "oak", "spruce", "birch", "jungle", "acacia", "mangrove", "cherry", "crimson", "warped", "bookshelf", "fence"]) {
        5
    } else {
        7
    }
}

/// A block's whole state as a falling block's NBT: `{Name:"minecraft:oak_stairs",Properties:{facing:"east"}}`.
pub(super) fn state_nbt(name: &str) -> String {
    let (plain, props) = match name.split_once('[') {
        Some((p, rest)) => (p, rest.trim_end_matches(']')),
        None => (name, ""),
    };
    let props: Vec<String> = props.split(',').filter_map(|kv| kv.split_once('=')).map(|(k, v)| format!("{k}:\"{v}\"")).collect();
    if props.is_empty() { format!("{{Name:\"{plain}\"}}") } else { format!("{{Name:\"{plain}\",Properties:{{{}}}}}", props.join(",")) }
}

fn interned(name: &str) -> &'static str {
    thread_local! {
        static NAMES: RefCell<HashMap<String, &'static str>> = RefCell::new(HashMap::new());
    }
    NAMES.with(|t| {
        let mut t = t.borrow_mut();
        if let Some(s) = t.get(name) {
            return *s;
        }
        let s: &'static str = Box::leak(name.to_string().into_boxed_str());
        t.insert(name.to_string(), s);
        s
    })
}

// ---- a tile: the read, and what holds -------------------------------------------------------------------------------

/// Kind codes in the grid (a cell's code is its kind and its span, `kind | span << 3`).
const E: u8 = 0;
const L: u8 = 1;
const M: u8 = 2;
const B: u8 = 3;
const F: u8 = 4;
/// A cell's flags: joined to what holds (pass A), held (pass B), fallen in an earlier round; and, set as it is read,
/// whether it holds whatever happens (`ANCH`: solid and in its column's unbroken run from the bottom, at the read's
/// edge, or left alone) or has to be worked out (`CAND`: ground or built, over a gap in its column).
const CONN: u8 = 1;
const HELD: u8 = 2;
const FELL: u8 = 4;
const ANCH: u8 = 8;
const CAND: u8 = 16;
/// Grid ids past the palette: a cell under what was read (held), and one over a column's top (air).
const UNDER: u16 = u16::MAX;
const OVER: u16 = u16::MAX - 1;

/// What the work costs, as the meter counts it (from `gravity_meter_reads_high`'s timing and `mod_soak`'s fuel: this
/// work runs some 40 wasm instructions a native nanosecond, so a native nanosecond of it is counted at no less than
/// 40): a column of a layer taken in, a cell worked over (`CAND`'s flags, a seed's six faces), one taken off the
/// search, a cell of a column of what fell walked, a cell looked over as the answer comes in.
const C_CELL: u64 = 240;
const C_SEED: u64 = 650;
const C_POP: u64 = 880;
const C_HOLD: u64 = 720;
const C_FALL: u64 = 260;
const C_SCAN: u64 = 66;
/// What starting on a tile's answer costs at the most (`Tile::new`: a look over it for anything over a gap).
const GV_TAKE_COST: u64 = C_SCAN * (GV_W * GV_W) as u64 * GV_BELOW as u64 + 4 * (GV_W * GV_W * GV_SPAN_Y) as u64 + 1_000_000;

/// A block that falls: where it stood, and its state.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Fall {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub name: &'static str,
}

/// A tile's column: where it stood and what of it falls (bottom first), what loose on it goes with it, and where each
/// fallen block lands when moved straight down (its new heights, in the same order).
#[derive(Default, Debug)]
pub(super) struct ColFall {
    pub x: i32,
    pub z: i32,
    pub falls: Vec<Fall>,
    pub gone: Vec<i32>,
    pub lands: Vec<i32>,
}

/// A tile read in and worked out a piece at a time (`work`): most of the land is each column's unbroken run of ground
/// from the bottom (`ANCH`), which holds without a look; only what is over a gap (`CAND`: a tree's crown, a cave's
/// roof, what a strike cut free) is worked over, in four passes (A, B, A, B), each resumed where the last tick left it.
pub(super) struct Tile {
    /// The window's corner (the tile's own, less the margin).
    x0: i32,
    z0: i32,
    ylo: i32,
    ny: i32,
    /// The read, what each of its palette's names is (`kind | span << 3`), the next layer to take in, and whether each
    /// column's run from the bottom has gone unbroken so far.
    a: Rc<columns::Answer>,
    pal: Vec<u8>,
    read: i32,
    run: Vec<bool>,
    code: Vec<u8>,
    flags: Vec<u8>,
    /// Each column's unbroken run from the bottom, in cells.
    base: Vec<u16>,
    /// The cells to work out, in order (so a layer's are together).
    cand: Vec<u32>,
    names: Vec<&'static str>,
    /// Which pass (0-3), where in it (`sub`: a connect pass's seeding 0 or its search 1), and how far through `cand`.
    pass: u8,
    sub: u8,
    pos: usize,
    stack: Vec<u32>,
    dist: Vec<u8>,
    touched: Vec<u32>,
}

impl Tile {
    /// The tile from a read's answer (its columns as `tile_columns` lists them); none when nothing in it can fall (no
    /// solid block of its own columns over air), which most of the land is.
    pub(super) fn new(x0: i32, z0: i32, a: Rc<columns::Answer>) -> Option<Tile> {
        let w = GV_W as usize;
        let n = w * w;
        let pal: Vec<u8> = a
            .palette
            .iter()
            .map(|p| match kind_of(p) {
                Kind::Empty => E,
                Kind::Loose => L,
                Kind::Mass => M,
                Kind::Built(s) => B | (s.min(31) << 3),
                Kind::Fixed => F,
            })
            .collect();
        let solid = |id: u16| pal.get(usize::from(id)).copied().unwrap_or(F) & 7 >= M;
        // the heights it works on: from the lowest column's read bottom to the highest top, at most GV_SPAN_Y
        let (mut lo, mut hi) = (i32::MAX, i32::MIN);
        let mut any_gap = false;
        for j in 0..n.min(a.columns.len()) {
            let Some((base, s)) = a.column(j) else { continue };
            lo = lo.min(base);
            hi = hi.max(base + s.len() as i32 - 1);
            // (a solid block over air or something loose, in the tile's own columns: something may fall)
            let (gx, gz) = ((j % w) as i32, (j / w) as i32);
            if !any_gap && inside(gx, gz) {
                any_gap = s.windows(2).any(|p| !solid(p[0]) && solid(p[1]));
            }
        }
        crate::meter::spend(C_SCAN * n as u64 * u64::from(GV_BELOW) + 1_000 * pal.len() as u64);
        if !any_gap || lo > hi {
            return None;
        }
        let ylo = lo.max(hi - GV_SPAN_Y + 1);
        let ny = hi - ylo + 1;
        let cells = n * ny as usize;
        let names = a.palette.iter().map(|p| interned(p)).collect();
        crate::meter::spend(4 * cells as u64);
        Some(Tile {
            x0,
            z0,
            ylo,
            ny,
            a,
            pal,
            read: 0,
            run: vec![true; n],
            code: vec![E; cells],
            flags: vec![0; cells],
            base: vec![0; n],
            cand: Vec::new(),
            names,
            pass: 0,
            sub: 0,
            pos: 0,
            stack: Vec::new(),
            dist: vec![u8::MAX; n],
            touched: Vec::new(),
        })
    }
    /// The read taken into the grid a layer at a time, from the bottom (each column's unbroken run from there holds
    /// whatever happens); true when it stopped for the meter.
    fn take_in(&mut self, limit: u64) -> bool {
        let n = self.n();
        while self.read < self.ny {
            let y = self.read;
            let wy = self.ylo + y;
            let row = y as usize * n;
            let had = self.cand.len();
            for j in 0..n {
                let id = match if j < self.a.columns.len() { self.a.column(j) } else { None } {
                    // (a column not loaded holds everything round it)
                    None => UNDER,
                    Some((b, _)) if wy < b => UNDER,
                    Some((b, s)) => s.get((wy - b) as usize).copied().unwrap_or(OVER),
                };
                let c = match id {
                    UNDER => F,
                    OVER => E,
                    id => self.pal.get(usize::from(id)).copied().unwrap_or(F),
                };
                let i = row + j;
                self.code[i] = c;
                if c & 7 < M {
                    self.run[j] = false;
                    continue;
                }
                if self.run[j] {
                    self.base[j] = y as u16 + 1;
                }
                if self.run[j] || border(j) || c & 7 == F {
                    self.flags[i] = ANCH;
                } else {
                    self.flags[i] = CAND;
                    self.cand.push(i as u32);
                }
            }
            crate::meter::spend(C_CELL * n as u64 + 80 * (self.cand.len() - had) as u64);
            self.read += 1;
            if crate::meter::spent() >= limit {
                return self.read < self.ny;
            }
        }
        false
    }
    fn n(&self) -> usize {
        (GV_W * GV_W) as usize
    }
    /// The cells it works over.
    #[cfg(test)]
    pub(super) fn candidates(&self) -> usize {
        self.cand.len()
    }
    /// Work on until done (true) or the call has spent `limit` (false: the next call goes on from there).
    pub(super) fn work(&mut self, limit: u64) -> bool {
        if self.take_in(limit) {
            return false;
        }
        while self.pass < 4 {
            let more = if self.pass.is_multiple_of(2) { self.connect(limit) } else { self.hold(limit) };
            if more {
                return false;
            }
            self.pass += 1;
            self.sub = 0;
            self.pos = 0;
        }
        true
    }
    /// Pass A: what over a gap is joined, face to face, to what holds whatever happens (`ANCH`), through ground, logs
    /// and what is built; what fell in an earlier round left out. True when it stopped for the meter.
    fn connect(&mut self, limit: u64) -> bool {
        let n = self.n();
        let w = GV_W as usize;
        let cells = self.flags.len();
        if self.sub == 0 {
            while self.pos < self.cand.len() {
                let end = (self.pos + 512).min(self.cand.len());
                for &c in &self.cand[self.pos..end] {
                    let i = c as usize;
                    self.flags[i] &= !(CONN | HELD);
                    if self.flags[i] & FELL != 0 {
                        continue;
                    }
                    let anch = faces(i, n, w, cells).iter().any(|&m| m != usize::MAX && self.flags[m] & ANCH != 0);
                    if anch {
                        self.flags[i] |= CONN;
                        self.stack.push(c);
                    }
                }
                crate::meter::spend(C_SEED * (end - self.pos) as u64);
                self.pos = end;
                if crate::meter::spent() >= limit {
                    return true;
                }
            }
            self.sub = 1;
        }
        loop {
            let mut k = 0u64;
            while k < 512 {
                let Some(c) = self.stack.pop() else { break };
                k += 1;
                for m in faces(c as usize, n, w, cells) {
                    if m != usize::MAX && self.flags[m] & (CAND | CONN | FELL) == CAND {
                        self.flags[m] |= CONN;
                        self.stack.push(m as u32);
                    }
                }
            }
            crate::meter::spend(C_POP * k);
            if self.stack.is_empty() {
                return false;
            }
            if crate::meter::spent() >= limit {
                return true;
            }
        }
    }
    /// Pass B, a layer at a time from the bottom: ground and logs joined hold; a built block holds standing on what
    /// holds, or within its span of such a block or of what holds beside it; the rest falls. True when it stopped for
    /// the meter.
    fn hold(&mut self, limit: u64) -> bool {
        let n = self.n();
        let w = GV_W as usize;
        while self.pos < self.cand.len() {
            let y = self.cand[self.pos] as usize / n;
            let mut end = self.pos;
            while end < self.cand.len() && self.cand[end] as usize / n == y {
                end += 1;
            }
            let mut q: VecDeque<usize> = VecDeque::new();
            // (what holds at once: ground and logs joined, built blocks joined and standing on what holds)
            for &c in &self.cand[self.pos..end] {
                let i = c as usize;
                let f = self.flags[i];
                if f & CONN == 0 {
                    continue;
                }
                let k = self.code[i] & 7;
                let below = y == 0 || self.flags[i - n] & (HELD | ANCH) != 0;
                if k == M || below {
                    self.flags[i] |= HELD;
                    let j = i % n;
                    self.dist[j] = 0;
                    self.touched.push(j as u32);
                    q.push_back(j);
                }
            }
            // (a built block beside what holds whatever happens: one along)
            for &c in &self.cand[self.pos..end] {
                let i = c as usize;
                let j = i % n;
                if self.flags[i] & (CONN | HELD) != CONN || self.code[i] >> 3 == 0 {
                    continue;
                }
                if side(j, w).iter().any(|&m| m != usize::MAX && self.flags[y * n + m] & ANCH != 0) {
                    self.flags[i] |= HELD;
                    self.dist[j] = 1;
                    self.touched.push(j as u32);
                    q.push_back(j);
                }
            }
            let mut pops = 0u64;
            while let Some(j) = q.pop_front() {
                pops += 1;
                let d = self.dist[j];
                for m in side(j, w) {
                    if m == usize::MAX || self.dist[m] != u8::MAX {
                        continue;
                    }
                    let i = y * n + m;
                    let (k, s) = (self.code[i] & 7, self.code[i] >> 3);
                    if self.flags[i] & (CAND | CONN) == CAND | CONN && k == B && d < s {
                        self.dist[m] = d + 1;
                        self.flags[i] |= HELD;
                        self.touched.push(m as u32);
                        q.push_back(m);
                    }
                }
            }
            for &c in &self.cand[self.pos..end] {
                let i = c as usize;
                if self.flags[i] & HELD == 0 {
                    self.flags[i] |= FELL;
                }
            }
            for &j in &self.touched {
                self.dist[j as usize] = u8::MAX;
            }
            self.touched.clear();
            crate::meter::spend(C_HOLD * (end - self.pos) as u64 + C_POP * pops + 2_000);
            self.pos = end;
            if crate::meter::spent() >= limit {
                return self.pos < self.cand.len();
            }
        }
        false
    }
    /// What falls in the tile's own columns, each column's falls bottom first, the loose blocks that go with them, and
    /// where each lands moved straight down.
    pub(super) fn falls(&self) -> Vec<ColFall> {
        let n = self.n();
        let w = GV_W as usize;
        let mut cols: Vec<usize> = self.cand.iter().map(|&c| c as usize).filter(|&i| self.flags[i] & FELL != 0).map(|i| i % n).collect();
        cols.sort_unstable();
        cols.dedup();
        let mut out = Vec::new();
        let mut walked = 0u64;
        for j in cols {
            let (gx, gz) = ((j % w) as i32, (j / w) as i32);
            if !inside(gx, gz) {
                continue;
            }
            let mut c = ColFall { x: self.x0 + gx, z: self.z0 + gz, ..Default::default() };
            // (the top of what stays under the next block down, and whether the block under this one went)
            let b = usize::from(self.base[j]);
            let mut top = self.ylo + b as i32 - 1;
            let mut went = false;
            for y in b..self.ny as usize {
                let i = y * n + j;
                let wy = self.ylo + y as i32;
                let k = self.code[i] & 7;
                if self.flags[i] & FELL != 0 {
                    let name = self.a.column(j).and_then(|(b, s)| s.get((wy - b) as usize)).and_then(|&id| self.names.get(usize::from(id)).copied()).unwrap_or("minecraft:stone");
                    c.falls.push(Fall { x: c.x, y: wy, z: c.z, name });
                    top += 1;
                    c.lands.push(top);
                    went = true;
                } else if k == L && went {
                    // (what hangs on a block that fell goes with it)
                    c.gone.push(wy);
                } else if k >= M {
                    top = wy;
                    went = false;
                } else {
                    went = went && k == L;
                }
            }
            walked += (self.ny as usize - b) as u64;
            if !c.falls.is_empty() {
                out.push(c);
            }
        }
        crate::meter::spend(C_FALL * walked + 300 * out.len() as u64 + 20 * self.cand.len() as u64);
        out
    }
}

/// Window column `j`'s on the read's edge.
fn border(j: usize) -> bool {
    let w = GV_W as usize;
    let (gx, gz) = (j % w, j / w);
    gx == 0 || gz == 0 || gx == w - 1 || gz == w - 1
}

/// Cell `i`'s six neighbours (`usize::MAX` past the window).
#[inline]
fn faces(i: usize, n: usize, w: usize, cells: usize) -> [usize; 6] {
    let j = i % n;
    let [a, b, c, d] = side(j, w);
    let lift = |m: usize| if m == usize::MAX { m } else { i - j + m };
    [lift(a), lift(b), lift(c), lift(d), if i >= n { i - n } else { usize::MAX }, if i + n < cells { i + n } else { usize::MAX }]
}

/// Column `j`'s four neighbours in its layer.
#[inline]
fn side(j: usize, w: usize) -> [usize; 4] {
    let (gx, gz) = (j % w, j / w);
    [
        if gx > 0 { j - 1 } else { usize::MAX },
        if gx + 1 < w { j + 1 } else { usize::MAX },
        if gz > 0 { j - w } else { usize::MAX },
        if gz + 1 < w { j + w } else { usize::MAX },
    ]
}

/// Whether window column (gx, gz) is the tile's own (not its margin).
fn inside(gx: i32, gz: i32) -> bool {
    (GV_MARGIN..GV_MARGIN + GV_TILE).contains(&gx) && (GV_MARGIN..GV_MARGIN + GV_TILE).contains(&gz)
}

/// The columns a tile reads (its window, row by row), the tile's own corner at (tx, tz).
pub(super) fn tile_columns(tx: i32, tz: i32) -> Vec<(i32, i32)> {
    let mut out = Vec::with_capacity((GV_W * GV_W) as usize);
    for gz in 0..GV_W {
        for gx in 0..GV_W {
            out.push((tx - GV_MARGIN + gx, tz - GV_MARGIN + gz));
        }
    }
    out
}

// ---- the work after a strike ----------------------------------------------------------------------------------------

/// What a tick of the settling gives lib.rs: commands (the falling blocks), edits (the blocks moved straight down),
/// in that order.
#[derive(Default)]
pub struct SettleOut {
    pub cmds: Vec<String>,
    pub edits: Vec<Blocks>,
}

/// The land round a strike's target settling once the strike is over (lib.rs's `settle`, a tick at a time).
pub struct Settle {
    /// The tiles' corners, nearest the player first, and the next to ask for.
    tiles: Vec<(i32, i32)>,
    next: usize,
    asked: VecDeque<(u32, (i32, i32), u32)>,
    cur: Option<Tile>,
    wait: u32,
    ticks: u32,
    pub thrown: u32,
    pub moved: u32,
    pub gone: u32,
    pub worked: u32,
    pub missed: u32,
}

impl Settle {
    /// Settling the land within `reach` of `target` (x, z), the player at `me`.
    pub fn new(target: (i32, i32), reach: f64, me: [f64; 3]) -> Settle {
        let r = reach.ceil() as i32;
        let lo = |c: i32| (c - r).div_euclid(GV_TILE) * GV_TILE;
        let r2 = i64::from(r) * i64::from(r);
        let mut tiles = Vec::new();
        let (mut tz, z1) = (lo(target.1), target.1 + r);
        while tz <= z1 {
            let (mut tx, x1) = (lo(target.0), target.0 + r);
            while tx <= x1 {
                // (a tile any of which is within reach; squared, no hypot: hundreds of them, and the wasm's is dear)
                let cx = i64::from(target.0.clamp(tx, tx + GV_TILE - 1) - target.0);
                let cz = i64::from(target.1.clamp(tz, tz + GV_TILE - 1) - target.1);
                if cx * cx + cz * cz <= r2 {
                    tiles.push((tx, tz));
                }
                tx += GV_TILE;
            }
            tz += GV_TILE;
        }
        // (nearest the player first: each tile's distance worked out once)
        let (mx, mz) = (me[0] as i64, me[2] as i64);
        tiles.sort_by_cached_key(|&(tx, tz)| {
            let (dx, dz) = (i64::from(tx + GV_TILE / 2) - mx, i64::from(tz + GV_TILE / 2) - mz);
            dx * dx + dz * dz
        });
        crate::meter::spend(400 * tiles.len() as u64 + 2_000);
        Settle { tiles, next: 0, asked: VecDeque::new(), cur: None, wait: GV_WAIT, ticks: 0, thrown: 0, moved: 0, gone: 0, worked: 0, missed: 0 }
    }
    pub fn tiles(&self) -> usize {
        self.tiles.len()
    }
    /// Whether it is all done.
    pub fn done(&self) -> bool {
        self.wait == 0 && self.next >= self.tiles.len() && self.asked.is_empty() && self.cur.is_none()
    }
    /// A tick of it: `quiet` once the strike's world changes have all gone in (it waits for them and `GV_WAIT` more),
    /// the player at `me`. What it gives goes into lib.rs's queues in order.
    pub fn tick(&mut self, quiet: bool, me: [f64; 3]) -> SettleOut {
        let mut out = SettleOut::default();
        if self.wait > 0 {
            if quiet {
                self.wait -= 1;
            }
            return out;
        }
        self.ticks += 1;
        self.ask_more();
        // the steps of the tile under way, and the next tiles' answers taken in, as long as the tick has room (the
        // first of them whatever: it never stalls)
        let mut first = true;
        loop {
            if crate::meter::full() {
                break;
            }
            if let Some(t) = &mut self.cur {
                if !t.work(crate::meter::SOFT) {
                    break;
                }
                let t = self.cur.take().expect("the tile");
                self.worked += 1;
                self.give(t.falls(), me, &mut out);
                continue;
            }
            // (a tile read in only if it fits, the first of a tick whatever: it never stalls)
            if !first && !crate::meter::room(GV_TAKE_COST) {
                break;
            }
            first = false;
            let Some(&(tag, (tx, tz), at)) = self.asked.front() else { break };
            match columns::take(tag) {
                Some(a) => {
                    self.asked.pop_front();
                    self.cur = Tile::new(tx - GV_MARGIN, tz - GV_MARGIN, a);
                }
                None if self.ticks - at > GV_TIMEOUT => {
                    self.asked.pop_front();
                    columns::forget(tag);
                    self.missed += 1;
                }
                None => break,
            }
            self.ask_more();
        }
        out
    }
    /// The next tiles asked of the world, up to `GV_FLIGHT` waiting.
    fn ask_more(&mut self) {
        while self.asked.len() < GV_FLIGHT && self.next < self.tiles.len() {
            let (tx, tz) = self.tiles[self.next];
            self.next += 1;
            let tag = columns::new_tag();
            columns::ask(tag, tile_columns(tx, tz), GV_BELOW, 0);
            self.asked.push_back((tag, (tx, tz), self.ticks));
        }
    }
    /// Forget what it asked of the world (it is dropped: a new strike, the player gone).
    pub fn forget(&mut self) {
        for (tag, _, _) in self.asked.drain(..) {
            columns::forget(tag);
        }
    }
    /// A tile's falls into commands and edits: falling blocks near the player while there are any left to throw, the
    /// rest moved straight down.
    fn give(&mut self, cols: Vec<ColFall>, me: [f64; 3], out: &mut SettleOut) {
        let mut gone = Blocks::default();
        let mut land = Blocks { clear_of_player: true, ..Default::default() };
        for c in cols {
            let near = (f64::from(c.x) + 0.5 - me[0]).hypot(f64::from(c.z) + 0.5 - me[2]) < GV_NEAR;
            self.gone += c.gone.len() as u32;
            if near && self.thrown + c.falls.len() as u32 <= GV_THROWN {
                self.thrown += c.falls.len() as u32;
                for &y in &c.gone {
                    out.cmds.push(format!("setblock {} {y} {} minecraft:air", c.x, c.z));
                }
                for f in &c.falls {
                    out.cmds.push(format!("setblock {} {} {} minecraft:air", f.x, f.y, f.z));
                }
                for f in &c.falls {
                    out.cmds.push(format!(
                        "summon minecraft:falling_block {}.5 {} {}.5 {{BlockState:{},Time:1,DropItem:0b}}",
                        f.x,
                        f.y,
                        f.z,
                        state_nbt(f.name)
                    ));
                }
            } else {
                self.moved += c.falls.len() as u32;
                for &y in &c.gone {
                    gone.run(c.x, c.z, y, y, "minecraft:air");
                }
                for f in &c.falls {
                    gone.run(f.x, f.z, f.y, f.y, "minecraft:air");
                }
                for (f, &y) in c.falls.iter().zip(&c.lands) {
                    land.run(f.x, f.z, y, y, f.name);
                }
            }
        }
        if !gone.runs.is_empty() {
            out.edits.push(gone);
            out.edits.push(land);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tile over a made-up land: flat ground (stone up to y 63), and what `put` adds over it (x, y, z, block).
    fn tile_over(put: &[(i32, i32, i32, &'static str)]) -> Tile {
        let (tx, tz) = (0, 0);
        let cols = tile_columns(tx, tz);
        let tag = columns::new_tag();
        let top_of = |x: i32, z: i32| put.iter().filter(|p| p.0 == x && p.2 == z).map(|p| p.1 + 1).max().unwrap_or(64).max(64);
        columns::test_answer(tag, &cols, GV_BELOW, 0, &|x, z| {
            let top = top_of(x, z);
            let base = top - GV_BELOW as i32;
            let names = (base..top)
                .map(|y| put.iter().find(|p| p.0 == x && p.1 == y && p.2 == z).map(|p| p.3).unwrap_or(if y < 64 { "minecraft:stone" } else { "minecraft:air" }))
                .collect();
            Some((base, names))
        });
        let a = columns::take(tag).expect("answered");
        Tile::new(tx - GV_MARGIN, tz - GV_MARGIN, a.clone()).expect("something may fall")
    }
    fn falls_of(put: &[(i32, i32, i32, &'static str)]) -> Vec<ColFall> {
        let mut t = tile_over(put);
        while !t.work(u64::MAX) {}
        t.falls()
    }
    fn fell(c: &[ColFall]) -> Vec<(i32, i32, i32)> {
        let mut v: Vec<_> = c.iter().flat_map(|c| c.falls.iter().map(|f| (f.x, f.y, f.z))).collect();
        v.sort();
        v
    }

    /// The meter against the clock (release, on its own: `cargo test --release gravity_meter_reads_high -- --ignored
    /// --nocapture`): over a heavy forest with caves and ruins, a hollow under the whole tile and a tile all cut free,
    /// every piece of the work is counted at no less than 30 a native nanosecond (typically 50 to 100; the real
    /// check is `mod_soak`'s fuel, `SOAK_FULL=1 SOAK_LASERS=tsunami`, whose settling ticks stay under a third of it), and no call runs past what it was let spend by more than a layer.
    #[test]
    #[ignore]
    fn gravity_meter_reads_high() {
        use std::time::Instant;
        let h = |x: i32, z: i32| ((x.wrapping_mul(73_856_093) ^ z.wrapping_mul(19_349_663)) as u32).wrapping_mul(2_654_435_761) >> 8;
        let lands: [(&str, &dyn Fn(i32, i32, i32) -> &'static str, &dyn Fn(i32, i32) -> i32); 3] = [
            (
                "forest",
                &|x, y, z| {
                    let g = 64 + ((x / 7 + z / 5) % 9) - 4;
                    if (40..44).contains(&y) && (x + z).rem_euclid(9) < 4 {
                        return "minecraft:cave_air";
                    }
                    if y <= g {
                        return if y == g { "minecraft:grass_block[snowy=false]" } else { "minecraft:stone" };
                    }
                    // (trees every 6, their crowns 5 across and 3 deep; slabs of planks floating here and there)
                    let (tx, tz) = (x.rem_euclid(6), z.rem_euclid(6));
                    if tx == 3 && tz == 3 && y <= g + 5 {
                        return "minecraft:oak_log[axis=y]";
                    }
                    if (1..=5).contains(&tx) && (1..=5).contains(&tz) && (g + 4..=g + 6).contains(&y) {
                        return "minecraft:oak_leaves[distance=1,persistent=false,waterlogged=false]";
                    }
                    if y == g + 9 && h(x / 4, z / 4) % 5 == 0 {
                        return "minecraft:oak_planks";
                    }
                    "minecraft:air"
                },
                &|x, z| 64 + ((x / 7 + z / 5) % 9) - 4 + 12,
            ),
            ("hollow", &|_, y, _| if y == 50 || y == 51 { "minecraft:air" } else if y < 80 { "minecraft:stone" } else { "minecraft:air" }, &|_, _| 82),
            ("cut free", &|_, y, _| if y < 36 { "minecraft:air" } else if y < 82 { "minecraft:dirt" } else { "minecraft:air" }, &|_, _| 82),
        ];
        let (tx, tz) = (0, 0);
        let cols = tile_columns(tx, tz);
        for (name, block, top) in lands {
            let tag = columns::new_tag();
            columns::test_answer(tag, &cols, GV_BELOW, 0, &|x, z| {
                let t = top(x, z);
                Some((t - GV_BELOW as i32, (t - GV_BELOW as i32..t).map(|y| block(x, y, z)).collect()))
            });
            let a = columns::take(tag).expect("answered");
            let check = |what: &str, spent: u64, ns: u128| {
                let k = spent as f64 / ns.max(1) as f64;
                println!("{name}: {what}: counted {:.2}M in {:.3} ms, {k:.1} a ns", spent as f64 / 1e6, ns as f64 / 1e6);
                assert!(spent < 2_000_000 || k >= 30.0, "{name}: {what} counted at {k:.1} a ns");
            };
            // (once first, so the heap has the room: the game's has long since grown, and a page's first touch costs
            // time natively that is no fuel in the game)
            let mut warm = Tile::new(tx - GV_MARGIN, tz - GV_MARGIN, a.clone()).expect("something may fall");
            while !warm.work(u64::MAX) {}
            drop(warm);
            crate::meter::reset();
            let t0 = Instant::now();
            let mut t = Tile::new(tx - GV_MARGIN, tz - GV_MARGIN, a.clone()).expect("something may fall");
            check("read in", crate::meter::spent(), t0.elapsed().as_nanos());
            assert!(crate::meter::spent() <= GV_TAKE_COST, "{name}: reading in over GV_TAKE_COST");
            println!("{name}: {} cells to work out", t.candidates());
            let mut calls = 0;
            loop {
                crate::meter::reset();
                let t0 = Instant::now();
                let done = t.work(crate::meter::SOFT);
                let (spent, ns) = (crate::meter::spent(), t0.elapsed().as_nanos());
                check("a call's work", spent, ns);
                assert!(spent <= crate::meter::SOFT + 2_500_000, "{name}: a call ran on to {spent}");
                calls += 1;
                if done {
                    break;
                }
            }
            crate::meter::reset();
            let t0 = Instant::now();
            let f = t.falls();
            check("what falls", crate::meter::spent(), t0.elapsed().as_nanos());
            println!("{name}: {calls} calls, {} columns fall", f.len());
        }
    }

    #[test]
    fn gravity_kinds() {
        assert_eq!(kind_of("minecraft:stone"), Kind::Mass);
        assert_eq!(kind_of("minecraft:oak_log[axis=y]"), Kind::Mass);
        assert_eq!(kind_of("minecraft:oak_planks"), Kind::Built(5));
        assert_eq!(kind_of("minecraft:glass"), Kind::Built(3));
        assert_eq!(kind_of("minecraft:stone_bricks"), Kind::Built(7));
        assert_eq!(kind_of("minecraft:chest[facing=north]"), Kind::Fixed);
        assert_eq!(kind_of("minecraft:oak_leaves[persistent=false]"), Kind::Mass);
        assert_eq!(kind_of("minecraft:water[level=0]"), Kind::Empty);
        assert_eq!(kind_of("minecraft:torch"), Kind::Loose);
        assert_eq!(kind_of("minecraft:oak_door[half=lower]"), Kind::Loose);
        assert_eq!(kind_of("minecraft:pointed_dripstone"), Kind::Loose);
        assert!(matches!(kind_of("minecraft:oak_fence"), Kind::Built(_)));
        assert_eq!(kind_of("somemod:thing"), Kind::Fixed);
        assert_eq!(state_nbt("minecraft:stone"), "{Name:\"minecraft:stone\"}");
        assert_eq!(state_nbt("minecraft:oak_stairs[facing=east,half=top]"), "{Name:\"minecraft:oak_stairs\",Properties:{facing:\"east\",half:\"top\"}}");
    }

    #[test]
    fn gravity_leaves_untouched_land_alone() {
        // a house standing on the ground, its roof out past it within its span (counted along and across: 3 + 2 of 5), a cave's roof, an overhang: nothing falls
        let mut put = Vec::new();
        for y in 64..68 {
            for (x, z) in [(10, 10), (14, 10), (10, 14), (14, 14)] {
                put.push((x, y, z, "minecraft:oak_planks"));
            }
        }
        for x in 10..=17 {
            for z in 10..=14 {
                put.push((x, 68, z, "minecraft:oak_planks"));
            }
        }
        // (a cave under the land: its roof joined to the rest of the ground)
        for x in 3..9 {
            for z in 3..9 {
                put.push((x, 60, z, "minecraft:air"));
                put.push((x, 61, z, "minecraft:air"));
            }
        }
        // (an overhang of ground off a mound)
        for y in 64..70 {
            put.push((20, y, 20, "minecraft:dirt"));
        }
        for x in 21..=23 {
            put.push((x, 69, 20, "minecraft:dirt"));
        }
        // (and a hanging stalactite in the cave: loose, it stays)
        put.push((5, 61, 5, "minecraft:pointed_dripstone"));
        put.sort_by_key(|p| (p.0, p.2, p.1));
        put.dedup_by_key(|p| (p.0, p.1, p.2));
        let f = fell(&falls_of(&put));
        assert!(f.is_empty(), "{f:?}");
    }

    #[test]
    fn gravity_brings_down_what_a_strike_cut_free() {
        let mut put = Vec::new();
        // a wall whose foot was blown out (stone bricks at 66..70 over air at 64..65)
        for y in 66..70 {
            put.push((5, y, 5, "minecraft:stone_bricks"));
        }
        // a floor reaching 9 out from the last post standing (stone 7: its last 2 fall), with a torch on its end
        put.push((12, 64, 12, "minecraft:stone_bricks"));
        put.push((12, 65, 12, "minecraft:stone_bricks"));
        for x in 12..=21 {
            put.push((x, 66, 12, "minecraft:stone_bricks"));
        }
        put.push((21, 67, 12, "minecraft:torch"));
        // a lump of ground floating where a crater's rim was cut away, and a log over a stump blown off
        for x in 2..4 {
            put.push((x, 70, 18, "minecraft:dirt"));
        }
        put.push((18, 68, 3, "minecraft:oak_log"));
        put.push((18, 69, 3, "minecraft:oak_log"));
        let c = falls_of(&put);
        let f = fell(&c);
        for y in 66..70 {
            assert!(f.contains(&(5, y, 5)), "the wall over its blown foot");
        }
        assert!(f.contains(&(20, 66, 12)) && f.contains(&(21, 66, 12)) && !f.contains(&(19, 66, 12)), "past the floor's span: {f:?}");
        assert!(f.contains(&(2, 70, 18)) && f.contains(&(3, 70, 18)) && f.contains(&(18, 68, 3)) && f.contains(&(18, 69, 3)));
        assert!(!f.iter().any(|p| p.1 < 64 || (p.0 == 12 && p.1 < 66)), "the ground and the post stay");
        // the wall lands on the ground in the order it stood, and the torch goes with the floor's end
        let wall = c.iter().find(|c| c.x == 5 && c.z == 5).unwrap();
        assert_eq!(wall.lands, vec![64, 65, 66, 67]);
        let end = c.iter().find(|c| c.x == 21 && c.z == 12).unwrap();
        assert_eq!(end.gone, vec![67]);
        assert_eq!(end.lands, vec![64]);
    }

    #[test]
    fn gravity_settles_nearest_first_and_throws_only_so_many() {
        let s = Settle::new((0, 0), 100.0, [90.0, 64.0, 0.0]);
        assert!(s.tiles() > 20);
        let d = |t: (i32, i32)| (f64::from(t.0 + GV_TILE / 2) - 90.0).hypot(f64::from(t.1 + GV_TILE / 2));
        assert!(s.tiles.windows(2).all(|p| d(p[0]) <= d(p[1])));
        // a whole floor of falls near the player: falling blocks up to GV_THROWN, the rest moved down in one edit
        let mut s = Settle::new((0, 0), 30.0, [0.0, 64.0, 0.0]);
        let cols: Vec<ColFall> = (0..600)
            .map(|k| ColFall { x: k % 30, z: k / 30, falls: vec![Fall { x: k % 30, y: 70, z: k / 30, name: "minecraft:stone_bricks" }], gone: vec![], lands: vec![64] })
            .collect();
        let mut out = SettleOut::default();
        s.give(cols, [0.0, 64.0, 0.0], &mut out);
        assert_eq!(s.thrown, GV_THROWN);
        assert_eq!(s.moved, 600 - GV_THROWN);
        assert_eq!(out.cmds.iter().filter(|c| c.starts_with("summon")).count() as u32, GV_THROWN);
        assert_eq!(out.edits.len(), 2);
        assert!(out.edits[1].clear_of_player);
    }
}
