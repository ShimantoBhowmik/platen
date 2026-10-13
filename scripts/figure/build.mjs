// Builds the typewriter figure for the web app.
//
// Run with `node scripts/figure/build.mjs`. It writes `web/assets/typewriter.json`: static SVG
// layers drawn with the vendored anatomy kit, plus the numbers the Rust app needs to move the
// type element, sink keys and set text on the sheet. Add `--preview` to also write
// `scripts/figure/preview.html`, a standalone page for visual and line-end checks.
//
// The machine is a typeball electric: the sheet and the platen stay still and a small type
// element travels along the platen under the column being typed. The sheet shows three lines
// of copy at once, and the keyboard is drawn at a real machine's scale against the paper.
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import * as k from "./kit/iso-kit.mjs";
import * as G from "./kit/lathe.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));
const ASSETS = join(HERE, "..", "..", "web", "assets");
const OUT = join(ASSETS, "typewriter.json");

// Type on the sheet: one cell per character. 60 cells across the same paper width make an
// eleven-word line of common words fill most of the sheet.
const CELL = 1000 / 60;
const COLS = 60;
const LINE_H = 43;
const FONT = 28;
const MARGIN = 18;
// Lines of copy shown on the sheet at once.
const ROWS = 3;
// Lines are laid out a few cells short of the paper, so extra letters never push copy off it.
const WRAP = COLS - 4;
const PAPER_W = COLS * CELL + MARGIN * 2;

// Platen, the strike point on the card holder below the copy, and the three rows of copy.
const PLATEN = { y: -70, z: 86, r: 15, half: PAPER_W / 2 + 26 };
const SHEET_Y = PLATEN.y + PLATEN.r + 0.6;
const PRINT_Z = PLATEN.z;
const BAIL_Z = PLATEN.z + PLATEN.r + 2;
const LAST_BASELINE_Z = BAIL_Z + 18;
const FIRST_BASELINE_Z = LAST_BASELINE_Z + LINE_H * (ROWS - 1);
const SHEET_TOP = FIRST_BASELINE_Z + FONT + 18;
const SHEET_BOTTOM = PRINT_Z - 14;

// Keyboard: four staggered rows of round keys on stems, rising toward the back, at a real
// machine's scale against the sheet: the rows span most of the paper's width.
const KEY_PITCH = 68;
const KEY_R = 25;
const KEY_ROW = 62;
const KEY_ROWS = ["1234567890", "qwertyuiop", "asdfghjkl;", "zxcvbnm,./"].map((chars, i) => ({
  chars,
  y: 52 + i * KEY_ROW,
  x0: -4.5 * KEY_PITCH - 20 + i * 18,
  top: 78 - i * 8,
}));
const SPACE = { x: -230, y: 52 + 4 * KEY_ROW - 6, w: 460, d: 22, r: 10, z: 48 };

// Body and bench: as wide as the platen, since the type element travels its whole length.
const BODY_HALF = PLATEN.half + 34;
const BODY = { x: -BODY_HALF, y: -96, w: BODY_HALF * 2, d: SPACE.y + SPACE.d + 128, r: 22 };
const BASE = { x: -BODY_HALF - 26, y: -120, w: BODY_HALF * 2 + 52, d: BODY.d + 52, r: 14 };
const BASE_H = 9;
const FOOT_H = 5;
const BODY_H = 30;
const REAR = { x: BODY.x, y: -96, w: BODY.w, d: 36, r: 12 };
const REAR_TOP = 66;
const DECK_Z = BODY_H + 4;

// The type element's carrier rides two rails between the platen and the keyboard. It is drawn
// at x = 0 and moved along x to each column.
const RAILS = [
  { y: -20, z: 50, r: 2.6 },
  { y: -32, z: 46, r: 1.8 },
];
const BALL = { y: -42, z: PRINT_Z - 17, r: 8.5 };

const WIDTH = 1200;
const AZIMUTH = 90;
const PAD = 6;

const colLeft = (c) => -PAPER_W / 2 + MARGIN + c * CELL;

// Fit the figure to the width, then size the viewBox height to the drawing.
const extent = [
  ...k.boxCorners(BASE, -BASE_H - FOOT_H, 0),
  [-PAPER_W / 2, SHEET_Y, SHEET_TOP],
  [PAPER_W / 2, SHEET_Y, SHEET_TOP],
];
const HEIGHT = (() => {
  const unit = { origin: [0, 0], scale: 1, azimuth: AZIMUTH };
  const flat = extent.map((point) => k.iso(point, unit));
  const span = (axis) => Math.max(...flat.map((p) => p[axis])) - Math.min(...flat.map((p) => p[axis]));
  return Math.ceil(((WIDTH - PAD * 2) * span(1)) / span(0) + PAD * 2);
})();
const P = k.fitProjection(extent, WIDTH, HEIGHT, { pad: PAD, azimuth: AZIMUTH });

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

  const strip = { x: BASE.x + 30, y: BASE.y + BASE.d - 17, w: 260, d: 7, r: 1 };
  out.push(S(k.slabOf(strip, 0, 0.6, P, 3), { tone: "lo", crease: "none" }));
  const ticks = k.topTicks(strip.x + 5, strip.x + strip.w - 5, CELL / 2, 10, strip.y + 0.8, 0.6, [2, 3.8], P);
  out.push(L(ticks.minor, { tone: "lo" }), L(ticks.major, { tone: "mid" }));

  const label = { x: BASE.x + BASE.w - 84, y: BASE.y + BASE.d - 21, w: 56, d: 12, r: 1.4 };
  out.push(S(k.slabOf(label, 0, 0.8, P, 3), { tone: "lo", crease: "none" }));
  out.push(L(k.planOutline(k.insetPlan(label, 1.6), 0.8, P, 3), { tone: "faint" }));
  out.push(L([38, 28, 20].map((w, i) => k.lineOnTop([label.x + 4, label.y + 3 + i * 3], [label.x + 4 + w, label.y + 3 + i * 3], 0.8, P)).join(""), { tone: "lo", free: true }));
  out.push(D([at([label.x + label.w - 4, label.y + 3.5, 0.8]), at([label.x + label.w - 4, label.y + label.d - 3.5, 0.8])], { size: 0.45 }));
  return out.join("");
}

function body() {
  const out = [`<path class="iso-halo" d="${k.haloOf(BODY, 0, 10, P)}"/>`];
  for (const [x, y] of byDepth(k.corners(BODY, 22))) {
    out.push(S(k.cylinder(x, y, 7, 0, 3, P, 24), { tone: "lo" }));
  }
  const seam = k.sideSeam(BODY, 14, P, 8);
  out.push(S(k.slabOf(BODY, 3, BODY_H - 3, P, 8, 1.4), { tone: "mid", inner: L(seam, { tone: "faint" }) }));
  // Front apron with the engraved name plate.
  const front = BODY.y + BODY.d;
  out.push(`<g transform="${planeOf(front)}">${L(flatRect(-56, 9, 56, 24, 2), { tone: "lo" })}${L(flatRect(-53, 11.5, 53, 21.5, 1.4), { tone: "faint" })}</g>`);
  out.push(k.faceTextSvg(planeOf(front), "PLATEN", { size: 6, tone: "mid", anchor: "middle" }).replace("<text ", `<text x="0" y="-14.4" `));
  out.push(D([at([-61, front, 16.5]), at([61, front, 16.5])], { size: 0.5, tone: "mid" }));
  // Grilles on the apron either side of the name plate, over the motor and the drive.
  for (const side of [-1, 1]) {
    const slots = Array.from({ length: 14 }, (_, i) => {
      const x = side * (BODY_HALF - 70 - i * 9);
      return flatRect(x - 1.2, 9, x + 1.2, 23, 1.2, 3);
    });
    out.push(`<g transform="${planeOf(front)}">${L(slots.join(""), { tone: "faint" })}</g>`);
  }
  // Deck plate the keys and the carrier rails sit on.
  const deck = { x: BODY.x + 10, y: -60, w: BODY.w - 20, d: BODY.d - 60, r: 14 };
  out.push(S(k.slabOf(deck, BODY_H, DECK_Z - BODY_H, P, 6, 0.8), { tone: "mid" }));
  out.push(L(k.planOutline(k.insetPlan(deck, 4), DECK_Z, P, 6), { tone: "faint" }));
  out.push(D(k.corners(deck, 7).map(([x, y]) => at([x, y, DECK_Z])), { size: 0.5 }));
  // Keyboard well: a recessed panel framing the keys.
  const well = { x: -6.5 * KEY_PITCH, y: KEY_ROWS[0].y - KEY_R - 14, w: 13 * KEY_PITCH, d: SPACE.y + SPACE.d + 18 - (KEY_ROWS[0].y - KEY_R - 14), r: 16 };
  out.push(L(k.planOutline(well, DECK_Z, P, 6), { tone: "lo" }));
  // Side cheeks at each end that carry the rails, and the controls on their tops.
  for (const x of [BODY.x + 10, BODY.x + BODY.w - 40]) {
    const cheek = { x, y: -60, w: 30, d: 74, r: 9 };
    out.push(S(k.slabOf(cheek, DECK_Z, 26, P, 6, 1), { tone: "mid", inner: L(k.sideSeam(cheek, DECK_Z + 9, P, 6), { tone: "faint" }) }));
    out.push(L(k.planOutline(k.insetPlan(cheek, 3.4), DECK_Z + 26, P, 6), { tone: "faint" }));
    out.push(D([at([x + 15, -52, DECK_Z + 26]), at([x + 15, 6, DECK_Z + 26])], { size: 0.5 }));
  }
  // Power rocker on the right of the deck, in front of the cheek.
  const rocker = { x: BODY.x + BODY.w - 84, y: 60, w: 40, d: 20, r: 3 };
  out.push(S(k.slabOf(rocker, DECK_Z, 3, P, 4, 0.4), { tone: "hi", crease: "lo" }));
  out.push(L(k.lineOnTop([rocker.x + rocker.w / 2, rocker.y + 3], [rocker.x + rocker.w / 2, rocker.y + rocker.d - 3], DECK_Z + 3, P), { tone: "lo" }));
  // Tab and margin release keys on the left, a pair of small square caps.
  for (const x of [BODY.x + 44, BODY.x + 78]) {
    const cap = { x, y: 60, w: 26, d: 18, r: 2.4 };
    out.push(S(k.slabOf(cap, DECK_Z, 4, P, 4, 0.4), { tone: "hi", crease: "lo" }));
  }
  return out.join("");
}

function rearHousing() {
  const out = [];
  out.push(S(k.slabOf(REAR, BODY_H, REAR_TOP - BODY_H, P, 8, 1.2), { tone: "mid", inner: L(k.sideSeam(REAR, BODY_H + 12, P, 8), { tone: "faint" }) }));
  const top = k.insetPlan(REAR, 5);
  out.push(L(k.planOutline(top, REAR_TOP, P, 6), { tone: "lo" }));
  out.push(D(k.corners(REAR, 8).map(([x, y]) => at([x, y, REAR_TOP])), { size: 0.5 }));
  // Margin stops on a toothed rack along the front of the housing top.
  const rack = { x: colLeft(0) - 24, y: -70, w: COLS * CELL + 48, d: 4, r: 1 };
  out.push(S(k.slabOf(rack, REAR_TOP, 1.4, P, 3), { tone: "lo", crease: "none" }));
  const teeth = k.topTicks(rack.x + 2, rack.x + rack.w - 2, CELL / 2, 2, rack.y + 0.4, REAR_TOP + 1.4, [1.6, 3], P);
  out.push(L(teeth.minor + teeth.major, { tone: "faint" }));
  for (const x of [colLeft(0) - 4, colLeft(COLS) + 1]) {
    out.push(S(k.slabOf({ x, y: rack.y - 1, w: 4, d: 6, r: 0.8 }, REAR_TOP + 1.4, 3.4, P, 3, 0.3), { tone: "hi", crease: "lo" }));
  }
  return out.join("");
}

function bell() {
  const x = BODY_HALF - 60;
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
  const pivot = [BODY_HALF - 72, -79, REAR_TOP + 2];
  return [
    S(k.cylinder(pivot[0], pivot[1], 1.6, REAR_TOP, 3, P, 12), { tone: "mid" }),
    S(rod([pivot[0], pivot[1], REAR_TOP + 3], [pivot[0] + 9.5, -79, REAR_TOP + 7], 0.7), { tone: "mid" }),
    S(G.sphereOf([pivot[0] + 9.6, -79, REAR_TOP + 7.2], 1.5, P), { tone: "hi" }),
  ].join("");
}

function platen() {
  const out = [];
  const plates = [-PLATEN.half - 16, PLATEN.half + 9];
  out.push(endPlate(plates[0]));
  // Platen: a rubber roller with its end bands, then the knobs.
  const F = along(-PLATEN.half, PLATEN.y, PLATEN.z);
  out.push(S(G.disc(-3, 0, PLATEN.r - 1.5, F, P), { tone: "mid" }));
  out.push(S(G.disc(0, PLATEN.half * 2, PLATEN.r, F, P, { smooth: true }), { tone: "lo", crease: "none" }));
  out.push(S(G.disc(PLATEN.half * 2, PLATEN.half * 2 + 3, PLATEN.r - 1.5, F, P), { tone: "mid" }));
  out.push(knob(-PLATEN.half - 3, -1));
  out.push(endPlate(plates[1]));
  out.push(knob(PLATEN.half + 3, 1));
  // Scale bar across the front, below the sheet: one tick per pitch cell hanging from its top
  // edge on the face toward the typist.
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
  out.push(S(k.slabOf(plate, REAR_TOP, PLATEN.z + 20 - REAR_TOP, P, 4, 0.6), { tone: "mid" }));
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

function sheet() {
  const x0 = -PAPER_W / 2;
  const shape = flatRect(x0, SHEET_BOTTOM, x0 + PAPER_W, SHEET_TOP, 0.6, 2);
  // The sheet curls over the platen at the bottom: a shade band where it turns away.
  const curl = flatRect(x0, SHEET_BOTTOM, x0 + PAPER_W, PRINT_Z - 9, 0, 2);
  return `<g transform="${planeOf(SHEET_Y)}"><path class="tw-sheet" d="${shape}"/><path class="tw-sheet-shade" d="${curl}"/><path class="tw-sheet-edge" d="${shape}"/></g>`;
}

function bail() {
  const out = [];
  // Paper bail: a rod across the sheet between two printed lines, on arms to the end plates.
  const bailZ = BAIL_Z;
  const y = SHEET_Y + 2.2;
  for (const side of [-1, 1]) {
    out.push(S(rod([side * (PLATEN.half + 12), PLATEN.y + 6, PLATEN.z + 12], [side * (PLATEN.half - 2), y, bailZ], 1.1), { tone: "mid" }));
  }
  const F = along(-PLATEN.half, y, bailZ);
  out.push(S(G.disc(0, PLATEN.half * 2, 1.4, F, P), { tone: "hi", crease: "lo" }));
  for (const x of [-PAPER_W / 3, 0, PAPER_W / 3]) {
    out.push(S(G.disc(x + PLATEN.half - 7, x + PLATEN.half + 7, 3.2, F, P, { smooth: true }), { tone: "lo" }));
  }
  return out.join("");
}

function rails() {
  // The carrier's two rails, from cheek to cheek: a round guide rod and the thinner print shaft.
  const ends = [BODY.x + 40, BODY.x + BODY.w - 40];
  return byDepth(RAILS.map((r) => [0, r.y, r.z]))
    .map(([, y, z]) => {
      const r = RAILS.find((rail) => rail.y === y).r;
      return S(G.disc(0, ends[1] - ends[0], r, along(ends[0], y, z), P, { smooth: true }), { tone: "hi", crease: "lo" });
    })
    .join("");
}

function carrier() {
  const out = [];
  // Carrier block riding the rails, with the ribbon cartridge on top behind the element.
  const block = { x: -30, y: -38, w: 60, d: 26, r: 4 };
  out.push(S(k.slabOf(block, 42, 14, P, 4, 0.6), { tone: "mid", inner: L(k.sideSeam(block, 49, P, 4), { tone: "faint" }) }));
  out.push(D([at([-24, -12, 52]), at([24, -12, 52])], { size: 0.5 }));
  const cartridge = { x: -40, y: -58, w: 80, d: 14, r: 3 };
  out.push(S(k.slabOf(cartridge, 56, 9, P, 4, 0.6), { tone: "hi", crease: "lo" }));
  out.push(L(k.planOutline(k.insetPlan(cartridge, 3), 65, P, 4), { tone: "faint" }));
  // Ribbon: black above red, from the cartridge's two ends up to the guide at the print point.
  const run = (x0, x1, y0, y1, z0, z1, h) =>
    k.pathOf([at([x0, y0, z0]), at([x1, y1, z1]), at([x1, y1, z1 - h]), at([x0, y0, z0 - h])], true);
  for (const side of [-1, 1]) {
    const black = run(side * 34, side * 15, -54, SHEET_Y + 3, 65, PRINT_Z - 7, 1.1);
    const red = run(side * 34, side * 15, -54, SHEET_Y + 3, 63.9, PRINT_Z - 8.1, 1.0);
    out.push(`<path class="tw-ribbon" d="${black}"/><path class="tw-ribbon-red" d="${red}"/>`);
  }
  // Card holder: the plate with a notch that points at the print point.
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

function ball() {
  // The type element on its post: a sphere with rows of moulded characters drawn as dots.
  const out = [S(k.cylinder(0, BALL.y, 2.4, 56, BALL.z - BALL.r - 54, P, 16), { tone: "lo", crease: "none" })];
  out.push(S(G.sphereOf([0, BALL.y, BALL.z], BALL.r, P), { tone: "mid" }));
  const view = G.viewOf(P);
  const glyphs = [];
  for (let row = -2; row <= 2; row++) {
    const lat = (row * Math.PI) / 7;
    for (let i = 0; i < 22; i++) {
      const lon = (i / 22) * Math.PI * 2;
      const n = [Math.cos(lat) * Math.cos(lon), Math.cos(lat) * Math.sin(lon), Math.sin(lat)];
      const facing = G.dot3(n, view);
      if (facing > 0.25) {
        glyphs.push([...at([n[0] * BALL.r, BALL.y + n[1] * BALL.r, BALL.z + n[2] * BALL.r]), Math.min(1, facing * 1.4)]);
      }
    }
  }
  out.push(D(glyphs, { size: 0.45, tone: "lo" }));
  out.push(S(k.cylinder(0, BALL.y, 3, BALL.z + BALL.r - 1.2, 2.4, P, 20), { tone: "hi", crease: "lo" }));
  return out.join("");
}

function keys() {
  const list = [];
  for (const row of KEY_ROWS) {
    [...row.chars].forEach((char, i) => {
      const x = row.x0 + i * KEY_PITCH;
      list.push({ char, x, y: row.y, top: row.top });
    });
  }
  const [r0, , r2, r3] = KEY_ROWS;
  const extra = [
    { char: "Shift", label: "shift", x: r3.x0 - KEY_PITCH - 6, y: r3.y, top: r3.top },
    { char: "ShiftRight", label: "shift", x: r3.x0 + 10 * KEY_PITCH + 6, y: r3.y, top: r3.top },
    { char: "Backspace", label: "back", x: r0.x0 + 10 * KEY_PITCH + 4, y: r0.y, top: r0.top },
    { char: "Enter", label: "return", x: r2.x0 + 10 * KEY_PITCH + 6, y: r2.y, top: r2.top },
  ];
  const all = [...list, ...extra].sort((a, b) => depth([a.x, a.y, a.top]) - depth([b.x, b.y, b.top]));
  return all.map(({ char, label, x, y, top }) => {
    const stem = S(k.cylinder(x, y, 3.4, DECK_Z, top - 4 - DECK_Z, P, 16), { tone: "lo", crease: "none" });
    const rim = S(k.cylinder(x, y, KEY_R, top - 4, 2.4, P, 48), { tone: "hi", crease: "lo" });
    const cap = S(k.cylinder(x, y, KEY_R - 2, top - 2, 2, P, 48, 0.6), { tone: "mid" });
    const dish = L(k.ring(x, y, KEY_R - 6, top, P, 40), { tone: "faint" });
    // The legend is printed on the key top, so it lies in the top plane and sinks with the key.
    const text = label ?? char.toUpperCase();
    const size = label ? 9 : 19;
    const legend = `<text class="tw-key-label" transform="${k.topMatrix([x, y], top, P)}" font-size="${size}" text-anchor="middle" y="${size * 0.36}">${text}</text>`;
    return { char, svg: stem + rim + cap + dish + legend };
  });
}

function spaceBar() {
  const last = KEY_ROWS[KEY_ROWS.length - 1];
  const arms = [SPACE.x + 40, SPACE.x + SPACE.w - 44].map((x) =>
    S(k.slabOf({ x, y: last.y + 10, w: 4, d: SPACE.y - last.y - 8, r: 1 }, DECK_Z, SPACE.z - DECK_Z - 3, P, 2), { tone: "lo" }),
  );
  return arms.join("") + S(k.slabOf(SPACE, SPACE.z - 3, 5, P, 6, 0.8), { tone: "hi", crease: "lo" });
}

const figure = {
  width: WIDTH,
  height: HEIGHT,
  label:
    "A wide electric typewriter on a bench plate, seen from the front: a long body with forty round keys and a space bar, a rubber platen with knurled knobs, a scale bar and a paper bail, and a type element on a carrier that travels along the platen. The sheet in the platen carries the test text.",
  back: [bench(), body(), rearHousing(), bell()].join(""),
  hammer: hammer(),
  platen: platen(),
  sheet: sheet(),
  bail: bail(),
  rails: rails(),
  carrier: carrier(),
  vibrator: vibrator(),
  mark: strikeMark(),
  ball: ball(),
  keys: keys(),
  space: spaceBar(),
  carrier_step: k.axisVector(P, [1, 0, 0]),
  key_down: k.axisVector(P, [0, 0, -2.2]),
  vibrator_lift: k.axisVector(P, [0, 0, 3.4]),
  ball_strike: k.axisVector(P, [0, -3.2, 1]),
  hammer_swing: k.axisVector(P, [2.2, 0, 0.6]),
  paper_matrix: planeOf(SHEET_Y + 0.05),
  strike_at: [at([0, SHEET_Y, PRINT_Z])[0] / WIDTH, at([0, SHEET_Y, FIRST_BASELINE_Z - LINE_H])[1] / HEIGHT],
  text: {
    x0: colLeft(0),
    cell: CELL,
    cols: COLS,
    wrap: WRAP,
    margin: MARGIN,
    line_h: LINE_H,
    font: FONT,
    rows: ROWS,
    baseline: -FIRST_BASELINE_Z,
    top: -(FIRST_BASELINE_Z + FONT - 2),
    bottom: -(LAST_BASELINE_Z - 8),
  },
};

writeFileSync(OUT, JSON.stringify(figure));
writeFileSync(join(ASSETS, "iso.css"), k.ISO_CSS);
console.log("wrote", OUT, `${(JSON.stringify(figure).length / 1024).toFixed(0)} KiB`, `${WIDTH}x${HEIGHT}`);

if (process.argv.includes("--preview")) {
  const theme = process.argv.includes("--light") ? "light" : "dark";
  const col = Number(process.argv.find((a) => a.startsWith("--col="))?.slice(6) ?? 30);
  const [sx, sy] = figure.carrier_step;
  const x = colLeft(col) + CELL / 2;
  const text = "the quick brown fox jumps over the lazy dog while seven people watch from far away";
  const glyphs = [0, 1, 2]
    .flatMap((row) =>
      [...text.slice(0, WRAP)].map(
        (c, i) =>
          `<text class="tw-ink${row === 0 && i < col ? "" : " tw-copy"}" x="${colLeft(i) + CELL / 2}" y="${-FIRST_BASELINE_Z + row * LINE_H}" font-size="${FONT}" text-anchor="middle">${c === " " ? "" : c}</text>`,
      ),
    )
    .join("");
  const svg = k.figureSvg({
    width: WIDTH,
    height: HEIGHT,
    label: figure.label,
    body: [
      figure.back,
      figure.hammer,
      figure.platen,
      figure.sheet,
      `<g transform="${figure.paper_matrix}">${glyphs}</g>`,
      figure.bail,
      figure.rails,
      `<g transform="translate(${(sx * x).toFixed(2)} ${(sy * x).toFixed(2)})">${figure.carrier}${figure.vibrator}${figure.mark}${figure.ball}</g>`,
      ...figure.keys.map((key) => key.svg),
      figure.space,
    ],
  });
  const css = `<style>${readFileSync(join(ASSETS, "typewriter.css"), "utf8")}</style>`;
  const body = css + k.plateHtml({ fig: "", title: "", hint: "Type to begin", readout: "ready", keys: [], caption: "", body: svg });
  const name = theme === "light" ? "preview-light.html" : "preview.html";
  writeFileSync(join(HERE, name), k.pageHtml({ title: "platen preview", theme, body, script: "", width: 1240 }));
  console.log("wrote", name);
}
