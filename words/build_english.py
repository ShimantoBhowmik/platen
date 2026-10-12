"""Builds words/english.txt: the 1,000 most common everyday English words.

Source: wordfreq 3.1.1 (https://github.com/rspeer/wordfreq), whose word frequency data is
licensed CC BY-SA 4.0. The generated list is therefore also CC BY-SA 4.0; see README.md.

Run from the repository root, in a virtualenv with `pip install wordfreq==3.1.1`:

    python words/build_english.py

The list keeps wordfreq's frequency order and drops words that do not belong in a typing
drill: anything that is not plain lowercase letters, single letters other than "a", and the
words in EXCLUDED below.
"""

import re
from pathlib import Path

from wordfreq import top_n_list

SIZE = 1000

EXCLUDED = set(
    # Profanity, sexual and violent terms.
    "ass damn fuck fucking hell sex sexy shit kill killed killing murder gun guns porn".split()
    # Names of people, places, organisations, brands and religions, which English capitalises.
    + """africa america american australia australian british california canada china chinese
    christ christian christmas david england english europe european facebook france french
    george german germany god google india indian james japan jesus john london michael obama
    paul peter russia russian smith texas trump uk washington william york""".split()
    # Months and days, which English capitalises.
    + """january february april june july august september october november december monday
    tuesday wednesday thursday friday saturday sunday""".split()
    # Abbreviations, fragments and foreign words.
    + "al anti co de dr es etc ii la mr mrs ms non pre pro re san st tv vs".split()
    # Interjections and internet slang.
    + "gonna gotta hey hi lol oh ok wanna wow yeah".split()
)


def build() -> list[str]:
    """The SIZE most frequent words that pass the filters, most frequent first."""
    words = []
    for word in top_n_list("en", 5000, wordlist="best"):
        if not re.fullmatch(r"[a-z]+", word):
            continue
        if len(word) == 1 and word != "a":
            continue
        if word in EXCLUDED:
            continue
        words.append(word)
        if len(words) == SIZE:
            return words
    raise SystemExit(f"only {len(words)} words passed the filters")


if __name__ == "__main__":
    out = Path(__file__).with_name("english.txt")
    out.write_text("\n".join(build()) + "\n")
    print(f"wrote {SIZE} words to {out}")
