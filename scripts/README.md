# Scripts

Generators for files that are committed to the repository. Run each one from the repository
root after you change its input, and commit the output.

## Typewriter drawing

`figure/build.mjs` draws the isometric typewriter with a vendored copy of the anatomy
isometric kit (`figure/kit/`, MIT). It writes `web/assets/typewriter.json` and
`web/assets/iso.css`. CI fails when the committed drawing is stale. Requires Node 20 or later.

```bash
node scripts/figure/build.mjs
```

Add `--preview` to also write `scripts/figure/preview.html` and `preview-light.html`,
standalone pages for visual checks. They are git-ignored.

## Word list

`build_word_list.py` writes `server/words/english.txt` from wordfreq. See
`server/words/README.md` for the source, the license and what to update after a change.
Requires Python 3.10 or later.

```bash
python3 -m venv .venv-words && .venv-words/bin/pip install wordfreq==3.1.1
```

```bash
.venv-words/bin/python scripts/build_word_list.py
```
