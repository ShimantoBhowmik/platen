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
rounded:
  none: "0px"
  hair: "1px"
  chip: "2px"
spacing:
  chip-gap: "4px"
  rail-row: "10px"
  page-gap: "14px"
  corner-inset: "18px 20px"
  page: "18px 24px 22px"
  rail-group: "28px"
  plate: "44px 24px"
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
  focus-prompt:
    backgroundColor: "{colors.card}"
    textColor: "{colors.strong}"
    typography: "{typography.caption}"
    padding: "8px 14px"
---

# Design System: type-cast

## Overview

**Creative North Star: "The Anatomy Plate"**

One typewriter, drawn as a technical figure on a framed card, is the whole interface. The sheet rolled into its carriage is the test: the copy is printed faint on the paper, the learner types over it, and the machine answers each key with a stepped mechanical response. Everything outside the figure is set like a plate in a manual: a hairline frame, four corner captions, a legend under the card, and a single rail of controls above it that wears the same caption type.

The world is monochrome. The bench, card and machine are four-tone grey solids with 0.6px hairline outlines, screws drawn as dots, and ruler ticks cut to the type pitch. Colour exists only as ink on paper: a black ribbon for correct letters and the red half of a bichrome ribbon for misses. The only lit part of the drawing is the strike-point mark. Dark (charcoal bench) and light (paper-white bench) are the same drawing with each tone remapped; the theme follows the system preference and has a toggle.

Density is low and fixed. The card holds the drawing's 3:2 shape inside whatever height the rail and legend leave, so the machine never scrolls on desktop. Motion is mechanical, not decorative: the escapement jumps one cell with no easing, a typebar strike holds for two frames, and only the carriage return and line feed glide.

**Key Characteristics:**
- One isometric anatomy figure owns the screen; the paper is the test surface.
- Four-tone monochrome solids, 0.6px hairlines, dots and ticks; no hue outside the ribbon.
- One lit part: the strike-point mark.
- Courier Prime on the paper; small tracked system mono for every caption and control.
- Square plate, near-square 2px chips, flat surfaces.
- Stepped motion; reduced motion holds the machine still.

## Colors

A neutral grey bench and machine with paper and ribbon ink as the only content colours; every token has a dark value and a light override.

### Primary
- **Ribbon Red** (`ribbon-red`): the red half of the bichrome ribbon. Marks a missed or extra letter on the sheet, the underscore under a skipped letter, the ribbon run in the drawing, and the "typed wrong" legend swatch. Same value in both themes. It is the only chromatic colour in the system.
- **Ribbon Ink** (`ink`, `ink-light`): the black ribbon. Correctly typed letters, at weight 700.

### Neutral
- **Charcoal Bench / Paper Bench** (`bench`, `bench-light`): the page behind the card, also painted on `html` and `body`.
- **Plate Card** (`card`, `card-light`): the framed card and the focus prompt. Slightly lifted from the bench by tone, not by shadow.
- **Lit Face and Top** (`face`, `top`, `face-light`): top faces of the machine's solids and the pressed chip fill. `shade-0` through `shade-3` are the side faces.
- **Caption Grey** (`text`, `text-light`): rail labels, unpressed chips, plate captions, legend text.
- **Strong** (`strong`, `strong-light`): the brand, pressed and hovered chips, the readout when a test is done, the focus prompt.
- **Hairline Tones** (`line-hi`, `line-mid`, `line-lo`, `line-faint` and light pairs): outline weights of the drawing. `line-mid` also draws the plate frame and the hover ring on chips; `line-faint` is the chip's resting ring; `line-hi` is the pressed ring, the focus outline and the selection fill.
- **Strike Light** (`lit`, `lit-light`): the strike-point mark only. White on charcoal, black on paper.
- **Sheet** (`sheet`, `sheet-shade`, `sheet-edge` and light pairs): the paper in the carriage and on the copy holder, its shaded back and its edge stroke.
- **Faint Copy** (`copy`, `copy-light`): the practice text still to type and the copy holder's read-ahead lines, at weight 400.
- **Ribbon Body** (`ribbon`, `ribbon-light`): the black half of the ribbon in the drawing.

### Named Rules
**The Ink Is The Only Colour Rule.** Hue appears only as ribbon ink on paper. The bench, card, machine, controls and captions stay on the grey ramp. The vendored kit's red, green and blue line tones are not part of this world.

**The One Lit Part Rule.** The strike-point mark is the only element painted with `lit`. No other solid in the figure is drawn lit, so the eye always finds the print point.

**The Paired Theme Rule.** Every colour is declared twice, dark and light, on the same token name; a new tone without a light override is incomplete.

## Typography

**Paper Font:** Courier Prime (with Courier New, Courier, monospace), loaded from Google Fonts at 400 and 700.
**Caption and Control Font:** ui-monospace (with SF Mono, Menlo, Consolas, monospace).

**Character:** The machine's own slab-serif typewriter face sets everything on paper; a quiet system mono, small, uppercase and tracked, sets everything around the plate like a figure's annotations.

### Hierarchy
- **Sheet Type** (700, 21 drawing units on a 12.5-unit pitch cell, 23-unit line): letters typed right on the current line, set large through the fixed strike point.
- **Sheet Copy** (400, same size and pitch): letters still to type, and the copy left visible under a miss.
- **Holder Copy** (400, 17 units, 21-unit line, pitch scaled to match): the next two lines on the copy holder behind the sheet.
- **Label** (400, 12px, 0.08em, uppercase): the rail, its group names and its chips. Numerals are tabular.
- **Brand** (400, 12px, 0.14em, uppercase): "type-cast" at the left of the rail. It is the only brand mark on the page apart from the nameplate drawn on the machine.
- **Caption** (400, 12px, line-height 1, 0.1em, uppercase): the plate's top-right title and bottom-left hint. "Fig 1" at top left uses the same size in sentence case at 0.04em.
- **Readout** (400, 13px, tabular numerals): time or words left, wpm and accuracy at bottom right, separated by middle dots.
- **Legend** (400, 12px, 0.04em, sentence case): ink keys under the card.

### Named Rules
**The Pitch Cell Rule.** Every character on paper sits centred in a fixed pitch cell (12.5 units at 21-unit type, 26 columns, wrapped at 23). The carriage scale carries one tick per cell. Proportional text never appears on the sheet.

**The Two Faces Rule.** Courier Prime is for paper only; system mono is for everything outside the paper. No third family is shown; the sans stack on the page root is a fallback that no visible text uses.

## Layout

One centred column, `--tw-w`, shared by the rail, the card and the legend: at most 1240px and never wider than the height left after the rail and legend allows at a 3:2 card (`(100dvh - 208px) * 1.5 + 48px`). The page is padded 18/24/22px with 14px between rail, card and legend, and is at least the viewport tall.

The rail is a single wrapping row: brand, then the "time" and "words" chip groups (22px between groups, 4px between chips), then the tools (sound, theme, new sheet) pushed to the right. The card carries its four captions on an inset of 18px by 20px; the figure fills the space between them with 44px top and bottom padding. The drawing is a fixed 900 by 600 viewBox at camera azimuth 80, so the sheet's baseline slopes about 5 degrees; the strike point is fixed and the carriage moves under it.

At 720px and below the column goes full width (padding 14/16/18px), the tools wrap onto their own row, and the card crops the figure to a window at least 320px tall, enlarged to 240% and translated to the carriage around the strike point; the crop runs to the card's frame. At 560px and below the kit lifts the hint above the readout so the two bottom captions stack.

## Elevation & Depth

Depth is drawn, not cast. The machine reads as solid through isometric projection and four flat shade tones per solid, with a single blurred halo (9px blur, offset 3px) grounding it on the card. Interface surfaces are flat: the card is separated from the bench by tone and a 1px `line-mid` ring, and chips use a 1px ring rather than a border or a shadow.

### Shadow Vocabulary
- **Plate frame** (`box-shadow: 0 0 0 1px var(--anatomy-mid)`): the card's hairline frame.
- **Chip ring** (`box-shadow: 0 0 0 1px` in `line-faint`, `line-mid` on hover, `line-hi` when pressed): control outline.
- **Lift** (the kit's `--anatomy-lift`, a stack of 1px inset and soft layered shadows): only on the "Click the machine to type" prompt that floats over the dimmed figure when the card loses focus.

### Named Rules
**The Drawn Depth Rule.** Volume belongs to the figure. Interface surfaces stay flat and outlined; the lift stack is reserved for the one transient prompt over the figure.

## Shapes

Square and hairline. The plate has no radius; chips have a near-square 2px radius; legend swatches 1px. Every outline is a single 1px ring in the interface and a 0.6px non-scaling stroke in the drawing. The machine's own forms are rounded rectangles, discs and rings from the anatomy kit, with screws as dots and ruler ticks hanging off scale bars.

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
- **Captions:** "Fig 1" top left; the object and test ("typewriter · 30s test") top right; the hint bottom left; the readout bottom right, turning `strong` when the test is done.
- **Unfocused state:** the figure dims to 55% opacity over 240ms and the focus prompt appears at its centre.

### Legend
A row of ink keys under the card: an 18 by 10px swatch with 1px radius and a hairline ring, then a sentence-case description. Swatches show ribbon ink, ribbon red and faint copy.

### Typewriter Figure (signature)
Generated by the figure build into static SVG layers; the app moves only named parts.
- **Sheet:** the test text, with up to four printed lines above the print line, current line large at the strike point.
- **Ink states:** right in `ink` 700; wrong as faint copy under a red letter; extra as a red letter; skipped as faint copy with a red underscore.
- **Copy holder:** a card on a stand behind the sheet carrying the next two lines.
- **Motion:** the escapement steps one cell per key with no transition; a key sinks for 45ms; a typebar holds against the platen for two frames (34ms); the bell hammer moves over 70ms when the line comes within 6 cells of the margin; the line feed glides 260ms on the kit ease; the carriage return glides 380ms (`cubic-bezier(0.2, 0.8, 0.2, 1)`). Reduced motion removes the transitions and holds the machine still.
- **Cursor:** the current cell is marked in `ink` at 40% opacity.

## Do's and Don'ts

### Do:
- **Do** put every new reading surface on paper inside the drawing, set in Courier Prime on the pitch grid.
- **Do** set controls as chips in the rail, using label type and the chip's ring states; show binary tools with `aria-pressed`.
- **Do** declare every new colour as a dark and light pair on the same token name.
- **Do** keep motion stepped: one cell per key, two-frame strikes, glides only for the return and the line feed, and nothing under reduced motion.
- **Do** keep the plate's captions to the four corners: figure number, object and test, hint, readout.

### Don't:
- **Don't** use hue anywhere but ribbon ink on the sheet; the kit's red, green and blue line tones stay out.
- **Don't** paint a second part of the figure with `lit`; the strike point is the only lit part.
- **Don't** add drop shadows to cards or chips; depth belongs to the drawing.
- **Don't** set proportional or sans text on the paper, or Courier Prime outside it.
- **Don't** use the kit's `muted` grey (#6e6e6e on the card) for text; this build raises captions and legend to `text` for contrast.
