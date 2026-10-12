---
name: type-cast
description: A typing test typed on one isometric typewriter drawn as an anatomy instrument.
colors:
  bench: "#0b0b0b"
  card: "#131313"
  face: "#1b1b1b"
  top: "#1c1c1c"
  shade-0: "#0f0f0f"
  shade-3: "#181818"
  text: "#a3a3a3"
  strong: "#e6e6e6"
  line-hi: "#d4d4d4"
  line-mid: "#6a6a6a"
  line-lo: "#3e3e3e"
  line-faint: "#2a2a2a"
  lit: "#ffffff"
  sheet: "#e8e6e0"
  sheet-shade: "#d9d6cf"
  sheet-edge: "#8e8b84"
  ink: "#161618"
  copy: "#5f5c56"
  ribbon: "#050506"
  ribbon-red: "#b02621"
  ribbon-red-plate: "#e5554c"
  bench-light: "#f2f2ef"
  card-light: "#fbfbfa"
  face-light: "#ffffff"
  shade-0-light: "#dcdcd9"
  text-light: "#57575a"
  strong-light: "#18181a"
  line-hi-light: "#26262a"
  line-mid-light: "#8a8a8f"
  line-lo-light: "#b4b4b8"
  line-faint-light: "#d4d4d6"
  lit-light: "#000000"
  sheet-light: "#ffffff"
  sheet-shade-light: "#f1f1ee"
  sheet-edge-light: "#a9a9a6"
  ink-light: "#121214"
  copy-light: "#6b6964"
  ribbon-light: "#232326"
  ribbon-red-plate-light: "#b02621"
typography:
  sheet-type:
    fontFamily: "Courier Prime, Courier New, Courier, monospace"
    fontSize: "21px"
    fontWeight: 700
  sheet-copy:
    fontFamily: "Courier Prime, Courier New, Courier, monospace"
    fontSize: "21px"
    fontWeight: 400
  holder-copy:
    fontFamily: "Courier Prime, Courier New, Courier, monospace"
    fontSize: "17px"
    fontWeight: 400
  label:
    fontFamily: "ui-monospace, SF Mono, Menlo, Consolas, monospace"
    fontSize: "12px"
    fontWeight: 400
    letterSpacing: "0.08em"
  brand:
    fontFamily: "ui-monospace, SF Mono, Menlo, Consolas, monospace"
    fontSize: "12px"
    fontWeight: 400
    letterSpacing: "0.14em"
  caption:
    fontFamily: "ui-monospace, SF Mono, Menlo, Consolas, monospace"
    fontSize: "12px"
    fontWeight: 400
    lineHeight: 1
    letterSpacing: "0.1em"
  readout:
    fontFamily: "ui-monospace, SF Mono, Menlo, Consolas, monospace"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1
    letterSpacing: "0.04em"
    fontFeature: "tnum"
  legend:
    fontFamily: "ui-monospace, SF Mono, Menlo, Consolas, monospace"
    fontSize: "12px"
    fontWeight: 400
    lineHeight: 1.55
    letterSpacing: "0.04em"
  note:
    fontFamily: "ui-monospace, SF Mono, Menlo, Consolas, monospace"
    fontSize: "11px"
    fontWeight: 400
    lineHeight: 1.3
    fontFeature: "tnum"
  result-figure:
    fontFamily: "Courier Prime, Courier New, Courier, monospace"
    fontSize: "clamp(52px, 9cqw, 112px)"
    fontWeight: 700
    lineHeight: 0.85
    letterSpacing: "-0.03em"
    fontFeature: "tnum"
  countdown:
    fontFamily: "Courier Prime, Courier New, Courier, monospace"
    fontSize: "clamp(40px, 7cqw, 88px)"
    fontWeight: 700
    lineHeight: 1
    letterSpacing: "-0.02em"
    fontFeature: "tnum"
  countdown-phone:
    fontFamily: "Courier Prime, Courier New, Courier, monospace"
    fontSize: "36px"
    fontWeight: 700
    lineHeight: 1
    letterSpacing: "-0.02em"
    fontFeature: "tnum"
  stat-value:
    fontFamily: "Courier Prime, Courier New, Courier, monospace"
    fontSize: "26px"
    fontWeight: 700
    lineHeight: 1.1
    fontFeature: "tnum"
  stat-value-phone:
    fontFamily: "Courier Prime, Courier New, Courier, monospace"
    fontSize: "20px"
    fontWeight: 700
    lineHeight: 1.1
    fontFeature: "tnum"
  missed-word:
    fontFamily: "Courier Prime, Courier New, Courier, monospace"
    fontSize: "17px"
    fontWeight: 700
    lineHeight: 1
  keycap:
    fontFamily: "Courier Prime, Courier New, Courier, monospace"
    fontSize: "16px"
    fontWeight: 700
    lineHeight: 1
  keycap-wide:
    fontFamily: "Courier Prime, Courier New, Courier, monospace"
    fontSize: "13px"
    fontWeight: 700
    lineHeight: 1
rounded:
  none: "0px"
  hair: "1px"
  chip: "2px"
  keycap: "17px"
spacing:
  chip-gap: "4px"
  rail-row: "10px"
  page-gap: "14px"
  corner-inset: "18px 20px"
  page: "18px 24px 22px"
  rail-group: "28px"
  plate: "44px 24px"
  result-gap: "26px"
  section-rule: "18px"
  stats-gap: "16px"
  practise-gap: "32px"
  phone-count-band: "86px"
components:
  chip:
    backgroundColor: "transparent"
    textColor: "{colors.text}"
    typography: "{typography.label}"
    rounded: "{rounded.chip}"
    padding: "6px 10px"
  chip-hover:
    textColor: "{colors.strong}"
  chip-pressed:
    backgroundColor: "{colors.face}"
    textColor: "{colors.strong}"
  plate:
    backgroundColor: "{colors.card}"
    rounded: "{rounded.none}"
    padding: "44px 24px"
  plate-caption:
    textColor: "{colors.text}"
    typography: "{typography.caption}"
  legend-swatch:
    rounded: "{rounded.hair}"
    width: "18px"
    height: "10px"
  countdown:
    textColor: "{colors.text}"
    typography: "{typography.countdown}"
  countdown-live:
    textColor: "{colors.strong}"
  result-figure:
    textColor: "{colors.strong}"
    typography: "{typography.result-figure}"
  stat-value:
    textColor: "{colors.strong}"
    typography: "{typography.stat-value}"
  stat-wrong:
    textColor: "{colors.ribbon-red-plate}"
  keycap:
    backgroundColor: "{colors.face}"
    textColor: "{colors.strong}"
    typography: "{typography.keycap}"
    rounded: "{rounded.keycap}"
    padding: "0 10px"
    height: "34px"
  missed-word:
    textColor: "{colors.ribbon-red-plate}"
    typography: "{typography.missed-word}"
  chart-tip:
    backgroundColor: "{colors.card}"
    textColor: "{colors.strong}"
    typography: "{typography.note}"
    padding: "7px 10px"
  focus-prompt:
    backgroundColor: "{colors.card}"
    textColor: "{colors.strong}"
    typography: "{typography.caption}"
    padding: "8px 14px"
---

# Design System: type-cast

## Overview

**Creative North Star: "The Anatomy Plate"**

One typewriter, drawn as a technical figure on a framed card, is the whole interface. The sheet rolled into its carriage is the test: the copy is printed faint on the paper, the learner types over it, and the machine answers each key with a stepped mechanical response. Everything outside the figure is set like a plate in a manual: a hairline frame, four corner captions, a legend under the card, and a single rail of controls above it that wears the same caption type. When the server returns the score, the same card turns to Fig 2, the result plate: the machine's figures, a hairline speed chart, a stats row and what to practise next.

The world is monochrome. The bench, card and machine are four-tone grey solids with 0.6px hairline outlines, screws drawn as dots, and ruler ticks cut to the type pitch. Colour exists only as ribbon ink: a black ribbon for correct letters and the red half of a bichrome ribbon for misses, on the paper and, lifted for contrast, on the result plate. The only lit part of the drawing is the strike-point mark. Dark (charcoal bench) and light (paper-white bench) are the same drawing with each tone remapped; the theme follows the system preference and has a toggle.

Density is low and fixed. The card holds the drawing's 3:2 shape inside whatever height the rail and legend leave, so the machine never scrolls on desktop, and Fig 2 keeps that shape so nothing jumps when it swaps in. Motion is mechanical, not decorative: the escapement jumps one cell with no easing, a typebar strike holds for two frames, and only the carriage return and line feed glide. The result chart is drawn once like a pen trace.

**Key Characteristics:**
- One isometric anatomy figure owns the screen; the paper is the test surface.
- Four-tone monochrome solids, 0.6px hairlines, dots and ticks; no hue outside the ribbon's ink.
- One lit part: the strike-point mark.
- Courier Prime on the paper and for the machine's figures; small tracked system mono for every caption, control and axis.
- Square plate, near-square 2px chips, round 17px key discs, flat surfaces.
- Stepped motion; reduced motion holds the machine still and draws the chart at once.

## Colors

A neutral grey bench and machine with paper and ribbon ink as the only content colours; every token has a dark value and a light override.

### Primary
- **Ribbon Red** (`ribbon-red`): the red half of the bichrome ribbon on paper. Marks a missed or extra letter on the sheet, the underscore under a skipped letter, the ribbon run in the drawing, and the "wrong" legend swatch. Same value in both themes.
- **Ribbon Red, Plate** (`ribbon-red-plate`, `ribbon-red-plate-light`): the same red ink off the paper, on the card. Lifted in dark so marks and text keep AA contrast on the charcoal card; equal to `ribbon-red` in light. Used only for wrong keys on Fig 2: the chart's error marks, the error axis and its "err" unit and counts, the wrong-keys count in the stats row, the typed words in "words left wrong", and the error mark in the chart legend.
- **Ribbon Ink** (`ink`, `ink-light`): the black ribbon. Correctly typed letters, at weight 700.

### Neutral
- **Charcoal Bench / Paper Bench** (`bench`, `bench-light`): the page behind the card, also painted on `html` and `body`.
- **Plate Card** (`card`, `card-light`): the framed card, the focus prompt and the chart's hover tip. Slightly lifted from the bench by tone, not by shadow.
- **Lit Face and Top** (`face`, `top`, `face-light`): top faces of the machine's solids, the pressed chip fill and the weak-key disc face. `shade-0` through `shade-3` are the side faces.
- **Caption Grey** (`text`, `text-light`): rail labels, unpressed chips, plate captions, legend text, the countdown before typing starts, chart axis labels, result labels and the small notes under figures.
- **Strong** (`strong`, `strong-light`): the brand, pressed and hovered chips, the readout when a test is done, the focus prompt, the running countdown, every value on Fig 2, the wpm line and its dots.
- **Hairline Tones** (`line-hi`, `line-mid`, `line-lo`, `line-faint` and light pairs): outline weights of the drawing. `line-mid` also draws the plate frame, the hover ring on chips, the chart's raw line, seconds ruler and hover guide, and the rings of the key discs and the tip; `line-faint` is the chip's resting ring, the chart's wpm gridlines and the rules above the stats row and the practise row; `line-hi` is the pressed ring, the focus outline and the selection fill.
- **Strike Light** (`lit`, `lit-light`): the strike-point mark only. White on charcoal, black on paper.
- **Sheet** (`sheet`, `sheet-shade`, `sheet-edge` and light pairs): the paper in the carriage and on the copy holder, its shaded back and its edge stroke.
- **Faint Copy** (`copy`, `copy-light`): the practice text still to type and the copy holder's read-ahead lines, at weight 400.
- **Ribbon Body** (`ribbon`, `ribbon-light`): the black half of the ribbon in the drawing.

### Named Rules
**The Ink Is The Only Colour Rule.** Hue appears only as ribbon ink: `ribbon-red` on paper, and `ribbon-red-plate` on the card, where it marks wrong keys and nothing else. The bench, card, machine, controls, captions and every other chart line stay on the grey ramp. The vendored kit's red, green and blue line tones are not part of this world.

**The One Lit Part Rule.** The strike-point mark is the only element painted with `lit`. No other solid in the figure is drawn lit, so the eye always finds the print point.

**The Paired Theme Rule.** Every colour is declared twice, dark and light, on the same token name; a new tone without a light override is incomplete.

## Typography

**Paper and Figure Font:** Courier Prime (with Courier New, Courier, monospace), loaded from Google Fonts at 400 and 700.
**Caption and Control Font:** ui-monospace (with SF Mono, Menlo, Consolas, monospace).

**Character:** The machine's own slab-serif typewriter face sets everything on paper and every figure the machine reports; a quiet system mono, small, uppercase and tracked, sets everything around them like a figure's annotations.

### Hierarchy
- **Result Figure** (700, `clamp(52px, 9cqw, 112px)` on the card's width, line-height 0.85, -0.03em, tabular): wpm and acc on Fig 2. The "%" unit drops to half size in `text`.
- **Countdown** (700, `clamp(40px, 7cqw, 88px)`, line-height 1, -0.02em, tabular; 36px on phones): seconds left, or words done with "/total" at 0.4em in `text`.
- **Stat Value** (700, 26px, line-height 1.1, tabular; 20px on phones): the five values in the stats row.
- **Sheet Type** (700, 21 drawing units on a 12.5-unit pitch cell, 23-unit line): letters typed right on the current line, set large through the fixed strike point.
- **Sheet Copy** (400, same size and pitch): letters still to type, and the copy left visible under a miss.
- **Holder Copy** (400, 17 units, 21-unit line, pitch scaled to match): the next two lines on the copy holder behind the sheet.
- **Missed Word** (700, 17px, line-height 1): the word as typed in "words left wrong".
- **Keycap** (700, 16px, line-height 1; 13px for the wide "space" cap): the weak-key letter on its disc.
- **Label** (400, 12px, 0.08em, uppercase): the rail, its group names and its chips. Numerals are tabular.
- **Brand** (400, 12px, 0.14em, uppercase): "type-cast" at the left of the rail. It is the only brand mark on the page apart from the nameplate drawn on the machine.
- **Caption** (400, 12px, line-height 1, 0.1em, uppercase): the plate's top-right title and bottom-left hint, the focus prompt, and the labels over each value and list on Fig 2. "Fig 1" and "Fig 2" at top left use the same size in sentence case at 0.04em.
- **Readout** (400, 13px): the plate's status at bottom right, in lower case: `feeding paper`, `ready`, `typing`, `checking the sheet`, `verified by the server`, or the failure message.
- **Legend** (400, 12px, 0.04em, sentence case): ink keys under the card.
- **Note** (400, 11px, line-height 1.3, tabular): chart axis labels and units, the hover tip, the sub-lines under stat values, key-disc counts, the expected word under a missed word, and "+N more".

### Named Rules
**The Pitch Cell Rule.** Every character on paper sits centred in a fixed pitch cell (12.5 units at 21-unit type, 26 columns, wrapped at 23). The carriage scale carries one tick per cell. Proportional text never appears on the sheet.

**The Two Faces Rule.** Courier Prime for paper and the machine's figures: the sheet, the copy holder, the countdown, the result figures, the stat values, the key discs and the typed missed words, always at 700 off the paper. System mono for everything else: captions, controls, the readout, axis labels, notes. No third family is shown; the sans stack on the page root is a fallback that no visible text uses.

**The Figures Scale With The Card Rule.** The countdown and the result figures size in container units of the card (`cqw`), not the window, so they keep their proportion to the drawing.

## Layout

One centred column, `--tw-w`, shared by the rail, the card and the legend: at most 1240px and never wider than the height left after the rail and legend allows at a 3:2 card (`(100dvh - 208px) * 1.5 + 48px`). At 910px wide and below, or 760px tall and below, the rail wraps onto a second row, so the column reserves 252px instead of 208px. The page is padded 18/24/22px with 14px between rail, card and legend, and is at least the viewport tall.

The rail is a single wrapping row: brand, then the "time" and "words" chip groups (22px between groups, 4px between chips), then the tools (sound, theme, new sheet) pushed to the right. The card carries its four captions on an inset of 18px by 20px; the figure fills the space between them with 44px top and bottom padding. The drawing is a fixed 900 by 600 viewBox at camera azimuth 80, so the sheet's baseline slopes about 5 degrees; the strike point is fixed and the carriage moves under it. The countdown sits in the empty bench at the card's top left (20px in, 46px down), clear of the copy holder.

Fig 2 takes the same 900:600 aspect so the card does not change size. It is three rows 26px apart: the figures (a 140 to 200px column, wpm above acc, 26px apart) beside the chart; the stats row in five equal columns, 16px apart; and the practise row, weak keys beside words left wrong at 1 : 1.4 with 32px between. Each lower row starts with a 1px `line-faint` rule and 18px of padding.

At 720px and below the column goes full width (padding 14/16/18px), the tools wrap onto their own row, and the card crops the figure to a window at least 320px tall, enlarged to 240% and translated to the carriage around the strike point; the crop runs to the card's frame. While the countdown shows, the card's top padding grows to 86px so the countdown (36px, 38px down) has its own band above the crop. On phones Fig 2 drops its aspect ratio and pads 8/20/18px with 22px between rows: wpm and acc sit side by side 36px apart above a 220px chart, the stats row becomes two columns with 20px between rows, and the practise row stacks. At 560px and below the kit lifts the hint above the readout so the two bottom captions stack.

## Elevation & Depth

Depth is drawn, not cast. The machine reads as solid through isometric projection and four flat shade tones per solid, with a single blurred halo (9px blur, offset 3px) grounding it on the card. Interface surfaces are flat: the card is separated from the bench by tone and a 1px `line-mid` ring, and chips, key discs and the chart tip use a 1px ring rather than a border or a shadow.

### Shadow Vocabulary
- **Plate frame** (`box-shadow: 0 0 0 1px var(--anatomy-mid)`): the card's hairline frame; the same ring outlines the key discs and the chart's hover tip.
- **Chip ring** (`box-shadow: 0 0 0 1px` in `line-faint`, `line-mid` on hover, `line-hi` when pressed): control outline.
- **Lift** (the kit's `--anatomy-lift`, a stack of 1px inset and soft layered shadows): only on the "Click the machine to type" prompt that floats over the dimmed figure when the card loses focus.

### Named Rules
**The Drawn Depth Rule.** Volume belongs to the figure. Interface surfaces stay flat and outlined; the lift stack is reserved for the one transient prompt over the figure.

## Shapes

Square and hairline. The plate has no radius; chips have a near-square 2px radius; legend swatches 1px. The weak-key discs are the one round interface shape: 34px tall with a 17px radius, so a single letter is a circle and "space" a pill, echoing the machine's round key tops. Every outline is a single 1px ring in the interface and a 0.6px non-scaling stroke in the drawing. The machine's own forms are rounded rectangles, discs and rings from the anatomy kit, with screws as dots and ruler ticks hanging off scale bars.

## Components

### Buttons (Chips)
Every control is the same chip: quiet caption type in a hairline ring.
- **Shape:** near-square (2px), padding 6px 10px, label type inherited from the rail.
- **Rest:** transparent fill, `text` colour, `line-faint` ring.
- **Hover:** `strong` text, `line-mid` ring. Colour, ring and fill transition over 160ms on the kit ease (`cubic-bezier(.32,.72,0,1)`).
- **Pressed (`aria-pressed="true"`):** `face` fill, `strong` text, `line-hi` ring. Used for the selected test length, sound on and light theme on.
- **Focus:** 1px `line-hi` outline offset 3px.
- **Active:** moves down 1px.

### Plate (Card)
- **Corner Style:** square.
- **Background:** `card`, with a 1px `line-mid` frame.
- **Captions:** "Fig 1" top left (or "Fig 2" on the result); the object and test top right ("typewriter · 30s test", or "speed per second · 30s test" on the result); the hint bottom left ("Type to begin · Esc for a fresh sheet", then "Esc for a fresh sheet"); the readout bottom right. The readout is a status, not live statistics: it turns `strong` at `verified by the server`.
- **Unfocused state:** the figure dims to 55% opacity over 240ms and the focus prompt appears at its centre. The result plate does not dim or show the prompt.

### Countdown
Large figure in the bench corner, announced as a timer. Seconds left in time mode; words done over the total in words mode. It shows from `ready` through `typing` and goes when the sheet is sent. `text` while ready, `strong` while typing, the change easing over 240ms.

### Legend
A row of keys under the card: an 18 by 10px swatch with 1px radius and a hairline ring, then a sentence-case description. On Fig 1 the swatches show ribbon ink, ribbon red and faint copy. On Fig 2 the legend switches to the chart's encoding: a 2px `strong` bar for "wpm so far", a 2px `line-mid` bar for "raw wpm of each second", and a 10px cross in `ribbon-red-plate` for "Wrong keys per second, right scale".

### Result Plate (Fig 2)
Replaces the figure inside the card when the server's result arrives, fading in over 240ms on the kit ease.
- **Figures:** "wpm" and "acc" as captions over Result Figure values in `strong`; the exact value to two decimals is in the title.
- **Stats row:** test, raw, keys, consistency and time, each a caption, a Stat Value in `strong` and a Note under it. Keys read right "/" wrong: the slash is 400 in `text` and the wrong count is `ribbon-red-plate`.
- **Weak keys:** up to six discs for keys with misses, each a flat `face` disc in a 1px `line-mid` ring with its Keycap letter in `strong`, and "misses/total" under it as a Note. 12px by 14px between discs. "No key missed." when there are none.
- **Words left wrong:** up to eight words, each the typed word in Missed Word type and `ribbon-red-plate` over the expected word as a Note; then "+N more". 10px by 22px between words. "No word left wrong." when there are none.

### Speed Chart
Plotted from the server's per-second timeline; the geometry is computed in Rust in the chart's measured pixels, so strokes and labels stay crisp at any size.
- **Plot:** inset 44px left, 30px right, 36px top, 34px bottom. WPM gridlines in `line-faint`, at most four steps of 5, 10, 20, 25, 40, 50, 100, 200 or 250, labelled at the left in Note type with a "wpm" unit above.
- **Encoding:** wpm so far as a 1.75px `strong` line with a 2px dot each second; raw wpm per second as a 1.25px `line-mid` line behind it; seconds with wrong keys as 7px crosses (1.5px stroke) in `ribbon-red-plate`, read on a right-hand axis.
- **Error axis:** a 1px `ribbon-red-plate` line at the plot's right edge with 4px ticks, an "err" unit and counts. Its top is the smallest count above the worst second at which no mark lands on a wpm gridline; every count is labelled up to five, then about four steps. It is drawn only when a second has wrong keys.
- **Ruler:** the plot's base is a `line-mid` seconds ruler, 7px ticks where labelled and 4px between. Labels thin to every 1, 2, 5, 10, 15, 30 or 60 seconds so each has about 26px, and fall only on multiples.
- **Hover:** each second owns a full-height column with a crosshair cursor; hover or tap draws a dashed (2 3) `line-mid` guide and a tip ("second 112: 66 wpm, raw 60, 1 error") in the band above the plot, 12px beside the point and flipped to its left past the centre. The tip is Note type in `strong` on `card` with a 1px `line-mid` ring. On phones it pins to the right of that band.
- **Motion:** the raw and wpm lines trace from the first second to the last over 1100ms (`cubic-bezier(0.32, 0.72, 0, 1)`, raw 120ms later); the dots and crosses fade in over 320ms after 900ms. Reduced motion shows the finished chart at once.
- **Access:** the chart is an image with a one-sentence summary of both ranges and the seconds with wrong keys. "This result has no per-second record." when the timeline is empty.

### Typewriter Figure (signature)
Generated by the figure build into static SVG layers; the app moves only named parts.
- **Sheet:** the test text, with up to four printed lines above the print line, current line large at the strike point.
- **Ink states:** right in `ink` 700; wrong as faint copy under a red letter; extra as a red letter; skipped as faint copy with a red underscore.
- **Copy holder:** a card on a stand behind the sheet carrying the next two lines.
- **Motion:** the escapement steps one cell per key with no transition; a key sinks for 45ms; a typebar holds against the platen for two frames (34ms); the bell hammer moves over 70ms when the line comes within 6 cells of the margin; the line feed glides 260ms on the kit ease; the carriage return glides 380ms (`cubic-bezier(0.2, 0.8, 0.2, 1)`). Reduced motion removes the transitions and holds the machine still.
- **Cursor:** the current cell is marked in `ink` at 40% opacity.

## Do's and Don'ts

### Do:
- **Do** put every reading surface of the test on paper inside the drawing, set in Courier Prime on the pitch grid; put results on Fig 2 inside the same card.
- **Do** set controls as chips in the rail, using label type and the chip's ring states; show binary tools with `aria-pressed`.
- **Do** declare every new colour as a dark and light pair on the same token name.
- **Do** keep machine motion stepped: one cell per key, two-frame strikes, glides only for the return and the line feed. Result motion is one pen trace and one fade. Nothing moves under reduced motion.
- **Do** keep the plate's captions to the four corners: figure number, object and test, hint, status readout.
- **Do** keep chart marks off the wpm gridlines by choosing the error axis top, not by nudging marks.

### Don't:
- **Don't** use hue anywhere but ribbon ink: `ribbon-red` on the sheet, `ribbon-red-plate` for wrong keys on the card. The kit's red, green and blue line tones stay out.
- **Don't** paint a second part of the figure with `lit`; the strike point is the only lit part.
- **Don't** add drop shadows to cards, chips, discs or the tip; depth belongs to the drawing.
- **Don't** set proportional or sans text on the paper, or Courier Prime anywhere but the paper and the machine's figures; captions, controls and axis labels stay in system mono.
- **Don't** show live wpm or accuracy during the test; the countdown is the only live figure.
- **Don't** use the kit's `muted` grey (#6e6e6e on the card) for text; this build raises captions and legend to `text` for contrast.
