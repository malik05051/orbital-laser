// Orbital Laser's post pass for the Earthquake (laser 19). Like the Sinkhole's, the mod builds it from shaders/strike.wgsl
// up to its fs_main (the helpers: the noise, project_rel, ign) less the other lasers' own functions, followed by this
// file, and sets it for an earthquake strike only. Validate with tools/check-shader.sh.

// ---- laser 19: the earthquake -------------------------------------------------------------------------------------
// No beam, nothing from the sky: a historic earthquake (strike/quake.rs's QK_PRESETS) on the land round the target. The
// game's own camera shakes (the plan's jolts) and this pass sways the whole picture with it (`qk_violence`: hard at 6+
// and 7), the aftershocks too (the plan's table, mod_data). Dust kicks up along the trace as the
// rupture runs out along it, and boils up in patches over the land where it shook at 6- and over, from the houses that
// fell and the slopes that slid, drifting and settling. A tsunami's water is real water blocks (strike/qk_sea.rs).
// Everything here is strike/quake.rs's model: keep QK_*, qk_preset and qk_pulse in step with QK_PRESETS and QkGeo.
//
// frame.mod_params: [1].z the fault's strike (radians), [3].xyz the player as it fired, [3].w the preset.
const QK_T0: f32 = 2.0;
const QK_COMPRESS: f32 = 3.0;
const QK_VS: f32 = 3.5;
const QK_VP: f32 = 6.0;
const QK_VR: f32 = 2.5;
const QK_DIP_W: f32 = 40.0;

struct QkP {
    kind: i32,          // 0 strike-slip, 1 reverse, 2 megathrust
    mw: f32,
    shindo: f32,
    depth: f32,
    kpb: f32,
    half: f32,
    hypo: f32,
    n: i32,             // its pulses (qk_pulse)
    len: f32,           // the shaking's seconds (the dust gone by then; a tsunami runs on with no pass to draw)
};
fn qk_preset(i: i32) -> QkP {
    switch i {
        case 1: { return QkP(0, 6.9, 6.6, 3.0, 0.15, 170.0, -0.2, 3, 40.0); }
        case 2: { return QkP(2, 7.9, 6.6, 15.0, 0.35, 260.0, -0.6, 3, 62.0); }
        case 3: { return QkP(0, 7.0, 6.6, 3.0, 0.15, 180.0, -0.7, 3, 46.0); }
        case 4: { return QkP(1, 7.5, 6.7, 5.0, 0.25, 220.0, -0.1, 3, 48.0); }
        case 5: { return QkP(2, 9.5, 6.8, 25.0, 0.8, 520.0, -0.9, 5, 100.0); }
        default: { return QkP(2, 9.1, 6.7, 30.0, 0.55, 420.0, -0.1, 4, 80.0); }
    }
}
// A preset's subevent k: (seconds after the first, share of the strongest, seconds it lasts).
fn qk_pulse(i: i32, k: i32) -> vec3<f32> {
    switch i {
        case 1: {
            switch k {
                case 0: { return vec3<f32>(0.0, 1.0, 8.0); }
                case 1: { return vec3<f32>(3.5, 0.7, 6.0); }
                default: { return vec3<f32>(24.0, 0.18, 3.0); }
            }
        }
        case 2: {
            switch k {
                case 0: { return vec3<f32>(0.0, 0.8, 11.0); }
                case 1: { return vec3<f32>(9.0, 1.0, 13.0); }
                default: { return vec3<f32>(34.0, 0.55, 7.0); }
            }
        }
        case 3: {
            switch k {
                case 0: { return vec3<f32>(0.0, 0.75, 6.0); }
                case 1: { return vec3<f32>(16.0, 1.0, 11.0); }
                default: { return vec3<f32>(34.0, 0.25, 4.0); }
            }
        }
        case 4: {
            switch k {
                case 0: { return vec3<f32>(0.0, 0.7, 6.0); }
                case 1: { return vec3<f32>(6.0, 1.0, 14.0); }
                default: { return vec3<f32>(36.0, 0.2, 4.0); }
            }
        }
        case 5: {
            switch k {
                case 0: { return vec3<f32>(0.0, 0.6, 18.0); }
                case 1: { return vec3<f32>(14.0, 0.9, 20.0); }
                case 2: { return vec3<f32>(34.0, 1.0, 20.0); }
                case 3: { return vec3<f32>(54.0, 0.6, 15.0); }
                default: { return vec3<f32>(74.0, 0.2, 5.0); }
            }
        }
        default: {
            switch k {
                case 0: { return vec3<f32>(0.0, 0.45, 13.0); }
                case 1: { return vec3<f32>(18.0, 1.0, 18.0); }
                case 2: { return vec3<f32>(36.0, 0.6, 14.0); }
                default: { return vec3<f32>(62.0, 0.22, 5.0); }
            }
        }
    }
}
fn qk_index() -> i32 {
    return clamp(i32(frame.mod_params[3].w + 0.5), 0, 5);
}
// The strongest pulse: (its time, its length).
fn qk_main(i: i32, P: QkP) -> vec2<f32> {
    var best = vec3<f32>(0.0);
    for (var k = 0; k < P.n; k = k + 1) {
        let p = qk_pulse(i, k);
        if (p.y > best.y) {
            best = p;
        }
    }
    return vec2<f32>(best.x, best.z);
}

// The fault's frame: along its strike, and across it towards the player's side.
struct QkF {
    t: vec2<f32>,
    st: vec2<f32>,
    nm: vec2<f32>,
};
fn qk_frame() -> QkF {
    let t = anchor_xz();
    let a = wing_a();
    let st = vec2<f32>(cos(a), sin(a));
    var nm = vec2<f32>(-st.y, st.x);
    let me = frame.mod_params[3].xyz;
    if (dot(nm, me.xz - t) < 0.0) {
        nm = -nm;
    }
    return QkF(t, st, nm);
}
fn qk_rh(P: QkP, u: f32, d: f32) -> f32 {
    let du = max(abs(u) - P.half, 0.0);
    var dd = abs(d);
    if (P.kind == 1) {
        dd = select(max(-d - QK_DIP_W, 0.0), d, d > 0.0);
    } else if (P.kind == 2) {
        dd = max(d, 0.0);
    }
    return length(vec2<f32>(du, dd));
}
fn qk_lpgv(mw: f32, x: f32) -> f32 {
    let m = min(mw, 8.3);
    return -log2(x + 0.0028 * pow(10.0, 0.5 * m)) * 0.30103 - 0.002 * x;
}
// The instrumental Shindo at (u, d).
fn qk_ii(P: QkP, u: f32, d: f32) -> f32 {
    let x = length(vec2<f32>(qk_rh(P, u, d) * P.kpb, P.depth));
    return clamp(P.shindo + 1.72 * (qk_lpgv(P.mw, x) - qk_lpgv(P.mw, P.depth)), 0.0, 7.0);
}
fn qk_shake(i: f32) -> f32 {
    return clamp((i - 2.5) / 4.5, 0.0, 1.0);
}
// How violent at intensity i (strike/quake.rs's qk_violence): steeply more over 6+ and 7.
fn qk_violence(i: f32) -> f32 {
    return qk_shake(i) * (1.0 + 1.6 * smoothstep(5.5, 7.0, i));
}

// The plan's table (strike/quake.rs's qk_table, QK_TAB_*): the aftershocks (a count, then each one's start, how much
// weaker it shakes, its length) and the sea drawing back (its start, the first wave's arrival, how far down, how fast
// it runs, the hand-over to the Tsunami, the sea's top water block).
const QK_TAB_AFTER: u32 = 0u;
const QK_TAB_SEA: u32 = 52u;
fn qk_tab(i: u32) -> f32 {
    if (i >= arrayLength(&mod_data)) {
        return 0.0;
    }
    return bitcast<f32>(mod_data[i]);
}
// The strongest aftershock's violence at (u, d) at t (Shindo `ii` there for the main shock).
fn qk_after(P: QkP, u: f32, d: f32, ii: f32, t: f32) -> f32 {
    let n = i32(qk_tab(QK_TAB_AFTER));
    if (n <= 0) {
        return 0.0;
    }
    let s = qk_arrive(P, u, d).x;
    var a = 0.0;
    for (var k = 0; k < min(n, 16); k = k + 1) {
        let i = QK_TAB_AFTER + 1u + 3u * u32(k);
        let x = (t - qk_tab(i) - s) / max(qk_tab(i + 2u), 0.5);
        if (x > 0.0 && x < 4.0) {
            a = max(a, qk_violence(ii - qk_tab(i + 1u)) * qk_pulse_at(x));
        }
    }
    return a;
}
// The sea's level over its own at the coast (blocks, below 0 drawn back) at t, before the first wave (qk_sea.rs's
// sea_s, its drawing back), at a point `x` out to sea (x < 0: the trough comes in from offshore, `level`).
fn qk_sea_s(x: f32, t: f32) -> f32 {
    let draw = qk_tab(QK_TAB_SEA);
    let arrive = qk_tab(QK_TAB_SEA + 1u);
    let tt = t - min(x, 0.0) / max(qk_tab(QK_TAB_SEA + 3u), 1.0);
    return -qk_tab(QK_TAB_SEA + 2u) * smoothstep(draw, draw + 0.6 * (arrive - draw), tt);
}
fn qk_front(P: QkP, u: f32) -> f32 {
    if (P.kind == 2) {
        return 0.0;
    }
    let uc = clamp(u, -P.half, P.half);
    return abs(uc - P.hypo * P.half) / (QK_VR * QK_COMPRESS / P.kpb);
}
// (S, P) arrival after a subevent sets out.
fn qk_arrive(P: QkP, u: f32, d: f32) -> vec2<f32> {
    let f = qk_front(P, u);
    let r = qk_rh(P, u, d) + P.depth / P.kpb * 0.5;
    return vec2<f32>(f + r / (QK_VS * QK_COMPRESS / P.kpb), f * 0.7 + r / (QK_VP * QK_COMPRESS / P.kpb));
}
fn qk_pulse_at(x: f32) -> f32 {
    if (x <= 0.0) {
        return 0.0;
    }
    return smoothstep(0.0, 0.12, x) * select(exp(-2.6 * (x - 0.12)), 1.0, x < 0.12);
}
// How hard it shakes at (u, d) at strike second t, 0 to about 1.
fn qk_env(i: i32, P: QkP, u: f32, d: f32, t: f32) -> f32 {
    let a = qk_arrive(P, u, d);
    let tau = t - QK_T0;
    var e = 0.0;
    for (var k = 0; k < P.n; k = k + 1) {
        let p = qk_pulse(i, k);
        e = max(e, p.y * qk_pulse_at((tau - a.x - p.x) / p.z));
    }
    let a0 = qk_pulse(i, 0).y;
    let pw = 0.2 * a0 * smoothstep(a.y, a.y + 0.4, tau) * (1.0 - smoothstep(a.x - 0.2, a.x + 0.3, tau));
    return max(e, pw);
}

// The dust at p (camera-relative), from the trace as it breaks and from the wrecks and slides where it shook hardest.
fn qk_dust(i: i32, P: QkP, F: QkF, p: vec3<f32>, cam: vec3<f32>, gy: f32, t: f32, mt: vec2<f32>) -> f32 {
    let w = p + cam;
    let q = w.xz - F.t;
    let u = dot(q, F.st);
    let d = dot(q, F.nm);
    let h = w.y - gy;
    if (h < -10.0 || h > 46.0) {
        return 0.0;
    }
    var den = 0.0;
    // (blown along a little, rolling)
    let drift = vec3<f32>(1.3, 0.25, 0.7) * t;
    let n = fbm3(w * 0.055 - drift * 0.05 + vec3<f32>(seed_v() * 0.37, 0.0, 0.0));
    // the wrecks and the slides: patches over the land where it shook at 6- and over
    let ii = qk_ii(P, u, d);
    let hard = smoothstep(5.8, 6.8, ii);
    if (hard > 0.0) {
        let a = qk_arrive(P, u, d);
        let x = t - (QK_T0 + mt.x + a.x + 0.15 * mt.y);
        if (x > 0.0) {
            let act = smoothstep(0.0, 2.5, x) * exp(-x / 22.0);
            let patchy = smoothstep(0.42, 0.68, fbm(w.xz * 0.018 + vec2<f32>(seed_v() * 0.13, 3.1)));
            let rise = exp(-max(h, 0.0) / (5.0 + 0.5 * x));
            den = den + 3.0 * hard * act * patchy * rise * smoothstep(0.25, 0.65, n);
        }
    }
    return den;
}

// The trace's dust, where the ray crosses the fault's plane (a band a few blocks wide is too thin to march through): a
// curtain thrown up as the rupture runs past, widening, rising and settling over half a minute. (opacity, height over
// the ground there)
fn qk_curtain(P: QkP, F: QkF, rd: vec3<f32>, cam: vec3<f32>, gy: f32, t: f32, mt: vec2<f32>, scene_len: f32) -> vec2<f32> {
    if (P.kind == 2) {
        return vec2<f32>(0.0);
    }
    let c = cam.xz - F.t;
    let dn = dot(rd.xz, F.nm);
    let d0 = dot(c, F.nm);
    if (abs(dn) < 1.0e-4) {
        return vec2<f32>(0.0);
    }
    let tc = -d0 / dn;
    if (tc <= 0.0 || tc > scene_len + 6.0) {
        return vec2<f32>(0.0);
    }
    let w = cam + rd * tc;
    let u = dot(w.xz - F.t, F.st);
    if (abs(u) > P.half) {
        return vec2<f32>(0.0);
    }
    let x = t - (QK_T0 + mt.x + qk_front(P, u));
    if (x <= 0.0) {
        return vec2<f32>(0.0);
    }
    let h = w.y - gy;
    let act = smoothstep(0.0, 0.5, x) * exp(-x / 18.0);
    let top = 4.0 + 1.1 * x;
    let rise = smoothstep(-3.0, 0.5, h) * (1.0 - smoothstep(top * 0.5, top, h));
    let n = fbm(vec2<f32>(u * 0.09 - t * 0.3, h * 0.12 - t * 0.25) + vec2<f32>(seed_v() * 0.41, 0.0));
    let ends = 1.0 - smoothstep(0.85, 1.0, abs(u) / P.half);
    // (thicker seen along it than across: the band's width over the ray's slant through it)
    let width = (3.0 + 0.4 * x) / max(abs(dn), 0.35);
    let den = act * rise * ends * smoothstep(0.2, 0.7, n) * width * 0.11;
    return vec2<f32>(1.0 - exp(-den), max(h, 0.0));
}

fn earthquake(in: StageIn) -> vec4<f32> {
    let uv = in.uv;
    let t = now();
    let cam = frame.camera_position.xyz;
    let k = intensity_v();
    let i = qk_index();
    let P = qk_preset(i);
    let F = qk_frame();
    let mt = qk_main(i, P);
    let gy = block_y() + 1.0;
    let sun = frame.sun.xyz;
    let day = saturate(sun.y * 2.5 + 0.35);
    let fade = 1.0 - smoothstep(P.len - 6.0, P.len, t);

    // ---- the shaking: the whole picture moved together, as a camera held in shaking hands (never a pixel by where it
    // is in the world: that tore the land into doubled, smeared bands at every edge in depth). How hard is where the
    // camera is, by its Shindo and the shaking there now (qk_env, the plan's own model); a great quake's long, slow
    // rolling heaves, a shallow one's quick, hard jolts, a rattle over both. Zoomed in a little so no edge shows. ----
    var uv2 = uv;
    if (t > QK_T0) {
        let q = cam.xz - F.t;
        let cu = dot(q, F.st);
        let cd = dot(q, F.nm);
        let ic = qk_ii(P, cu, cd);
        let a = max(qk_violence(ic) * qk_env(i, P, cu, cd, t) * fade, qk_after(P, cu, cd, ic, t)) * k;
        if (a > 0.001) {
            let great = smoothstep(7.6, 8.8, P.mw);
            // (at 6+ and 7 the hard, sudden jerks win over the rolling: the ground throwing everything about)
            let wild = smoothstep(1.2, 2.4, a);
            let s0 = seed_v() * 1.7;
            let fr = mix(2.7, 0.75, great);
            let roll = vec2<f32>(sin(t * fr * 6.2832 + s0) + 0.45 * sin(t * fr * 2.31 * 6.2832 + s0 * 2.0), cos(t * fr * 0.83 * 6.2832 + s0 * 3.0) + 0.4 * sin(t * fr * 1.73 * 6.2832 + s0));
            let jolt = vec2<f32>(fbm(vec2<f32>(t * 7.0, s0)) - 0.5, fbm(vec2<f32>(s0, t * 7.3)) - 0.5) * 2.0;
            let rattle = vec2<f32>(sin(t * 61.0 + s0), sin(t * 53.0 + s0 * 1.3));
            let jerk = vec2<f32>(fbm(vec2<f32>(t * 13.0, s0 + 7.0)) - 0.5, fbm(vec2<f32>(s0 + 9.0, t * 12.1)) - 0.5) * 2.0;
            let off = (roll * mix(0.55, 0.85, great) + jolt * mix(0.6, 0.25, great) + jerk * 0.7 * wild + rattle * (0.06 + 0.1 * wild)) * a * 0.016;
            let rot = (sin(t * fr * 1.1 * 6.2832 + s0 * 5.0) * mix(0.25, 0.5, great) + (fbm(vec2<f32>(t * 5.0, s0 + 3.0)) - 0.5) * (1.0 + wild)) * a * 0.012;
            let zoom = 1.0 + a * 0.055;
            let aspect = frame.resolution.x / frame.resolution.y;
            var c = (uv - vec2<f32>(0.5)) * vec2<f32>(aspect, 1.0) / zoom;
            c = vec2<f32>(c.x * cos(rot) - c.y * sin(rot), c.x * sin(rot) + c.y * cos(rot));
            uv2 = clamp(c / vec2<f32>(aspect, 1.0) + vec2<f32>(0.5) + off, vec2<f32>(0.001), vec2<f32>(0.999));
        }
    }
    var col = srgb_to_linear(textureSampleLevel(scene_color, linear_sampler, uv2, 0.0).rgb);
    let d = depth_at(uv2);
    let sky = is_sky(d);
    let rel = world_offset(uv2, select(d, 0.5, sky));
    let scene_len = select(1.0e9, length(rel), !sky);
    let rd = normalize(world_offset(uv2, 0.5));

    // ---- the dust: a short march through the layer over the land --------------------------------------------------------
    if (t > QK_T0 && fade > 0.001) {
        let y_lo = gy - 10.0 - cam.y;
        let y_hi = gy + 46.0 - cam.y;
        var t0 = 0.0;
        var t1 = min(scene_len, 420.0);
        if (abs(rd.y) > 1.0e-4) {
            let ta = y_lo / rd.y;
            let tb = y_hi / rd.y;
            t0 = max(t0, min(ta, tb));
            t1 = min(t1, max(ta, tb));
        } else if (0.0 < y_lo || 0.0 > y_hi) {
            t1 = -1.0;
        }
        if (t1 > t0) {
            let steps = 28;
            let dt = (t1 - t0) / f32(steps);
            let j = ign(in.position.xy + vec2<f32>(fract(t * 7.3) * 61.0, 0.0));
            var tr = 1.0;
            var acc = vec3<f32>(0.0);
            let Ls = normalize(select(vec3<f32>(0.3, 1.0, 0.2), sun, sun.y > 0.05));
            let amb = mix(vec3<f32>(0.03, 0.035, 0.05), vec3<f32>(0.34, 0.36, 0.4), day);
            let sunc = vec3<f32>(1.2, 1.12, 1.0) * day * (0.5 + 0.5 * saturate(Ls.y * 2.0));
            for (var s = 0; s < steps; s = s + 1) {
                let tt = t0 + dt * (f32(s) + j);
                let p = rd * tt;
                let den = qk_dust(i, P, F, p, cam, gy, t, mt);
                if (den <= 0.0) {
                    continue;
                }
                let a = 1.0 - exp(-den * dt * 0.03);
                let up = smoothstep(-6.0, 30.0, p.y + cam.y - gy);
                let dc = mix(vec3<f32>(0.2, 0.17, 0.14), vec3<f32>(0.62, 0.55, 0.45), up) * (amb * 1.2 + sunc * 0.6);
                acc = acc + tr * a * dc;
                tr = tr * (1.0 - a);
                if (tr < 0.03) {
                    break;
                }
            }
            col = mix(col, col * tr + acc, k * fade);
        }
        // (the curtain over what lies behind it; the march's dust in front of it is a fair approximation left over it)
        let cu = qk_curtain(P, F, rd, cam, gy, t, mt, scene_len);
        if (cu.x > 0.0) {
            let amb = mix(vec3<f32>(0.03, 0.035, 0.05), vec3<f32>(0.34, 0.36, 0.4), day);
            let up = smoothstep(0.0, 20.0, cu.y);
            let dc = mix(vec3<f32>(0.22, 0.18, 0.14), vec3<f32>(0.66, 0.58, 0.47), up) * (amb * 1.2 + vec3<f32>(0.7, 0.66, 0.6) * day);
            col = mix(col, dc, cu.x * k * fade);
        }
    }

    // ---- the sea drawing back (a tsunami's): the water blocks go down a block at a time (qk_sea.rs's sea_work); here the
    // bed it bares glistens wet, water streams off it back out to sea, and the sea's own surface runs out seaward. ----
    let draw_t = qk_tab(QK_TAB_SEA);
    if (draw_t > 0.0 && t > draw_t && t < qk_tab(QK_TAB_SEA + 4u) + 0.6 && !sky) {
        let w = cam + rel;
        let q = w.xz - F.t;
        let u = dot(q, F.st);
        let dd = dot(q, F.nm);
        let sea_y = qk_tab(QK_TAB_SEA + 5u);
        let inside = (1.0 - smoothstep(170.0, 184.0, abs(u))) * (1.0 - smoothstep(110.0, 122.0, -dd)) * (1.0 - smoothstep(0.0, 6.0, dd));
        if (inside > 0.0) {
            let s = qk_sea_s(dd, t);
            // (the water's surface as the blocks have it: the top water block's, its surface 7/8 up it)
            let lvl = sea_y + round(s) + 0.875;
            let on = smoothstep(draw_t, draw_t + 2.0, t) * inside * k;
            let skyc = mix(vec3<f32>(0.04, 0.05, 0.08), vec3<f32>(0.42, 0.56, 0.8), day);
            let graze = pow(1.0 - saturate(-rd.y), 3.0);
            let flow = t * 2.2;
            if (w.y > lvl + 0.04 && w.y < sea_y + 1.02) {
                // the bared bed: dark, wet, the sky in it, rivulets running off it out to sea (along -nm), the sun glinting
                let streak = smoothstep(0.55, 0.8, fbm(vec2<f32>(u * 0.42, (dd + flow * 3.0) * 0.09) + vec2<f32>(seed_v() * 0.3, 0.0)));
                var wet = col * 0.5 + skyc * (0.06 + 0.3 * graze);
                let glint = pow(saturate(dot(reflect(rd, vec3<f32>(0.0, 1.0, 0.0)), normalize(sun))), 60.0) * step(0.0, sun.y);
                wet = wet + vec3<f32>(1.0, 0.95, 0.85) * glint * (0.4 + streak) * day;
                wet = mix(wet, skyc * 0.6 + vec3<f32>(0.03, 0.05, 0.06), streak * 0.55);
                col = mix(col, wet, on);
            } else if (abs(w.y - lvl) < 0.2) {
                // the sea's surface running out: bands of foam and ripples streaming seaward
                let band = smoothstep(0.62, 0.86, fbm(vec2<f32>(u * 0.08, (dd + flow * 5.0) * 0.05) + vec2<f32>(3.7, seed_v() * 0.2)));
                col = mix(col, vec3<f32>(0.75, 0.8, 0.82) * (0.25 + 0.75 * day), band * 0.45 * on);
            }
        }
    }

    let srgb = linear_to_srgb(max(col, vec3<f32>(0.0)));
    if (capture() > 2.5) {
        let sp = floor(in.position.xy);
        let chk = fract((sp.x + sp.y) * 0.5);
        let ycc_y = srgb.g * 0.5 + (srgb.r + srgb.b) * 0.25;
        let ycc_co = srgb.r - srgb.b;
        let ycc_cg = srgb.g - (srgb.r + srgb.b) * 0.5;
        return vec4<f32>(ycc_y, select(ycc_cg, ycc_co, chk < 0.25) * 0.5 + 0.5, 0.0, 1.0);
    }
    return vec4<f32>(srgb, 1.0);
}

@fragment
fn fs_main(in: StageIn) -> @location(0) vec4<f32> {
    return earthquake(in);
}
