// Random trigger-heavy levels shared by the differential tests (every ported trigger kind, dense groups).
export const GENERIC_IDS = [3643, 3643, 1912, 1912, 2068, 3607, 3607, 3619, 3619, 3619, 3620, 3620, 3641, 3614, 3614, 3615, 3617, 2067, 2067, 1935, 3600, 3618, 1916, 2901, 1913, 2015, 2899, 3604, 3604, 3609, 2066, 1917, 1917, 1932, 3662, 2062, 2925, 1914];
export function makeFuzz(rnd, opts = {}) {
return function fuzzLevel() {
  const parts = ["kA4,0"];
  const pick = (a) => a[Math.floor(rnd() * a.length)];
  const grp = () => 1 + Math.floor(rnd() * 8);
  const num = (lo, hi) => (lo + rnd() * (hi - lo)).toFixed(3);
  const n = 40 + Math.floor(rnd() * 160);
  for (let i = 0; i < n; i++) {
    const x = Math.floor(rnd() * 3000) * 0.5, y = Math.floor(rnd() * 40) * 0.5;
    const base = ["1", "ID", "2", x, "3", y];
    const kind = rnd();
    if (opts.beast && rnd() < 0.1) { // a beast with random look
      const o = ["1", pick([918, 1327, 1328, 1584, 2012]), "2", x, "3", y];
      const gs = Array.from({ length: Math.floor(rnd() * 3) }, grp);
      if (gs.length) o.push("57", gs.join("."));
      if (rnd() < 0.4) o.push("32", num(0.4, 2.5));
      if (rnd() < 0.3) o.push("6", num(-180, 180));
      if (rnd() < 0.3) o.push("4", 1);
      if (rnd() < 0.3) o.push("5", 1);
      if (rnd() < 0.3) o.push("21", pick([1, 2, 3, 4, 1004, 1009, 12]));
      if (rnd() < 0.3) o.push("22", pick([1, 2, 3, 4, 1004, 1009, 12]));
      if (rnd() < 0.2) o.push("96", 1);
      parts.push(o.join(","));
      continue;
    }
    if (opts.text && rnd() < 0.12) { // a text object (914) with random look and groups
      const b64 = (s) => Buffer.from(s, "utf8").toString("base64").replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
      const word = pick(["Hello", "GO!", "Jump here", "x", "Dash 123", "a b c", "WOW"]);
      const o = ["1", 914, "2", x, "3", y, "31", b64(word)];
      const gs = Array.from({ length: Math.floor(rnd() * 3) }, grp);
      if (gs.length) o.push("57", gs.join("."));
      if (rnd() < 0.5) o.push("21", pick([1, 2, 3, 4, 1004, 1009, 12]));
      if (rnd() < 0.4) o.push("32", num(0.4, 2.5));
      if (rnd() < 0.3) o.push("6", num(-180, 180));
      if (rnd() < 0.2) o.push("4", 1);
      if (rnd() < 0.2) o.push("5", 1);
      if (rnd() < 0.3) o.push("24", pick([-5, -3, -1, 1, 3, 5, 7, 9]));
      if (rnd() < 0.3) o.push("25", Math.floor(rnd() * 10) - 5);
      if (rnd() < 0.1) { o.push("131", num(-40, 40)); o.push("132", num(-40, 40)); }
      parts.push(o.join(","));
      continue;
    }
    if (opts.generic && rnd() < 0.35) {
      const ids = opts.genericIds || GENERIC_IDS;
      const gid = pick(ids);
      const o = ["1", gid, "2", x, "3", y];
      const add = (k, v) => o.push(String(k), String(v));
      add(51, grp());
      if (rnd() < 0.3) add(71, grp());
      if (rnd() < 0.3) add(11, 1);
      if (rnd() < (opts.spawny ? 0.7 : 0.35)) add(62, 1);
      if (rnd() < 0.3) add(87, 1);
      if (rnd() < 0.4) add(57, Array.from({ length: 1 + Math.floor(rnd() * 2) }, grp).join("."));
      const item = () => 1 + Math.floor(rnd() * 4);
      const pairs = (lo, hi) => Array.from({ length: 1 + Math.floor(rnd() * 3) }, () => `${grp()}.${lo + Math.floor(rnd() * (hi - lo + 1))}`).join(".");
      if (gid === 1912) add(10, num(0, 100));
      if (gid === 2068) add(152, pairs(1, 6));
      if (gid === 3607) { add(435, pairs(1, 3)); add(436, Math.floor(rnd() * 3)); add(437, num(0, 0.4)); add(438, num(0, 1.5)); add(439, Math.floor(rnd() * 2)); }
      if (gid === 3619 || gid === 3620) {
        add(80, item()); add(95, item()); add(476, Math.floor(rnd() * 6)); add(477, Math.floor(rnd() * 6)); add(478, Math.floor(rnd() * 4));
        add(479, pick([num(-3, 3), 1, 2, 0, 0.5, 5])); add(483, pick([num(-3, 3), 1, 2, 0, 3])); add(480, Math.floor(rnd() * 5)); add(481, Math.floor(rnd() * 5)); add(482, Math.floor(rnd() * 6));
        add(484, num(0, 1)); add(485, Math.floor(rnd() * 4)); add(486, Math.floor(rnd() * 4)); add(578, Math.floor(rnd() * 3)); add(579, Math.floor(rnd() * 3));
      }
      if (gid === 3643) { add(99, rnd() < 0.5 ? 1 : 0); add(56, rnd() < 0.5 ? 1 : 0); add(445, rnd() < 0.3 ? 1 : 0); add(504, rnd() < 0.4 ? 1 : 0); }
      if (gid === 3641) { add(80, item()); add(491, rnd() < 0.5 ? 1 : 0); add(492, rnd() < 0.2 ? 1 : 0); add(493, rnd() < 0.3 ? 1 : 0); add(494, rnd() < 0.4 ? 1 : 0); }
      if (gid === 3614) { add(80, item()); add(467, num(-2, 3)); add(473, num(0, 5)); add(474, rnd() < 0.5 ? 1 : 0); add(468, rnd() < 0.3 ? 1 : 0); add(469, rnd() < 0.3 ? 1 : 0); add(470, pick([1, -1, 2, 0.5, num(-2, 2)])); add(471, rnd() < 0.2 ? 1 : 0); }
      if (gid === 3615) { add(80, item()); add(473, num(0, 6)); add(475, rnd() < 0.4 ? 1 : 0); }
      if (gid === 3617) { add(80, item()); add(472, Math.floor(rnd() * 3)); }
      if (gid === 2067) { add(150, num(0.2, 3)); add(151, num(0.2, 3)); add(153, rnd() < 0.2 ? 1 : 0); add(154, rnd() < 0.2 ? 1 : 0); add(10, num(0, 2)); add(30, Math.floor(rnd() * 19)); add(85, num(0.5, 4)); add(133, rnd() < 0.3 ? 1 : 0); }
      if (gid === 1935) add(120, pick([num(0, 3), 0.5, 1, 1.5, 2, 0.05, "abc"]));
      if (gid === 3600) { add(460, rnd() < 0.5 ? 1 : 0); add(461, rnd() < 0.5 ? 1 : 0); add(487, rnd() < 0.5 ? 1 : 0); }
      if (gid === 1916) { add(28, num(-60, 60)); add(29, num(-60, 60)); add(101, Math.floor(rnd() * 3)); add(10, num(0, 2)); add(30, Math.floor(rnd() * 19)); add(85, num(0, 4)); }
      if (gid === 2901) { add(28, num(-60, 60)); add(29, num(-60, 60)); add(58, rnd() < 0.7 ? 1 : 0); add(59, rnd() < 0.7 ? 1 : 0); add(101, Math.floor(rnd() * 3)); }
      if (gid === 1913) { add(371, pick([num(-1, 4), 0.5, 1, 2, 3])); add(10, num(0, 2)); add(30, Math.floor(rnd() * 19)); add(85, num(0, 4)); }
      if (gid === 2015) { add(68, num(-900, 900)); add(70, rnd() < 0.4 ? 1 : 0); add(394, rnd() < 0.4 ? 1 : 0); add(10, num(0, 2)); add(30, Math.floor(rnd() * 19)); add(85, num(0, 4)); }
      if (gid === 2899) { for (const k of [161, 162, 163, 165, 199, 532, 573]) if (rnd() < 0.5) add(k, Math.floor(rnd() * 3)); add(574, num(0, 12)); }
      if (gid === 3604) { add(430, Array.from({ length: 1 + Math.floor(rnd() * 3) }, () => 1 + Math.floor(rnd() * 80)).join(".")); add(447, Math.floor(rnd() * 2)); add(525, Math.floor(rnd() * 2)); }
      if (gid === 3609) { add(80, pick([10000, 10001, 1, 2, 3])); add(95, pick([1, 2, 3, 10000, 10001])); add(138, rnd() < 0.3 ? 1 : 0); add(200, rnd() < 0.3 ? 1 : 0); add(201, rnd() < 0.2 ? 1 : 0); }
      if (gid === 2066) { add(148, num(0, 3)); add(200, rnd() < 0.3 ? 1 : 0); add(138, rnd() < 0.3 ? 1 : 0); add(201, rnd() < 0.2 ? 1 : 0); }
      if (gid === 1932) { for (const k of [540, 541, 542, 543, 138, 200]) add(k, rnd() < 0.5 ? 1 : 0); }
      if (gid === 2062) add(164, Math.floor(rnd() * 6));
      if (gid === 2925) { add(111, rnd() < 0.5 ? 1 : 0); add(112, rnd() < 0.6 ? 1 : 0); add(113, num(-1, 45)); add(114, num(-0.3, 1.3)); }
      if (gid === 1914) { add(10, num(0, 2)); add(30, Math.floor(rnd() * 19)); add(85, num(0, 4)); add(101, Math.floor(rnd() * 3)); add(110, rnd() < 0.4 ? 1 : 0); add(465, rnd() < 0.4 ? 1 : 0); }
      parts.push(o.join(","));
      continue;
    }
    if (kind < 0.45) {
      const id = pick([1, 1, 1, 8, 8, 103, 35, 36, 10, 13, 48, 200, 201, 11, 12, 38]);
      const o = ["1", id, "2", x, "3", y];
      const gs = Array.from({ length: Math.floor(rnd() * 3) }, grp);
      if (gs.length) o.push("57", gs.join("."));
      if (rnd() < 0.2) o.push("6", Math.floor(rnd() * 4) * 90);
      parts.push(o.join(","));
      continue;
    }
    const id = pick([901, 901, 901, 1346, 1346, 1268, 1268, 1268, 1268, 1616, 1049, 1049, 1814, 1347, 1347, 1812, 1595, 1817, 1611, 1811, 1815, 1816, 1816, 1816, 29, 30, 899, 105, 221, 1006, 1006, 1007, 1007, 1007, 1520, 1520, 1612, 1613, 1585, 1585, 23, 24, 25, 26, 27, 28, 55, 56, 57, 58, 59]);
    const o = ["1", id, "2", id === 1816 ? 300 + 15 * Math.floor(rnd() * 20) : x, "3", id === 1816 ? 15 * Math.floor(rnd() * 6) : y];
    const add = (k, v) => o.push(String(k), String(v));
    add(51, grp());
    if (rnd() < 0.4) add(11, 1);
    if (rnd() < (opts.spawny ? 0.7 : 0.3)) add(62, 1);
    if (rnd() < 0.3) add(57, Array.from({ length: 1 + Math.floor(rnd() * 2) }, grp).join("."));
    if (rnd() < 0.6) add(10, num(0, 3));
    if (id === 901) { add(28, num(-90, 90)); add(29, num(-90, 90)); add(30, Math.floor(rnd() * 20)); add(85, num(0.5, 4)); if (rnd() < 0.15) add(pick([58, 59, 302, 303]), 1); }
    if (id === 1346) { add(68, num(-360, 360)); add(69, Math.floor(rnd() * 3)); add(70, rnd() < 0.3 ? 1 : 0); add(71, grp()); add(30, Math.floor(rnd() * 20)); add(85, num(0.5, 4)); }
    if (id === 1268) { add(63, num(0, 2)); if (rnd() < 0.3) add(556, num(0, 1)); if (opts.remap !== false && rnd() < (opts.spawny ? 0.85 : 0.4)) add(442, Array.from({ length: 1 + Math.floor(rnd() * 3) }, () => `${grp()}.${grp()}`).join(".")); }
    if (id === 1049) add(56, rnd() < 0.5 ? 1 : 0);
    if (id === 1814) { add(90, num(0, 3)); add(91, num(0, 1)); add(92, num(-30, 30)); add(105, num(0, 5)); }
    if (id === 1347) { add(71, grp()); add(72, num(-2, 2)); add(73, num(-2, 2)); }
    if (id === 29 || id === 30 || id === 899 || id === 105 || id === 221) { add(7, Math.floor(rnd() * 256)); add(8, Math.floor(rnd() * 256)); add(9, Math.floor(rnd() * 256)); add(14, rnd() < 0.3 ? 1 : 0); if (id > 100) { add(23, pick([1, 2, 3, 4, 1002, 1003, 1004, 1009, 8, 20])); add(17, rnd() < 0.3 ? 1 : 0); add(35, num(0, 1)); if (rnd() < 0.3) add(50, pick([1, 2, 3, 4, 8, 20])); if (rnd() < 0.3) add(49, `${Math.floor(rnd() * 360)}a${num(0, 2)}a${num(0, 2)}a0a0`); } }
    if (id === 1007) add(35, num(-0.3, 1.3));
    if (id === 1520) { add(75, num(-5, 120)); add(84, num(0, 0.6)); }
    if (id === 1585) add(76, Math.floor(rnd() * 12));
    if (id === 1006) { add(52, rnd() < 0.7 ? 0 : 1); add(7, Math.floor(rnd() * 256)); add(8, Math.floor(rnd() * 256)); add(9, Math.floor(rnd() * 256)); add(45, num(0, 1)); add(46, num(0, 1)); add(47, num(0, 1)); if (rnd() < 0.3) { add(48, 1); add(49, `${Math.floor(rnd() * 360)}a${num(0, 2)}a${num(0, 2)}a0a0`); } if (rnd() < 0.3) add(50, pick([1, 2, 3, 4, 8, 20])); }
    if (id === 1812) add(56, rnd() < 0.5 ? 1 : 0);
    if (id === 1595) { add(56, Math.floor(rnd() * 3)); add(81, rnd() < 0.3 ? 1 : 0); add(82, rnd() < 0.4 ? 1 : 0); }
    if (id === 1817) { add(80, 1 + Math.floor(rnd() * 4)); add(77, Math.floor(rnd() * 4)); }
    if (id === 1611 || id === 1811) { add(80, 1 + Math.floor(rnd() * 4)); add(77, Math.floor(rnd() * 5)); add(56, rnd() < 0.7 ? 1 : 0); if (id === 1611) add(104, rnd() < 0.3 ? 1 : 0); else add(88, Math.floor(rnd() * 3)); }
    if (id === 1815) { add(80, 1 + Math.floor(rnd() * 3)); add(95, 1 + Math.floor(rnd() * 3)); add(56, rnd() < 0.5 ? 1 : 0); add(93, rnd() < 0.3 ? 1 : 0); }
    if (id === 1816) { add(80, 1 + Math.floor(rnd() * 3)); add(94, rnd() < 0.6 ? 1 : 0); }
    parts.push(o.join(","));
  }
  return parts.join(";");
};
}
