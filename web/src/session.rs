//! The typing session: what the learner has typed, laid out on the sheet.
//!
//! This mirrors the server's replay rules (space commits a word, backspace may only reopen a
//! word typed wrong, a word test ends on its last word) so the machine shows exactly what the
//! server will score. The server stays the authority: the client only sends the keystroke log.

/// The key code the server reads as backspace.
pub const BACKSPACE: char = '\u{8}';

/// One key the learner pressed, as the session understands it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stroke {
    /// A printable character, including space.
    Char(char),
    /// Backspace.
    Back,
}

/// What a key press did, so the machine can animate it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Nothing changed: a space before any letter, or input after the end.
    Ignored,
    /// A character printed in the next cell.
    Struck,
    /// A space moved the type element one cell.
    Spaced,
    /// The type element stepped back.
    Backed,
    /// The test is over: the last word was typed exactly (word test) or spaced past.
    Finished,
}

/// How one cell of the sheet is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ink {
    /// Practice copy not typed yet.
    Copy,
    /// Typed and correct: black ribbon.
    Hit,
    /// Typed and wrong: red ribbon, the copy still faint underneath.
    Miss,
    /// Typed past the end of the word: red, nothing underneath.
    Extra,
    /// Skipped when the word was committed: copy with a red underscore.
    Skipped,
}

/// One cell on the sheet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cell {
    /// Where the cell is drawn on the line, in cells: its column plus the stretch of the word
    /// gaps before it.
    pub x: f64,
    /// The character printed by the typist, if any.
    pub typed: Option<char>,
    /// The practice copy under it, if any.
    pub copy: Option<char>,
    /// How the cell is inked.
    pub ink: Ink,
}

/// One line of the copy, laid out once when the session starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Line {
    /// Index of the line's first word.
    start: usize,
    /// Cells the copy falls short of the wrap width, spread evenly over the line's word gaps
    /// so it ends flush. Zero on the last line, which stays left-aligned, and on a line of one
    /// word, which has no gap to spread them over.
    spare: usize,
}

/// A typing session over a fixed list of words.
#[derive(Debug, Clone)]
pub struct Session {
    words: Vec<String>,
    lines: Vec<Line>,
    inputs: Vec<String>,
    slack: usize,
    finishes_on_last_word: bool,
    finished: bool,
    keys: String,
    times_ms: Vec<u32>,
}

impl Session {
    /// Lays out `words` on lines of at most `cols` cells, as many words as fit, each line but
    /// the last justified to end at `cols`.
    ///
    /// # Arguments
    ///
    /// * `words` - The test text, from the server
    /// * `cols` - Cells the copy is wrapped at
    /// * `slack` - Cells left on the paper after `cols` for extra letters; past them the
    ///   margin locks and further extras overstrike the last cell
    /// * `finishes_on_last_word` - True for a word test, false for a time test
    pub fn new(words: Vec<String>, cols: usize, slack: usize, finishes_on_last_word: bool) -> Self {
        let mut lines = Vec::new();
        let mut start = 0;
        let mut width = 0;
        for (index, word) in words.iter().enumerate() {
            let len = word.chars().count();
            let needed = if width == 0 { len } else { width + 1 + len };
            if width > 0 && needed > cols {
                let one_word = index - start == 1;
                lines.push(Line {
                    start,
                    spare: if one_word { 0 } else { cols - width },
                });
                start = index;
                width = len;
            } else {
                width = needed;
            }
        }
        lines.push(Line { start, spare: 0 });
        Self {
            words,
            lines,
            inputs: vec![String::new()],
            slack,
            finishes_on_last_word,
            finished: false,
            keys: String::new(),
            times_ms: Vec::new(),
        }
    }

    /// Applies one key press at `time_ms` after the first key.
    ///
    /// # Returns
    ///
    /// The [`Effect`], for the machine's animation and sound.
    pub fn press(&mut self, stroke: Stroke, time_ms: u32) -> Effect {
        if self.finished {
            return Effect::Ignored;
        }
        let effect = match stroke {
            Stroke::Back => self.backspace(),
            Stroke::Char(' ') => self.space(),
            Stroke::Char(character) => self.character(character),
        };
        if effect != Effect::Ignored || stroke != Stroke::Char(' ') {
            self.keys.push(match stroke {
                Stroke::Back => BACKSPACE,
                Stroke::Char(character) => character,
            });
            self.times_ms.push(time_ms);
        }
        effect
    }

    fn word_index(&self) -> usize {
        self.inputs.len() - 1
    }

    fn current(&self) -> &str {
        &self.inputs[self.word_index()]
    }

    fn target(&self) -> &str {
        &self.words[self.word_index()]
    }

    fn is_last_word(&self) -> bool {
        self.word_index() + 1 == self.words.len()
    }

    fn character(&mut self, character: char) -> Effect {
        let index = self.word_index();
        self.inputs[index].push(character);
        if self.finishes_on_last_word && self.is_last_word() && self.current() == self.target() {
            self.finished = true;
            return Effect::Finished;
        }
        Effect::Struck
    }

    fn space(&mut self) -> Effect {
        if self.current().is_empty() {
            return Effect::Ignored;
        }
        if self.is_last_word() {
            self.finished = true;
            return Effect::Finished;
        }
        self.inputs.push(String::new());
        Effect::Spaced
    }

    fn backspace(&mut self) -> Effect {
        let index = self.word_index();
        if self.inputs[index].pop().is_some() {
            return Effect::Backed;
        }
        if index == 0 || self.inputs[index - 1] == self.words[index - 1] {
            return Effect::Ignored;
        }
        self.inputs.pop();
        Effect::Backed
    }

    /// Ends a time test when its clock runs out.
    pub fn finish(&mut self) {
        self.finished = true;
    }

    /// How many lines the copy fills.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Words committed so far.
    pub fn words_done(&self) -> usize {
        self.word_index() + usize::from(self.finished)
    }

    /// Every key pressed, in the server's format.
    pub fn keys(&self) -> &str {
        &self.keys
    }

    /// Milliseconds since the first key, one per logged key.
    pub fn times_ms(&self) -> &[u32] {
        &self.times_ms
    }

    /// Which line the current word is on.
    pub fn current_line(&self) -> usize {
        self.line_of(self.word_index())
    }

    fn line_of(&self, word: usize) -> usize {
        self.lines.partition_point(|line| line.start <= word) - 1
    }

    fn words_on(&self, line: usize) -> std::ops::Range<usize> {
        let start = self.lines[line].start;
        let end = self
            .lines
            .get(line + 1)
            .map_or(self.words.len(), |next| next.start);
        start..end
    }

    /// Cells drawn on `line`, including the spaces between words.
    pub fn cells(&self, line: usize) -> Vec<Cell> {
        self.layout(line).0
    }

    /// Where the type element is over, in cells: where the next character lands.
    pub fn cursor_x(&self) -> f64 {
        self.layout(self.current_line()).1.unwrap_or(0.0)
    }

    /// Lays out one line, and the cursor position when it is the current line.
    ///
    /// Extra letters take the line's slack cells; once those are used the margin locks, and
    /// further extras overstrike the last cell, as a typewriter does at its right margin. The
    /// justification stretch is fixed from the copy, so extras push later words by whole cells.
    fn layout(&self, line: usize) -> (Vec<Cell>, Option<f64>) {
        let mut cells = Vec::new();
        let mut cursor = None;
        let mut col = 0;
        let mut extras = 0;
        let current = self.word_index();
        let words = self.words_on(line);
        let spare = self.lines[line].spare;
        let gaps = words.len().saturating_sub(1).max(1);
        // Multiplying before dividing puts the last word exactly flush at the wrap width.
        let stretch = |gaps_before: usize| (spare * gaps_before) as f64 / gaps as f64;
        for (gaps_before, word) in words.clone().enumerate() {
            if gaps_before > 0 {
                cells.push(Cell {
                    x: col as f64 + stretch(gaps_before - 1),
                    typed: None,
                    copy: None,
                    ink: Ink::Copy,
                });
                col += 1;
            }
            let shift = stretch(gaps_before);
            let copy: Vec<char> = self.words[word].chars().collect();
            let typed: Vec<char> = self
                .inputs
                .get(word)
                .map(|s| s.chars().collect())
                .unwrap_or_default();
            let committed = word < current || (self.finished && word == current);
            let cells_in_word = copy.len().max(typed.len());
            for i in 0..cells_in_word {
                if word == current && i == typed.len() {
                    cursor = Some(col as f64 + shift);
                }
                let ink = match (typed.get(i), copy.get(i)) {
                    (Some(t), Some(c)) if t == c => Ink::Hit,
                    (Some(_), Some(_)) => Ink::Miss,
                    (Some(_), None) => Ink::Extra,
                    (None, Some(_)) if committed => Ink::Skipped,
                    (None, _) => Ink::Copy,
                };
                let at = if ink == Ink::Extra && extras >= self.slack {
                    col - 1
                } else {
                    extras += usize::from(ink == Ink::Extra);
                    col += 1;
                    col - 1
                };
                cells.push(Cell {
                    x: at as f64 + shift,
                    typed: typed.get(i).copied(),
                    copy: copy.get(i).copied(),
                    ink,
                });
            }
            if word == current && typed.len() >= copy.len() {
                let locked = typed.len() > copy.len() && extras >= self.slack;
                cursor = Some(if locked { col - 1 } else { col } as f64 + shift);
            }
        }
        (cells, cursor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(words: &[&str], finishes: bool) -> Session {
        Session::new(
            words.iter().map(|w| (*w).to_owned()).collect(),
            12,
            3,
            finishes,
        )
    }

    fn type_all(s: &mut Session, text: &str) -> Vec<Effect> {
        text.chars()
            .map(|c| {
                s.press(
                    if c == BACKSPACE {
                        Stroke::Back
                    } else {
                        Stroke::Char(c)
                    },
                    0,
                )
            })
            .collect()
    }

    fn xs(s: &Session, line: usize) -> Vec<f64> {
        s.cells(line).iter().map(|cell| cell.x).collect()
    }

    #[test]
    fn wraps_words_into_lines_of_the_sheet() {
        let s = session(&["alpha", "beta", "gamma", "delta"], true);
        assert_eq!(s.line_count(), 2);
        let copy: String = s.cells(1).iter().filter_map(|cell| cell.copy).collect();
        assert_eq!(copy, "gammadelta");
    }

    #[test]
    fn fills_a_line_with_as_many_short_words_as_fit_and_ends_it_flush() {
        let words: Vec<String> = ["a"; 13].iter().map(|w| (*w).to_owned()).collect();
        let s = Session::new(words, 24, 4, true);
        assert_eq!(s.line_count(), 2);
        let first = xs(&s, 0);
        assert_eq!(first.len(), 23);
        assert_eq!(first[2], 2.0 + 1.0 / 11.0);
        assert_eq!(first[22], 23.0);
        assert_eq!(xs(&s, 1), [0.0]);
    }

    #[test]
    fn justifies_a_line_by_spreading_the_spare_cells_over_its_gaps() {
        let s = session(&["ab", "cd", "ef", "ghijkl"], true);
        assert_eq!(xs(&s, 0), [0.0, 1.0, 2.0, 5.0, 6.0, 7.0, 10.0, 11.0]);
    }

    #[test]
    fn leaves_the_last_line_and_a_one_word_line_ragged() {
        let s = session(&["abcdefghij", "kl", "mn"], true);
        assert_eq!(
            xs(&s, 0),
            [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]
        );
        assert_eq!(xs(&s, 1), [0.0, 1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn space_carries_the_type_element_across_the_stretched_gap() {
        let mut s = session(&["ab", "cd", "ef", "ghijkl"], true);
        type_all(&mut s, "ab");
        assert_eq!(s.cursor_x(), 2.0);
        type_all(&mut s, " ");
        assert_eq!(s.cursor_x(), 5.0);
        type_all(&mut s, "cd ef");
        assert_eq!(s.cursor_x(), 12.0);
        type_all(&mut s, " ");
        assert_eq!(s.cursor_x(), 0.0);
    }

    #[test]
    fn extra_letters_push_later_words_by_whole_cells_on_a_justified_line() {
        let mut s = session(&["ab", "cd", "ef", "ghijkl"], true);
        type_all(&mut s, "abx");
        assert_eq!(xs(&s, 0), [0.0, 1.0, 2.0, 3.0, 6.0, 7.0, 8.0, 11.0, 12.0]);
        assert_eq!(s.cursor_x(), 3.0);
    }

    #[test]
    fn logs_keys_in_server_format_and_skips_leading_space() {
        let mut s = session(&["ab", "cd"], true);
        type_all(&mut s, " ax\u{8}b c");
        assert_eq!(s.keys(), "ax\u{8}b c");
        assert_eq!(s.times_ms().len(), 6);
    }

    #[test]
    fn finishes_word_test_on_last_word() {
        let mut s = session(&["ab", "cd"], true);
        let effects = type_all(&mut s, "ab cd");
        assert_eq!(effects.last(), Some(&Effect::Finished));
        assert_eq!(s.press(Stroke::Char('x'), 0), Effect::Ignored);
    }

    #[test]
    fn backspace_cannot_reopen_a_correct_word() {
        let mut s = session(&["ab", "cd", "ef"], true);
        type_all(&mut s, "ab ");
        assert_eq!(s.press(Stroke::Back, 0), Effect::Ignored);
        type_all(&mut s, "cx ");
        assert_eq!(s.press(Stroke::Back, 0), Effect::Backed);
        assert_eq!(s.cursor_x(), 5.0);
    }

    #[test]
    fn inks_hits_misses_extras_and_skips() {
        let mut s = session(&["abc", "de"], true);
        type_all(&mut s, "axcz");
        let inks: Vec<Ink> = s.cells(0).iter().map(|cell| cell.ink).collect();
        assert_eq!(
            inks,
            [
                Ink::Hit,
                Ink::Miss,
                Ink::Hit,
                Ink::Extra,
                Ink::Copy,
                Ink::Copy,
                Ink::Copy
            ]
        );
        let mut s = session(&["abc", "de"], true);
        type_all(&mut s, "a ");
        let inks: Vec<Ink> = s.cells(0).iter().map(|cell| cell.ink).collect();
        assert_eq!(
            inks,
            [
                Ink::Hit,
                Ink::Skipped,
                Ink::Skipped,
                Ink::Copy,
                Ink::Copy,
                Ink::Copy
            ]
        );
    }

    #[test]
    fn cursor_follows_extra_letters() {
        let mut s = session(&["ab", "cd"], true);
        type_all(&mut s, "abx");
        assert_eq!(s.cursor_x(), 3.0);
        type_all(&mut s, " c");
        assert_eq!(s.cursor_x(), 5.0);
    }

    #[test]
    fn margin_locks_after_the_slack_cells() {
        let mut s = session(&["ab", "cd"], true);
        type_all(&mut s, "abwxyz");
        assert_eq!(xs(&s, 0), [0.0, 1.0, 2.0, 3.0, 4.0, 4.0, 5.0, 6.0, 7.0]);
        assert_eq!(s.cursor_x(), 4.0);
        type_all(&mut s, " c");
        assert_eq!(s.cursor_x(), 7.0);
    }
}
