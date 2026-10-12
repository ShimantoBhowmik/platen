---
version: 1
slug: "web-src-main-rs"
primary_target: "web/src/main.rs"
related_targets: []
---

# Typing test surface

Scope: the typing test screen (mode picker and live test). Mode: Operate. Build path: code-led.
Audience and job: a learner at a physical keyboard runs a 15-120 s or 10-100 word test, often
several in a row, and needs to read the text, type it, and see mistakes as they happen.
Constraints: Dioxus web (Rust/WASM); every number shown comes from the server or from the
same model that moves the machine; WCAG 2.2 AA; light and dark with a toggle; sound on by
default with a mute toggle; reduced motion holds the machine still.

## Direction contract

THESIS: the test is typed on a real machine. One isometric typewriter, drawn as an anatomy
instrument, owns the screen; the sheet in its carriage is the text. It refuses the category
default: a centred grey word strip with a blinking caret.

OWN-WORLD: charcoal bench (paper-white bench in light), four-tone monochrome solids with
hairline outlines, screws as dots, ruler ticks. Exactly one lit part: the strike point. Ink
is the only colour: black ribbon for correct, red half of a bichrome ribbon for errors.
Type is the machine's own slab-serif typewriter face on the sheet; small tracked mono for
the card's captions and controls.

STORY: the learner sees practice copy printed faint on the sheet, types over it, and the
machine answers each key: key sinks, typebar strikes, carriage escapes one pitch, red ink
marks a miss, the bell rings near the margin, and the return feeds a line.

FIRST VIEWPORT: a framed card fills the viewport. Top left "Fig 1", top right "type-cast".
The typewriter sits low and centred at a near-frontal camera so the sheet reads straight;
the current line runs through the fixed strike point at the centre, set large. Mode
controls, theme and sound toggles sit in a hairline rail above the card; the live readout
(time or words left, wpm, accuracy) sits at bottom right, the hint at bottom left.

FORM: pinned by the user: typewriter in the anatomy style, paper is the test; the roll's
assigned index 3 (a letterpress composing stick) is outranked by the pin. Seed key 1bfe4d3d.
Raises: from the Crouwel grid specimen, every character sits in a fixed pitch cell and the
carriage scale's ticks match the pitch; from the acetate manual, mechanical motion is
stepped, never floated: the escapement jumps one cell per key and a strike is two frames.

ADAPTATIONS (after the first finish review): the top-right caption names the object and the
test ("typewriter · 30s test"), as anatomy cards do; the brand sits in the rail. A copy holder
on a stand behind the sheet carries the next two lines of copy, so the learner can read ahead.
The camera sits at azimuth 80 so the sheet's baseline slopes about 5 degrees. On phones the
card crops to the carriage around the strike point.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review,
the verdict, DESIGN.md, and every shipping raster carrying its provenance
