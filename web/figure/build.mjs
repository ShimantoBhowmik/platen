// Builds the typewriter figure for the web app.
//
// Run with `node web/figure/build.mjs`. It writes `web/assets/typewriter.json`: static SVG
// layers drawn with the vendored anatomy kit, plus the numbers the Rust app needs to move the
// carriage, sink keys, strike typebars and set text on the sheet. Add `--preview` to also write
// `web/figure/preview.html`, a standalone page for visual and line-end checks.
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import * as k from "./kit/iso-kit.mjs";
import * as G from "./kit/lathe.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));
const OUT = join(HERE, "..", "assets", "typewriter.json");

// Type on the sheet: pica-like pitch, one cell per character.
const CELL = 12.5;
const COLS = 26;
const LINE_H = 23;
const FONT = 21;
const MARGIN = 14;
// Lines are laid out a few cells short of the paper, so extra letters never push copy off it.
const WRAP = COLS - 3;
const PAPER_W = COLS * CELL + MARGIN * 2;

// Platen and print line.
const PLATEN = { y: -70, z: 86, r: 15, half: 192 };
const SHEET_Y = PLATEN.y + PLATEN.r + 0.6;
const PRINT_Z = PLATEN.z;
const BASELINE_Z = PRINT_Z - 3.5;
const SHEET_TOP = PRINT_Z + 3.5 + LINE_H * 3 + 18;
const SHEET_BOTTOM = PRINT_Z - 14;

// Copy holder: a card on a stand behind the machine carrying the next lines of practice copy.
const COPY = { y: -108, z0: SHEET_TOP - 30, z1: SHEET_TOP + 58, font: 17, line_h: 21 };
COPY.cell = (CELL * COPY.font) / FONT;

// Body and bench.
const BASE = { x: -196, y: -120, w: 392, d: 246, r: 12 };
const BASE_H = 9;
const FOOT_H = 5;
const BODY = { x: -168, y: -96, w: 336, d: 206, r: 16 };
const BODY_H = 30;
const REAR = { x: -168, y: -96, w: 336, d: 36, r: 12 };
const REAR_TOP = 66;
const DECK_Z = BODY_H + 4;

// Keyboard: four staggered rows of round keys on stems, rising toward the back.
const ROWS = [
  { chars: "1234567890", y: 32, x0: -95, top: 60 },
  { chars: "qwertyuiop", y: 49, x0: -89, top: 56 },
  { chars: "asdfghjkl;", y: 66, x0: -83, top: 52 },
  { chars: "zxcvbnm,./", y: 83, x0: -77, top: 48 },
];
const KEY_PITCH = 19;
const KEY_R = 7;
const SPACE = { x: -78, y: 97, w: 156, d: 7, r: 3.5, z: 42 };

// Typebars fan out from a segment in front of the print point.
const STRIKE = [0, SHEET_Y + 0.4, PRINT_Z];
const SEG = { cx: 0, cy: SHEET_Y, r: 24, z: 58 };
const REST_R = 66;
const REST_Z = 41;

const WIDTH = 900;
const HEIGHT = 600;
const AZIMUTH = 80;

// Carriage travel: column c sits at the strike point when the carriage is offset by
// dxOf(c) along x. The figure is fitted with the carriage at both extremes.
const colLeft = (c) => -PAPER_W / 2 + MARGIN + c * CELL;
const dxOf = (c) => -(colLeft(c) + CELL / 2);
const CARRIAGE_HALF = PLATEN.half + 22;

const P = k.fitProjection(
  [
    ...k.boxCorners(BASE, -BASE_H - FOOT_H, 0),
    ...[dxOf(0), dxOf(WRAP)].flatMap((dx) => [
      [dx - PAPER_W / 2, SHEET_Y, SHEET_TOP],
      [dx + PAPER_W / 2, SHEET_Y, SHEET_TOP],
    ]),
    [-PAPER_W / 2 - 8, COPY.y, COPY.z1],
    [PAPER_W / 2 + 8, COPY.y, COPY.z1],
  ],
  WIDTH,
  HEIGHT,
  { pad: 6, azimuth: AZIMUTH },
);

const at = (point) => k.iso(point, P);
const S = (paths, style) => k.solidSvg(paths, style);
const L = (d, style) => k.lineSvg(d, style);
const D = (points, style) => k.dotsSvg(points, style);
const depth = ([x, y, z = 0]) => k.depthOf([x, y, z], P);
const byDepth = (points) => [...points].sort((a, b) => depth(a) - depth(b));
const planeOf = (y) => k.sideMatrix([0, y], 0, P, "left");
const flatRect = (x0, z0, x1, z1, r, steps = 4) =>
  k.pathOf(k.roundedPlan({ x: x0, y: z0, w: x1 - x0, d: z1 - z0, r }, steps).map(([u, z]) => [u, -z]), true);
const rod = (from, to, r, options = {}) => {
  const dir = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
  const len = Math.hypot(...dir);
  const F = G.frameAlong(from, dir.map((v) => v / len));
  return G.solidOf([[0, r], [len, r]], F, P, options);
};
const along = (x, y, z) => G.frameAlong([x, y, z], [1, 0, 0]);

function bench() {
  const out = [`<path class="iso-halo" d="${k.haloOf(BASE, -BASE_H - FOOT_H, BASE_H, P)}"/>`];
  for (const [x, y] of byDepth(k.corners(BASE, 24))) {
    out.push(S(k.cylinder(x, y, 8, -BASE_H - FOOT_H, 1.6, P, 28), { tone: "lo" }));
    out.push(S(k.cylinder(x, y, 6.2, -BASE_H - FOOT_H + 1.6, FOOT_H - 1.6, P, 28), { tone: "mid" }));
  }
  out.push(S(k.slabOf(BASE, -BASE_H, BASE_H, P, 8, 1.6), { tone: "mid" }));
  out.push(L(k.planOutline(k.insetPlan(BASE, 6), 0, P, 8), { tone: "lo" }));
  const screws = k.corners(BASE, 11);
  out.push(L(screws.map(([x, y]) => k.ring(x, y, 2.3, 0, P, 16)).join(""), { tone: "lo" }));
  out.push(D(screws.map(([x, y]) => at([x, y, 0])), { size: 0.5 }));

  const strip = { x: BASE.x + 26, y: BASE.y + BASE.d - 17, w: 210, d: 7, r: 1 };
  out.push(S(k.slabOf(strip, 0, 0.6, P, 3), { tone: "lo", crease: "none" }));
  const ticks = k.topTicks(strip.x + 5, strip.x + strip.w - 5, CELL / 2, 10, strip.y + 0.8, 0.6, [2, 3.8], P);
  out.push(L(ticks.minor, { tone: "lo" }), L(ticks.major, { tone: "mid" }));

  const label = { x: BASE.x + BASE.w - 74, y: BASE.y + BASE.d - 21, w: 50, d: 12, r: 1.4 };
  out.push(S(k.slabOf(label, 0, 0.8, P, 3), { tone: "lo", crease: "none" }));
  out.push(L(k.planOutline(k.insetPlan(label, 1.6), 0.8, P, 3), { tone: "faint" }));
  out.push(L([34, 26, 18].map((w, i) => k.lineOnTop([label.x + 4, label.y + 3 + i * 3], [label.x + 4 + w, label.y + 3 + i * 3], 0.8, P)).join(""), { tone: "lo", free: true }));
  out.push(D([at([label.x + label.w - 4, label.y + 3.5, 0.8]), at([label.x + label.w - 4, label.y + label.d - 3.5, 0.8])], { size: 0.45 }));
  return out.join("");
}

function body() {
  const out = [`<path class="iso-halo" d="${k.haloOf(BODY, 0, 10, P)}"/>`];
  for (const [x, y] of byDepth(k.corners(BODY, 20))) {
    out.push(S(k.cylinder(x, y, 7, 0, 3, P, 24), { tone: "lo" }));
  }
  const seam = k.sideSeam(BODY, 14, P, 8);
  out.push(S(k.slabOf(BODY, 3, BODY_H - 3, P, 8, 1.4), { tone: "mid", inner: L(seam, { tone: "faint" }) }));
  // Front apron with the engraved name plate.
  const front = BODY.y + BODY.d;
  out.push(`<g transform="${planeOf(front)}">${L(flatRect(-56, 9, 56, 24, 2), { tone: "lo" })}${L(flatRect(-53, 11.5, 53, 21.5, 1.4), { tone: "faint" })}</g>`);
  out.push(k.faceTextSvg(planeOf(front), "TYPE-CAST", { size: 6, tone: "mid", anchor: "middle" }).replace("<text ", `<text x="0" y="-14.4" `));
  out.push(D([at([-61, front, 16.5]), at([61, front, 16.5])], { size: 0.5, tone: "mid" }));
  // Right side vents.
  const side = BODY.x + BODY.w;
  const vents = Array.from({ length: 9 }, (_, i) => {
    const y = BODY.y + 44 + i * 6;
    return k.pathOf(k.roundedPlan({ x: y - 1, y: 8, w: 2, d: 14, r: 1 }, 4).map(([u, z]) => at([side, u, z])), true);
  });
  out.push(L(vents.join(""), { tone: "faint" }));
  // Deck plate the keys and basket sit on.
  const deck = { x: BODY.x + 8, y: -60, w: BODY.w - 16, d: 162, r: 10 };
  out.push(S(k.slabOf(deck, BODY_H, DECK_Z - BODY_H, P, 6, 0.8), { tone: "mid" }));
  out.push(L(k.planOutline(k.insetPlan(deck, 4), DECK_Z, P, 6), { tone: "faint" }));
  out.push(D(k.corners(deck, 7).map(([x, y]) => at([x, y, DECK_Z])), { size: 0.5 }));
  // Side frames rising around the basket, the cast cheeks that carry the segment.
  for (const x of [BODY.x + 6, BODY.x + BODY.w - 30]) {
    const cheek = { x, y: -60, w: 24, d: 74, r: 9 };
    out.push(S(k.slabOf(cheek, DECK_Z, 26, P, 6, 1), { tone: "mid", inner: L(k.sideSeam(cheek, DECK_Z + 9, P, 6), { tone: "faint" }) }));
    out.push(L(k.planOutline(k.insetPlan(cheek, 3.4), DECK_Z + 26, P, 6), { tone: "faint" }));
    out.push(D([at([x + 12, -52, DECK_Z + 26]), at([x + 12, 6, DECK_Z + 26])], { size: 0.5 }));
  }
  return out.join("");
}

function rearHousing() {
  const out = [];
  out.push(S(k.slabOf(REAR, BODY_H, REAR_TOP - BODY_H, P, 8, 1.2), { tone: "mid", inner: L(k.sideSeam(REAR, BODY_H + 12, P, 8), { tone: "faint" }) }));
  const top = k.insetPlan(REAR, 5);
  out.push(L(k.planOutline(top, REAR_TOP, P, 6), { tone: "lo" }));
  out.push(D(k.corners(REAR, 8).map(([x, y]) => at([x, y, REAR_TOP])), { size: 0.5 }));
  // Carriage rail the carriage rides on, with end stops.
  const rail = { x: REAR.x + 10, y: -90, w: REAR.w - 20, d: 8, r: 1.4 };
  out.push(S(k.slabOf(rail, REAR_TOP, 3, P, 3, 0.4), { tone: "lo" }));
  for (const x of [rail.x, rail.x + rail.w - 6]) {
    out.push(S(k.slabOf({ x, y: rail.y - 2, w: 6, d: 12, r: 1.2 }, REAR_TOP, 6, P, 3, 0.4), { tone: "mid" }));
    out.push(D([at([x + 3, rail.y + 4, REAR_TOP + 6])], { size: 0.45 }));
  }
  // Margin stops on a toothed rack along the front of the housing top.
  const rack = { x: REAR.x + 16, y: -70, w: REAR.w - 32, d: 4, r: 1 };
  out.push(S(k.slabOf(rack, REAR_TOP, 1.4, P, 3), { tone: "lo", crease: "none" }));
  const teeth = k.topTicks(rack.x + 2, rack.x + rack.w - 2, CELL / 2, 2, rack.y + 0.4, REAR_TOP + 1.4, [1.6, 3], P);
  out.push(L(teeth.minor + teeth.major, { tone: "faint" }));
  for (const x of [colLeft(0) - 4, colLeft(COLS) + 1]) {
    out.push(S(k.slabOf({ x, y: rack.y - 1, w: 4, d: 6, r: 0.8 }, REAR_TOP + 1.4, 3.4, P, 3, 0.3), { tone: "hi", crease: "lo" }));
  }
  // Fixed margin pointer the scale passes over.
  out.push(S(k.slabOf({ x: -2, y: -66, w: 4, d: 5, r: 0.8 }, REAR_TOP, 1.6, P, 3), { tone: "hi", crease: "lo" }));
  return out.join("");
}

function bell() {
  const x = 140;
  const y = -79;
  const F = G.frameOf([x, y, REAR_TOP], [0, 0, 1], [1, 0, 0], [0, 1, 0]);
  const out = [S(k.cylinder(x, y, 3, REAR_TOP, 3, P, 16), { tone: "lo" })];
  const dome = [[3, 9.5], ...Array.from({ length: 9 }, (_, i) => {
    const t = (i + 1) / 10;
    return [3 + 7 * t, 9.5 * Math.sqrt(1 - t * t), 1];
  }), [10, 0]];
  out.push(S(G.solidOf(dome, F, P), { tone: "hi", crease: "lo" }));
  out.push(D([at([x, y, REAR_TOP + 10.2])], { size: 0.5, tone: "hi" }));
  return out.join("");
}

function hammer() {
  const pivot = [128, -79, REAR_TOP + 2];
  return [
    S(k.cylinder(pivot[0], pivot[1], 1.6, REAR_TOP, 3, P, 12), { tone: "mid" }),
    S(rod([pivot[0], pivot[1], REAR_TOP + 3], [137.5, -79, REAR_TOP + 7], 0.7), { tone: "mid" }),
    S(G.sphereOf([137.6, -79, REAR_TOP + 7.2], 1.5, P), { tone: "hi" }),
  ].join("");
}

function carriageBack() {
  const out = [];
  const plates = [-CARRIAGE_HALF + 2, CARRIAGE_HALF - 9];
  // Carriage frame bar along the back and the rollers on the rail.
  const bar = { x: -CARRIAGE_HALF + 4, y: -96, w: CARRIAGE_HALF * 2 - 8, d: 7, r: 1.6 };
  out.push(S(k.slabOf(bar, REAR_TOP + 3, 9, P, 3, 0.4), { tone: "lo" }));
  out.push(D(Array.from({ length: 7 }, (_, i) => at([bar.x + 20 + i * ((bar.w - 40) / 6), bar.y + bar.d, REAR_TOP + 7.5])), { size: 0.45, tone: "mid" }));
  // Left end plate, the return lever and the left platen knob are behind the sheet's left edge.
  out.push(endPlate(plates[0]));
  out.push(returnLever(plates[0]));
  // Platen: a rubber roller with its end bands, then the knobs.
  const F = along(-PLATEN.half, PLATEN.y, PLATEN.z);
  out.push(S(G.disc(-3, 0, PLATEN.r - 1.5, F, P), { tone: "mid" }));
  out.push(S(G.disc(0, PLATEN.half * 2, PLATEN.r, F, P, { smooth: true }), { tone: "lo", crease: "none" }));
  out.push(S(G.disc(PLATEN.half * 2, PLATEN.half * 2 + 3, PLATEN.r - 1.5, F, P), { tone: "mid" }));
  out.push(knob(-PLATEN.half - 3, -1));
  out.push(endPlate(plates[1]));
  out.push(knob(PLATEN.half + 3, 1));
  // Scale bar on the carriage front, one tick per pitch cell.
  // Scale bar across the carriage front, below the sheet: one tick per pitch cell hanging
  // from its top edge on the face toward the typist.
  const scale = { x: colLeft(0) - 12, y: -60, w: COLS * CELL + 24, d: 8, r: 1 };
  const scaleTop = SHEET_BOTTOM - 3;
  out.push(S(k.slabOf(scale, scaleTop - 7, 7, P, 3, 0.4), { tone: "hi", crease: "lo" }));
  const t = k.sideTicks(colLeft(0), colLeft(COLS), CELL, 5, scale.y + scale.d, scaleTop, [2.4, 4.6], P);
  out.push(L(t.minor, { tone: "mid" }), L(t.major, { tone: "hi" }));
  return out.join("");
}

function endPlate(x) {
  const out = [];
  const plate = { x, y: -98, w: 7, d: 52, r: 3 };
  out.push(S(k.slabOf(plate, REAR_TOP + 3, PLATEN.z + 20 - REAR_TOP, P, 4, 0.6), { tone: "mid" }));
  out.push(D([at([x + 7, -90, REAR_TOP + 10]), at([x + 7, -56, REAR_TOP + 10]), at([x + 7, -90, PLATEN.z + 16])], { size: 0.5 }));
  out.push(L(k.ring(x + 3.5, PLATEN.y, 3.2, PLATEN.z + 23, P, 12), { tone: "faint" }));
  return out.join("");
}

function knob(x, side) {
  const F = along(x, PLATEN.y, PLATEN.z);
  const s0 = side < 0 ? -20 : 0;
  const out = [];
  out.push(S(G.disc(s0 + (side < 0 ? 14 : 0), s0 + (side < 0 ? 20 : 6), 5, F, P), { tone: "mid" }));
  const capS = side < 0 ? [s0, s0 + 14] : [6, 20];
  out.push(S(G.disc(capS[0], capS[1], 11, F, P), { tone: "hi", crease: "lo" }));
  out.push(k.fadedSvg(G.ribsOf(F, capS[0] + 1.5, capS[1] - 1.5, 11, 40, P, { fade: [0.05, 0.4], seams: true }), { tone: "lo" }));
  return out.join("");
}

function returnLever(x) {
  const pivot = [x - 2, -68, PLATEN.z + 16];
  const tip = [x - 44, -26, PLATEN.z + 30];
  return [
    S(G.disc(-3, 3, 4.4, G.frameAlong(pivot, [1, 0, 0]), P), { tone: "mid" }),
    S(rod(pivot, tip, 1.6), { tone: "hi", crease: "lo" }),
    S(G.sphereOf(tip, 4.2, P), { tone: "hi" }),
  ].join("");
}

function sheet() {
  const x0 = -PAPER_W / 2;
  const shape = flatRect(x0, SHEET_BOTTOM, x0 + PAPER_W, SHEET_TOP, 0.6, 2);
  // The sheet curls over the platen at the bottom: a shade band where it turns away.
  const curl = flatRect(x0, SHEET_BOTTOM, x0 + PAPER_W, PRINT_Z - 9, 0, 2);
  return `<g transform="${planeOf(SHEET_Y)}"><path class="tw-sheet" d="${shape}"/><path class="tw-sheet-shade" d="${curl}"/><path class="tw-sheet-edge" d="${shape}"/></g>`;
}

function copyHolder() {
  const out = [];
  const half = PAPER_W / 2 + 8;
  // Two posts on the bench behind the body, each in a foot block.
  for (const x of [-half + 26, half - 26]) {
    out.push(S(k.slabOf({ x: x - 7, y: COPY.y - 6, w: 14, d: 10, r: 2 }, 0, 6, P, 4, 0.5), { tone: "mid" }));
    out.push(S(k.cylinder(x, COPY.y - 1, 2.4, 6, COPY.z0 - 6, P, 16), { tone: "lo", crease: "none" }));
  }
  const board = { x: -half, y: COPY.y, w: half * 2, d: 3, r: 1.2 };
  out.push(S(k.slabOf(board, COPY.z0, COPY.z1 - COPY.z0, P, 3, 0.5), { tone: "mid" }));
  out.push(`<g transform="${planeOf(COPY.y + 3)}">`);
  const card = flatRect(-half + 6, COPY.z0 + 6, half - 6, COPY.z1 - 6, 1, 2);
  out.push(`<path class="tw-sheet" d="${card}"/><path class="tw-sheet-edge" d="${card}"/>`);
  out.push(`</g>`);
  out.push(D([[-half + 3, COPY.z1 - 3], [half - 3, COPY.z1 - 3]].map(([x, z]) => at([x, COPY.y + 3, z])), { size: 0.5 }));
  return out.join("");
}

function carriageFront() {
  const out = [];
  // Paper bail: a rod across the sheet between two printed lines, on arms to the end plates.
  const bailZ = BASELINE_Z + LINE_H + 13.5;
  const y = SHEET_Y + 2.2;
  for (const side of [-1, 1]) {
    out.push(S(rod([side * (CARRIAGE_HALF - 6), PLATEN.y + 6, PLATEN.z + 12], [side * (PLATEN.half - 2), y, bailZ], 1.1), { tone: "mid" }));
  }
  const F = along(-PLATEN.half, y, bailZ);
  out.push(S(G.disc(0, PLATEN.half * 2, 1.4, F, P), { tone: "hi", crease: "lo" }));
  for (const x of [-60, 60]) {
    out.push(S(G.disc(x + PLATEN.half - 7, x + PLATEN.half + 7, 3.2, F, P, { smooth: true }), { tone: "lo" }));
  }
  return out.join("");
}

function basket() {
  const out = [];
  // Felt rest pad the typebar heads lie on, a half ring around the segment.
  const arc = (r, from, to) =>
    Array.from({ length: 41 }, (_, i) => {
      const a = Math.PI * (from + (to - from) * (i / 40));
      return [SEG.cx + Math.cos(a) * r, SEG.cy + Math.sin(a) * r];
    });
  const pad = [...arc(REST_R + 2.5, 0.06, 0.94), ...arc(REST_R - 2.5, 0.94, 0.06)];
  out.push(S(k.extrude(pad, DECK_Z, 0.8, P, { convex: false }), { tone: "lo", crease: "none" }));
  // Segment: a raised slotted half ring.
  const ring = [];
  for (let i = 0; i <= 32; i++) {
    const a = Math.PI * (i / 32);
    ring.push([SEG.cx + Math.cos(a) * (SEG.r + 5), SEG.cy + Math.sin(a) * (SEG.r + 5)]);
  }
  for (let i = 32; i >= 0; i--) {
    const a = Math.PI * (i / 32);
    ring.push([SEG.cx + Math.cos(a) * (SEG.r - 3), SEG.cy + Math.sin(a) * (SEG.r - 3)]);
  }
  out.push(S(k.extrude(ring, DECK_Z, SEG.z - DECK_Z, P, { convex: false, bevel: 0.4 }), { tone: "mid" }));
  out.push(typebars());
  return out.join("");
}

const BARS = ROWS.flatMap((row) => [...row.chars]);
function barAngle(index) {
  return Math.PI * (0.08 + 0.84 * (index / (BARS.length - 1)));
}
function pivotOf(index) {
  const a = barAngle(index);
  return [SEG.cx + Math.cos(a) * SEG.r, SEG.cy + Math.sin(a) * SEG.r, SEG.z - 1];
}
function typebars() {
  const items = BARS.map((_, i) => {
    const a = barAngle(i);
    const pivot = pivotOf(i);
    const head = [SEG.cx + Math.cos(a) * REST_R, SEG.cy + Math.sin(a) * REST_R, REST_Z + 1.6];
    return { pivot, head, d: depth(head) };
  }).sort((a, b) => a.d - b.d);
  return items
    .map(({ pivot, head }) => S(rod(pivot, head, 0.9), { tone: "mid", crease: "none" }) + S(G.sphereOf(head, 1.7, P), { tone: "mid" }))
    .join("");
}

function strikes() {
  const out = {};
  BARS.forEach((char, i) => {
    const pivot = pivotOf(i);
    const tip = [STRIKE[0], STRIKE[1] + 2.6, STRIKE[2] - 1];
    out[char] = S(rod(pivot, tip, 1.1), { tone: "hi", crease: "lo" }) + S(G.sphereOf(tip, 2.6, P), { tone: "hi" });
  });
  return out;
}

function spools() {
  const out = [];
  for (const x of [-112, 112]) {
    const y = -34;
    out.push(S(k.cylinder(x, y, 4, DECK_Z, 10, P, 16), { tone: "lo" }));
    out.push(S(k.cylinder(x, y, 24, DECK_Z + 10, 2.4, P, 56, 0.6), { tone: "mid" }));
    out.push(S(k.cylinder(x, y, 18, DECK_Z + 12.4, 6, P, 56), { tone: "lo", crease: "faint" }));
    out.push(L(k.ring(x, y, 12, DECK_Z + 18.4, P, 40) + k.ring(x, y, 6, DECK_Z + 18.4, P, 24), { tone: "faint" }));
    out.push(S(k.cylinder(x, y, 2.6, DECK_Z + 18.4, 3, P, 14), { tone: "hi", crease: "lo" }));
    out.push(D([[x + 9, y], [x - 9, y], [x, y + 9], [x, y - 9]].map(([px, py]) => at([px, py, DECK_Z + 12.4])), { size: 0.45 }));
  }
  return out.join("");
}

function ribbon() {
  // Two-colour ribbon: black above red, from each spool to the vibrator guides.
  const run = (x0, x1, y0, y1, z0, z1, h) =>
    k.pathOf([at([x0, y0, z0]), at([x1, y1, z1]), at([x1, y1, z1 - h]), at([x0, y0, z0 - h])], true);
  const out = [];
  for (const side of [-1, 1]) {
    const black = run(side * 94, side * 15, -34 - 18 * 0.5, SHEET_Y + 3, DECK_Z + 18.4, PRINT_Z - 7, 1.1);
    const red = run(side * 94, side * 15, -34 - 18 * 0.5, SHEET_Y + 3, DECK_Z + 17.3, PRINT_Z - 8.1, 1.0);
    out.push(`<path class="tw-ribbon" d="${black}"/><path class="tw-ribbon-red" d="${red}"/>`);
  }
  return out.join("");
}

function cardHolder() {
  const out = [];
  const y = SHEET_Y + 1.4;
  out.push(`<g transform="${planeOf(y)}">`);
  const plate = k.pathOf([[-18, -(PRINT_Z - 11)], [-18, -(PRINT_Z - 6)], [-2.4, -(PRINT_Z - 6)], [0, -(PRINT_Z - 8.6)], [2.4, -(PRINT_Z - 6)], [18, -(PRINT_Z - 6)], [18, -(PRINT_Z - 11)]], true);
  out.push(`<path class="iso-fill" d="${plate}"/><path class="tw-holder" d="${plate}"/>`);
  out.push(L(plate, { tone: "mid" }));
  out.push(`</g>`);
  out.push(D([at([-15, y, PRINT_Z - 8.5]), at([15, y, PRINT_Z - 8.5])], { size: 0.45, tone: "mid" }));
  return out.join("");
}

function vibrator() {
  const y = SHEET_Y + 3.2;
  const out = [`<g transform="${planeOf(y)}">`];
  const fork = k.pathOf([[-15, -(PRINT_Z - 13)], [-15, -(PRINT_Z - 6.6)], [-12.6, -(PRINT_Z - 6.6)], [-12.6, -(PRINT_Z - 10.6)], [12.6, -(PRINT_Z - 10.6)], [12.6, -(PRINT_Z - 6.6)], [15, -(PRINT_Z - 6.6)], [15, -(PRINT_Z - 13)]], true);
  out.push(`<path class="iso-fill" d="${fork}"/>`, L(fork, { tone: "lo" }));
  out.push(`<path class="tw-ribbon" d="${flatRect(-12.6, PRINT_Z - 9.2, 12.6, PRINT_Z - 6.8, 0, 2)}"/>`);
  out.push(`<path class="tw-ribbon-red" d="${flatRect(-12.6, PRINT_Z - 10.6, 12.6, PRINT_Z - 9.2, 0, 2)}"/>`);
  out.push(`</g>`);
  return out.join("");
}

function strikeMark() {
  const y = SHEET_Y + 1.5;
  return `<g transform="${planeOf(y)}"><path class="tw-mark" d="M0 ${-(PRINT_Z - 7)}L-3.2 ${-(PRINT_Z - 11.6)}L3.2 ${-(PRINT_Z - 11.6)}Z"/></g>`;
}

function keys() {
  const list = [];
  for (const row of ROWS) {
    [...row.chars].forEach((char, i) => {
      const x = row.x0 + i * KEY_PITCH;
      list.push({ char, x, y: row.y, top: row.top });
    });
  }
  const extra = [
    { char: "Shift", x: ROWS[3].x0 - KEY_PITCH - 3, y: ROWS[3].y, top: ROWS[3].top },
    { char: "ShiftRight", x: ROWS[3].x0 + 10 * KEY_PITCH + 3, y: ROWS[3].y, top: ROWS[3].top },
    { char: "Backspace", x: ROWS[0].x0 + 10 * KEY_PITCH + 2, y: ROWS[0].y, top: ROWS[0].top },
    { char: "Enter", x: ROWS[2].x0 + 10 * KEY_PITCH + 3, y: ROWS[2].y, top: ROWS[2].top },
  ];
  const all = [...list, ...extra].sort((a, b) => depth([a.x, a.y, a.top]) - depth([b.x, b.y, b.top]));
  return all.map(({ char, x, y, top }) => {
    const stem = S(k.cylinder(x, y, 1.5, DECK_Z, top - 3 - DECK_Z, P, 12), { tone: "lo", crease: "none" });
    const rim = S(k.cylinder(x, y, KEY_R, top - 3, 1.4, P, 32), { tone: "hi", crease: "lo" });
    const cap = S(k.cylinder(x, y, KEY_R - 0.9, top - 1.6, 1.6, P, 32, 0.4), { tone: "mid" });
    const dish = L(k.ring(x, y, KEY_R - 2.4, top, P, 24), { tone: "faint" });
    return { char, svg: stem + rim + cap + dish };
  });
}

function spaceBar() {
  const arms = [SPACE.x + 16, SPACE.x + SPACE.w - 18].map((x) =>
    S(k.slabOf({ x, y: ROWS[3].y + 4, w: 2, d: SPACE.y - ROWS[3].y - 2, r: 0.6 }, DECK_Z, SPACE.z - DECK_Z - 2, P, 2), { tone: "lo" }),
  );
  return arms.join("") + S(k.slabOf(SPACE, SPACE.z - 2, 3.4, P, 4, 0.5), { tone: "hi", crease: "lo" });
}

const frontStatic = [basket(), spools(), ribbon(), cardHolder()].join("");
const defs = "";
const figure = {
  width: WIDTH,
  height: HEIGHT,
  label:
    "A desk typewriter on a bench plate, seen from the front right: a stepped body with forty round keys and a space bar, a fan of typebars in the basket, two ribbon spools, and a carriage with a rubber platen, knurled knobs, a scale bar, a paper bail and a return lever. The sheet in the carriage carries the test text.",
  back: [defs, bench(), copyHolder(), body(), rearHousing(), bell()].join(""),
  hammer: hammer(),
  carriage_back: carriageBack(),
  sheet: sheet(),
  carriage_front: carriageFront(),
  front: frontStatic,
  vibrator: vibrator(),
  mark: strikeMark(),
  keys: keys(),
  space: spaceBar(),
  strikes: strikes(),
  carriage_step: k.axisVector(P, [1, 0, 0]),
  key_down: k.axisVector(P, [0, 0, -2.2]),
  vibrator_lift: k.axisVector(P, [0, 0, 3.4]),
  hammer_swing: k.axisVector(P, [2.2, 0, 0.6]),
  paper_matrix: planeOf(SHEET_Y + 0.05),
  copy_matrix: planeOf(COPY.y + 3.05),
  copy: { x0: -PAPER_W / 2 + 14, cell: COPY.cell, font: COPY.font, baselines: [-(SHEET_TOP + 8 + COPY.line_h), -(SHEET_TOP + 8)] },
  crop_at: [at([-PAPER_W / 2 + 6, COPY.y, SHEET_TOP])[0] / WIDTH, at(STRIKE)[1] / HEIGHT],
  text: { x0: colLeft(0), cell: CELL, cols: COLS, wrap: WRAP, margin: MARGIN, line_h: LINE_H, font: FONT, baseline: -BASELINE_Z, top: -SHEET_TOP, bottom: -SHEET_BOTTOM, start_dx: dxOf(0) },
};

writeFileSync(OUT, JSON.stringify(figure));
writeFileSync(join(HERE, "..", "assets", "iso.css"), k.ISO_CSS);
console.log("wrote", OUT, `${(JSON.stringify(figure).length / 1024).toFixed(0)} KiB`);

if (process.argv.includes("--preview")) {
  const theme = process.argv.includes("--light") ? "light" : "dark";
  const col = Number(process.argv.find((a) => a.startsWith("--col="))?.slice(6) ?? 8);
  const [sx, sy] = figure.carriage_step;
  const dx = dxOf(col);
  const text = "the quick brown fox jumps over";
  const glyphs = [...text]
    .map((c, i) => `<text class="tw-ink${i < col ? "" : " tw-copy"}" x="${colLeft(i) + CELL / 2}" y="${-BASELINE_Z}" font-size="${FONT}" text-anchor="middle">${c === " " ? "" : c}</text>`)
    .join("");
  const svg = k.figureSvg({
    width: WIDTH,
    height: HEIGHT,
    label: figure.label,
    body: [
      figure.back,
      figure.hammer,
      `<g transform="translate(${(sx * dx).toFixed(2)} ${(sy * dx).toFixed(2)})">${figure.carriage_back}${figure.sheet}<g transform="${figure.paper_matrix}">${glyphs}</g>${figure.carriage_front}</g>`,
      figure.front,
      figure.vibrator,
      figure.mark,
      ...figure.keys.map((key) => key.svg),
      figure.space,
    ],
  });
  const css = `<style>${readFileSync(join(HERE, "..", "assets", "typewriter.css"), "utf8")}</style>`;
  const body = css + k.plateHtml({ fig: "Fig 1", title: "type-cast", hint: "Type to begin", readout: "0:30 · 0 wpm", keys: [], caption: "", body: svg });
  const name = theme === "light" ? "preview-light.html" : "preview.html";
  writeFileSync(join(HERE, name), k.pageHtml({ title: "type-cast preview", theme, body, script: "", width: 1100 }));
  console.log("wrote", name);
}

