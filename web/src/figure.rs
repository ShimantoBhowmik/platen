//! The typewriter drawing, precomputed by `scripts/figure/build.mjs` with the anatomy kit.
//!
//! The build writes static SVG layers and the camera numbers the app needs to move parts. The
//! file is embedded at compile time and parsed once.

use std::sync::LazyLock;

use serde::Deserialize;

/// A screen vector in SVG user units: `[dx, dy]`.
pub type ScreenVec = [f64; 2];

/// One key of the keyboard: what it types and its drawing.
#[derive(Debug, Deserialize)]
pub struct Key {
    /// The character, or a name such as `Shift` for non-printing keys.
    pub char: String,
    /// The key's SVG markup.
    pub svg: String,
}

/// Where text sits on the sheet, in the sheet's plane coordinates (x across, y = -height).
#[derive(Debug, Deserialize)]
pub struct TextLayout {
    /// Left edge of column 0.
    pub x0: f64,
    /// Width of one character cell.
    pub cell: f64,
    /// Cells per line on the paper.
    pub cols: usize,
    /// Cells the copy is wrapped at, a few short of `cols` so extra letters stay on the paper.
    pub wrap: usize,
    /// Paper left and right of the text.
    pub margin: f64,
    /// Distance between lines.
    pub line_h: f64,
    /// Font size of the type.
    pub font: f64,
    /// Baseline of the line at the print point.
    pub baseline: f64,
    /// Top edge of the sheet.
    pub top: f64,
}

/// Where the upcoming copy sits on the copy holder card, in the card's plane coordinates.
#[derive(Debug, Deserialize)]
pub struct CopyLayout {
    /// Left edge of the first character.
    pub x0: f64,
    /// Width of one character cell.
    pub cell: f64,
    /// Font size.
    pub font: f64,
    /// Baselines of the next line and the one after it.
    pub baselines: [f64; 2],
}

/// The precomputed figure.
#[derive(Debug, Deserialize)]
pub struct Figure {
    /// ViewBox width.
    pub width: f64,
    /// ViewBox height, sized by the build to the drawing.
    pub height: f64,
    /// Accessible description of the drawing.
    pub label: String,
    /// Bench, body, rear housing and copy holder.
    pub back: String,
    /// The bell hammer, which swings at the end of a line.
    pub hammer: String,
    /// The platen, its knobs and end plates, and the scale bar.
    pub platen: String,
    /// The sheet of paper.
    pub sheet: String,
    /// The paper bail, in front of the sheet.
    pub bail: String,
    /// The two rails the carrier rides on.
    pub rails: String,
    /// The carrier's fixed parts: block, ribbon cartridge, ribbon runs and card holder.
    pub carrier: String,
    /// The ribbon guide, which lifts on each strike.
    pub vibrator: String,
    /// The lit strike mark on the card holder.
    pub mark: String,
    /// The type element on its post, which tilts into the platen on each strike.
    pub ball: String,
    /// Keys, already in painter's order.
    pub keys: Vec<Key>,
    /// The space bar.
    pub space: String,
    /// Screen offset of the carrier for one world unit along x.
    pub carrier_step: ScreenVec,
    /// Screen offset of a pressed key.
    pub key_down: ScreenVec,
    /// Screen offset of the lifted ribbon guide.
    pub vibrator_lift: ScreenVec,
    /// Screen offset of the type element as it strikes.
    pub ball_strike: ScreenVec,
    /// Screen offset of the swung bell hammer.
    pub hammer_swing: ScreenVec,
    /// SVG transform onto the sheet's plane.
    pub paper_matrix: String,
    /// Text layout on the sheet.
    pub text: TextLayout,
    /// SVG transform onto the copy holder card's plane.
    pub copy_matrix: String,
    /// Text layout on the copy holder card.
    pub copy: CopyLayout,
    /// The print point with the carrier at x = 0, as a fraction of the viewBox. Phones crop
    /// the drawing around it as the carrier travels.
    pub strike_at: [f64; 2],
}

impl Figure {
    /// Centre of column `col` on the sheet, in plane coordinates. It is also how far the
    /// carrier travels along x to print in that column.
    pub fn col_x(&self, col: usize) -> f64 {
        self.text.x0 + (col as f64 + 0.5) * self.text.cell
    }

    /// The print point over column `col`, as a fraction of the viewBox.
    pub fn strike_point(&self, col: usize) -> [f64; 2] {
        let [sx, sy] = self.carrier_step;
        let travel = self.col_x(col);
        [
            self.strike_at[0] + sx * travel / self.width,
            self.strike_at[1] + sy * travel / self.height,
        ]
    }

    /// The key drawn for a typed character, if the keyboard has one.
    pub fn key_index(&self, character: char) -> Option<usize> {
        let lower = character.to_ascii_lowercase();
        let name = match lower {
            ' ' => return None,
            '\u{8}' => "Backspace".to_owned(),
            other => other.to_string(),
        };
        self.keys.iter().position(|key| key.char == name)
    }
}

/// The figure, parsed on first use. `LazyLock` is a lazily initialised global.
pub static FIGURE: LazyLock<Figure> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../assets/typewriter.json"))
        .expect("typewriter.json is generated by scripts/figure/build.mjs and always valid")
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn figure_parses_and_has_every_letter() {
        for letter in 'a'..='z' {
            assert!(FIGURE.key_index(letter).is_some(), "missing key {letter}");
        }
    }

    #[test]
    fn strike_point_steps_one_cell_per_column_and_stays_on_the_drawing() {
        let first = FIGURE.strike_point(0);
        let second = FIGURE.strike_point(1);
        let step = (second[0] - first[0]) * FIGURE.width;
        assert!((step - FIGURE.carrier_step[0] * FIGURE.text.cell).abs() < 1e-9);
        let last = FIGURE.strike_point(FIGURE.text.cols - 1);
        for point in [first, last] {
            assert!((0.0..=1.0).contains(&point[0]) && (0.0..=1.0).contains(&point[1]));
        }
    }
}
