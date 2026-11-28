// Player visuals: a port of the drawing side of player.js (PlayerObject) — icon layers for every game mode, the robot and spider
// animations, trails, particles, the death explosion and the level-end fly-out. The simulation stays in wasm; this only reads
// its state (wd_pv_state / wd_pv_events) and draws.
(function () {
  "use strict";
  const T = 460;                       // b(y) = T - y
  const TAU = Math.PI * 2;
  const rnd = (a, b) => a + Math.random() * (b - a);
  const lerp = (a, b, t) => a + (b - a) * t;
  const clamp = (v, lo, hi) => Math.max(lo, Math.min(hi, v));
  const ICON_BASE_RE = /^((?:player_ball|player|ship|dart|bird|spider|robot|swing)_\d{2,3})(?:_.*)?_001\.png$/;
  const iconBaseOf = (n) => { const m = ICON_BASE_RE.exec(String(n || "")); return m ? m[1] : null; };
  const parsePair = (value, fx = 0, fy = 0) => {
    const m = String(value ?? "").match(/\{\s*([-+]?\d*\.?\d+)\s*,\s*([-+]?\d*\.?\d+)\s*\}/);
    if (!m) return { x: fx, y: fy };
    const x = parseFloat(m[1]), y = parseFloat(m[2]);
    return { x: Number.isFinite(x) ? x : fx, y: Number.isFinite(y) ? y : fy };
  };
  const variantName = (n, v) => !n ? n : v === "glow" ? n.replace(/_001\.png$/, "_glow_001.png") : v === "overlay" ? n.replace(/_001\.png$/, "_2_001.png") : v === "extra" ? n.replace(/_001\.png$/, "_extra_001.png") : n;
  const mixWhite = (c, amt) => {
    const t = clamp(Number(amt) || 0, 0, 1), col = Number.isFinite(Number(c)) ? Number(c) : 0xffffff;
    const r = (col >> 16) & 255, g = (col >> 8) & 255, b = col & 255;
    return (Math.round(r + (255 - r) * t) << 16) | (Math.round(g + (255 - g) * t) << 8) | Math.round(b + (255 - b) * t);
  };
  const mixColors = (a, b, amt) => {
    const t = clamp(Number(amt) || 0, 0, 1);
    const ar = (a >> 16) & 255, ag = (a >> 8) & 255, ab = a & 255, br = (b >> 16) & 255, bg = (b >> 8) & 255, bb = b & 255;
    return (Math.round(ar + (br - ar) * t) << 16) | (Math.round(ag + (bg - ag) * t) << 8) | Math.round(ab + (bb - ab) * t);
  };
  const sortBySuffix = (a, b) => (parseInt((String(a).match(/_(\d+)\.png$/) || [0, 0])[1], 10) || 0) - (parseInt((String(b).match(/_(\d+)\.png$/) || [0, 0])[1], 10) || 0);

  // ---- icon atlases (plist + png, repacked like config.js `_buildIconAtlasFromPlist`) ----
  const plistNode = (el) => {
    switch (el.tagName) {
      case "dict": { const o = {}; const kids = Array.from(el.children); for (let i = 0; i + 1 < kids.length; i += 2) o[kids[i].textContent] = plistNode(kids[i + 1]); return o; }
      case "array": return Array.from(el.children).map(plistNode);
      case "integer": return parseInt(el.textContent, 10);
      case "real": return parseFloat(el.textContent);
      case "true": return true;
      case "false": return false;
      default: return el.textContent;
    }
  };
  const plistNums = (s) => (String(s).match(/-?\d+(?:\.\d+)?/g) || []).map(Number);
  const plistFrames = (plist) => {
    const fmt = (plist.metadata && plist.metadata.format) || 3, out = [];
    for (const name of Object.keys(plist.frames || {})) {
      const d = plist.frames[name]; let x, y, w, h, rotated, ox, oy, srcW, srcH;
      if (fmt >= 3) { [x, y] = plistNums(d.textureRect); [w, h] = plistNums(d.spriteSize); rotated = !!d.textureRotated; [ox, oy] = plistNums(d.spriteOffset); [srcW, srcH] = plistNums(d.spriteSourceSize); }
      else { [x, y, w, h] = plistNums(d.frame); rotated = !!d.rotated; [ox, oy] = plistNums(d.offset); [srcW, srcH] = plistNums(d.sourceSize); }
      out.push({ name, x, y, w, h, rotated, srcW, srcH, sx: Math.round((srcW - w) / 2 + ox), sy: Math.round((srcH - h) / 2 - oy) });
    }
    return out;
  };
  const packFrames = (frames, minW) => {
    const pad = 2, maxW = Math.max(minW || 0, ...frames.map((f) => f.w + pad * 2));
    frames.sort((a, b) => b.h - a.h || (a.name < b.name ? -1 : 1));
    let x = pad, y = pad, rowH = 0;
    for (const f of frames) { if (x + f.w + pad > maxW) { x = pad; y += rowH + pad; rowH = 0; } f.dx = x; f.dy = y; x += f.w + pad; rowH = Math.max(rowH, f.h); }
    return { w: maxW, h: y + rowH + pad };
  };

  // ---- small particle engine (Phaser ParticleEmitter semantics for the configs player.js uses) ----
  class Emitter {
    constructor(cfg) { Object.assign(this, { x: 0, y: 0, px: 0, py: 0, parts: [], acc: 0, on: false, alphaMul: 1, freq: 0, quantity: 1, gx: 0, gy: 0, additive: true }, cfg); }
    start() { if (!this.on) { this.on = true; this.acc = 0; } }
    stop() { this.on = false; }
    clear() { this.parts.length = 0; }
    emit(n, x, y) {
      for (let i = 0; i < n; i++) {
        const a = rnd(this.angle[0], this.angle[1]) * Math.PI / 180, sp = typeof this.speed === "number" ? this.speed : rnd(this.speed[0], this.speed[1]);
        const life = typeof this.life === "number" ? this.life : rnd(this.life[0], this.life[1]);
        const ox = this.spreadX ? rnd(this.spreadX[0], this.spreadX[1]) : 0;
        this.parts.push({ x: (x ?? this.px) + ox, y: y ?? this.py, vx: Math.cos(a) * sp, vy: Math.sin(a) * sp, age: 0, life });
      }
    }
    update(dt) { // dt in ms
      if (this.on && this.freq > 0) { this.acc += dt; while (this.acc >= this.freq) { this.acc -= this.freq; this.emit(this.quantity); } }
      const s = dt / 1000;
      for (let i = this.parts.length - 1; i >= 0; i--) {
        const p = this.parts[i]; p.age += dt;
        if (p.age >= p.life) { this.parts.splice(i, 1); continue; }
        p.vx += this.gx * s; p.vy += this.gy * s; p.x += p.vx * s; p.y += p.vy * s;
      }
    }
    tintAt(t) {
      const c = this.tint;
      if (c && typeof c === "object") return mixColors(c.start, c.end, Math.round(t * 24) / 24); // 25 steps: every distinct tint is a cached canvas
      return c == null ? -1 : c;
    }
    draw(draw, ox, oy) {
      for (const p of this.parts) {
        const t = p.age / p.life;
        draw("square.png", p.x + ox, p.y + oy, lerp(this.scale[0], this.scale[1], t), lerp(this.alpha[0], this.alpha[1], t) * this.alphaMul, this.tintAt(t), this.additive);
      }
    }
  }

  function create(env) {
    const { ctx, atlas, drawFrame, loadImage, loadText, loadJson, W, H } = env;
    env.tintedCanvas = env.tinted || env.tintedCanvas;
    const icons = {};                  // base -> { img, frames }
    const failed = new Set(), pending = new Map();
    let generation = 0;
    const cfg = {};
    const loadCfg = () => {
      const g = (k, d) => { try { return localStorage.getItem(k) || d; } catch (e) { return d; } };
      cfg.main = parseInt(g("iconMainColor", "04FF00"), 16);
      cfg.second = parseInt(g("iconSecondaryColor", "00FBFF"), 16);
      cfg.player = g("iconCurrentPlayer", "player_01"); cfg.ship = g("iconCurrentShip", "ship_01"); cfg.ball = g("iconCurrentBall", "player_ball_01");
      cfg.wave = g("iconCurrentWave", "dart_01"); cfg.bird = g("iconCurrentBird", "bird_01"); cfg.spider = g("iconCurrentSpider", "spider_01"); cfg.robot = g("iconCurrentRobot", "robot_01"); cfg.swing = g("iconCurrentSwing", "swing_01");
      let s = {}; try { s = JSON.parse(localStorage.getItem("gd_settings") || "{}") || {}; } catch (e) {}
      cfg.trail = Math.min(7, Math.max(1, parseInt(g("iconCurrentTrail", "1"), 10) || 1)); cfg.fire = Math.min(6, Math.max(1, parseInt(g("iconCurrentShipFire", "1"), 10) || 1)); cfg.death = Math.min(20, Math.max(1, parseInt(g("iconCurrentDeathEffect", "1"), 10) || 1)); cfg.explode = g("iconExplode", "true") !== "false";
      cfg.slowRobot = g("iconSlowRobotRun", "false") === "true"; cfg.fastRobot = g("iconFastRobotRun", "false") === "true"; cfg.fastSpider = g("iconFastSpiderRun", "false") === "true";
      cfg.glow = g("iconGlowEnabled", "false") === "true"; cfg.glowColor = parseInt(g("iconGlowColor", "7DFF00"), 16);
      cfg.ldm = !!s.enableLDM; cfg.miniIcon = !!s.enableMiniIcon; cfg.solidWave = !!s.solidWaveTrail; cfg.showHitboxes = !!s.showHitboxes;
    };
    loadCfg();
    const equippedBases = () => [...new Set([cfg.player, cfg.ship, cfg.ball, cfg.wave, cfg.bird, cfg.spider, cfg.robot, cfg.swing].map((n) => iconBaseOf(n + "_001.png")).filter(Boolean))];

    const extraSheets = {};
    const loadSheet = async (name) => {
      if (atlas[name] || extraSheets[name]) return;
      try {
        const [j, img] = await Promise.all([loadJson(`../ref/assets/sheets/${name}.json`), loadImage(`../ref/assets/sheets/${name}.png`)]);
        let frames = j.frames; if (!frames && j.textures) frames = j.textures.flatMap((t) => t.frames);
        if (Array.isArray(frames)) frames = Object.fromEntries(frames.map((f) => [f.filename, f]));
        extraSheets[name] = { img, frames };
      } catch (e) { /* optional sheet */ }
    };
    const sheetOf = (name) => atlas[name] || extraSheets[name];
    const ATLAS_LIST = ["GJ_WebSheet", "GJ_GameSheet", "GJ_GameSheet02", "GJ_GameSheet03", "GJ_GameSheet04", "GJ_GameSheetEditor", "GJ_GameSheetGlow", "GJ_GameSheetIcons", "GJ_LaunchSheet", "player_ball_00", "player_dart_00", "GJ_ParticleSheet", "PixelSheet_01", "FireSheet_01-hd", "Wavesheet"];
    const PLAYER_PRIORITY = ["GJ_GameSheet03", "GJ_GameSheet", "GJ_GameSheet02", "GJ_GameSheet04", "GJ_GameSheetEditor", "GJ_GameSheetGlow", "GJ_GameSheetIcons", "GJ_WebSheet", "GJ_LaunchSheet", "player_ball_00", "player_dart_00"];
    let customCube = null; // canvas of the imported cube icon (60 px)
    const customFrame = () => customCube && { img: customCube, d: { frame: { x: 0, y: 0, w: customCube.width, h: customCube.height }, sourceSize: { w: customCube.width, h: customCube.height }, spriteSourceSize: { x: 0, y: 0 } }, key: "custom_cube" };
    const frameCache = new Map();
    // `getAtlasFrame`: the per-icon atlas wins when it is loaded; otherwise the shared sheets
    const getFrame = (name) => {
      if (name === "__custom_cube__") return customFrame();
      const c = frameCache.get(name);
      if (c && c.gen === generation) return c.v;
      let v = null;
      const base = iconBaseOf(name);
      if (base && icons[base]) { const d = icons[base].frames[name]; v = d ? { img: icons[base].img, d, key: "icon_" + base } : null; }
      else {
        for (const a of (name.startsWith("player_") ? PLAYER_PRIORITY : ATLAS_LIST)) { const sh = sheetOf(a); if (sh && sh.frames && sh.frames[name]) { v = { img: sh.img, d: sh.frames[name], key: a }; break; } }
      }
      frameCache.set(name, { gen: generation, v });
      return v;
    };

    const loadIconAtlas = (base) => {
      if (icons[base] || failed.has(base)) return Promise.resolve(!!icons[base]);
      if (pending.has(base)) return pending.get(base);
      const pr = (async () => {
        try {
          const [xml, img] = await Promise.all([loadText(`../ref/assets/icons/${base}-hd.plist`), loadImage(`../ref/assets/icons/${base}-hd.png`)]);
          const doc = new DOMParser().parseFromString(xml, "text/xml");
          const plist = plistNode(doc.documentElement.firstElementChild);
          const frames = plistFrames(plist);
          if (!frames.length) throw new Error("no frames");
          const size = packFrames(frames, img.width);
          const cv = document.createElement("canvas"); cv.width = size.w; cv.height = size.h;
          const g = cv.getContext("2d");
          for (const f of frames) {
            if (f.rotated) { g.save(); g.translate(f.dx, f.dy + f.h); g.rotate(-Math.PI / 2); g.drawImage(img, f.x, f.y, f.h, f.w, 0, 0, f.h, f.w); g.restore(); }
            else g.drawImage(img, f.x, f.y, f.w, f.h, f.dx, f.dy, f.w, f.h);
          }
          cv.src = "icon:" + base; // keeps the host's tint cache keys apart
          const fr = {};
          for (const f of frames) fr[f.name] = { frame: { x: f.dx, y: f.dy, w: f.w, h: f.h }, rotated: false, trimmed: !(f.w === f.srcW && f.h === f.srcH && !f.sx && !f.sy), spriteSourceSize: { x: f.sx, y: f.sy, w: f.w, h: f.h }, sourceSize: { w: f.srcW, h: f.srcH } };
          icons[base] = { img: cv, frames: fr };
          atlas["icon_" + base] = { img: cv, frames: fr };
          generation++;
          if (env.onIconAtlas) env.onIconAtlas(base, fr);
          return true;
        } catch (e) { failed.add(base); return false; } finally { pending.delete(base); }
      })();
      pending.set(base, pr);
      return pr;
    };
    const ensureIcons = (bases) => Promise.all(bases.map(loadIconAtlas));

    // ---- animation descriptors for the robot and spider ----
    const anim = { robot: null, spider: null };
    const groupsOf = (desc, prefix, names) => {
      const keys = Object.keys(desc.animationContainer), out = {};
      for (const [k, re] of Object.entries(names)) out[k] = keys.filter((x) => new RegExp("^" + prefix + re + "_\\d+\\.png$").test(x)).sort(sortBySuffix);
      return out;
    };
    const entriesOf = (desc, fallback) => {
      let e = desc && desc.usedTextures ? Object.values(desc.usedTextures).slice().sort((a, b) => (parseInt(a.tag || "0", 10) || 0) - (parseInt(b.tag || "0", 10) || 0)) : [];
      return e.length ? e : fallback;
    };

    // ---- one player ----
    class Vis {
      constructor(index) {
        this.index = index; this.inverted = index === 1;
        this.s = new Float64Array(27); this.prev = new Float64Array(27); this.swingFlames = [{ g: 0 }, { g: 0 }, { g: 0 }]; this.swingTime = 0;
        this.items = []; this.prevItems = [];
        this.rotation = 0; this.hidden = false;
        this.dashFrame = 0; this.dashTimer = 0; this.dashShown = false;
        this.spiderTimer = 0; this.spiderFlash = 0; this.robotTimer = 0; this.robotState = "run";
        this.ufoLast = null; this.ufoTilt = 0; this.ufoRot = NaN;
        this.circles = []; this.dashFx = null;
        this.pieces = null; this.burst = null; this.flash = null; this.deathFx = null; this.fireTime = 0; this.ended = null;
        this.lastCamY = 0; this.landIdx = false; this.wasDead = false; this.wasActive = false;
        this.streak = newStreak(); this.boost = false; this.lastMode = -1; this.wave = { pts: [], pos: null, active: false, pool: [] };
        this.em = {
          ground: new Emitter({ speed: [110, 190], angle: [225, 315], life: [150, 450], scale: [0.5, 0], alpha: [1, 0], gy: 600, freq: 1000 / 30 }),
          fly: new Emitter({ speed: [22, 38], angle: [225, 315], life: [150, 450], scale: [0.5, 0], alpha: [1, 0], gy: 600, freq: 1000 / 30, tint: { start: 16737280, end: 16711680 } }),
          fly2: new Emitter({ speed: [220, 380], angle: [180, 360], life: [150, 450], scale: [0.75, 0], alpha: [1, 0], gy: 600, freq: 1000 / 30, tint: { start: 16760320, end: 16711680 } }),
          drag: new Emitter({ speed: [223.8, 343.8], angle: [205, 295], life: [80, 220], scale: [0.375, 0], alpha: [1, 0], gx: -700, gy: 600, freq: 25, spreadX: [-18, 18] }),
          land1: new Emitter({ speed: [250, 350], angle: [210, 330], life: [50, 600], scale: [0.625, 0], alpha: [1, 0], gy: 1000 }),
          land2: new Emitter({ speed: [250, 350], angle: [210, 330], life: [50, 600], scale: [0.625, 0], alpha: [1, 0], gy: 1000 }),
        };
        this.fx = [];                                   // orb / pad / portal / gravity effects (world space)
      }
      primary() { return this.inverted ? cfg.second : cfg.main; }
      secondary() { return this.inverted ? cfg.main : cfg.second; }
      tintOf(kind) { return kind === "base" ? this.primary() : kind === "glow" ? cfg.glowColor : (kind === "overlay" || kind === "secondary") ? this.secondary() : -1; }

      // The layer set of one icon: frame names resolved through the atlas lookup
      layerFrames(prefix, style) {
        const base = `${prefix}_001.png`;
        if (style === "bird") return [{ kind: "glow", name: `${prefix}_glow_001.png`, depth: 12.05 }, { kind: "secondary", name: `${prefix}_2_001.png`, depth: 12.2 }, { kind: "base", name: base, depth: 12.3 }, { kind: "overlay", name: `${prefix}_3_001.png`, depth: 12.1 }, { kind: "extra", name: `${prefix}_extra_001.png`, depth: 12.4 }];
        if (style === "ship") return [{ kind: "glow", name: `${prefix}_glow_001.png`, depth: 12.2 }, { kind: "base", name: base, depth: 12.3 }, { kind: "overlay", name: `${prefix}_2_001.png`, depth: 12.1 }, { kind: "extra", name: `${prefix}_extra_001.png`, depth: 12.4 }];
        return [{ kind: "glow", name: `${prefix}_glow_001.png`, depth: 9 }, { kind: "base", name: base, depth: 10 }, { kind: "overlay", name: `${prefix}_2_001.png`, depth: 8 }, { kind: "extra", name: `${prefix}_extra_001.png`, depth: 12 }];
      }

      push(group, kind, name, depth, x, y, rot, sx, sy, tint, extraKey) {
        const fr = getFrame(name);
        if (!fr) return;
        this.items.push({ group, kind, fr, depth, order: this.items.length, x, y, rot, sx, sy, tint, alpha: 1 });
      }

      // Everything player.js `syncSprites` places, for one frame. (x, y) are container coordinates (b(y) without the camera).
      layout(p, dt) {
        const mini = p.mini ? 0.6 : 1, gf = p.gf, mir = p.mir, rot = this.rotation;
        const mrot = mir ? -rot : rot;
        const X = p.x, Y = T - p.y;
        const sxs = mir ? -1 : 1, sys = gf ? -1 : 1;
        const cubeName = cfg.miniIcon && p.mini ? "player_00" : cfg.player;
        const cubeLayers = () => {
          if (customCube) return [{ kind: "base", name: "__custom_cube__", depth: 10 }];
          const a = this.layerFrames(cfg.player);
          if (cfg.miniIcon && p.mini) { a[1].name = "player_00_001.png"; a[2].name = "player_00_2_001.png"; }
          return a;
        };
        const ballLayers = () => {
          const a = this.layerFrames(cfg.ball);
          if (cfg.miniIcon && p.mini) { a[1].name = "player_ball_00_001.png"; a[2].name = "player_ball_00_2_001.png"; }
          return a;
        };
        const emit = (group, layers, x, y, r, sx, sy, glowVisible) => {
          for (const l of layers) {
            if (l.kind === "glow" && !(glowVisible || cfg.glow)) continue;
            this.push(group, l.kind, l.name, l.depth, x, y, r, sx, sy, l.name === "__custom_cube__" ? -1 : this.tintOf(l.kind));
          }
        };
        if (p.ship || p.ufo) {
          const ship = p.ship, off = ship ? 10 : (gf ? -10 : 10) * mini;
          const cs = Math.cos(rot), sn = Math.sin(rot);
          const shipX = -off * sn * (mir ? -1 : 1), shipY = off * cs;
          const playerOffset = (gf ? -10 : 10) * mini;
          const cubeDX = playerOffset * sn * (mir ? -1 : 1), cubeDY = -playerOffset * cs;
          const ufoMode = p.ufo && !p.ship;
          if (ship) { emit("ship", this.layerFrames(cfg.ship, "ship"), X + shipX, Y + shipY + (gf ? -20 * mini : 0), mrot, sxs * mini, sys * mini, false); this.fireLayout(p, X + shipX, Y + shipY + (gf ? -20 * mini : 0), mrot, mini, dt); }
          let birdX = 0, birdY = 0, cubeX, cubeY, cubeRot = mrot, birdRot = mrot;
          if (p.ufo) { birdX = X + shipX; birdY = Y + shipY + (gf ? -5 : 5) * mini; }
          if (ship) { const cubeSide = gf ? -1 : 1; cubeX = X + shipX + cubeSide * 20 * mini * sn * (mir ? -1 : 1); cubeY = Y + shipY + (gf ? -20 * mini : 0) - cubeSide * 20 * mini * cs; }
          else { cubeX = X + cubeDX; cubeY = Y + cubeDY; }
          if (ufoMode) {
            const ux = p.x, last = this.ufoLast || (this.ufoLast = { x: ux, y: p.y });
            const dx = ux - last.x, dy = p.y - last.y;
            if (dx * dx + dy * dy >= 4.8) {
              let aim = 0.4 * Math.atan2(dy, Math.abs(dx));
              aim = gf ? Math.min(aim, 0.1) : Math.max(aim, -0.1);
              this.ufoTilt = aim; last.x = ux; last.y = p.y;
            }
            const target = mir ? -this.ufoTilt : this.ufoTilt;
            if (!Number.isFinite(this.ufoRot)) this.ufoRot = target;
            if (Number.isFinite(p.slope)) {
              const st = mir ? -p.slope : p.slope;
              const sp = p.speed, gdSpeed = sp === 9.30222544655 ? 0.7 : sp === 14.3488938625 ? 1.1 : sp === 17.3333393414 ? 1.3 : sp === 21.3333407279 ? 1.6 : 0.9;
              const mix = Math.min(1, gdSpeed * 0.175 * dt * 60);
              this.ufoRot += (st - this.ufoRot) * mix;
            } else {
              const mix = 1 - Math.pow(0.85, Math.max(0, dt * 60));
              this.ufoRot += (target - this.ufoRot) * mix;
              this.ufoRot = Math.max(-0.11, Math.min(3, this.ufoRot));
            }
            const ur = this.ufoRot, delta = ur - mrot, c = Math.cos(delta), s = Math.sin(delta);
            const ox = cubeX - birdX, oy = cubeY - birdY;
            cubeX = birdX + ox * c - oy * s; cubeY = birdY + ox * s + oy * c; cubeRot = ur; birdRot = ur;
          }
          if (p.ufo) for (const l of this.layerFrames(cfg.bird, "bird")) { if (l.kind === "glow" && !cfg.glow) continue; if (l.kind === "extra" && !getFrame(l.name)) continue; this.push("bird", l.kind, l.name, l.depth, birdX, birdY, birdRot, sxs * mini, sys * mini, this.tintOf(l.kind)); }
          const cm = mini * 0.55;
          emit("cube", cubeLayers(), cubeX, cubeY, cubeRot, sxs * cm, sys * cm, false);
          return;
        }
        if (p.swing) { this.swingLayout(p, X, Y, dt, mini, mrot, sxs); return; }
        if (p.spider) { this.spiderLayout(p, X, Y, dt); return; }
        if (p.robot) { this.robotLayout(p, X, Y, dt); return; }
        if (p.ball) { emit("ball", ballLayers(), X, Y, rot, sxs * mini, mini, false); return; }
        if (p.wave) {
          const m = mini * 0.94;
          const layers = this.layerFrames(cfg.wave);
          for (const l of layers) {
            if (l.kind === "glow" && !cfg.glow) continue;
            const off = l.kind === "base" ? [1.5 * (mir ? 1 : -1), -1] : [0, 0];
            this.push("wave", l.kind, l.name, l.depth, X + off[0], Y + off[1], mrot, sxs * m, sys * m, this.tintOf(l.kind));
          }
          return;
        }
        emit("cube", cubeLayers(), X, Y, mrot, sxs * mini, sys * mini, false);
      }

      // robot / spider: per-part layers positioned by the AnimDesc frames
      partLayout(kind, p, X, Y, dt, frameKey, flashAmount) {
        const a = anim[kind]; if (!a) return;
        const frame = a.desc.animationContainer[frameKey]; if (!frame) return;
        const mini = p.mini ? 0.6 : 1, mirSign = p.mir ? -1 : 1, gravSign = p.gf ? -1 : 1, posYSign = p.gf ? 1 : -1;
        const baseName = cfg[kind];
        const resolve = (n) => String(n || "").replace(new RegExp("^" + kind + "_\\d+"), baseName);
        for (const key of Object.keys(frame)) {
          if (!key.startsWith("sprite_")) continue;
          const sd = frame[key], tag = parseInt(sd.tag || "0", 10) || 0;
          if (!a.tags.has(tag)) continue;
          const pos = parsePair(sd.position, 0, 0), sc = parsePair(sd.scale, 1, 1), fl = parsePair(sd.flipped, 0, 0);
          const z = parseFloat(sd.zValue || tag || "0") || 0, rotDeg = parseFloat(sd.rotation || "0") || 0;
          let localY, localXs;
          if (kind === "spider") { const leg = [0, 1, 4, 5].includes(tag); localY = tag === 3 ? 5 : leg ? -9 : 0; localXs = leg ? 1.8 : 1; }
          else { const leg = [0, 2, 4, 6].includes(tag), arm = [1, 5].includes(tag); localY = tag === 3 ? 5 : arm ? -1 : leg ? -9 : 0; localXs = leg || arm ? 1.8 : 1; }
          const cx = X + pos.x * localXs * mirSign * mini, cy = Y + (pos.y + localY) * posYSign * mini;
          let cr = rotDeg * Math.PI / 180; if (p.mir) cr = -cr; if (p.gf) cr = -cr;
          const bsx = sc.x * (fl.x ? -1 : 1) * mirSign * mini, bsy = sc.y * (fl.y ? -1 : 1) * gravSign * mini;
          const tex = resolve(sd.texture || a.textures[tag]);
          for (const [vk, off] of [...(cfg.glow ? [["glow", -0.04]] : []), ["base", 0], ["overlay", -0.02], ["extra", 0.08]]) {
            const fn = resolve(variantName(tex, vk === "base" ? "base" : vk));
            const fr = getFrame(fn); if (!fr) continue;
            let tint = this.tintOf(vk);
            if (flashAmount > 0) tint = mixWhite(tint < 0 ? 0xffffff : tint, flashAmount);
            this.items.push({ group: kind, kind: vk, fr, depth: 8 + z * 0.1 + off, order: this.items.length, x: cx, y: cy, rot: cr, sx: bsx, sy: bsy, tint, alpha: 1 });
          }
        }
      }
      speedFactor(p) {
        const one = 11.540004, half = 9.30222544655, four = 21.3333407279, sp = Number.isFinite(p.speed) ? p.speed : one;
        if (sp >= one) return 1.75 + clamp((sp - one) / Math.max(1e-6, four - one), 0, 1) * 0.35;
        return 1.75 - clamp((one - sp) / Math.max(1e-6, one - half), 0, 1) * 0.15;
      }
      // `_syncShipFire`: the animated flame behind the ship (fire styles 2..6; style 1 is the particle fire)
      fireLayout(p, sx, sy, angle, mini, dt) {
        const id = cfg.fire, count = [0, 0, 9, 10, 6, 16, 5][id] || 0;
        if (id === 1 || !count || cfg.ldm || p.dead) { this.fireTime = 0; return; }
        const recs = Array.from({ length: count }, (_, i) => loadEffectImage(`shipfire${String(id).padStart(2, "0")}_${String(i + 1).padStart(3, "0")}.png`));
        if (!recs.every((r) => r && r.img)) return;
        this.fireTime += Math.max(0, dt);
        const rec = recs[Math.floor(this.fireTime * 24) % count], img = rec.img, maxH = Math.max(1, ...recs.map((r) => r.img.height));
        const mirror = p.mir ? -1 : 1, gravity = p.gf ? -1 : 1, ox = -24 * mini * mirror, oy = 8 * mini * gravity;
        const px = sx + ox * Math.cos(angle) - oy * Math.sin(angle), py = sy + ox * Math.sin(angle) + oy * Math.cos(angle), rot = angle - Math.PI / 2 * mirror;
        const base = 63 * mini / maxH, speedRatio = Math.min(1, Math.abs(Number(p.yVel) || 0) / (16 * (p.mini ? 1.176470588 : 1)));
        const scx = base * (gravity < 0 ? -1 : 1), scy = base * 0.8 * (1 + 0.5 * speedRatio);
        const cx = px + (img.height * scy / 2) * Math.sin(rot), cy = py - (img.height * scy / 2) * Math.cos(rot);
        const d = { frame: { x: 0, y: 0, w: img.width, h: img.height }, rotated: false, trimmed: false, sourceSize: { w: img.width, h: img.height }, spriteSourceSize: { x: 0, y: 0, w: img.width, h: img.height } };
        this.items.push({ group: "ship", kind: "fx", fr: { img, d }, depth: 12.1, order: this.items.length, x: cx, y: cy, rot, sx: scx, sy: scy, tint: -1, alpha: 1 });
      }
      // the swing copter: cube-like layers that never flip vertically, plus three fire flames (`_syncSwingFlames`)
      swingLayout(p, X, Y, dt, mini, mrot, sxs) {
        const parts = [["glow", "_glow_001.png", 9], ["overlay", "_2_001.png", 8], ["base", "_001.png", 10], ["extra", "_extra_001.png", 12]];
        for (const [kind, suffix, depth] of parts) {
          if (kind === "glow" && !cfg.glow) continue;
          let name = `${cfg.swing}${suffix}`;
          if (!getFrame(name) && kind !== "extra") name = `swing_01${suffix}`;
          this.push("swing", kind, name, depth, X, Y, mrot, sxs * mini, mini, this.tintOf(kind));
        }
        if (cfg.ldm) return;
        const fr = getFrame("fireBoost_001.png"); if (!fr) return;
        const h = (fr.d.sourceSize && fr.d.sourceSize.h) || fr.d.frame.h;
        this.swingTime += Math.max(0, dt);
        const mirror = p.mir ? -1 : 1, angle = mrot;
        this.swingFlames.forEach((fl, i) => {
          const target = i === 0 || (i === 1 ? !p.gf : p.gf) ? 1 : 0;
          if (fl.g < target) fl.g = Math.min(target, fl.g + dt / 0.2); else fl.g = Math.max(target, fl.g - dt / 0.5);
          const gr = fl.g * fl.g * (3 - 2 * fl.g);
          if (gr <= 0) return;
          const ox = (i === 0 ? -33 : -28) * mini * mirror * gr, oy = (i === 0 ? 0 : i === 1 ? -23 : 23) * mini * gr;
          const px = X + ox * Math.cos(angle) - oy * Math.sin(angle), py = Y + ox * Math.sin(angle) + oy * Math.cos(angle);
          const rot = angle + (i === 0 ? Math.PI / 2 * mirror : i === 1 ? Math.PI - Math.PI / 4 * mirror : Math.PI / 4 * mirror);
          const sx = mini * 0.825 * gr, sy = mini * (0.975 + 0.195 * Math.sin(this.swingTime * (65 / 1.5) + i)) * gr;
          // origin (0.5, 0): the sprite hangs from its top edge
          const cx = px - Math.sin(rot) * (h * sy / 2), cy = py + Math.cos(rot) * (h * sy / 2);
          this.items.push({ group: "swing", kind: "fx", fr, depth: 7.75, order: this.items.length, x: cx, y: cy, rot, sx, sy, tint: 0xffffff, alpha: 0.9, additive: true });
        });
      }
      spiderLayout(p, X, Y, dt) {
        const a = anim.spider; if (!a) return;
        const g = a.groups;
        this.spiderTimer += Math.max(0, dt) * this.speedFactor(p);
        const fastRun = cfg.fastSpider && p.speed >= 17.3333393414 - 0.001 && g.run2 && g.run2.length;
        let group = !p.onGround && !p.onCeiling ? g.fall : (fastRun ? g.run2 : g.run);
        if (!group || !group.length) group = g.walk;
        if (!group || !group.length) group = g.idle;
        let key = null;
        if (group && group.length) { const fps = p.onGround || p.onCeiling ? 16 : 12; key = group[Math.floor(this.spiderTimer * fps) % group.length]; }
        else if (a.desc.animationContainer["Spider_idle_001.png"]) key = "Spider_idle_001.png";
        const flashDur = 0.5, flashT = this.spiderFlash > 0 ? clamp(this.spiderFlash / flashDur, 0, 1) : 0;
        this.partLayout("spider", p, X, Y, dt, key, flashT * flashT);
        if (this.spiderFlash > 0) this.spiderFlash = Math.max(0, this.spiderFlash - Math.max(0, dt));
      }
      robotLayout(p, X, Y, dt) {
        const a = anim.robot; if (!a) return;
        const g = a.groups, speed = this.speedFactor(p) * 1.2;
        const grounded = p.onGround || p.onCeiling, up = p.gf ? p.yVel < 0 : p.yVel > 0;
        const next = grounded ? "run" : up ? "jump" : "fall";
        if (this.robotState !== next) { this.robotState = next; this.robotTimer = 0; }
        this.robotTimer += Math.max(0, dt) * speed;
        const t = this.robotTimer;
        const startLoop = (start, loop, fps) => {
          start = start || []; loop = loop || [];
          if (start.length) { const sd = start.length / fps; if (t < sd || !loop.length) return start[Math.min(start.length - 1, Math.floor(t * fps))]; return loop[Math.floor((t - sd) * fps) % loop.length]; }
          return loop.length ? loop[Math.floor(t * fps) % loop.length] : null;
        };
        let key = null;
        if (p.dashing) { const gr = g.fallLoop; key = gr.length ? gr[Math.floor(t * 16) % gr.length] : null; }
        else if (next === "run") { const half = 9.30222544655, three = 17.3333393414, alt = cfg.slowRobot && p.speed <= half + 0.001 ? g.skip : cfg.fastRobot && p.speed >= three - 0.001 ? g.run2 : null; const gr = alt && alt.length ? alt : g.run.length ? g.run : (g.idle1.length ? g.idle1 : g.idle); if (gr && gr.length) key = gr[Math.floor(t * 16) % gr.length]; }
        else if (next === "jump") key = startLoop(g.jumpStart, g.jumpLoop, 15);
        else key = startLoop(g.fallStart, g.fallLoop, 15);
        if (!key && a.desc.animationContainer["Robot_idle_001.png"]) key = "Robot_idle_001.png";
        this.partLayout("robot", p, X, Y, dt, key, 0);
      }

      draw(drawFn, ox, oy) {
        const items = this.items.slice().sort((a, b) => a.depth - b.depth || a.order - b.order);
        for (const it of items) drawFn(it, ox, oy);
      }

      // `playEndAnimation`: 1 s along a bezier into the end portal, spinning and fading
      startEnd(p, o) {
        const seg = p.spider || p.robot, layers = this.prevItems.map((it) => ({ ...it }));
        const toX = (wx, camX) => { const sx = wx - camX; return p.mir ? o.screenW - sx : sx; };
        this.ended = {
          t: 0, seg, p0x: p.x, p0y: p.y, endX: o.endX, endY: o.endY || 240, mir: p.mir, ship: p.ship,
          startScreenX: p.x - o.camX, startScreenY: T - p.y + o.camY,
          items: layers.map((it) => ({ it, startRot: it.rot || 0, localX: seg ? it.x - p.x : 0, localY: seg ? it.y - (T - p.y) : (p.ship ? (it.group === "ship" ? 10 : it.group === "cube" ? -10 : 0) : 0) })),
          toX,
        };
        this.streakKeep = true;
      }
      stepEnd(dtMs, o) {
        const E = this.ended; E.t += dtMs;
        const tl = clamp(E.t / 1000, 0, 1), val = Math.pow(tl, 1.2), u = 1 - val;
        const bez = (a, b, c, d) => u * u * u * a + u * u * 3 * val * b + u * 3 * val * val * c + val * val * val * d;
        const wx = bez(E.p0x, E.p0x, E.p0x + 80, E.endX + 100), gy = bez(E.p0y, E.p0y, E.endY + 300, E.endY - 40);
        let sx = E.toX(wx, o.camX), sy = T - gy + o.camY;
        if (E.seg) {
          sx = bez(E.startScreenX, E.startScreenX, E.toX(E.p0x + 80, o.camX), E.toX(E.endX + 100, o.camX));
          sy = bez(E.startScreenY, E.startScreenY, T - (E.endY + 300) + o.camY, T - (E.endY - 40) + o.camY);
        }
        const alpha = 1 - val * val, spin = E.seg ? TAU * Math.pow(tl, 1.5) : 360 * Math.PI / 180 * Math.pow(tl, 1.5);
        const baseRot = E.items.length ? E.items[0].startRot + (E.seg ? spin : spin) : 0;
        const ang = E.seg ? spin : baseRot, c = Math.cos(ang), s = Math.sin(ang);
        this.items = [];
        if (tl < 1) {
          for (const e of E.items) {
            const it = e.it, dx = e.localX * c - e.localY * s, dy = e.localX * s + e.localY * c;
            this.items.push({ ...it, x: sx + dx, y: sy + dy, rot: e.startRot + spin, alpha, screen: true });
          }
        }
        streakSetPos(this.streak, wx, T - gy);
        streakUpdate(this, dtMs / 1000);
        if (tl >= 1) { streakStop(this.streak); streakReset(this.streak); }
      }
    }

    // ---- trails ----
    // StreakManager (player.js): the trail behind a boosting or flying icon, seven looks (`window.currentTrail`)
    const STREAK_STYLES = [null, [0.231, 17], [0.35, 24], [0.55, 13], [0.4, 18], [2.4, 8], [2.4, 6], [0.38, 24]];
    function newStreak() { return { pts: [], pos: null, posInit: false, active: false, style: 0, baseLife: 0.231, stroke: 17, long: false, distance: 0, breakNext: true, suppress: false, mini: false, speed: 11.54, camX: 0, tex: null }; }
    const streakReset = (S) => { S.pts.length = 0; S.posInit = false; S.breakNext = true; S.distance = 0; };
    const streakStart = (S) => { if (!S.active) S.breakNext = true; S.active = true; };
    const streakStop = (S) => { if (S.active) S.breakNext = true; S.active = false; };
    const streakSetStyle = (S, id) => {
      id = Number(id); if (!Number.isInteger(id) || id < 1 || id > 7) id = 1;
      if (S.style === id) return;
      S.style = id; [S.baseLife, S.stroke] = STREAK_STYLES[id]; S.long = id === 5 || id === 6; S.tex = `streak_0${id}_001.png`;
      loadEffectImage(S.tex); streakReset(S);
    };
    const streakSetPos = (S, x, y) => { S.pos = { x, y }; S.posInit = Number.isFinite(x) && Number.isFinite(y); };
    const streakUpdate = (v, dt) => {
      const S = v.streak; dt = Number.isFinite(dt) ? Math.max(0, dt) : 0;
      if (!S.posInit) return;
      const life = S.baseLife;
      for (const pt of S.pts) if (!S.long || !S.active) pt.state -= dt / life;
      let alive = 0; for (const pt of S.pts) if (pt.state > 0) S.pts[alive++] = pt;
      S.pts.length = alive;
      if (S.suppress) S.breakNext = true;
      if (S.active && !S.suppress) {
        const pos = S.pos, size = S.mini ? 0.6 : 1, last = S.pts[S.pts.length - 1];
        const dist = last ? Math.hypot(pos.x - last.x, pos.y - last.y) : 0;
        const maxStep = Math.max(100, Math.abs(Number(S.speed) || 11.54) * 60 * dt * 3);
        if (!last || S.breakNext || dist > maxStep) { S.pts.push({ x: pos.x, y: pos.y, distance: S.distance, state: 1, size, breakBefore: true }); S.breakNext = false; }
        else if (dist > 0.001) {
          const pieces = Math.min(128, Math.max(1, Math.ceil(dist / 8))), startDistance = S.distance;
          for (let j = 1; j <= pieces; j++) {
            const t = j / pieces;
            S.pts.push({ x: last.x + (pos.x - last.x) * t, y: last.y + (pos.y - last.y) * t, distance: startDistance + dist * t, state: S.long ? 1 : Math.max(0, 1 - dt * (1 - t) / life), size: last.size + (size - last.size) * t });
          }
          S.distance += dist;
        } else { last.state = 1; last.size = size; }
      }
      if (S.long) {
        const reach = W + 240, x = S.pos.x; let drop = 0;
        while (drop + 1 < S.pts.length && S.pts[drop + 1].x < x - reach) drop++;
        if (drop) S.pts.splice(0, drop);
      }
      if (S.pts.length > 2048) S.pts.splice(0, S.pts.length - 2048);
    };
    // images of the icon effects (trail textures, ship fire frames), loaded on demand from ref/assets/icon-effects
    const effectImgs = new Map();
    const loadEffectImage = (name) => {
      if (!name || effectImgs.has(name)) return effectImgs.get(name) || null;
      const rec = { img: null, colored: false }; effectImgs.set(name, rec);
      loadImage(`../ref/assets/icon-effects/${name}`).then((img) => {
        rec.img = img;
        try { const c = document.createElement("canvas"); c.width = c.height = 32; const g = c.getContext("2d", { willReadFrequently: true }); g.drawImage(img, 0, 0, 32, 32); const d = g.getImageData(0, 0, 32, 32).data; for (let i = 0; i < d.length; i += 4) if (d[i + 3] > 64 && Math.max(d[i], d[i + 1], d[i + 2]) - Math.min(d[i], d[i + 1], d[i + 2]) > 30) { rec.colored = true; break; } } catch (e) {}
      }).catch(() => {});
      return rec;
    };
    const hsv2rgb = (h, sat, val) => { const i = Math.floor(h * 6), f = h * 6 - i, p = val * (1 - sat), q = val * (1 - f * sat), t = val * (1 - (1 - f) * sat); const m = ((i % 6) + 6) % 6; const [r, g, b] = [[val, t, p], [q, val, p], [p, val, t], [p, q, val], [t, p, val], [val, p, q]][m]; return (Math.floor(r * 255) << 16) | (Math.floor(g * 255) << 8) | Math.floor(b * 255); };
    const streakDraw = (v, ox, oy) => {
      const S = v.streak; if (S.pts.length < 2 || !S.posInit) return;
      const color = v.secondary(), camX = -ox, pos = S.pos;
      const pointColor = (pt) => S.style === 2 ? hsv2rgb((pt.distance / 180) % 1, 1, 1) : S.style === 7 ? 0xffffff : color;
      const progressOf = (pt) => { const fallback = W * 0.5, vis = Number.isFinite(camX) ? pos.x - camX : fallback; return clamp((pos.x - pt.x) / Math.max(1, vis > 0 ? vis : fallback), 0, 1); };
      const alphaOf = (pt) => { const a = clamp(pt.state, 0, 1); return S.long ? a * (1 - 0.35 * progressOf(pt)) : a * 0.7; };
      const edges = (pts, width, taper, offset) => {
        const normals = [];
        for (let i = 1; i < pts.length; i++) { const dx = pts[i].x - pts[i - 1].x, dy = pts[i].y - pts[i - 1].y, len = Math.hypot(dx, dy) || 1; normals.push({ x: -dy / len, y: dx / len }); }
        return pts.map((pt, i) => {
          const a = normals[Math.max(0, i - 1)], b = normals[Math.min(i, normals.length - 1)];
          let nx = a.x + b.x, ny = a.y + b.y; const n = Math.hypot(nx, ny);
          if (n < 0.01) { nx = b.x; ny = b.y; } else { nx /= n; ny /= n; }
          const miter = 1 / Math.max(0.5, nx * b.x + ny * b.y), size = pt.size ?? 1;
          const shrink = S.style === 5 ? 1 - 0.9 * progressOf(pt) : 1;
          const half = width * size * 0.5 * (taper ? Math.max(0.02, pt.state) : 1) * shrink, center = offset * size * miter;
          return { ax: pt.x + nx * (center + half * miter), ay: pt.y + ny * (center + half * miter), bx: pt.x + nx * (center - half * miter), by: pt.y + ny * (center - half * miter) };
        });
      };
      const hex = (c) => "#" + (c & 0xffffff).toString(16).padStart(6, "0");
      const quad = (e, f, lo, hi) => {
        const L = (a, b, t) => a + (b - a) * t;
        const ax = L(e.ax, f.ax, lo) + ox, ay = L(e.ay, f.ay, lo) + oy, bx = L(e.bx, f.bx, lo) + ox, by = L(e.by, f.by, lo) + oy;
        const cx = L(e.ax, f.ax, hi) + ox, cy = L(e.ay, f.ay, hi) + oy, dx = L(e.bx, f.bx, hi) + ox, dy = L(e.by, f.by, hi) + oy;
        ctx.beginPath(); ctx.moveTo(ax, ay); ctx.lineTo(bx, by); ctx.lineTo(cx, cy); ctx.closePath(); ctx.moveTo(bx, by); ctx.lineTo(dx, dy); ctx.lineTo(cx, cy); ctx.closePath(); ctx.fill();
      };
      const ribbon = (pts, width, fixed, taper, offset) => {
        const ed = edges(pts, width, taper, offset);
        for (let i = 1; i < pts.length; i++) {
          const a = pts[i - 1], b = pts[i], alpha = (alphaOf(a) + alphaOf(b)) * 0.5;
          if (alpha <= 0.001) continue;
          ctx.globalAlpha = Math.min(1, alpha); ctx.fillStyle = hex(fixed ?? pointColor(a));
          if (S.style === 6) {
            const span = b.distance - a.distance; if (span <= 0) continue;
            const period = 40, dash = 24;
            for (let d = a.distance; d < b.distance - 1e-6;) {
              const phase = ((d % period) + period) % period, on = phase < dash - 1e-6;
              const next = Math.min(b.distance, d + Math.max(1e-5, on ? dash - phase : period - phase));
              if (on) quad(ed[i - 1], ed[i], (d - a.distance) / span, (next - a.distance) / span);
              d = next;
            }
          } else quad(ed[i - 1], ed[i], 0, 1);
        }
      };
      const texTri = (img, s0, s1, s2, d0, d1, d2) => {
        const det = (s1.x - s0.x) * (s2.y - s0.y) - (s2.x - s0.x) * (s1.y - s0.y); if (!det) return;
        ctx.save(); ctx.beginPath(); ctx.moveTo(d0.x, d0.y); ctx.lineTo(d1.x, d1.y); ctx.lineTo(d2.x, d2.y); ctx.closePath(); ctx.clip();
        const a = ((d1.x - d0.x) * (s2.y - s0.y) - (d2.x - d0.x) * (s1.y - s0.y)) / det, b = ((d1.y - d0.y) * (s2.y - s0.y) - (d2.y - d0.y) * (s1.y - s0.y)) / det;
        const c = ((d2.x - d0.x) * (s1.x - s0.x) - (d1.x - d0.x) * (s2.x - s0.x)) / det, d = ((d2.y - d0.y) * (s1.x - s0.x) - (d1.y - d0.y) * (s2.x - s0.x)) / det;
        ctx.transform(a, b, c, d, d0.x - a * s0.x - c * s0.y, d0.y - b * s0.x - d * s0.y); ctx.drawImage(img, 0, 0); ctx.restore();
      };
      const textured = (pts) => {
        const rec = S.long ? null : loadEffectImage(S.tex); if (!rec || !rec.img) return false;
        if (S.style === 2 && !rec.colored) return false;          // a grey texture needs the per-point hue tint: the flat ribbon does that
        const img = rec.img, W0 = img.width, H0 = img.height, ed = edges(pts, S.stroke, false, 0), span = Math.max(0.001, pts[pts.length - 1].distance - pts[0].distance);
        const tinted = S.style === 2 || S.style === 7 ? img : (color === 0xffffff ? img : tintedCanvas(img, color));
        for (let i = 1; i < pts.length; i++) {
          const a = pts[i - 1], b = pts[i], alpha = (alphaOf(a) + alphaOf(b)) * 0.5; if (alpha <= 0.001) continue;
          const v0 = (a.distance - pts[0].distance) / span * H0, v1 = (b.distance - pts[0].distance) / span * H0, e = ed[i - 1], f = ed[i];
          ctx.globalAlpha = Math.min(1, alpha);
          const P = (x, y) => ({ x: x + ox, y: y + oy });
          texTri(tinted, { x: W0, y: v0 }, { x: 0, y: v0 }, { x: W0, y: v1 }, P(e.bx, e.by), P(e.ax, e.ay), P(f.bx, f.by));
          texTri(tinted, { x: 0, y: v0 }, { x: 0, y: v1 }, { x: W0, y: v1 }, P(e.ax, e.ay), P(f.ax, f.ay), P(f.bx, f.by));
        }
        return true;
      };
      ctx.save(); ctx.globalCompositeOperation = "lighter";
      let run = [];
      const drawRun = () => {
        if (run.length < 2) return;
        if (textured(run)) return;
        if (S.style === 3) { ribbon(run, S.stroke * 0.28, null, false, -S.stroke * 0.35); ribbon(run, S.stroke * 0.28, null, false, S.stroke * 0.35); }
        else if (S.style === 4) ribbon(run, S.stroke, null, true, 0);
        else if (S.style === 7) { ribbon(run, S.stroke, 0xff4511, true, 0); ribbon(run, S.stroke * 0.45, 0xffdb42, true, 0); }
        else ribbon(run, S.stroke, null, false, 0);
      };
      for (const pt of S.pts) { if (pt.breakBefore) { drawRun(); run = []; } run.push(pt); }
      drawRun();
      ctx.restore();
    };
    const tintCanvases = new Map();
    const tintedCanvas = (img, color) => {
      let per = tintCanvases.get(img); if (!per) tintCanvases.set(img, per = new Map());
      let c = per.get(color); if (c) return c;
      c = document.createElement("canvas"); c.width = img.width; c.height = img.height; const g = c.getContext("2d");
      g.drawImage(img, 0, 0); g.globalCompositeOperation = "multiply"; g.fillStyle = "#" + (color & 0xffffff).toString(16).padStart(6, "0"); g.fillRect(0, 0, c.width, c.height);
      g.globalCompositeOperation = "destination-in"; g.drawImage(img, 0, 0);
      per.set(color, c); return c;
    };
    // WaveTrail (note: player.js hands it seconds although it expects milliseconds, so points only leave by scrolling off)
    const intersect = (xs, ys, i, p1x, p1y, p2x, p2y, p3x, p3y, p4x, p4y) => {
      const d1x = p2x - p1x, d1y = p2y - p1y, d2x = p4x - p3x, d2y = p4y - p3y, den = d1x * d2y - d1y * d2x;
      if (Math.abs(den) < 1e-6) { xs[i] = p2x; ys[i] = p2y; return; }
      const t = ((p3x - p1x) * d2y - (p3y - p1y) * d2x) / den, tc = Math.max(-3, Math.min(3, t));
      xs[i] = p1x + d1x * tc; ys[i] = p1y + d1y * tc;
    };
    // a triangle that always winds the same way, so overlapping ones in one path add up instead of cancelling
    const tri = (c, ax, ay, bx, by, cx, cy) => { c.moveTo(ax, ay); if ((bx - ax) * (cy - ay) - (by - ay) * (cx - ax) >= 0) { c.lineTo(bx, by); c.lineTo(cx, cy); } else { c.lineTo(cx, cy); c.lineTo(bx, by); } c.closePath(); };
    const edgeBuf = { n: 0 };
    const buildEdges = (pts, halfW) => {
      const n = pts.length; if (edgeBuf.n < n) { const m = Math.max(n, edgeBuf.n * 2, 256); for (const k of ["ux", "uy", "lx", "ly", "nx", "ny"]) edgeBuf[k] = new Float64Array(m); edgeBuf.n = m; }
      const { ux, uy, lx, ly, nx, ny } = edgeBuf; // reused every call: this runs three times a frame while the wave trail is on screen
      for (let i = 0; i < n - 1; i++) { const dx = pts[i + 1].x - pts[i].x, dy = pts[i + 1].y - pts[i].y, len = Math.sqrt(dx * dx + dy * dy) || 1; nx[i] = -dy / len; ny[i] = dx / len; }
      for (let i = 0; i < n; i++) {
        const p = pts[i]; let a, b;
        if (i === 0) { a = nx[0]; b = ny[0]; }
        else if (i === n - 1) { a = nx[n - 2]; b = ny[n - 2]; }
        else {
          const n1x = nx[i - 1], n1y = ny[i - 1], n2x = nx[i], n2y = ny[i], pr = pts[i - 1], nt = pts[i + 1];
          intersect(ux, uy, i, pr.x + n1x * halfW, pr.y + n1y * halfW, p.x + n1x * halfW, p.y + n1y * halfW, p.x + n2x * halfW, p.y + n2y * halfW, nt.x + n2x * halfW, nt.y + n2y * halfW);
          intersect(lx, ly, i, pr.x - n1x * halfW, pr.y - n1y * halfW, p.x - n1x * halfW, p.y - n1y * halfW, p.x - n2x * halfW, p.y - n2y * halfW, nt.x - n2x * halfW, nt.y - n2y * halfW);
          continue;
        }
        ux[i] = p.x + a * halfW; uy[i] = p.y + b * halfW; lx[i] = p.x - a * halfW; ly[i] = p.y - b * halfW;
      }
      return { ux, uy, lx, ly };
    };
    const ribbon = (pts, halfW, color, baseAlpha, ox, oy, additive) => {
      const n = pts.length; if (n < 2) return;
      const { ux, uy, lx, ly } = buildEdges(pts, halfW);
      ctx.save(); ctx.globalCompositeOperation = additive ? "lighter" : "source-over"; ctx.fillStyle = "#" + (color & 0xffffff).toString(16).padStart(6, "0");
      // the trail fades from its tail: segments are ordered by age, so runs of (nearly) equal alpha share one path and one fill
      const STEPS = 32; let cur = -1, open = false;
      const flush = () => { if (open) { ctx.fill(); open = false; } };
      const left = -halfW * 2 - 20; // segments that ended left of the screen are not drawn (the trail keeps 200 px behind the camera)
      for (let i = 0; i < n - 1; i++) {
        if (pts[i + 1].x + ox < left) continue;
        const a = Math.max(0, 1 - (pts[i].age + pts[i + 1].age) * 0.5);
        if (a * baseAlpha <= 0.01) continue;
        const q = Math.max(1, Math.round(a * STEPS));
        if (q !== cur) { flush(); cur = q; ctx.globalAlpha = q / STEPS * baseAlpha; ctx.beginPath(); open = true; }
        tri(ctx, ux[i] + ox, uy[i] + oy, ux[i + 1] + ox, uy[i + 1] + oy, lx[i] + ox, ly[i] + oy);
        tri(ctx, ux[i + 1] + ox, uy[i + 1] + oy, lx[i + 1] + ox, ly[i + 1] + oy, lx[i] + ox, ly[i] + oy);
      }
      flush();
      ctx.restore();
    };
    window.__waveBench = (n, reps, draw) => { // test hook: a zig-zag trail of n points, drawn reps times; returns ms per draw of the three ribbons
      const pts = []; for (let i = 0; i < n; i++) pts.push({ x: 100 + i * 1.5, y: 300 + (Math.floor(i / 40) % 2 ? 1 : -1) * ((i % 40) * 1.5), age: 1 - i / n });
      const t0 = performance.now(); for (let r = 0; r < reps; r++) { ribbon(pts, 14, 0x00ff66, 0.22, 0, 0, true); ribbon(pts, 7, 0x00ff66, 0.95, 0, 0, false); ribbon(pts, 2, 0xffffff, 0.5, 0, 0, false); }
      return (performance.now() - t0) / reps;
    };
    const waveUpdate = (v, deltaArg, camX, mini) => {
      const Wt = v.wave; if (!Wt.pos) return;
      const decay = (deltaArg / 1000) / 0.6; let k = 0;
      for (let i = 0; i < Wt.pts.length; i++) { const pt = Wt.pts[i]; pt.age += decay; if (pt.age < 1) Wt.pts[k++] = pt; else Wt.pool.push(pt); }
      Wt.pts.length = k;
      const cutoff = camX - 200; let drop = 0;
      while (Wt.pts.length - drop > 2 && Wt.pts[drop + 1].x < cutoff) drop++;
      if (drop) { for (let i = 0; i < drop; i++) Wt.pool.push(Wt.pts[i]); Wt.pts.splice(0, drop); }
      if (Wt.active) {
        const n = Wt.pts.length; let add = true;
        if (n > 0) { const last = Wt.pts[n - 1], dx = Wt.pos.x - last.x, dy = Wt.pos.y - last.y; if (dx * dx + dy * dy < 2.25) add = false; }
        if (add) { const pt = Wt.pool.pop() || { x: 0, y: 0, age: 0 }; pt.x = Wt.pos.x; pt.y = Wt.pos.y; pt.age = 0; Wt.pts.push(pt); }
      }
      Wt.mini = mini ? 0.6 : 1;
    };
    const waveDraw = (v, ox, oy) => {
      const Wt = v.wave; if (Wt.pts.length < 2) return;
      const hw = 7 * (Wt.mini || 1), ghw = 14 * (Wt.mini || 1), col = v.primary();
      if (cfg.solidWave) ribbon(Wt.pts, hw, col, 1.0, ox, oy, false);
      else { ribbon(Wt.pts, ghw, col, 0.22, ox, oy, true); ribbon(Wt.pts, hw, col, 0.95, ox, oy, false); ribbon(Wt.pts, Math.round(hw * 0.32), 0xffffff, 0.5, ox, oy, false); }
    };

    // ---- drawing ----
    const drawItem = (it, ox, oy) => {
      const d = it.fr;
      drawFrame({ img: d.img, d: d.d }, it.x + ox, it.y + oy, it.rot, it.sx, it.sy, it.alpha, it.tint, 0, 0);
    };
    const drawSquare = (name, x, y, scale, alpha, tint, additive) => {
      const fr = getFrame(name); if (!fr) return;
      drawFrame({ img: fr.img, d: fr.d }, x, y, 0, scale, scale, alpha, tint, additive ? 1 : 0, 0);
    };

    const players = [new Vis(0), new Vis(1)];
    const parse = (v, a) => ({
      y: a[0], x: a[1], rot: a[2], dead: !!a[3], mini: !!a[4], ship: !!a[5], wave: !!a[6], ball: !!a[7], ufo: !!a[8], spider: !!a[9], robot: !!a[10],
      gf: !!a[11], mir: !!a[12], onGround: !!a[13], onCeiling: !!a[14], dashing: !!a[15], dashY: a[16], yVel: a[17], jumping: !!a[18], upDown: !!a[19],
      slope: a[20], speed: a[21], dual: !!a[25], swing: !!a[26],
    });

    // the death effects (`PlayerExplosion_NN` atlases, style 2..20): a frame animation at 20 fps where the icon died
    const deathAtlases = new Map();
    const deathAtlasFor = (id) => {
      if (!(id >= 2 && id <= 20)) return null;
      if (deathAtlases.has(id)) return deathAtlases.get(id);
      const rec = { ready: false, failed: false }; deathAtlases.set(id, rec);
      (async () => {
        const base = `../ref/assets/icon-effects/PlayerExplosion_${String(id - 1).padStart(2, "0")}-hd`;
        try {
          const [xml, img] = await Promise.all([loadText(base + ".plist"), loadImage(base + ".png")]);
          const frames = plistFrames(plistNode(new DOMParser().parseFromString(xml, "text/xml").documentElement.firstElementChild)).filter((f) => /\.png$/i.test(f.name));
          if (!frames.length) throw new Error("no frames");
          const size = packFrames(frames, img.width), cv = document.createElement("canvas"); cv.width = size.w; cv.height = size.h;
          const g = cv.getContext("2d");
          for (const f of frames) {
            if (f.rotated) { g.save(); g.translate(f.dx, f.dy + f.h); g.rotate(-Math.PI / 2); g.drawImage(img, f.x, f.y, f.h, f.w, 0, 0, f.h, f.w); g.restore(); }
            else g.drawImage(img, f.x, f.y, f.w, f.h, f.dx, f.dy, f.w, f.h);
          }
          cv.src = "death:" + id;
          const fr = {}; for (const f of frames) fr[f.name] = { frame: { x: f.dx, y: f.dy, w: f.w, h: f.h }, rotated: false, trimmed: !(f.w === f.srcW && f.h === f.srcH && !f.sx && !f.sy), spriteSourceSize: { x: f.sx, y: f.sy, w: f.w, h: f.h }, sourceSize: { w: f.srcW, h: f.srcH } };
          rec.img = cv; rec.frames = fr; rec.names = frames.map((f) => f.name).sort((a, b) => a.localeCompare(b, undefined, { numeric: true })); rec.res = 2; rec.ready = true;
        } catch (e) { rec.failed = true; }
      })();
      return rec;
    };
    const killEffects = (v, p, ox, oy, camY) => {
      // killPlayer(): burst of 100 squares, a flash circle and the player split into flying pieces
      const cxw = p.x, cyw = T - p.y;
      const sx = cxw + ox, sy = cyw + oy;
      const col = v.primary();
      const atlas = deathAtlasFor(cfg.death);
      if (atlas && atlas.ready) v.deathFx = { atlas, x: sx, y: sy, size: p.mini ? 0.6 : 1, t: 0 };
      const burst = []; for (let i = 0; i < 100; i++) { const a = rnd(0, 360) * Math.PI / 180, sp = rnd(200, 800); burst.push({ x: sx + rnd(-20, 20), y: sy + rnd(-20, 20), vx: Math.cos(a) * sp, vy: Math.sin(a) * sp, age: 0, life: rnd(50, 800) }); }
      if (!v.deathFx) { v.burst = { parts: burst, tint: col, paused: false }; v.flash = { t: 0, x: sx, y: sy }; }
      if (!cfg.explode) return;
      // pieces from the last drawn layers
      const size = Math.round(0.9 * 40 * 2);
      const cv = document.createElement("canvas"); cv.width = cv.height = size;
      const g = cv.getContext("2d");
      const order = v.prevItems.slice().sort((a, b) => ((a.kind === "glow" || a.kind === "overlay") ? 0 : 1) - ((b.kind === "glow" || b.kind === "overlay") ? 0 : 1) || a.order - b.order);
      const savedCtx = env.ctx;
      for (const it of order) {
        g.save(); g.translate(size / 2 + (it.x - cxw), size / 2 + (it.y - cyw)); if (it.rot) g.rotate(it.rot); g.scale(it.sx, it.sy);
        const d = it.fr.d, f = d.frame, src = d.sourceSize || { w: f.w, h: f.h }, ss = d.spriteSourceSize || { x: 0, y: 0 };
        const img = it.tint >= 0 && it.tint !== 0xffffff ? env.tintedCanvas({ img: it.fr.img, d }, it.tint) : null;
        if (img) g.drawImage(img, -src.w / 2 + ss.x, -src.h / 2 + ss.y); else g.drawImage(it.fr.img, f.x, f.y, f.w, f.h, -src.w / 2 + ss.x, -src.h / 2 + ss.y, f.w, f.h);
        g.restore();
      }
      let nx = 2 + Math.round(Math.random() * 2), ny = 2 + Math.round(Math.random() * 2);
      const r = Math.random(); if (r > 0.95) nx = 1; else if (r > 0.9) ny = 1;
      const speedBase = 7.4779225920000005, v1 = speedBase * 0.5, v2 = speedBase, jit = 0.45;
      const cw = size / nx, ch = size / ny, ws = [], hs = [], xs = [0], ys = [0]; let ax = 0, ay = 0;
      for (let i = 0; i < nx - 1; i++) { const q = Math.round(cw * (0.55 + Math.random() * jit * 2)); ws.push(q); ax += q; xs.push(ax); } ws.push(size - ax);
      for (let i = 0; i < ny - 1; i++) { const q = Math.round(ch * (0.55 + Math.random() * jit * 2)); hs.push(q); ay += q; ys.push(ay); } hs.push(size - ay);
      const pieces = []; let count = 0;
      for (let i = 0; i < nx; i++) for (let j = 0; j < ny; j++) {
        const pw = ws[i], ph = hs[j]; if (pw <= 0 || ph <= 0) continue;
        count++;
        const piece = { cv, sx: xs[i], sy: ys[j], w: pw, h: ph, x: xs[i] + pw / 2 - size / 2, y: -(ys[j] + ph / 2 - size / 2), angle: 0, alpha: 1,
          xVel: (v1 + (Math.random() * 2 - 1) * v2) * (p.mir ? -1 : 1), yVel: -(12 + (Math.random() * 2 - 1) * 6), timer: 1.4, fade: 0.5, rotDelta: (Math.random() * 2 - 1) * 360 / 60, half: Math.min(pw, ph) / 2, trail: null };
        if (count % 2 === 0) piece.trail = new Emitter({ speed: 0, angle: [0, 0], life: rnd(200, 400), scale: [0.5, 0], alpha: [1, 0], freq: 25, tint: col, on: true });
        pieces.push(piece);
      }
      v.pieces = { list: pieces, cx: sx, cy: sy, groundY: T + camY - sy, tint: col };
    };

    const updateDeath = (v, dtMs) => {
      if (v.burst && !v.burst.paused) {
        const s = dtMs / 1000;
        for (let i = v.burst.parts.length - 1; i >= 0; i--) { const q = v.burst.parts[i]; q.age += dtMs; if (q.age >= q.life) { v.burst.parts.splice(i, 1); continue; } q.x += q.vx * s; q.y += q.vy * s; }
        if (!v.burst.parts.length) v.burst = null;
      }
      if (v.flash) { v.flash.t += dtMs / 300; if (v.flash.t >= 1) v.flash = null; }
      if (v.deathFx) { v.deathFx.t += dtMs; if (v.deathFx.t >= v.deathFx.atlas.names.length * 50) v.deathFx = null; }
      const P = v.pieces; if (!P) return;
      const sec = dtMs / 1000, k = Math.min(sec * 60 * 0.9, 2), grav = k * 0.5 * 2;
      for (let i = 0; i < P.list.length;) {
        const q = P.list[i]; q.timer -= sec;
        if (q.timer > 0) {
          q.yVel += grav; q.xVel *= 0.98 + (1 - k) * 0.02;
          let nxp = q.x + q.xVel * k, nyp = q.y + q.yVel * k;
          const lim = P.groundY - q.half;
          if (nyp > lim && q.yVel > 0) { nyp = lim; q.yVel *= -0.8; if (Math.abs(q.yVel) < 3) q.yVel = -3; }
          q.x = nxp; q.y = nyp; q.angle += q.rotDelta * k;
          if (q.timer < q.fade) q.alpha = q.timer / q.fade;
          if (q.trail) {
            q.trail.alphaMul = q.alpha;
            q.trail.update(dtMs);
          }
          i++;
        } else P.list.splice(i, 1);
      }
      if (!P.list.length) v.pieces = null;
    };
    const drawDeath = (v) => {
      if (v.deathFx) {
        const D = v.deathFx, A = D.atlas, name = A.names[Math.min(A.names.length - 1, Math.floor(D.t / 50))], k = D.size * (40 / 30) / A.res * 1.5;
        drawFrame({ img: A.img, d: A.frames[name] }, D.x, D.y, 0, k, k, 1, -1, 0, 0);
      }
      if (v.burst) for (const q of v.burst.parts) { const t = q.age / q.life; drawSquare("square.png", q.x, q.y, lerp(18 / 32, 0, t), 1 - t, v.burst.tint, true); }
      if (v.flash) {
        const t = v.flash.t, radius = 14 + t * 76, alpha = Math.max(0, 1 - t * t);
        ctx.save(); ctx.globalCompositeOperation = "lighter"; ctx.globalAlpha = alpha; ctx.fillStyle = "#" + (v.primary() & 0xffffff).toString(16).padStart(6, "0");
        ctx.beginPath(); ctx.arc(v.flash.x, v.flash.y, radius, 0, TAU); ctx.fill(); ctx.restore();
      }
      const P = v.pieces; if (!P) return;
      for (const q of P.list) {
        if (q.trail) for (const pt of q.trail.parts) { const t = pt.age / pt.life; drawSquare("square.png", P.cx + pt.x + (0), P.cy + pt.y, lerp(0.5, 0, t), (1 - t) * q.alpha, P.tint, true); }
        ctx.save(); ctx.translate(P.cx + q.x, P.cy + q.y); ctx.rotate(q.angle * Math.PI / 180); ctx.globalAlpha = clamp(q.alpha, 0, 1);
        ctx.drawImage(q.cv, q.sx, q.sy, q.w, q.h, -q.w / 2, -q.h / 2, q.w, q.h); ctx.restore();
        if (q.trail && Math.random() < 0) {}
      }
    };

    // trail emitters of a piece follow it: positions are refreshed before drawing
    const followPieces = (v) => { const P = v.pieces; if (!P) return; for (const q of P.list) if (q.trail) { q.trail.px = q.x + (Math.random() * 2 - 1) * 6; q.trail.py = q.y + (Math.random() * 2 - 1) * 6; } };

    return {
      cfg, loadCfg, ensureIcons, loadIconAtlas, getFrame, failed, icons, iconBaseOf, equippedBases,
      reloadCfg() { loadCfg(); return ensureIcons(equippedBases()); },
      // imported cube icon: square-cropped and halved down to 60 px like config.js `_customCubeToCanvas`
      setCustomCube(src) {
        return new Promise((res, rej) => {
          if (!src) { customCube = null; generation++; return res(null); }
          const img = new Image();
          img.onload = () => {
            const w0 = img.naturalWidth || img.width, h0 = img.naturalHeight || img.height, side = Math.min(w0, h0);
            let cur = document.createElement("canvas"); cur.width = cur.height = side;
            let g = cur.getContext("2d"); g.imageSmoothingEnabled = true; g.imageSmoothingQuality = "high";
            g.drawImage(img, (w0 - side) / 2, (h0 - side) / 2, side, side, 0, 0, side, side);
            while (cur.width / 2 >= 60) { const nx = document.createElement("canvas"); nx.width = nx.height = Math.floor(cur.width / 2); const ng = nx.getContext("2d"); ng.imageSmoothingEnabled = true; ng.imageSmoothingQuality = "high"; ng.drawImage(cur, 0, 0, nx.width, nx.height); cur = nx; }
            const out = document.createElement("canvas"); out.width = out.height = 60;
            const og = out.getContext("2d"); og.imageSmoothingEnabled = true; og.imageSmoothingQuality = "high"; og.drawImage(cur, 0, 0, 60, 60);
            out.src = "custom_cube:" + Date.now();
            customCube = out; generation++; res(img);
          };
          img.onerror = () => rej(new Error("Failed to load image"));
          img.src = src;
        });
      },
      hasCustomCube() { return !!customCube; },
      async init() {
        await Promise.all(["player_ball_00", "player_dart_00", "Wavesheet"].map(loadSheet));
        const [rd, sd] = await Promise.all([loadJson("../ref/assets/sheets/Robot_AnimDesc.json").catch(() => null), loadJson("../ref/assets/sheets/Spider_AnimDesc.json").catch(() => null)]);
        const mk = (desc, kind, prefix, names, fallback) => {
          if (!desc || !desc.animationContainer) return null;
          const entries = entriesOf(desc, fallback), tags = new Set(), textures = {};
          for (const e of entries) { const t = parseInt(e.tag || "0", 10) || 0; tags.add(t); textures[t] = e.texture; }
          return { desc, groups: groupsOf(desc, prefix, names), tags, textures };
        };
        anim.robot = mk(rd, "robot", "Robot_", { run: "run", run2: "run2", run3: "run3", skip: "skip", jumpStart: "jump_start", jumpLoop: "jump_loop", fallStart: "fall_start", fallLoop: "fall_loop", idle: "idle", idle1: "idle01", idle2: "idle02" },
          [0, 1, 2, 3, 4, 5, 6].map((t) => ({ tag: String(t), texture: ["robot_01_03_001.png", "robot_01_02_001.png", "robot_01_04_001.png", "robot_01_01_001.png", "robot_01_03_001.png", "robot_01_02_001.png", "robot_01_04_001.png"][t] })));
        anim.spider = mk(sd, "spider", "Spider_", { run: "run", run2: "run2", walk: "walk", fall: "fall_loop", idle: "idle01", idle2: "idle02" },
          [0, 1, 2, 3, 4, 5].map((t) => ({ tag: String(t), texture: ["spider_01_02_001.png", "spider_01_02_001.png", "spider_01_04_001.png", "spider_01_01_001.png", "spider_01_03_001.png", "spider_01_02_001.png"][t] })));
        await ensureIcons(equippedBases());
      },
      players,
      // Reads the wasm state for both players, runs the per-frame logic (dt in seconds) and rebuilds the layers
      frame(dt, o) {
        const { w } = env;
        this.mirrorP = o.mirrorP || 0;
        loadCfg2();
        const evs = o.events || [];
        for (const v of players) {
          const base = new Float64Array(w.memory.buffer, w.wd_pv_state(v.index), 27);
          v.prev.set(v.s); v.s.set(base);
        }
        const dual = players[0].s[25] !== 0;
        for (const v of players) {
          const p = parse(v, v.s), was = v.prev[3] !== 0;
          const active = v.index === 0 || dual;
          v.rotation = p.rot;
          v.items = [];
          const camX = o.camX, camY = o.camY, ox = o.ox, oy = o.oy;
          if (cfg.death >= 2) deathAtlasFor(cfg.death);
          v.lastCamY = camY;
          // events (shared log, applied to the primary player except the explicit second-player tags)
          // death / respawn edges
          if (!active) { v.wasActive = false; v.em.ground.stop(); v.em.fly.stop(); v.em.fly2.stop(); v.em.drag.stop(); streakStop(v.streak); streakReset(v.streak); v.wave.active = false; v.wave.pts.length = 0; v.pieces = v.burst = v.flash = v.deathFx = null; v.prevItems = []; continue; }
          if (p.dead && !was && !o.menu && !v.ended) { killEffects(v, p, ox, oy, camY); streakStop(v.streak); streakReset(v.streak); v.wave.active = false; v.wave.pts.length = 0; for (const e of Object.values(v.em)) { if (e !== v.em.land1 && e !== v.em.land2) { e.stop(); e.clear(); } } }
          if (!p.dead && was) { v.pieces = v.burst = v.flash = v.deathFx = null; v.ended = null; v.circles.length = 0; v.dashFx = null; }
          if (o.restart) { v.pieces = v.burst = v.flash = v.deathFx = null; v.ended = null; streakReset(v.streak); v.boost = false; v.wave.pts.length = 0; v.circles.length = 0; v.dashFx = null; v.fx.length = 0; }
          // level end fly-out
          if (!o.won && v.ended) v.ended = null;
          if (o.won && !v.ended && !p.dead && v.prevItems.length) v.startEnd(p, o);
          if (v.ended) { v.stepEnd(dtMsOf(dt), o); }
          else if (!p.dead && !o.menu && !v.hidden) {
            v.layout(p, dt);
            v.prevItems = v.items.map((i) => ({ ...i }));
          }
          // dash glow sprite
          const dashOn = p.dashing && !p.dead && !v.ended && !o.menu;
          if (dashOn) {
            v.dashTimer += dt * 1000;
            if (v.dashTimer >= 16.67) { v.dashTimer = 0; v.dashFrame = (v.dashFrame % 12) + 1; }
            if (!v.dashFrame) v.dashFrame = 1;
            const fr = getFrame(`playerDash2_${String(v.dashFrame).padStart(3, "0")}.png`);
            if (fr) { const m = p.mini ? 0.6 : 1; v.items.push({ group: "dash", kind: "fx", fr, depth: 7, order: v.items.length, x: p.x, y: T - p.y, rot: Math.atan2(-(p.dashY || 0), p.speed * 0.9), sx: m, sy: p.gf ? -m : m, tint: 0xffffff, alpha: 1, additive: true }); }
          } else { v.dashFrame = 0; v.dashTimer = 0; }
          // particles and trails (`_updateParticles`)
          const live = !cfg.ldm && !o.menu && !p.dead && !v.ended && !v.hidden;
          const E = v.em;
          if (!live) { for (const e of [E.ground, E.fly, E.fly2, E.drag]) e.stop(); if (!v.ended) { streakStop(v.streak); v.wave.active = false; } if (cfg.ldm) { v.streak.pts.length = 0; v.wave.pts.length = 0; } }
          else {
            const wx = p.x, wy = T - p.y, cs = Math.cos(p.rot), sn = Math.sin(p.rot);
            E.ground.px = wx - 20; E.ground.py = wy + (p.gf ? (-26 + (p.ufo ? -5 : 0)) : (26 + (p.ufo ? 5 : 0)));
            const onG = p.onGround && !p.ship && !p.wave && !p.swing;
            onG ? E.ground.start() : E.ground.stop();
            const miniFlip = p.ship && p.mini && p.gf, dxo = p.wave || p.ufo ? 0 : (miniFlip ? -24 * 0.6 : -24), dyo = miniFlip ? 10 + (-18 - 10) * 0.6 : (p.wave ? 4 : (p.ufo ? 5 : 18)) * (p.gf ? -1 : 1);
            const fx = wx + dxo * cs - dyo * sn, fy = wy + dxo * sn + dyo * cs, jit = (Math.random() * 2 - 1) * 4;
            E.fly.px = fx; E.fly.py = fy + jit; E.fly2.px = fx; E.fly2.py = fy + jit;
            v.wave.pos = { x: wx, y: wy };
            if (p.swing) {
              const size = p.mini ? 0.6 : 1, mirror = p.mir ? -1 : 1, rearX = -23 * size * mirror, thrustY = (p.gf ? 23 : -23) * size;
              E.fly.px = wx + rearX * cs; E.fly.py = wy + rearX * sn; E.fly2.px = wx - thrustY * sn; E.fly2.py = wy + thrustY * cs;
              E.fly.angle = mirror > 0 ? [155, 205] : [-25, 25];
              const ang = (p.gf ? 90 : 270) + p.rot * 180 / Math.PI; E.fly2.angle = [ang - 30, ang + 30];
              v.swingAngles = true;
            } else if (v.swingAngles) { v.swingAngles = false; E.fly.angle = [225, 315]; E.fly2.angle = [180, 360]; }
            // the trail (`_updateParticles`): where it hangs on the icon, and when it runs
            { const S = v.streak, size = p.mini ? 0.6 : 1, flight = p.ship || p.ufo || p.swing, airborne = !p.onGround && !p.onCeiling;
              streakSetStyle(S, cfg.trail); S.mini = p.mini; S.speed = p.speed;
              const mode = (p.ship ? 1 : 0) | (p.ball ? 2 : 0) | (p.wave ? 4 : 0) | (p.ufo ? 8 : 0) | (p.spider ? 16 : 0) | (p.robot ? 32 : 0) | (p.swing ? 64 : 0);
              if (mode !== v.lastMode) { if (v.lastMode >= 0 && !S.long) streakReset(S); v.lastMode = mode; }
              if (p.ship) { const gravity = p.gf ? -1 : 1, mx = -24 * size, my = 10 + 8 * size * gravity; streakSetPos(S, wx + mx * cs - my * sn, wy + mx * sn + my * cs - (p.gf ? 20 * size : 0)); }
              else if (p.swing) { const mx = -23 * size; streakSetPos(S, wx + mx * cs, wy + mx * sn); }
              else if (p.ufo) { const off = 15 * size * (p.gf ? -1 : 1); streakSetPos(S, wx - off * sn, wy + off * cs); }
              else if (!flight) streakSetPos(S, wx, wy);
              S.suppress = p.wave;
              const longTrail = cfg.trail === 5 || cfg.trail === 6;
              if (!v.hidden && !o.menu && (longTrail || (!p.wave && (flight || (airborne && v.boost) || p.dashing)))) streakStart(S); else streakStop(S); }
            if (p.wave && !v.wave.active) { v.wave.pts.length = 0; v.wave.active = true; } else if (!p.wave && v.wave.active) { v.wave.active = false; v.wave.pts.length = 0; }
            streakUpdate(v, dt);
            waveUpdate(v, dt, camX, p.mini);
            const flyOn = p.ship || p.ufo || p.swing; flyOn ? E.fly.start() : E.fly.stop();
            const fly2 = (p.ship && p.upDown) || (p.ufo && p.jumping) || p.swing; fly2 ? E.fly2.start() : E.fly2.stop();
            E.drag.px = wx; E.drag.py = p.gf ? T - p.y + camY + 10 : T - p.y + camY + 30;
            E.drag.angle = p.mir ? [245, 335] : [205, 295]; E.drag.gx = p.mir ? 700 : -700;
            const drag = p.ship && p.onGround && (p.gf ? p.onCeiling : !p.onCeiling);
            drag ? E.drag.start() : E.drag.stop();
          }
          E.ground.tint = v.primary(); E.land1.tint = v.primary(); E.land2.tint = v.primary();
          if (!o.paused) for (const e of Object.values(E)) e.update(dtMsOf(dt));
          if (!o.paused) { updateDeath(v, dtMsOf(dt)); followPieces(v); }
          // timed fx (orb rings, gravity lines, teleport circles)
          if (!o.paused) { for (let i = v.fx.length - 1; i >= 0; i--) { const f = v.fx[i]; f.t += dtMsOf(dt); if (f.t >= f.dur) v.fx.splice(i, 1); } }
        }
        // shared events from the simulation (one drain per frame)
        for (const ev of evs) {
          const v = players[0], p = parse(v, v.s);
          const [kind, args] = ev.split(":"); const a = (args || "").split(",").map(Number);
          if (kind === "boost") { players[a[0] ? 1 : 0].boost = !!a[1]; }
          else if (kind === "land" && !cfg.ldm && !o.menu) {
            v.landIdx = !v.landIdx; const e = v.landIdx ? v.em.land1 : v.em.land2;
            e.emit(10, p.x, a[0] ? T - p.y - 30 : T - p.y + 30);
          } else if (kind === "grav" && !cfg.ldm && !o.menu) {
            v.fx.push({ type: "grav", up: !!a[0], t: 0, dur: 400, lines: Array.from({ length: 14 }, (_, i) => ({ x: 32 + i * ((o.screenW - 60) / 14) + Math.random() * ((o.screenW - 60) / 14), yOff: (Math.random() * 1 - 1) * 10, sx: Math.random() * 14 + 2, sy: Math.random() * 1 + 2, alpha: (60 + Math.random() * 60) / 255 })) });
          } else if (kind === "fx" && !cfg.ldm && !o.menu) {
            const [x, y, ty, id, flags] = a, isPad = ty === 5, isCoin = ty === 7;
            let color;
            if (flags & 2) color = 0xd0d0d0; else if (isCoin) color = 0xffff00;
            else color = ({ 35: 0xffff00, 36: 0xffff00, 67: 0x00ffff, 84: 0x00ffff, 1022: 0x00ff00, 1330: 0x000000, 1332: 0xff0000, 1333: 0xff0000, 140: 0xff00ff, 141: 0xff00ff, 1704: 0x00ff00, 1751: 0xff00ff })[id];
            if (color === undefined) color = v.primary();
            v.fx.push({ type: "ring", x, y: T - y, color, max: isPad ? 82 : 68, t: 0, dur: 300 });
          } else if (kind === "tp") {
            v.streak.breakNext = true;
            if (cfg.ldm || o.menu) continue;
            if (p.spider) { v.spiderFlash = 0.5; v.spiderTimer += 0.12; }
            const wx = p.x, oldY = T - a[0], newY = T - a[1], tint = mixColors(0xffffff, v.primary(), 0.42);
            v.fx.push({ type: "tpold", x: wx, y: oldY, color: tint, t: 0, dur: 300 });
            v.fx.push({ type: "tpnew", x: wx, y: newY, color: tint, mir: p.mir, t: 0, dur: 400 });
            v.fx.push({ type: "tpdash", x: wx, y: (oldY + newY) * 0.5, up: newY < oldY, len: Math.abs(newY - oldY), color: tint, t: 0, dur: 500 });
          }
        }
      },
      // world-space layer: trails, particles and the player icons
      drawWorld(ox, oy, o) {
        for (const v of players) {
          if (v.index === 1 && players[0].s[25] === 0) continue;
          const E = v.em, dsq = (n, x, y, s, a, t, add) => drawSquare(n, x, y, s, a, t, add);
          { const t0 = performance.now(); streakDraw(v, ox, oy); const P = window.__prof; if (P) { P.wvT = (P.wvT || 0) * 0.95 + (performance.now() - t0) * 0.05; P.wvN = v.wave.pts.length; } }
          E.ground.draw(dsq, ox, oy); E.fly.draw(dsq, ox, oy); E.fly2.draw(dsq, ox, oy); E.drag.draw(dsq, ox, oy);
          this.drawFx(v, ox, oy, false);
          const items = v.items.slice().sort((a, b) => a.depth - b.depth || a.order - b.order);
          for (const it of items) {
            const px = it.screen ? 0 : ox, py = it.screen ? 0 : oy;
            // `_getMirrorXOffset`: while the mirror portal's effect runs the player icon is drawn at x + (width - 2x) * progress
            const sxp = it.x + px, mxp = this.mirrorP && !it.screen ? sxp + (W - 2 * sxp) * this.mirrorP : sxp;
            if (it.additive) drawFrame({ img: it.fr.img, d: it.fr.d }, mxp, it.y + py, it.rot, it.sx, it.sy, it.alpha, it.tint, 1, 0);
            else drawFrame({ img: it.fr.img, d: it.fr.d }, mxp, it.y + py, it.rot, it.sx, it.sy, it.alpha, it.tint, 0, 0);
          }
        }
      },
      // after the top layer: landing dust, effect circles
      drawTop(ox, oy, o) {
        for (const v of players) {
          if (v.index === 1 && players[0].s[25] === 0) { /* the second player is not shown */ } else { const t0 = performance.now(); waveDraw(v, ox, oy); const P = window.__prof; if (P) { P.wvT = (P.wvT || 0) * 0.95 + (performance.now() - t0) * 0.05; P.wvN = v.wave.pts.length; } } // the wave trail lives in the level's top container now
          const dsq = (n, x, y, s, a, t, add) => drawSquare(n, x, y, s, a, t, add);
          v.em.land1.draw(dsq, ox, oy); v.em.land2.draw(dsq, ox, oy);
          this.drawFx(v, ox, oy, true, o);
        }
      },
      // screen-space death effects (drawn last)
      drawScreen() { for (const v of players) drawDeath(v); },
      drawFx(v, ox, oy, top, o) {
        for (const f of v.fx) {
          const t = clamp(f.t / f.dur, 0, 1);
          if (f.type === "ring" && top) {
            const radius = 10 + (1 - (1 - t) * (1 - t)) * (f.max - 10), alpha = Math.max(0, 1 - (1 - (1 - t) * (1 - t)));
            ctx.save(); ctx.globalCompositeOperation = "lighter"; ctx.globalAlpha = alpha; ctx.fillStyle = "#" + (f.color & 0xffffff).toString(16).padStart(6, "0");
            ctx.beginPath(); ctx.arc(f.x + ox, f.y + oy, radius, 0, TAU); ctx.fill(); ctx.restore();
          } else if (f.type === "tpold" && !top) {
            const e = 1 - Math.pow(1 - t, 3), s = lerp(0.85, 0.03, e);
            ctx.save(); ctx.globalCompositeOperation = "lighter"; ctx.globalAlpha = 0.45 * (1 - e); ctx.fillStyle = "#" + (f.color & 0xffffff).toString(16).padStart(6, "0");
            ctx.beginPath(); ctx.arc(f.x + ox, f.y + oy, 30 * s, 0, TAU); ctx.fill(); ctx.restore();
          } else if (f.type === "tpnew" && !top) {
            const e = 1 - Math.pow(1 - t, 3), s = lerp(1.25, 0.05, e), x = f.x + (f.mir ? 28 : -28) * e;
            ctx.save(); ctx.globalCompositeOperation = "lighter"; ctx.globalAlpha = 1 - e; ctx.fillStyle = "#" + (f.color & 0xffffff).toString(16).padStart(6, "0");
            ctx.beginPath(); ctx.arc(x + ox, f.y + oy, 44 * s, 0, TAU); ctx.fill(); ctx.restore();
          } else if (f.type === "tpdash" && !top) {
            const idx = Math.min(8, Math.max(1, Math.floor(t * 8) + 1)), fr = getFrame(`spiderDash_${String(idx).padStart(3, "0")}.png`);
            if (fr) drawFrame({ img: fr.img, d: fr.d }, f.x + ox, f.y + oy, (f.up ? 90 : -90) * Math.PI / 180, Math.max(0.12, f.len / 400), 0.8, Math.max(0, 1 - t), f.color, 1, 0);
          } else if (f.type === "grav" && top && o) {
            const sh = 640, startY = f.up ? sh + 95 : -95, targetY = f.up ? -95 : sh + 95, e = 1 - (1 - t) * (1 - t), cy = lerp(startY, targetY, e), al = 1 - e;
            const fr = getFrame("gravityLine_001.png") || null;
            for (const l of f.lines) {
              const col = f.up ? 0xffff00 : 0x00ffff;
              if (fr) { drawFrame({ img: fr.img, d: fr.d }, l.x, cy + l.yOff, 0, l.sx, l.sy * (f.up ? 1 : -1), l.alpha * al, col, 1, 0); }
              else { ctx.save(); ctx.globalCompositeOperation = "lighter"; ctx.globalAlpha = l.alpha * al; ctx.fillStyle = f.up ? "#ffff00" : "#00ffff"; ctx.fillRect(l.x - l.sx * 4, cy + l.yOff - 2, l.sx * 8, 4); ctx.restore(); }
            }
          }
        }
      },
    };

    function loadCfg2() { /* settings can change while playing */ if ((loadCfg2.t = (loadCfg2.t || 0) + 1) % 30 === 0) loadCfg(); }
    function dtMsOf(dt) { return dt * 1000; }
  }

  // ---- level end: the icon flies into the portal along a bezier while spinning and fading ----
  Object.defineProperty(window, "__pvEndHelpers", { value: true, configurable: true });
  window.PlayerVis = { create };
})();
