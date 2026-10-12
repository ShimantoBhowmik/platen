//! type-cast web app: a typing test typed on an isometric typewriter.

mod api;
mod figure;
mod session;
mod sound;

use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

use crate::api::{ModeKind, ModeSpec, NewTest, Stats};
use crate::figure::FIGURE;
use crate::session::{Effect, Ink, Session, Stroke};
use crate::sound::Cue;

/// Lines of printed text kept visible above the print line.
const LINES_SHOWN: usize = 4;
/// Cells before the right end of a line where the margin bell rings.
const BELL_AT: usize = 6;
/// How long a typebar stays up against the platen: two frames.
const STRIKE_MS: u32 = 34;
/// How long a key stays down.
const KEY_MS: u32 = 90;
const STORAGE_THEME: &str = "type-cast:theme";
const STORAGE_SOUND: &str = "type-cast:sound";

fn main() {
    dioxus::launch(App);
}

/// Where the test is in its life.
#[derive(Debug, Clone, PartialEq)]
enum Phase {
    /// Waiting for a test from the server.
    Loading,
    /// A fresh sheet; the clock starts on the first key.
    Ready,
    /// Typing.
    Running,
    /// Sent to the server, waiting for the verified scores.
    Scoring,
    /// The server's verified result.
    Done(Stats),
    /// Something failed; the message says what and the hint says how to recover.
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Theme {
    Dark,
    Light,
}

impl Theme {
    fn name(self) -> &'static str {
        match self {
            Theme::Dark => "dark",
            Theme::Light => "light",
        }
    }
}

fn now_ms() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map_or(0.0, |p| p.now())
}

fn storage_get(key: &str) -> Option<String> {
    web_sys::window()?
        .local_storage()
        .ok()??
        .get_item(key)
        .ok()?
}

fn storage_set(key: &str, value: &str) {
    // A preference that cannot be stored (private mode, blocked storage) only resets next visit.
    if let Some(Ok(Some(storage))) = web_sys::window().map(|w| w.local_storage()) {
        let _ = storage.set_item(key, value);
    }
}

fn initial_theme() -> Theme {
    match storage_get(STORAGE_THEME).as_deref() {
        Some("light") => Theme::Light,
        Some("dark") => Theme::Dark,
        _ => {
            let prefers_light = web_sys::window()
                .and_then(|w| {
                    w.match_media("(prefers-color-scheme: light)")
                        .ok()
                        .flatten()
                })
                .is_some_and(|query| query.matches());
            if prefers_light {
                Theme::Light
            } else {
                Theme::Dark
            }
        }
    }
}

fn reduced_motion() -> bool {
    web_sys::window()
        .and_then(|w| {
            w.match_media("(prefers-reduced-motion: reduce)")
                .ok()
                .flatten()
        })
        .is_some_and(|query| query.matches())
}

fn mode_label(spec: ModeSpec) -> String {
    match spec.mode {
        ModeKind::Time => format!("{}s", spec.amount),
        ModeKind::Words => format!("{}", spec.amount),
    }
}

fn clock(ms: u32) -> String {
    let seconds = ms.div_ceil(1_000);
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// Everything the machine animates, set by a key press and cleared by timers.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Motion {
    key: Option<usize>,
    space: bool,
    strike: Option<char>,
    returning: bool,
    bell: bool,
}

#[component]
fn App() -> Element {
    let mut theme = use_signal(initial_theme);
    let mut sound_on = use_signal(|| storage_get(STORAGE_SOUND).as_deref() != Some("off"));
    let mut mode = use_signal(|| ModeSpec {
        mode: ModeKind::Time,
        amount: 30,
    });
    let mut phase = use_signal(|| Phase::Loading);
    let mut test = use_signal(|| None::<NewTest>);
    let mut session = use_signal(|| None::<Session>);
    let mut started = use_signal(|| None::<f64>);
    let mut now = use_signal(now_ms);
    let mut motion = use_signal(Motion::default);
    let mut bell_line = use_signal(|| None::<usize>);
    let mut generation = use_signal(|| 0_u64);
    let mut stage = use_signal(|| None::<std::rc::Rc<MountedData>>);
    let calm = use_hook(reduced_motion);

    let focus_stage = move || {
        if let Some(stage) = stage() {
            spawn(async move {
                let _ = stage.set_focus(true).await;
            });
        }
    };

    let mut load = move |spec: ModeSpec| {
        generation += 1;
        let this = generation();
        phase.set(Phase::Loading);
        session.set(None);
        started.set(None);
        bell_line.set(None);
        motion.set(Motion::default());
        spawn(async move {
            let result = api::new_test(spec).await;
            if generation() != this {
                return;
            }
            match result {
                Ok(fresh) => {
                    let finishes = spec.mode == ModeKind::Words;
                    session.set(Some(Session::new(
                        fresh.words.clone(),
                        FIGURE.text.wrap,
                        FIGURE.text.cols - FIGURE.text.wrap,
                        finishes,
                    )));
                    test.set(Some(fresh));
                    phase.set(Phase::Ready);
                }
                Err(error) => phase.set(Phase::Failed(format!("No sheet: {error}."))),
            }
        });
    };

    use_hook(move || load(mode()));

    let mut submit = move || {
        let this = generation();
        let (Some(issued), Some(typed)) = (test(), session()) else {
            return;
        };
        phase.set(Phase::Scoring);
        spawn(async move {
            let result = api::submit(&issued.token, typed.keys(), typed.times_ms()).await;
            if generation() != this {
                return;
            }
            phase.set(match result {
                Ok(stats) => Phase::Done(stats),
                Err(error) => Phase::Failed(format!("Not scored: {error}.")),
            });
        });
    };

    let run_clock = move || {
        let this = generation();
        spawn(async move {
            loop {
                TimeoutFuture::new(100).await;
                if generation() != this || phase() != Phase::Running {
                    return;
                }
                now.set(now_ms());
                let elapsed = started().map_or(0.0, |start| now() - start);
                if mode()
                    .limit_ms()
                    .is_some_and(|limit| elapsed >= f64::from(limit))
                {
                    session.with_mut(|s| {
                        if let Some(s) = s {
                            s.finish();
                        }
                    });
                    submit();
                    return;
                }
            }
        });
    };

    let mut animate =
        move |effect: Effect, key: Option<usize>, character: Option<char>, line_changed: bool| {
            let space = matches!(effect, Effect::Spaced { .. });
            let struck =
                matches!(effect, Effect::Struck { .. } | Effect::Finished) && character.is_some();
            motion.set(Motion {
                key,
                space,
                strike: if struck && !calm { character } else { None },
                returning: line_changed,
                bell: motion().bell,
            });
            if sound_on() {
                sound::play(match effect {
                    Effect::Spaced { .. } if line_changed => Cue::Return,
                    Effect::Spaced { .. } | Effect::Backed => Cue::Space,
                    _ => Cue::Clack,
                });
            }
            let this = generation();
            spawn(async move {
                TimeoutFuture::new(STRIKE_MS).await;
                motion.with_mut(|m| m.strike = None);
                TimeoutFuture::new(KEY_MS - STRIKE_MS).await;
                if generation() == this {
                    motion.with_mut(|m| {
                        m.key = None;
                        m.space = false;
                    });
                }
                TimeoutFuture::new(400).await;
                motion.with_mut(|m| m.returning = false);
            });
        };

    let mut ring_bell = move || {
        motion.with_mut(|m| m.bell = true);
        if sound_on() {
            sound::play(Cue::Bell);
        }
        spawn(async move {
            TimeoutFuture::new(140).await;
            motion.with_mut(|m| m.bell = false);
        });
    };

    let onkeydown = move |event: KeyboardEvent| {
        let modifiers = event.modifiers();
        if modifiers.ctrl() || modifiers.meta() || modifiers.alt() {
            return;
        }
        let stroke = match event.key() {
            Key::Escape => {
                event.prevent_default();
                load(mode());
                return;
            }
            Key::Backspace => Stroke::Back,
            Key::Character(text) => match text.chars().collect::<Vec<_>>()[..] {
                [character] => Stroke::Char(character),
                _ => return,
            },
            _ => return,
        };
        event.prevent_default();
        if !matches!(phase(), Phase::Ready | Phase::Running) {
            return;
        }
        let at = now_ms();
        let start = started().unwrap_or(at);
        let time_ms = (at - start).max(0.0) as u32;
        let Some(mut typed) = session() else { return };
        let line_before = typed.current_line();
        let effect = typed.press(stroke, time_ms);
        let line_after = typed.current_line();
        let col = typed.cursor_col();
        session.set(Some(typed));
        now.set(at);
        if effect == Effect::Ignored {
            return;
        }
        if started().is_none() {
            started.set(Some(at));
        }
        if phase() == Phase::Ready {
            phase.set(Phase::Running);
            run_clock();
        }
        let character = match stroke {
            Stroke::Char(c) => Some(c),
            Stroke::Back => None,
        };
        let key = match stroke {
            Stroke::Back => FIGURE.key_index('\u{8}'),
            Stroke::Char(c) => FIGURE.key_index(c),
        };
        animate(effect, key, character, line_after != line_before);
        if col + BELL_AT >= FIGURE.text.cols && bell_line() != Some(line_after) {
            bell_line.set(Some(line_after));
            ring_bell();
        }
        if effect == Effect::Finished {
            submit();
        }
    };

    let choose = move |spec: ModeSpec| {
        move |_| {
            mode.set(spec);
            load(spec);
            focus_stage();
        }
    };

    let elapsed_ms = started().map_or(0.0, |start| now() - start).max(0.0) as u32;
    let typed = session();
    let spec = mode();
    let readout = match phase() {
        Phase::Loading => "feeding paper".to_owned(),
        Phase::Ready => match spec.limit_ms() {
            Some(limit) => format!("{} · ready", clock(limit)),
            None => format!("0 / {} words · ready", spec.amount),
        },
        Phase::Running => {
            let wpm = typed.as_ref().map_or(0.0, |s| s.live_wpm(elapsed_ms));
            let accuracy = typed
                .as_ref()
                .and_then(Session::live_accuracy)
                .map_or("--".to_owned(), |a| format!("{a:.0}%"));
            let progress = match spec.limit_ms() {
                Some(limit) => clock(limit.saturating_sub(elapsed_ms)),
                None => format!(
                    "{} / {}",
                    typed.as_ref().map_or(0, Session::words_done),
                    spec.amount
                ),
            };
            format!("{progress} · {wpm:.0} wpm · {accuracy}")
        }
        Phase::Scoring => "checking the sheet".to_owned(),
        Phase::Done(stats) => format!(
            "{:.1} wpm · {:.1}% · raw {:.0} · steady {:.0}",
            stats.wpm, stats.accuracy, stats.raw_wpm, stats.consistency
        ),
        Phase::Failed(ref message) => message.clone(),
    };
    let hint = match phase() {
        Phase::Ready => "Type to begin · Esc for a fresh sheet",
        Phase::Running => "Esc for a fresh sheet",
        Phase::Done(_) => "Verified by the server · Esc to go again",
        Phase::Failed(_) => "Esc to try again",
        Phase::Loading | Phase::Scoring => "",
    };
    let status = match phase() {
        Phase::Done(stats) => format!(
            "Test scored. {:.1} words per minute, {:.1} percent accuracy.",
            stats.wpm, stats.accuracy
        ),
        Phase::Failed(ref message) => message.clone(),
        _ => String::new(),
    };

    let m = motion();
    let current_line = typed.as_ref().map_or(0, Session::current_line);
    let cursor = typed.as_ref().map_or(0, Session::cursor_col);
    let dx = FIGURE.carriage_dx(cursor);
    let [cx, cy] = FIGURE.carriage_step;
    let carriage = format!("transform: translate({:.2}px, {:.2}px)", cx * dx, cy * dx);
    let [kx, ky] = FIGURE.key_down;
    let [vx, vy] = FIGURE.vibrator_lift;
    let [hx, hy] = FIGURE.hammer_swing;
    let vars = format!(
        "--kd: translate({kx:.2}px, {ky:.2}px); --vl: translate({vx:.2}px, {vy:.2}px); --hs: translate({hx:.2}px, {hy:.2}px)"
    );
    let [cx, cy] = FIGURE.crop_at;
    let crop = format!("--cx: {:.2}%; --cy: {:.2}%", cx * 100.0, cy * 100.0);
    let upcoming: Vec<(f64, String)> = typed
        .as_ref()
        .map(|s| {
            (1..=2)
                .filter(|ahead| current_line + ahead < s.line_count())
                .map(|ahead| {
                    let copy = s
                        .cells(current_line + ahead)
                        .iter()
                        .map(|cell| cell.copy.unwrap_or(' '))
                        .collect();
                    (FIGURE.copy.baselines[ahead - 1], copy)
                })
                .collect()
        })
        .unwrap_or_default();
    let first_line = current_line.saturating_sub(LINES_SHOWN);
    let text = &FIGURE.text;
    let target_line = typed.as_ref().map(|s| {
        s.cells(current_line)
            .iter()
            .map(|cell| cell.copy.unwrap_or(' '))
            .collect::<String>()
    });

    let title = match spec.mode {
        ModeKind::Time => format!("typewriter · {} test", mode_label(spec)),
        ModeKind::Words => format!("typewriter · {} words", spec.amount),
    };

    rsx! {
        document::Title { "type-cast" }
        document::Link { rel: "preconnect", href: "https://fonts.googleapis.com" }
        document::Link { rel: "preconnect", href: "https://fonts.gstatic.com", crossorigin: "anonymous" }
        document::Stylesheet { href: "https://fonts.googleapis.com/css2?family=Courier+Prime:wght@400;700&display=swap" }
        document::Stylesheet { href: asset!("/assets/iso.css") }
        document::Stylesheet { href: asset!("/assets/typewriter.css") }
        document::Stylesheet { href: asset!("/assets/app.css") }
        div { class: "iso tw-page", "data-theme": theme().name(),
            header { class: "tw-rail",
                span { class: "tw-brand", "type-cast" }
                nav { class: "tw-modes", aria_label: "Test length",
                    for kind in [ModeKind::Time, ModeKind::Words] {
                        div { class: "tw-group", role: "group",
                            aria_label: if kind == ModeKind::Time { "Seconds" } else { "Words" },
                            span { class: "tw-group-name", if kind == ModeKind::Time { "time" } else { "words" } }
                            for option in ModeSpec::ALL.into_iter().filter(|o| o.mode == kind) {
                                button {
                                    class: "tw-chip",
                                    r#type: "button",
                                    aria_pressed: "{option == spec}",
                                    onclick: choose(option),
                                    {mode_label(option)}
                                }
                            }
                        }
                    }
                }
                div { class: "tw-tools",
                    button {
                        class: "tw-chip",
                        r#type: "button",
                        aria_pressed: "{sound_on()}",
                        aria_label: "Typewriter sound",
                        onclick: move |_| {
                            let next = !sound_on();
                            sound_on.set(next);
                            storage_set(STORAGE_SOUND, if next { "on" } else { "off" });
                            focus_stage();
                        },
                        "sound"
                    }
                    button {
                        class: "tw-chip",
                        r#type: "button",
                        aria_pressed: "{theme() == Theme::Light}",
                        aria_label: "Light theme",
                        onclick: move |_| {
                            let next = if theme() == Theme::Dark { Theme::Light } else { Theme::Dark };
                            theme.set(next);
                            storage_set(STORAGE_THEME, next.name());
                            focus_stage();
                        },
                        "light"
                    }
                    button {
                        class: "tw-chip",
                        r#type: "button",
                        onclick: move |_| {
                            load(mode());
                            focus_stage();
                        },
                        "new sheet"
                    }
                }
            }
            main { class: "iso-plate tw-plate",
                div { class: "iso-plate-corners",
                    span { class: "iso-plate-corner", "data-corner": "fig", "Fig 1" }
                    span { class: "iso-plate-corner", "data-corner": "title", "{title}" }
                    span { class: "iso-plate-corner", "data-corner": "hint", "{hint}" }
                    span { class: "iso-plate-corner tw-readout", "data-corner": "readout", "data-phase": if matches!(phase(), Phase::Done(_)) { "done" } else { "" }, "{readout}" }
                }
                div {
                    id: "tw-stage",
                    class: "tw-stage",
                    tabindex: "0",
                    role: "application",
                    style: "{crop}",
                    aria_label: "Typewriter. Type the faint text on the sheet. Escape gives a fresh sheet.",
                    autofocus: true,
                    onmounted: move |event| stage.set(Some(event.data())),
                    onclick: move |_| focus_stage(),
                    onkeydown,
                    svg {
                        class: "iso-svg tw-svg",
                        view_box: "0 0 {FIGURE.width} {FIGURE.height}",
                        role: "img",
                        "aria-label": "{FIGURE.label}",
                        style: "{vars}",
                        g { dangerous_inner_html: "{FIGURE.back}" }
                        g { transform: "{FIGURE.copy_matrix}",
                            for (baseline, copy) in upcoming {
                                for (col, character) in copy.chars().enumerate().filter(|(_, c)| *c != ' ') {
                                    text {
                                        class: "tw-ink tw-copy",
                                        x: "{FIGURE.copy.x0 + (col as f64 + 0.5) * FIGURE.copy.cell:.2}",
                                        y: "{baseline:.2}",
                                        font_size: "{FIGURE.copy.font}",
                                        text_anchor: "middle",
                                        "{character}"
                                    }
                                }
                            }
                        }
                        g { class: "tw-hammer", "data-on": "{m.bell}", dangerous_inner_html: "{FIGURE.hammer}" }
                        g { class: "tw-carriage", "data-return": "{m.returning}", style: "{carriage}",
                            g { dangerous_inner_html: "{FIGURE.carriage_back}" }
                            g { dangerous_inner_html: "{FIGURE.sheet}" }
                            g { transform: "{FIGURE.paper_matrix}",
                                clipPath { id: "tw-sheet-clip",
                                    rect {
                                        x: "{text.x0 - text.margin}",
                                        y: "{text.top}",
                                        width: "{text.cols as f64 * text.cell + 2.0 * text.margin}",
                                        height: "{text.baseline - text.top + 6.0}",
                                    }
                                }
                                g { clip_path: "url(#tw-sheet-clip)",
                                    if let Some(s) = typed.as_ref() {
                                        for line in first_line..=current_line {
                                            g {
                                                key: "{line}",
                                                class: "tw-line",
                                                style: "transform: translate(0px, {-((current_line - line) as f64) * text.line_h:.2}px)",
                                                for cell in s.cells(line) {
                                                    SheetCell { cell, line_is_current: line == current_line }
                                                }
                                            }
                                        }
                                        if matches!(phase(), Phase::Ready | Phase::Running) {
                                            rect {
                                                class: "tw-cursor",
                                                x: "{FIGURE.col_x(cursor) - text.cell * 0.42:.2}",
                                                y: "{text.baseline + 2.4:.2}",
                                                width: "{text.cell * 0.84:.2}",
                                                height: "1.1",
                                            }
                                        }
                                    }
                                }
                            }
                            g { dangerous_inner_html: "{FIGURE.carriage_front}" }
                        }
                        g { dangerous_inner_html: "{FIGURE.front}" }
                        g { class: "tw-vibrator", "data-on": "{m.strike.is_some()}", dangerous_inner_html: "{FIGURE.vibrator}" }
                        g { dangerous_inner_html: "{FIGURE.mark}" }
                        for (index, key) in FIGURE.keys.iter().enumerate() {
                            g { key: "{index}", class: "tw-key", "data-down": "{m.key == Some(index)}", dangerous_inner_html: "{key.svg}" }
                        }
                        g { class: "tw-key", "data-down": "{m.space}", dangerous_inner_html: "{FIGURE.space}" }
                        if let Some(character) = m.strike {
                            if let Some(bar) = FIGURE.strikes.get(&character.to_ascii_lowercase()) {
                                g { dangerous_inner_html: "{bar}" }
                            }
                        }
                    }
                    if let Some(line) = target_line {
                        p { class: "tw-sr", "Current line: {line.trim_end()}" }
                    }
                    p { class: "tw-sr", role: "status", aria_live: "polite", "{status}" }
                }
            }
            footer { class: "iso-legend tw-legend",
                div { class: "iso-keys",
                    span { class: "iso-key", span { class: "tw-swatch", "data-ink": "hit" } "Typed right: black ribbon" }
                    span { class: "iso-key", span { class: "tw-swatch", "data-ink": "miss" } "Typed wrong: red ribbon over the copy" }
                    span { class: "iso-key", span { class: "tw-swatch", "data-ink": "copy" } "Still to type: faint copy" }
                }
            }
        }
    }
}

/// One character cell on the sheet.
#[component]
fn SheetCell(cell: session::Cell, line_is_current: bool) -> Element {
    let x = FIGURE.col_x(cell.col);
    let y = FIGURE.text.baseline;
    let size = FIGURE.text.font;
    let copy = cell.copy.map(String::from).unwrap_or_default();
    let typed = cell.typed.map(String::from).unwrap_or_default();
    match cell.ink {
        Ink::Copy if line_is_current => rsx! {
            text { class: "tw-ink tw-copy", x: "{x:.2}", y: "{y:.2}", font_size: "{size}", text_anchor: "middle", "{copy}" }
        },
        Ink::Copy => rsx! {},
        Ink::Hit => rsx! {
            text { class: "tw-ink", x: "{x:.2}", y: "{y:.2}", font_size: "{size}", text_anchor: "middle", "{typed}" }
        },
        Ink::Miss => rsx! {
            text { class: "tw-ink tw-copy", x: "{x:.2}", y: "{y:.2}", font_size: "{size}", text_anchor: "middle", "{copy}" }
            text { class: "tw-ink tw-miss", x: "{x:.2}", y: "{y:.2}", font_size: "{size}", text_anchor: "middle", "{typed}" }
        },
        Ink::Extra => rsx! {
            text { class: "tw-ink tw-miss", x: "{x:.2}", y: "{y:.2}", font_size: "{size}", text_anchor: "middle", "{typed}" }
        },
        Ink::Skipped => rsx! {
            text { class: "tw-ink tw-copy", x: "{x:.2}", y: "{y:.2}", font_size: "{size}", text_anchor: "middle", "{copy}" }
            text { class: "tw-ink tw-miss", x: "{x:.2}", y: "{y + 2.0:.2}", font_size: "{size}", text_anchor: "middle", "_" }
        },
    }
}
