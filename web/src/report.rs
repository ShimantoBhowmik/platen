//! The result plate: the verified scores, the speed chart and what to practise next.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::api::{KeyStat, ModeKind, ModeSpec, Report};
use crate::chart;

/// Weakest keys shown, at most.
const WEAK_KEYS_SHOWN: usize = 6;
/// Words left wrong shown, at most.
const MISSED_WORDS_SHOWN: usize = 8;
/// Half the size of an error mark.
const ERROR_MARK: f64 = 3.5;

/// The key's name as the keycap shows it.
fn key_name(stat: &KeyStat) -> String {
    match stat.key {
        ' ' => "space".to_owned(),
        key => key.to_string(),
    }
}

/// The verified result of one test, drawn as the second figure of the plate.
#[component]
pub fn ResultPlate(report: Rc<Report>, spec: ModeSpec) -> Element {
    // The chart is laid out in the pixels it is drawn at, measured as the plate resizes.
    let mut size = use_signal(|| (720.0_f64, 280.0_f64));
    let mut hovered = use_signal(|| None::<usize>);
    let (width, height) = size();
    let layout = chart::layout(&report.timeline, width, height);
    let summary = chart::summary(&report.timeline);
    let stats = report.stats;

    let (test_kind, test_amount) = match spec.mode {
        ModeKind::Time => ("time", format!("{}s", spec.amount)),
        ModeKind::Words => ("words", spec.amount.to_string()),
    };
    let seconds = f64::from(stats.elapsed_ms) / 1_000.0;
    let time = if stats.elapsed_ms % 1_000 == 0 {
        format!("{seconds:.0}s")
    } else {
        format!("{seconds:.1}s")
    };
    let weak_keys: Vec<KeyStat> = report
        .breakdown
        .keys
        .iter()
        .filter(|stat| stat.misses > 0)
        .take(WEAK_KEYS_SHOWN)
        .copied()
        .collect();
    let missed_words = &report.breakdown.missed_words;

    rsx! {
        div { class: "tw-result",
            h2 { class: "tw-sr", "Result" }
            div { class: "tw-result-top",
                dl { class: "tw-figures",
                    div {
                        dt { "wpm" }
                        dd { title: "{stats.wpm:.2} words per minute", "{stats.wpm:.0}" }
                    }
                    div {
                        dt { "acc" }
                        dd { title: "{stats.accuracy:.2} percent accuracy", "{stats.accuracy:.0}"
                            span { class: "tw-unit", "%" }
                        }
                    }
                }
                div {
                    class: "tw-chart",
                    onresize: move |event| {
                        if let Ok(box_size) = event.get_content_box_size() {
                            size.set((box_size.width, box_size.height));
                        }
                    },
                    onmouseleave: move |_| hovered.set(None),
                    if let Some(drawn) = layout {
                        ChartSvg { drawn, summary, hovered }
                    } else {
                        p { class: "tw-empty", "This result has no per-second record." }
                    }
                }
            }
            dl { class: "tw-stats",
                div {
                    dt { "test" }
                    dd { "{test_kind} {test_amount}" }
                    dd { class: "tw-sub", "english 200" }
                }
                div {
                    dt { "raw" }
                    dd { title: "{stats.raw_wpm:.2}", "{stats.raw_wpm:.0}" }
                    dd { class: "tw-sub", "wpm of the final text" }
                }
                div {
                    dt { "keys" }
                    dd { "{stats.correct_keystrokes}"
                        span { class: "tw-slash", "/" }
                        span { class: "tw-wrong", "{stats.incorrect_keystrokes}" }
                    }
                    dd { class: "tw-sub", "right / wrong" }
                }
                div {
                    dt { "consistency" }
                    dd { title: "{stats.consistency:.2} percent", "{stats.consistency:.0}%" }
                    dd { class: "tw-sub", "steadiness of speed" }
                }
                div {
                    dt { "time" }
                    dd { "{time}" }
                    dd { class: "tw-sub", if spec.mode == ModeKind::Time { "time limit" } else { "time taken" } }
                }
            }
            div { class: "tw-practise",
                section { aria_label: "Weakest keys",
                    h3 { "weak keys" }
                    if weak_keys.is_empty() {
                        p { class: "tw-empty", "No key missed." }
                    } else {
                        ul { class: "tw-keycaps",
                            for stat in weak_keys {
                                li {
                                    title: "{key_name(&stat)}: missed {stat.misses} of {stat.hits + stat.misses}",
                                    span { class: "tw-keycap", "data-wide": "{stat.key == ' '}", "{key_name(&stat)}" }
                                    span { class: "tw-keycap-count", "{stat.misses}/{stat.hits + stat.misses}" }
                                }
                            }
                        }
                    }
                }
                section { aria_label: "Words left wrong",
                    h3 { "words left wrong" }
                    if missed_words.is_empty() {
                        p { class: "tw-empty", "No word left wrong." }
                    } else {
                        ul { class: "tw-words",
                            for word in missed_words.iter().take(MISSED_WORDS_SHOWN) {
                                li { aria_label: "{word.expected}, typed {word.typed}",
                                    span { class: "tw-word-typed", "{word.typed}" }
                                    span { class: "tw-word-expected", "{word.expected}" }
                                }
                            }
                            if missed_words.len() > MISSED_WORDS_SHOWN {
                                li { class: "tw-more", "+{missed_words.len() - MISSED_WORDS_SHOWN} more" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The speed chart: gridlines, the seconds ruler, both lines, error marks and hover columns.
#[component]
fn ChartSvg(drawn: chart::Chart, summary: String, hovered: Signal<Option<usize>>) -> Element {
    let [left, top, right, bottom] = drawn.plot;
    let focus = hovered().and_then(|second| drawn.samples.get(second).cloned());
    rsx! {
        svg {
            class: "tw-chart-svg",
            width: "{drawn.width}",
            height: "{drawn.height}",
            view_box: "0 0 {drawn.width} {drawn.height}",
            role: "img",
            "aria-label": "{summary}",
            for tick in drawn.wpm_ticks.iter() {
                line { class: "tw-grid", x1: "{left}", x2: "{right}", y1: "{tick.y:.1}", y2: "{tick.y:.1}" }
                text { class: "tw-axis", x: "{left - 10.0}", y: "{tick.y + 4.0:.1}", text_anchor: "end", "{tick.wpm}" }
            }
            text { class: "tw-axis tw-unit-label", x: "{left - 10.0}", y: "{top - 12.0}", text_anchor: "end", "wpm" }
            if !drawn.error_ticks.is_empty() {
                text { class: "tw-axis tw-axis-err", x: "{right + 10.0}", y: "{top - 12.0}", "err" }
                line { class: "tw-err-axis", x1: "{right}", x2: "{right}", y1: "{top}", y2: "{bottom}" }
            }
            for tick in drawn.error_ticks.iter() {
                line { class: "tw-err-axis", x1: "{right}", x2: "{right + 4.0}", y1: "{tick.y:.1}", y2: "{tick.y:.1}" }
                text { class: "tw-axis tw-axis-err", x: "{right + 8.0}", y: "{tick.y + 4.0:.1}", "{tick.errors}" }
            }
            line { class: "tw-ruler", x1: "{left}", x2: "{right}", y1: "{bottom}", y2: "{bottom}" }
            for tick in drawn.second_ticks.iter() {
                line {
                    class: "tw-ruler",
                    x1: "{tick.x:.1}",
                    x2: "{tick.x:.1}",
                    y1: "{bottom}",
                    y2: "{bottom + tick.length()}",
                }
                if let Some(label) = tick.label {
                    text { class: "tw-axis", x: "{tick.x:.1}", y: "{bottom + 21.0}", text_anchor: "middle", "{label}" }
                }
            }
            if let Some(sample) = focus.as_ref() {
                line { class: "tw-guide", x1: "{sample.x:.1}", x2: "{sample.x:.1}", y1: "{top}", y2: "{bottom}" }
            }
            path { class: "tw-plot tw-raw", d: "{drawn.raw_path}", "pathLength": "1" }
            path { class: "tw-plot tw-wpm", d: "{drawn.wpm_path}", "pathLength": "1" }
            g { class: "tw-marks",
                for sample in drawn.samples.iter() {
                    circle { class: "tw-dot", cx: "{sample.x:.1}", cy: "{sample.wpm_y:.1}", r: "2" }
                    if let Some(y) = sample.error_y {
                        path {
                            class: "tw-err",
                            d: "M{sample.x - ERROR_MARK:.1},{y - ERROR_MARK:.1} L{sample.x + ERROR_MARK:.1},{y + ERROR_MARK:.1} M{sample.x - ERROR_MARK:.1},{y + ERROR_MARK:.1} L{sample.x + ERROR_MARK:.1},{y - ERROR_MARK:.1}",
                        }
                    }
                }
            }
            for (second, sample) in drawn.samples.iter().enumerate() {
                rect {
                    class: "tw-column",
                    x: "{sample.column_x:.1}",
                    y: "{top}",
                    width: "{drawn.column_width:.1}",
                    height: "{bottom - top}",
                    onmouseenter: move |_| hovered.set(Some(second)),
                    onclick: move |_| hovered.set(Some(second)),
                }
            }
        }
        if let Some(sample) = focus {
            p {
                class: "tw-tip",
                "data-side": if sample.x > (left + right) / 2.0 { "left" } else { "right" },
                style: "--x: {sample.x:.1}px; --y: {top - 34.0}px",
                "{sample.label}"
            }
        }
    }
}
