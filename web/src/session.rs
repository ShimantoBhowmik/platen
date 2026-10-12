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
    /// A character printed in this cell; `correct` picks the ribbon colour.
    Struck { correct: bool },
    /// A space moved the carriage one cell; `correct` is whether the word matched.
    Spaced { correct: bool },
    /// The carriage stepped back.
    Backed,
    /// The test is over (the last word of a word test).
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    /// Column on the line.
    pub col: usize,
    /// The character printed by the typist, if any.
    pub typed: Option<char>,
    /// The practice copy under it, if any.
    pub copy: Option<char>,
    /// How the cell is inked.
    pub ink: Ink,
}

/// A typing session over a fixed list of words.
#[derive(Debug, Clone)]
pub struct Session {
    words: Vec<String>,
    /// First word of each line, laid out once from the copy.
    line_starts: Vec<usize>,
    inputs: Vec<String>,
    slack: usize,
    finishes_on_last_word: bool,
    finished: bool,
    keys: String,
    times_ms: Vec<u32>,
    correct_strokes: u32,
    total_strokes: u32,
}

impl Session {
    /// Lays out `words` on lines of at most `cols` cells.
    ///
    /// # Arguments
    ///
    /// * `words` - The test text, from the server
    /// * `cols` - Cells the copy is wrapped at
    /// * `slack` - Cells left on the paper after `cols` for extra letters; past them the
    ///   margin locks and further extras overstrike the last cell
    /// * `finishes_on_last_word` - True for a word test, false for a time test
    pub fn new(words: Vec<String>, cols: usize, slack: usize, finishes_on_last_word: bool) -> Self {
        let mut line_starts = vec![0];
        let mut width = 0;
        for (index, word) in words.iter().enumerate() {
            let len = word.chars().count();
            let needed = if width == 0 { len } else { width + 1 + len };
            if width > 0 && needed > cols {
                line_starts.push(index);
                width = len;
            } else {
                width = needed;
            }
        }
        Self {
            words,
            line_starts,
            inputs: vec![String::new()],
            slack,
            finishes_on_last_word,
            finished: false,
            keys: String::new(),
            times_ms: Vec::new(),
            correct_strokes: 0,
            total_strokes: 0,
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
        let position = self.current().chars().count();
        let correct = self.target().chars().nth(position) == Some(character);
        self.record(correct);
        let index = self.word_index();
        self.inputs[index].push(character);
        if self.finishes_on_last_word && self.is_last_word() && self.current() == self.target() {
            self.finished = true;
            return Effect::Finished;
        }
        Effect::Struck { correct }
    }

    fn space(&mut self) -> Effect {
        if self.current().is_empty() {
            return Effect::Ignored;
        }
        let correct = self.current() == self.target();
        self.record(correct);
        if self.is_last_word() {
            self.finished = true;
            return Effect::Finished;
        }
        self.inputs.push(String::new());
        Effect::Spaced { correct }
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

    fn record(&mut self, correct: bool) {
        self.total_strokes += 1;
        if correct {
            self.correct_strokes += 1;
        }
    }

    /// Ends a time test when its clock runs out.
    pub fn finish(&mut self) {
        self.finished = true;
    }

    /// How many lines the copy fills.
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
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
        self.line_starts.partition_point(|&start| start <= word) - 1
    }

    fn words_on(&self, line: usize) -> std::ops::Range<usize> {
        let start = self.line_starts[line];
        let end = self
            .line_starts
            .get(line + 1)
            .copied()
            .unwrap_or(self.words.len());
        start..end
    }

    /// Cells drawn on `line`, including the spaces between words.
    pub fn cells(&self, line: usize) -> Vec<Cell> {
        self.layout(line).0
    }

    /// The column the strike point is over: where the next character lands.
    pub fn cursor_col(&self) -> usize {
        self.layout(self.current_line()).1.unwrap_or(0)
    }

    /// Lays out one line, and the cursor column when it is the current line.
    ///
    /// Extra letters take the line's slack cells; once those are used the margin locks, and
    /// further extras overstrike the last cell, as a typewriter does at its right margin.
    fn layout(&self, line: usize) -> (Vec<Cell>, Option<usize>) {
        let mut cells = Vec::new();
        let mut cursor = None;
        let mut col = 0;
        let mut extras = 0;
        let current = self.word_index();
        let words = self.words_on(line);
        for word in words.clone() {
            if word > words.start {
                cells.push(Cell {
                    col,
                    typed: None,
                    copy: None,
                    ink: Ink::Copy,
                });
                col += 1;
            }
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
                    cursor = Some(col);
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
                    col: at,
                    typed: typed.get(i).copied(),
                    copy: copy.get(i).copied(),
                    ink,
                });
            }
            if word == current && typed.len() >= copy.len() {
                let locked = typed.len() > copy.len() && extras >= self.slack;
                cursor = Some(if locked { col - 1 } else { col });
            }
        }
        (cells, cursor)
    }

    /// Live speed from correctly typed words, by the server's rule, at `elapsed_ms`.
    pub fn live_wpm(&self, elapsed_ms: u32) -> f64 {
        if elapsed_ms == 0 {
            return 0.0;
        }
        let current = self.word_index();
        let committed: usize = (0..current)
            .filter(|&word| self.inputs[word] == self.words[word])
            .map(|word| self.words[word].chars().count() + 1)
            .sum();
        let partial = if self.target().starts_with(self.current()) {
            self.current().chars().count()
        } else {
            0
        };
        (committed + partial) as f64 / 5.0 / (f64::from(elapsed_ms) / 60_000.0)
    }

    /// Live accuracy in percent, or `None` before any character key.
    pub fn live_accuracy(&self) -> Option<f64> {
        (self.total_strokes > 0)
            .then(|| f64::from(self.correct_strokes) * 100.0 / f64::from(self.total_strokes))
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

    #[test]
    fn wraps_words_into_lines_of_the_sheet() {
        let s = session(&["alpha", "beta", "gamma", "delta"], true);
        assert_eq!(s.line_starts, [0, 2]);
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
        assert_eq!(s.cursor_col(), 5);
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
        assert_eq!(s.cursor_col(), 3);
        type_all(&mut s, " c");
        assert_eq!(s.cursor_col(), 5);
    }

    #[test]
    fn margin_locks_after_the_slack_cells() {
        let mut s = session(&["ab", "cd"], true);
        type_all(&mut s, "abwxyz");
        let cols: Vec<usize> = s.cells(0).iter().map(|cell| cell.col).collect();
        assert_eq!(cols, [0, 1, 2, 3, 4, 4, 5, 6, 7]);
        assert_eq!(s.cursor_col(), 4);
        type_all(&mut s, " c");
        assert_eq!(s.cursor_col(), 7);
    }

    #[test]
    fn live_stats_follow_server_rules() {
        let mut s = session(&["ab", "cd"], false);
        type_all(&mut s, "ax\u{8}b c");
        assert_eq!(s.live_accuracy(), Some(80.0));
        assert_eq!(s.live_wpm(60_000), 4.0 / 5.0);
    }
}
