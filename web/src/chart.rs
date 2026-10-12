//! Geometry of the result's speed chart, computed in Rust so the page only draws it.
//!
//! The chart plots the server's timeline: WPM so far and raw WPM per second as two lines, and
//! the seconds with wrong keys as marks scaled on a right-hand axis. Coordinates are in CSS
//! pixels of the chart's measured width, so strokes and labels stay crisp at any size.

use std::fmt::Write;

use crate::api::TimelinePoint;

/// Room for the WPM labels left of the plot.
const LEFT: f64 = 44.0;
/// Room for the error labels right of the plot.
const RIGHT: f64 = 30.0;
/// Band above the plot for the axis units and the hover tip, so neither covers data.
const TOP: f64 = 36.0;
/// Room below the plot for the seconds ruler and its labels.
const BOTTOM: f64 = 34.0;
/// WPM steps the vertical axis may use, smallest first.
const WPM_STEPS: [u32; 9] = [5, 10, 20, 25, 40, 50, 100, 200, 250];
/// The most WPM gridlines above zero.
const MAX_WPM_LINES: u32 = 4;

/// A labelled gridline on the WPM axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WpmTick {
    /// Height in the chart.
    pub y: f64,
    /// WPM at that height.
    pub wpm: u32,
}

/// A labelled tick on the right-hand error axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ErrorTick {
    /// Height in the chart.
    pub y: f64,
    /// Wrong keys at that height.
    pub errors: u32,
}

/// A tick on the seconds ruler under the plot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SecondTick {
    /// Position across the chart.
    pub x: f64,
    /// The second, set only on the ticks that carry a label.
    pub label: Option<u32>,
}

impl SecondTick {
    /// How far the tick hangs below the ruler: longer where it carries a label.
    pub fn length(&self) -> f64 {
        if self.label.is_some() { 7.0 } else { 4.0 }
    }
}

/// One second of the test as drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    /// Position across the chart.
    pub x: f64,
    /// Height of the WPM-so-far point.
    pub wpm_y: f64,
    /// Height of the error mark; set only when the second has wrong keys.
    pub error_y: Option<f64>,
    /// Left edge of the second's hover column.
    pub column_x: f64,
    /// What the hover column says about the second.
    pub label: String,
}

/// Everything needed to draw the chart.
#[derive(Debug, Clone, PartialEq)]
pub struct Chart {
    /// Chart width.
    pub width: f64,
    /// Chart height.
    pub height: f64,
    /// Plot edges: left, top, right, bottom.
    pub plot: [f64; 4],
    /// WPM gridlines, from zero up.
    pub wpm_ticks: Vec<WpmTick>,
    /// The error axis, from zero up; empty when no second had wrong keys.
    pub error_ticks: Vec<ErrorTick>,
    /// The seconds ruler.
    pub second_ticks: Vec<SecondTick>,
    /// SVG path of WPM so far.
    pub wpm_path: String,
    /// SVG path of raw WPM per second.
    pub raw_path: String,
    /// One entry per second.
    pub samples: Vec<Sample>,
    /// Width of each hover column.
    pub column_width: f64,
}

/// The step and top of the WPM axis: the smallest step that covers `peak` in at most
/// [`MAX_WPM_LINES`] gridlines.
fn wpm_scale(peak: f64) -> (u32, u32) {
    let peak = peak.max(1.0);
    let step = WPM_STEPS
        .into_iter()
        .find(|&step| (peak / f64::from(step)).ceil() <= f64::from(MAX_WPM_LINES))
        .unwrap_or(WPM_STEPS[WPM_STEPS.len() - 1]);
    let lines = (peak / f64::from(step)).ceil().max(1.0) as u32;
    (step, step * lines)
}

/// The top of the error axis: the smallest count above `errors_max` that puts no error mark
/// on a WPM gridline, so a mark never reads as a WPM value. `wpm_lines` is the number of WPM
/// gridlines above zero.
fn errors_scale(errors_max: u32, wpm_lines: u32) -> u32 {
    // A mark lands on a gridline when errors / top == line / wpm_lines. A top above every mark
    // keeps the marks off the top line, and `errors_max * wpm_lines + 1` always qualifies.
    (errors_max + 1..=errors_max * wpm_lines + 1)
        .find(|&top| (1..=errors_max).all(|errors| !(errors * wpm_lines).is_multiple_of(top)))
        .unwrap_or(errors_max * wpm_lines + 1)
}

/// Seconds between labels on the ruler, so a long test does not crowd it.
fn label_every(seconds: usize, plot_width: f64) -> usize {
    // A label needs about 26 px.
    let room = (plot_width / 26.0).max(1.0) as usize;
    [1, 2, 5, 10, 15, 30, 60]
        .into_iter()
        .find(|&every| seconds.div_ceil(every) <= room)
        .unwrap_or(60)
}

/// A polyline through `points` as an SVG path.
fn path(points: impl Iterator<Item = (f64, f64)>) -> String {
    let mut out = String::new();
    for (index, (x, y)) in points.enumerate() {
        // Writing to a `String` cannot fail, so the result is safely ignored.
        let _ = write!(out, "{}{x:.1},{y:.1}", if index == 0 { "M" } else { " L" });
    }
    out
}

/// What one second's hover column says, e.g. `"second 4: 82 wpm, raw 96, 1 error"`.
fn describe(second: usize, point: &TimelinePoint) -> String {
    let errors = match point.errors {
        0 => String::new(),
        1 => ", 1 error".to_owned(),
        many => format!(", {many} errors"),
    };
    format!(
        "second {}: {:.0} wpm, raw {:.0}{errors}",
        second + 1,
        point.wpm,
        point.raw_wpm
    )
}

/// Lays out the chart for a timeline.
///
/// # Arguments
///
/// * `timeline` - One point per second, from the server
/// * `width` - Chart width in CSS pixels
/// * `height` - Chart height in CSS pixels
///
/// # Returns
///
/// The chart geometry, or `None` for an empty timeline.
///
/// # Examples
///
/// ```ignore
/// let chart = layout(&report.timeline, 720.0, 260.0).expect("non-empty timeline");
/// assert_eq!(chart.samples.len(), report.timeline.len());
/// ```
pub fn layout(timeline: &[TimelinePoint], width: f64, height: f64) -> Option<Chart> {
    if timeline.is_empty() {
        return None;
    }
    let plot = [LEFT, TOP, width - RIGHT, height - BOTTOM];
    let [left, top, right, bottom] = plot;
    let plot_height = bottom - top;
    let peak = timeline.iter().fold(0.0_f64, |peak, point| {
        peak.max(point.wpm).max(point.raw_wpm)
    });
    let (step, wpm_top) = wpm_scale(peak);
    let errors_max = timeline.iter().map(|point| point.errors).max().unwrap_or(0);
    let errors_top = errors_scale(errors_max, wpm_top / step);
    let wpm_y = |wpm: f64| bottom - wpm / f64::from(wpm_top) * plot_height;
    let error_y = |errors: u32| bottom - f64::from(errors) / f64::from(errors_top) * plot_height;
    // Every count is labelled up to five; above that, about four steps.
    let error_step = if errors_top <= 5 {
        1
    } else {
        errors_top.div_ceil(4)
    };
    let error_ticks = if errors_max == 0 {
        Vec::new()
    } else {
        (0..=errors_top)
            .step_by(error_step as usize)
            .map(|errors| ErrorTick {
                y: error_y(errors),
                errors,
            })
            .collect()
    };

    // One second spans the plot evenly; a single second sits in the middle.
    let last = timeline.len() - 1;
    let x_of = |second: usize| {
        if last == 0 {
            (left + right) / 2.0
        } else {
            left + second as f64 / last as f64 * (right - left)
        }
    };
    let column_width = if last == 0 {
        right - left
    } else {
        (right - left) / last as f64
    };

    let every = label_every(timeline.len(), right - left);
    let second_ticks = (0..timeline.len())
        .map(|second| SecondTick {
            x: x_of(second),
            // Labels sit on multiples of `every`, so the first second is labelled only when
            // every second is.
            label: ((second + 1) % every == 0).then_some(second as u32 + 1),
        })
        .collect();
    let samples = timeline
        .iter()
        .enumerate()
        .map(|(second, point)| {
            let x = x_of(second);
            Sample {
                x,
                wpm_y: wpm_y(point.wpm),
                error_y: (point.errors > 0).then(|| error_y(point.errors)),
                column_x: x - column_width / 2.0,
                label: describe(second, point),
            }
        })
        .collect();

    Some(Chart {
        width,
        height,
        plot,
        wpm_ticks: (0..=wpm_top / step)
            .map(|line| WpmTick {
                y: wpm_y(f64::from(line * step)),
                wpm: line * step,
            })
            .collect(),
        error_ticks,
        second_ticks,
        wpm_path: path(
            timeline
                .iter()
                .enumerate()
                .map(|(second, point)| (x_of(second), wpm_y(point.wpm))),
        ),
        raw_path: path(
            timeline
                .iter()
                .enumerate()
                .map(|(second, point)| (x_of(second), wpm_y(point.raw_wpm))),
        ),
        samples,
        column_width,
    })
}

/// A sentence that describes the chart to a screen reader.
///
/// # Arguments
///
/// * `timeline` - One point per second, from the server
///
/// # Returns
///
/// For example `"Speed over 30 seconds: wpm so far from 64 to 92, raw wpm per second from 40
/// to 120. Wrong keys in seconds 10, 11 and 17."`
pub fn summary(timeline: &[TimelinePoint]) -> String {
    let range = |value: fn(&TimelinePoint) -> f64| {
        let (low, high) = timeline
            .iter()
            .map(value)
            .fold((f64::MAX, f64::MIN), |(low, high), v| {
                (low.min(v), high.max(v))
            });
        format!("from {low:.0} to {high:.0}")
    };
    let wrong: Vec<String> = timeline
        .iter()
        .enumerate()
        .filter(|(_, point)| point.errors > 0)
        .map(|(second, _)| (second + 1).to_string())
        .collect();
    let errors = match wrong.as_slice() {
        [] => "No wrong keys.".to_owned(),
        [one] => format!("Wrong keys in second {one}."),
        [rest @ .., last] => format!("Wrong keys in seconds {} and {last}.", rest.join(", ")),
    };
    let seconds = match timeline.len() {
        1 => "1 second".to_owned(),
        many => format!("{many} seconds"),
    };
    format!(
        "Speed over {seconds}: wpm so far {}, raw wpm per second {}. {errors}",
        range(|point| point.wpm),
        range(|point| point.raw_wpm),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(wpm: f64, raw_wpm: f64, errors: u32) -> TimelinePoint {
        TimelinePoint {
            wpm,
            raw_wpm,
            errors,
        }
    }

    #[test]
    fn wpm_axis_covers_the_peak_in_few_lines() {
        assert_eq!(wpm_scale(92.0), (25, 100));
        assert_eq!(wpm_scale(116.0), (40, 120));
        assert_eq!(wpm_scale(350.0), (100, 400));
        assert_eq!(wpm_scale(0.0), (5, 5));
    }

    #[test]
    fn error_marks_avoid_wpm_gridlines() {
        assert_eq!(errors_scale(2, 4), 3);
        assert_eq!(errors_scale(1, 4), 3);
        assert_eq!(errors_scale(3, 3), 4);
        assert_eq!(errors_scale(1, 1), 2);
        for (errors_max, lines) in [(1, 2), (4, 4), (7, 3), (12, 4)] {
            let top = errors_scale(errors_max, lines);
            assert!(top > errors_max);
            assert!((1..=errors_max).all(|errors| !(errors * lines).is_multiple_of(top)));
        }
    }

    #[test]
    fn ruler_labels_thin_out_for_long_tests() {
        assert_eq!(label_every(15, 900.0), 1);
        assert_eq!(label_every(120, 900.0), 5);
        assert_eq!(label_every(120, 300.0), 15);
    }

    #[test]
    fn layout_spans_the_plot_and_scales_errors() {
        let timeline = [
            point(60.0, 72.0, 0),
            point(80.0, 96.0, 2),
            point(90.0, 84.0, 1),
        ];

        let chart = layout(&timeline, 400.0, 200.0).expect("non-empty timeline");

        let [left, top, right, bottom] = chart.plot;
        assert_eq!(chart.samples[0].x, left);
        assert_eq!(chart.samples[2].x, right);
        // Four WPM gridlines and at most two errors put the error axis top at three, so no
        // mark sits on a gridline.
        let third = (bottom - top) / 3.0;
        assert_eq!(chart.samples[0].error_y, None);
        let near = |y: Option<f64>, want: f64| y.is_some_and(|y| (y - want).abs() < 1e-9);
        assert!(near(chart.samples[1].error_y, bottom - 2.0 * third));
        assert!(near(chart.samples[2].error_y, bottom - third));
        let counts: Vec<u32> = chart.error_ticks.iter().map(|tick| tick.errors).collect();
        assert_eq!(counts, [0, 1, 2, 3]);
        assert_eq!(chart.wpm_ticks.first().map(|tick| tick.y), Some(bottom));
        assert_eq!(chart.wpm_ticks.last().map(|tick| tick.wpm), Some(100));
        assert!(chart.wpm_path.starts_with('M'));
        assert_eq!(chart.raw_path.matches(" L").count(), 2);
        assert_eq!(chart.samples[1].label, "second 2: 80 wpm, raw 96, 2 errors");
        assert!(chart.second_ticks.iter().all(|tick| tick.label.is_some()));
    }

    #[test]
    fn thinned_ruler_labels_only_multiples() {
        let timeline = vec![point(50.0, 50.0, 0); 15];

        let chart = layout(&timeline, 300.0, 200.0).expect("non-empty timeline");

        let labels: Vec<u32> = chart
            .second_ticks
            .iter()
            .filter_map(|tick| tick.label)
            .collect();
        assert_eq!(labels, [2, 4, 6, 8, 10, 12, 14]);
    }

    #[test]
    fn single_second_sits_in_the_middle() {
        let chart = layout(&[point(40.0, 40.0, 0)], 400.0, 200.0).expect("one point");

        let [left, _, right, _] = chart.plot;
        assert_eq!(chart.samples[0].x, (left + right) / 2.0);
    }

    #[test]
    fn empty_timeline_has_no_chart() {
        assert_eq!(layout(&[], 400.0, 200.0), None);
    }

    #[test]
    fn summary_names_the_ranges_and_wrong_seconds() {
        let timeline = [
            point(60.0, 72.0, 0),
            point(80.0, 96.0, 2),
            point(90.0, 84.0, 1),
        ];

        assert_eq!(
            summary(&timeline),
            "Speed over 3 seconds: wpm so far from 60 to 90, raw wpm per second from 72 to \
             96. Wrong keys in seconds 2 and 3."
        );
        assert_eq!(
            summary(&timeline[..1]),
            "Speed over 1 second: wpm so far from 60 to 60, raw wpm per second from 72 to \
             72. No wrong keys."
        );
    }
}
