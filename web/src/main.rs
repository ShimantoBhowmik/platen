//! platen web app: a typing test typed on an isometric typewriter.

mod api;
mod chart;
mod figure;
mod report;
mod session;
mod sound;

use std::rc::Rc;

use dioxus::prelude::*;
use dioxus::web::WebEventExt;
use gloo_timers::future::TimeoutFuture;
use web_sys::wasm_bindgen::JsCast;
use web_sys::{FocusOptions, HtmlElement};

use crate::api::{ModeKind, ModeSpec, NewTest, Report};
use crate::figure::FIGURE;
use crate::report::ResultPlate;
use crate::session::{Effect, Ink, Session, Stroke};
use crate::sound::Cue;

/// Cells before the right end of a line where the margin bell rings.
const BELL_AT: usize = 6;
/// How long the type element stays against the platen: two frames.
const STRIKE_MS: u32 = 34;
/// How long a key stays down.
const KEY_MS: u32 = 90;
const STORAGE_THEME: &str = "platen:theme";
const STORAGE_SOUND: &str = "platen:sound";

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
    /// The server's verified result. `Rc` makes the frequent clones of the phase cheap.
    Done(Rc<Report>),
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

/// Focuses the typing stage without scrolling it into view, so tapping a rail chip on a phone
/// never moves the page.
fn focus_stage() {
    // `dyn_into` is a checked downcast from the generic DOM `Element` to `HtmlElement`, which
    // is the type that can take focus.
    let stage = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id("tw-stage"))
        .and_then(|element| element.dyn_into::<HtmlElement>().ok());
    if let Some(stage) = stage {
        let options = FocusOptions::new();
        options.set_prevent_scroll(true);
        // Focus can only fail on a detached element; typing then waits for a click.
        let _ = stage.focus_with_options(&options);
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

/// Milliseconds until the whole seconds left on a countdown change, or until it ends.
///
/// The clock sleeps this long between renders, so a running time test renders once a second
/// instead of polling.
///
/// # Arguments
///
/// * `left_ms` - Milliseconds left on the countdown
///
/// # Returns
///
/// The wait, at least 1 ms, so a countdown at a whole second waits a full second
fn ms_until_next_second(left_ms: f64) -> u32 {
    // A countdown shows `ceil(left / 1000)`, which drops when `left` reaches the next lower
    // multiple of 1000.
    let next_change = ((left_ms / 1_000.0).ceil() - 1.0) * 1_000.0;
    (left_ms - next_change).max(1.0).ceil() as u32
}

/// Copies the browser's Caps Lock state from a key event into `caps_lock`.
///
/// Dioxus's modifiers do not carry Caps Lock, so this reads the raw event. The signal is set
/// only when the state changes, so an ordinary key costs no extra render.
fn sync_caps_lock(mut caps_lock: Signal<bool>, event: &KeyboardEvent) {
    let on = event.as_web_event().get_modifier_state("CapsLock");
    if *caps_lock.peek() != on {
        caps_lock.set(on);
    }
}

fn kind_name(kind: ModeKind) -> &'static str {
    match kind {
        ModeKind::Time => "time",
        ModeKind::Words => "words",
    }
}

/// Everything the machine animates, set by a key press and cleared by timers.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Motion {
    key: Option<usize>,
    space: bool,
    strike: bool,
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
    // The length last chosen for each kind, so switching kind restores it.
    let mut last_time = use_signal(|| 30_u16);
    let mut last_words = use_signal(|| 25_u16);
    let mut phase = use_signal(|| Phase::Loading);
    let mut test = use_signal(|| None::<NewTest>);
    let mut session = use_signal(|| None::<Session>);
    let mut started = use_signal(|| None::<f64>);
    let mut now = use_signal(now_ms);
    let mut motion = use_signal(Motion::default);
    let mut bell_line = use_signal(|| None::<usize>);
    let mut generation = use_signal(|| 0_u64);
    let caps_lock = use_signal(|| false);
    let calm = use_hook(reduced_motion);

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
                Ok(report) => Phase::Done(Rc::new(report)),
                Err(error) => Phase::Failed(format!("Not scored: {error}.")),
            });
        });
    };

    let run_clock = move || {
        let this = generation();
        // A word test shows words typed, not time, so only a time test needs the clock.
        let Some(limit) = mode().limit_ms().map(f64::from) else {
            return;
        };
        spawn(async move {
            loop {
                let left = limit - started().map_or(0.0, |start| now_ms() - start);
                TimeoutFuture::new(ms_until_next_second(left)).await;
                if generation() != this || phase() != Phase::Running {
                    return;
                }
                now.set(now_ms());
                let elapsed = started().map_or(0.0, |start| now() - start);
                if elapsed >= limit {
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

    let mut animate = move |effect: Effect, key: Option<usize>, line_changed: bool| {
        motion.set(Motion {
            key,
            space: matches!(effect, Effect::Spaced),
            strike: matches!(effect, Effect::Struck | Effect::Finished) && !calm,
            returning: line_changed,
            bell: motion().bell,
        });
        if sound_on() {
            sound::play(match effect {
                Effect::Spaced if line_changed => Cue::Return,
                Effect::Spaced => Cue::Space,
                Effect::Backed => Cue::Backspace,
                _ => Cue::Clack,
            });
        }
        let this = generation();
        spawn(async move {
            TimeoutFuture::new(STRIKE_MS).await;
            motion.with_mut(|m| m.strike = false);
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
        sync_caps_lock(caps_lock, &event);
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
        // Pressing the key in place; `session()` would clone every issued word on each key.
        let pressed = session.with_mut(|typed| {
            let typed = typed.as_mut()?;
            let line_before = typed.current_line();
            let effect = typed.press(stroke, time_ms);
            Some((
                line_before,
                effect,
                typed.current_line(),
                typed.cursor_col(),
            ))
        });
        let Some((line_before, effect, line_after, col)) = pressed else {
            return;
        };
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
        let key = match stroke {
            Stroke::Back => FIGURE.key_index('\u{8}'),
            Stroke::Char(c) => FIGURE.key_index(c),
        };
        animate(effect, key, line_after != line_before);
        if col + BELL_AT >= FIGURE.text.cols && bell_line() != Some(line_after) {
            bell_line.set(Some(line_after));
            ring_bell();
        }
        if effect == Effect::Finished {
            submit();
        }
    };

    let fresh_sheet = move |_| {
        load(mode());
        focus_stage();
    };

    let mut choose = move |spec: ModeSpec| {
        match spec.mode {
            ModeKind::Time => last_time.set(spec.amount),
            ModeKind::Words => last_words.set(spec.amount),
        }
        mode.set(spec);
        load(spec);
        focus_stage();
    };

    let elapsed_ms = started().map_or(0.0, |start| now() - start).max(0.0) as u32;
    // A borrow, not a clone: the page renders several times per key.
    let typed = session.read();
    let spec = mode();
    let current = phase();
    let done = match current {
        Phase::Done(ref report) => Some(Rc::clone(report)),
        _ => None,
    };
    // The countdown: whole seconds left, or words done out of the total.
    let countdown = match current {
        Phase::Ready | Phase::Running => Some(match spec.limit_ms() {
            Some(limit) => (
                limit.saturating_sub(elapsed_ms).div_ceil(1_000).to_string(),
                None,
            ),
            None => (
                typed.as_ref().map_or(0, Session::words_done).to_string(),
                Some(format!("/{}", spec.amount)),
            ),
        }),
        _ => None,
    };
    let readout = match current {
        Phase::Loading => "feeding paper".to_owned(),
        Phase::Ready => "ready".to_owned(),
        Phase::Running => "typing".to_owned(),
        Phase::Scoring => "checking the sheet".to_owned(),
        Phase::Done(_) => String::new(),
        Phase::Failed(ref message) => message.clone(),
    };
    let hint = match current {
        Phase::Ready => "Type to begin · Tab + Enter restarts",
        Phase::Running | Phase::Done(_) => "Tab + Enter restarts",
        Phase::Failed(_) => "Tab + Enter to try again",
        Phase::Loading | Phase::Scoring => "",
    };
    let status = match current {
        Phase::Done(ref report) => format!(
            "Test scored. {:.1} words per minute, {:.1} percent accuracy.",
            report.stats.wpm, report.stats.accuracy
        ),
        Phase::Failed(ref message) => message.clone(),
        _ => String::new(),
    };

    let m = motion();
    let current_line = typed.as_ref().map_or(0, Session::current_line);
    let cursor = typed.as_ref().map_or(0, Session::cursor_col);
    let travel = FIGURE.col_x(cursor);
    let [cx, cy] = FIGURE.carrier_step;
    let carrier = format!(
        "transform: translate({:.2}px, {:.2}px)",
        cx * travel,
        cy * travel
    );
    let [kx, ky] = FIGURE.key_down;
    let [vx, vy] = FIGURE.vibrator_lift;
    let [bx, by] = FIGURE.ball_strike;
    let [hx, hy] = FIGURE.hammer_swing;
    let vars = format!(
        "--kd: translate({kx:.2}px, {ky:.2}px); --vl: translate({vx:.2}px, {vy:.2}px); --bs: translate({bx:.2}px, {by:.2}px); --hs: translate({hx:.2}px, {hy:.2}px)"
    );
    let [px, py] = FIGURE.strike_point(cursor);
    let crop = format!("--cx: {:.2}%; --cy: {:.2}%", px * 100.0, py * 100.0);
    let text = &FIGURE.text;
    // The sheet shows `rows` lines: the current line starts at the top, then stays on the
    // second row with the finished line above it, so the next line is always in view.
    let first_row = current_line.saturating_sub(1);
    let line_count = typed.as_ref().map_or(0, Session::line_count);
    // One line past each edge is drawn too, clipped, so lines glide in and out as the
    // sheet feeds.
    let drawn = first_row.saturating_sub(1)..(first_row + text.rows + 1).min(line_count);
    let row_y = |line: usize| text.baseline + (line as f64 - first_row as f64) * text.line_h;
    let target_line = typed.as_ref().map(|s| {
        s.cells(current_line)
            .iter()
            .map(|cell| cell.copy.unwrap_or(' '))
            .collect::<String>()
    });
    let kind_amount = move |kind: ModeKind| match kind {
        ModeKind::Time => last_time(),
        ModeKind::Words => last_words(),
    };

    rsx! {
        document::Title { "platen" }
        document::Link { rel: "preconnect", href: "https://fonts.googleapis.com" }
        document::Link { rel: "preconnect", href: "https://fonts.gstatic.com", crossorigin: "anonymous" }
        document::Stylesheet { href: "https://fonts.googleapis.com/css2?family=Courier+Prime:wght@400;700&display=swap" }
        document::Stylesheet { href: asset!("/assets/iso.css") }
        document::Stylesheet { href: asset!("/assets/typewriter.css") }
        document::Stylesheet { href: asset!("/assets/app.css") }
        div { class: "iso tw-page", "data-theme": theme().name(),
            header { class: "tw-bar",
                span { class: "tw-brand", "platen" }
                nav { class: "tw-modes", aria_label: "Test length",
                    div { class: "tw-seg", role: "group", aria_label: "Test kind",
                        for kind in [ModeKind::Time, ModeKind::Words] {
                            button {
                                class: "tw-seg-item",
                                r#type: "button",
                                aria_pressed: "{kind == spec.mode}",
                                onclick: move |_| choose(ModeSpec { mode: kind, amount: kind_amount(kind) }),
                                {kind_name(kind)}
                            }
                        }
                    }
                    div { class: "tw-seg", role: "group",
                        aria_label: if spec.mode == ModeKind::Time { "Seconds" } else { "Words" },
                        for option in ModeSpec::ALL.into_iter().filter(|o| o.mode == spec.mode) {
                            button {
                                class: "tw-seg-item",
                                r#type: "button",
                                aria_pressed: "{option == spec}",
                                onclick: move |_| choose(option),
                                {mode_label(option)}
                            }
                        }
                    }
                }
                div { class: "tw-tools",
                    button {
                        class: "tw-icon",
                        r#type: "button",
                        aria_pressed: "{sound_on()}",
                        aria_label: "Typewriter sound",
                        title: if sound_on() { "Sound on" } else { "Sound off" },
                        onclick: move |_| {
                            let next = !sound_on();
                            sound_on.set(next);
                            storage_set(STORAGE_SOUND, if next { "on" } else { "off" });
                            focus_stage();
                        },
                        svg { view_box: "0 0 20 20", "aria-hidden": "true",
                            path { d: "M3.5 7.5h3l4.5-3.5v12l-4.5-3.5h-3z" }
                            if sound_on() {
                                path { d: "M13.6 7.6a3.4 3.4 0 0 1 0 4.8M15.8 5.4a6.5 6.5 0 0 1 0 9.2" }
                            } else {
                                path { d: "M13.5 8l4 4M17.5 8l-4 4" }
                            }
                        }
                    }
                    button {
                        class: "tw-icon",
                        r#type: "button",
                        aria_pressed: "{theme() == Theme::Light}",
                        aria_label: "Light theme",
                        title: if theme() == Theme::Light { "Light theme" } else { "Dark theme" },
                        onclick: move |_| {
                            let next = if theme() == Theme::Dark { Theme::Light } else { Theme::Dark };
                            theme.set(next);
                            storage_set(STORAGE_THEME, next.name());
                            focus_stage();
                        },
                        svg { view_box: "0 0 20 20", "aria-hidden": "true",
                            if theme() == Theme::Light {
                                circle { cx: "10", cy: "10", r: "3.4" }
                                path { d: "M10 2.5v2M10 15.5v2M2.5 10h2M15.5 10h2M4.7 4.7l1.4 1.4M13.9 13.9l1.4 1.4M4.7 15.3l1.4-1.4M13.9 6.1l1.4-1.4" }
                            } else {
                                path { d: "M15.8 12.4A6.3 6.3 0 0 1 7.6 4.2a6.3 6.3 0 1 0 8.2 8.2z" }
                            }
                        }
                    }
                    button {
                        class: "tw-icon",
                        r#type: "button",
                        aria_label: "New sheet",
                        title: "New sheet",
                        onclick: fresh_sheet,
                        svg { view_box: "0 0 20 20", "aria-hidden": "true",
                            path { d: "M4.6 8.2A5.8 5.8 0 1 1 4.4 12" }
                            path { d: "M4.2 3.8v4.6h4.6" }
                        }
                    }
                }
            }
            main { class: "iso-plate tw-plate",
                div { class: "iso-plate-corners",
                    span { class: "iso-plate-corner tw-readout", "data-corner": "readout", "{readout}" }
                }
                if let Some((big, small)) = countdown {
                    div {
                        class: "tw-count",
                        role: "timer",
                        aria_label: if spec.mode == ModeKind::Time { "Seconds left" } else { "Words typed" },
                        "data-live": "{current == Phase::Running}",
                        "{big}"
                        if let Some(small) = small {
                            span { class: "tw-count-of", "{small}" }
                        }
                    }
                }
                // The live region stays while typing and only its text changes, so screen
                // readers announce the notice when it appears.
                if matches!(current, Phase::Ready | Phase::Running) {
                    div { class: "tw-caps", role: "status",
                        if caps_lock() {
                            span { class: "tw-caps-chip", "Caps Lock" }
                        }
                    }
                }
                div {
                    id: "tw-stage",
                    class: "tw-stage",
                    tabindex: "0",
                    // Application mode passes every key to the typewriter; the result is a
                    // region, so screen readers can browse its figures and lists.
                    role: if done.is_some() { "region" } else { "application" },
                    style: "{crop}",
                    "data-done": "{done.is_some()}",
                    "data-return": "{m.returning}",
                    aria_label: if done.is_some() { "Result. Tab then Enter, or Escape, gives a fresh sheet." } else { "Typewriter. Type the faint text on the sheet. Tab then Enter, or Escape, gives a fresh sheet." },
                    autofocus: true,
                    onclick: move |_| focus_stage(),
                    onkeydown,
                    // macOS reports Caps Lock turning off only on the key's release.
                    onkeyup: move |event: KeyboardEvent| sync_caps_lock(caps_lock, &event),
                    if let Some(report) = done.clone() {
                        ResultPlate { report, spec }
                    } else {
                    svg {
                        class: "iso-svg tw-svg",
                        view_box: "0 0 {FIGURE.width} {FIGURE.height}",
                        role: "img",
                        "aria-label": "{FIGURE.label}",
                        style: "{vars}",
                        Layer { markup: Markup(&FIGURE.back) }
                        g { class: "tw-hammer", "data-on": "{m.bell}", Layer { markup: Markup(&FIGURE.hammer) } }
                        Layer { markup: Markup(&FIGURE.platen) }
                        Layer { markup: Markup(&FIGURE.sheet) }
                        g { transform: "{FIGURE.paper_matrix}",
                            clipPath { id: "tw-sheet-clip",
                                rect {
                                    x: "{text.x0 - text.margin}",
                                    y: "{text.top}",
                                    width: "{text.cols as f64 * text.cell + 2.0 * text.margin}",
                                    height: "{text.bottom - text.top}",
                                }
                            }
                            g { clip_path: "url(#tw-sheet-clip)",
                                if let Some(s) = typed.as_ref() {
                                    for line in drawn {
                                        g {
                                            key: "{line}",
                                            class: "tw-line",
                                            style: "transform: translate(0px, {row_y(line):.2}px)",
                                            for cell in s.cells(line) {
                                                SheetCell { cell }
                                            }
                                        }
                                    }
                                    if matches!(phase(), Phase::Ready | Phase::Running) {
                                        rect {
                                            class: "tw-cursor",
                                            x: "{FIGURE.col_x(cursor) - text.cell * 0.42:.2}",
                                            y: "{row_y(current_line) + 3.0:.2}",
                                            width: "{text.cell * 0.84:.2}",
                                            height: "1.1",
                                        }
                                    }
                                }
                            }
                        }
                        Layer { markup: Markup(&FIGURE.bail) }
                        Layer { markup: Markup(&FIGURE.rails) }
                        g { class: "tw-carrier", "data-return": "{m.returning}", style: "{carrier}",
                            Layer { markup: Markup(&FIGURE.carrier) }
                            g { class: "tw-vibrator", "data-on": "{m.strike}", Layer { markup: Markup(&FIGURE.vibrator) } }
                            Layer { markup: Markup(&FIGURE.mark) }
                            g { class: "tw-ball", "data-on": "{m.strike}", Layer { markup: Markup(&FIGURE.ball) } }
                        }
                        for (index, key) in FIGURE.keys.iter().enumerate() {
                            g { key: "{index}", class: "tw-key", "data-down": "{m.key == Some(index)}", Layer { markup: Markup(&key.svg) } }
                        }
                        g { class: "tw-key", "data-down": "{m.space}", Layer { markup: Markup(&FIGURE.space) } }
                    }
                    }
                    if let Some(line) = target_line.filter(|_| done.is_none()) {
                        p { class: "tw-sr", "Current line: {line.trim_end()}" }
                    }
                    p { class: "tw-sr", role: "status", aria_live: "polite", "{status}" }
                }
                // After the stage in document order, so Tab from the stage lands here and
                // Enter presses it.
                if !hint.is_empty() {
                    div { class: "iso-plate-corners",
                        button {
                            class: "iso-plate-corner tw-restart",
                            "data-corner": "hint",
                            r#type: "button",
                            onclick: fresh_sheet,
                            "{hint}"
                        }
                    }
                }
            }
            footer { class: "iso-legend tw-legend",
                if done.is_some() {
                    div { class: "iso-keys",
                        span { class: "iso-key", span { class: "tw-swatch", "data-ink": "wpm" } "wpm so far" }
                        span { class: "iso-key", span { class: "tw-swatch", "data-ink": "raw" } "raw wpm of each second" }
                        span { class: "iso-key", span { class: "tw-swatch", "data-ink": "error" } "Wrong keys per second, right scale" }
                    }
                } else {
                    div { class: "iso-keys",
                        span { class: "iso-key", span { class: "tw-swatch", "data-ink": "hit" } "Right: black ribbon" }
                        span { class: "iso-key", span { class: "tw-swatch", "data-ink": "miss" } "Wrong: red ribbon over the copy" }
                        span { class: "iso-key", span { class: "tw-swatch", "data-ink": "copy" } "Still to type: faint copy" }
                    }
                }
            }
        }
    }
}

/// Static figure markup, borrowed from [`FIGURE`] for the life of the program.
///
/// Two values are equal when they point at the same markup. That makes the comparison
/// constant-time: a re-render of the page skips every [`Layer`] without copying or diffing its
/// markup, which is about 1 MB in all.
#[derive(Debug, Clone, Copy)]
struct Markup(&'static str);

impl PartialEq for Markup {
    fn eq(&self, other: &Self) -> bool {
        // `ptr::eq` compares addresses and lengths, like Python's `is`, not the contents.
        std::ptr::eq(self.0, other.0)
    }
}

/// A group of static figure markup, rendered once and then skipped on every re-render.
#[component]
fn Layer(markup: Markup) -> Element {
    rsx! {
        g { dangerous_inner_html: markup.0 }
    }
}

/// One character cell on the sheet, drawn at baseline 0 of its line.
#[component]
fn SheetCell(cell: session::Cell) -> Element {
    let x = FIGURE.col_x(cell.col);
    let y = 0.0;
    let size = FIGURE.text.font;
    let copy = cell.copy.map(String::from).unwrap_or_default();
    let typed = cell.typed.map(String::from).unwrap_or_default();
    match cell.ink {
        Ink::Copy => rsx! {
            text { class: "tw-ink tw-copy", x: "{x:.2}", y: "{y:.2}", font_size: "{size}", text_anchor: "middle", "{copy}" }
        },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_wakes_when_the_shown_second_changes() {
        assert_eq!(ms_until_next_second(29_500.0), 500);
        assert_eq!(ms_until_next_second(29_000.0), 1_000);
        assert_eq!(ms_until_next_second(28_999.2), 1_000);
        assert_eq!(ms_until_next_second(0.4), 1);
    }

    #[test]
    fn markup_is_equal_only_to_the_same_markup() {
        let back = Markup(&FIGURE.back);
        let same_text_elsewhere = Markup(Box::leak(FIGURE.back.clone().into_boxed_str()));

        assert_eq!(back, Markup(&FIGURE.back));
        assert_ne!(back, same_text_elsewhere);
        assert_ne!(back, Markup(&FIGURE.platen));
    }
}
