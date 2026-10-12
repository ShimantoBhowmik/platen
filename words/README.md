# Word list

`english.txt` holds the 1,000 most common everyday English words, most frequent first, one
lowercase word per line. Tests draw their text from it at random. `build_english.py` generates
it; see that file for the filters and the words it excludes.

## License and attribution

`english.txt` is derived from the word frequency data of
[wordfreq](https://github.com/rspeer/wordfreq) 3.1.1 by Robyn Speer, which is licensed under
[CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/). `english.txt` is therefore
also licensed under CC BY-SA 4.0. The changes: only the most frequent plain lowercase words
are kept, and the words listed in `build_english.py` are removed. The rest of this repository
is not affected by this license.

> Robyn Speer. (2022). rspeer/wordfreq: v3.0 (v3.0.2). Zenodo.
> https://doi.org/10.5281/zenodo.7199437

wordfreq is built on these sources, which its terms ask users to credit:

- Google Books Ngrams (<http://books.google.com/ngrams>).
- SUBTLEX-US and SUBTLEX-UK by Marc Brysbaert et al. SUBTLEX is freely available data
  (<http://crr.ugent.be/programs-data/subtitle-frequencies>).
- OpenSubtitles (<http://www.opensubtitles.org/>), via OPUS OpenSubtitles 2018.
- The Leeds Internet Corpus, University of Leeds Centre for Translation Studies.
- Wikipedia (<http://www.wikipedia.org>).
- ParaCrawl (<https://paracrawl.eu>).

## Regenerate

Requires Python 3.10 or later.

```bash
python3 -m venv .venv-words && .venv-words/bin/pip install wordfreq==3.1.1
```

```bash
.venv-words/bin/python words/build_english.py
```

Changing the list changes the text for every seed. Update the pinned sequence test in
`src/words.rs`. Tests issued before a deploy that changes the list are scored against the new
text, so deploy list changes when few tests are in progress.
