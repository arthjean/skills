#!/usr/bin/env python3
"""Lint a Markdown article draft against the write-articles contract.

Errors (exit 1): dashes in prose, placeholders (TODO, TBD, FIXME, XXX),
embedded assets that do not exist.
Warnings: stock phrases, formulaic oppositions, overlong paragraphs,
generic headings, images without alt text, hyphenated names to confirm,
root-relative assets left unverified without --assets-root.
Also prints the heading outline, word count, and reading time.
"""
import argparse
import re
import sys
from pathlib import Path

STOCK_PHRASES = [
    "a testament to", "at the end of the day", "it's worth noting", "it is worth noting",
    "it's important to note", "it is important to note", "needless to say", "without further ado",
    "let's dive", "dive into", "deep dive into", "delve", "buckle up", "in today's",
    "fast-paced", "ever-evolving", "ever-changing", "game changer", "game-changer",
    "seamless", "seamlessly", "robust", "leverage", "leveraging", "unlock", "unleash",
    "supercharge", "revolutionize", "revolutionary", "cutting-edge", "next-level",
    "empower", "elevate", "harness the power", "the power of", "plays a crucial role",
    "pivotal", "myriad", "tapestry", "embark", "journey", "landscape", "realm",
    "navigate the", "in the world of", "in this article", "in this post, we",
    "in this blog post", "in conclusion", "to sum up", "in summary", "to summarize",
    "final thoughts", "stay tuned", "the best part?", "here's the thing",
]

OPPOSITION = re.compile(
    r"\b(it'?s|this is|that'?s) not (just |only |merely )?[^.;:!?]{1,60}[,;] (it'?s|but)\b"
    r"|\bnot only\b[^.;!?]{1,80}\bbut also\b",
    re.IGNORECASE,
)

GENERIC_HEADINGS = {
    "introduction", "intro", "overview", "background", "conclusion", "conclusions",
    "summary", "final thoughts", "closing thoughts", "wrapping up", "wrap up",
    "takeaways", "key takeaways", "the problem", "the solution", "results",
}

MAX_PARAGRAPH_WORDS = 120
WORDS_PER_MINUTE = 230

FENCE = re.compile(r"^\s*(```|~~~)")
HEADING = re.compile(r"^(#{1,6})\s+(.*?)\s*#*\s*$")
LIST_ITEM = re.compile(r"^\s*([-*+]|\d+[.)])\s+")
TABLE_RULE = re.compile(r"^\s*\|?\s*:?-{2,}:?\s*(\|\s*:?-{2,}:?\s*)*\|?\s*$")
HR = re.compile(r"^\s*([-*_])(\s*\1){2,}\s*$")
IMAGE = re.compile(r"!\[([^\]]*)\]\(([^)]*)\)")
TODO = re.compile(r"\b(TODO|TBD|FIXME|XXX)\b")
ASSET_ATTR = re.compile(r"""\b(?:src|poster)\s*=\s*["']([^"']+)["']""")
WORD = re.compile(r"[\w'’]+", re.UNICODE)

EDGE_PUNCTUATION = ".,;:!?()[]{}\"'“”‘’«»*_"
DASH_GLYPHS = re.compile("[\\u2012\\u2013\\u2014\\u2015]")
ISO_DATE = re.compile(r"(?<![\w.-])\d{4}-\d{2}-\d{2}(?![\w.-])")
NUMERIC_RANGE = re.compile(r"(?<![\w.-])\d+(?:[.,]\d+)?-\d+(?:[.,]\d+)?(?![\w.-])")


def strip_front_matter(lines):
    if lines and lines[0].strip() == "---":
        for i in range(1, len(lines)):
            if lines[i].strip() in ("---", "..."):
                return [""] * (i + 1) + lines[i + 1:]
    return lines


def mask_code_blocks(lines):
    masked, inside = [], False
    for line in lines:
        if FENCE.match(line):
            inside = not inside
            masked.append("")
            continue
        masked.append("" if inside else line)
    return masked


def prose_view(line):
    text = re.sub(r"`[^`]*`", " ", line)
    text = IMAGE.sub(lambda m: " " + m.group(1) + " ", text)
    text = re.sub(r"\]\([^)]*\)", "]", text)
    text = re.sub(r"<[^>]+>", " ", text)
    text = re.sub(r"https?://\S+|www\.\S+", " ", text)
    text = re.sub(r"\S+@\S+\.\S+", " ", text)
    text = re.sub(r"\S*[/\\]\S*", " ", text)
    text = re.sub(r"\S+:\d+(-\d+)?\b", " ", text)
    text = re.sub(r"\S+\.(md|mdx|rs|py|ts|tsx|js|json|toml|yml|yaml|sh|ps1)\b", " ", text)
    text = LIST_ITEM.sub(" ", text, count=1)
    text = re.sub(r"^\s*>\s?", " ", text)
    return text


def looks_like_name(token, allowed):
    if token in allowed:
        return True
    parts = token.split("-")
    if any(ch.isdigit() for ch in token):
        return True
    return any(any(c.isupper() for c in part[1:]) or (i > 0 and part[:1].isupper())
               for i, part in enumerate(parts))


def snippet(text, start, end, width=40):
    a, b = max(0, start - width), min(len(text), end + width)
    return text[a:b].strip().replace("\n", " ")


def lint(path, lang, allowed, assets_root=None):
    raw = Path(path).read_text(encoding="utf-8").splitlines()
    lines = mask_code_blocks(strip_front_matter(raw))
    errors, warnings, outline = [], [], []
    words = 0
    paragraph, paragraph_start = [], 0

    def flush():
        nonlocal paragraph
        if paragraph:
            count = len(WORD.findall(" ".join(paragraph)))
            if count > MAX_PARAGRAPH_WORDS:
                warnings.append((paragraph_start, "long paragraph",
                                 f"{count} words, split it at its second idea"))
        paragraph = []

    for number, line in enumerate(lines, start=1):
        if not line.strip() or TABLE_RULE.match(line) or HR.match(line):
            flush()
            continue

        heading = HEADING.match(line)
        if heading:
            flush()
            level, title = len(heading.group(1)), heading.group(2)
            outline.append((level, title))
            if title.strip().lower().rstrip(":") in GENERIC_HEADINGS:
                warnings.append((number, "generic heading",
                                 f'"{title}" names a slot, not the section\'s point'))
        elif LIST_ITEM.match(line) or line.lstrip().startswith("|"):
            flush()
        else:
            if not paragraph:
                paragraph_start = number
            paragraph.append(prose_view(line))

        for match in IMAGE.finditer(line):
            if not match.group(1).strip():
                warnings.append((number, "image without alt text", match.group(0)))

        text = prose_view(heading.group(2) if heading else line)
        words += len(WORD.findall(text))

        for match in DASH_GLYPHS.finditer(text):
            errors.append((number, "dash", snippet(text, match.start(), match.end())))
        for match in ISO_DATE.finditer(text):
            errors.append((number, "ISO date in prose, write it out (November 19, 2026)",
                           match.group(0)))
        for match in re.finditer(r"(?<=\S) -{1,2} (?=\S)", text):
            errors.append((number, "spaced hyphen used as a dash",
                           snippet(text, match.start(), match.end())))
        for match in NUMERIC_RANGE.finditer(text):
            errors.append((number, "numeric range with a hyphen, write 'to'",
                           snippet(text, match.start(), match.end())))

        if lang == "en":
            for raw_token in text.split():
                token = raw_token.strip(EDGE_PUNCTUATION)
                if ("-" not in token.strip("-") or NUMERIC_RANGE.fullmatch(token)
                        or ISO_DATE.fullmatch(token)):
                    continue
                if looks_like_name(token, allowed):
                    if token not in allowed:
                        warnings.append((number, "hyphenated name, keep only if official",
                                         token))
                else:
                    errors.append((number, "hyphenated compound, open it or rephrase",
                                   token))
            lowered = text.lower()
            for phrase in STOCK_PHRASES:
                for match in re.finditer(r"(?<!\w)" + re.escape(phrase) + r"(?!\w)", lowered):
                    warnings.append((number, "stock phrase",
                                     snippet(text, match.start(), match.end())))
            for match in OPPOSITION.finditer(text):
                warnings.append((number, "formulaic opposition",
                                 snippet(text, match.start(), match.end())))
    flush()

    todos = 0
    for number, line in enumerate(lines, start=1):
        for match in TODO.finditer(line):
            todos += 1
            errors.append((number, "placeholder left in the article, move the gap to the delivery report",
                           snippet(line, match.start(), match.end())))

    article_dir = Path(path).resolve().parent
    for number, line in enumerate(lines, start=1):
        sources = [m.group(2) for m in IMAGE.finditer(line)] + ASSET_ATTR.findall(line)
        for source in sources:
            target = source.split()[0].split("#")[0].split("?")[0]
            if not target or re.match(r"^(https?:)?//|^data:|^\{", target):
                continue
            if target.startswith("/"):
                if not assets_root:
                    warnings.append((number, "asset not verified, pass --assets-root", target))
                    continue
                candidate = Path(assets_root) / target.lstrip("/")
            else:
                candidate = article_dir / target
            if not candidate.exists():
                errors.append((number, "embedded asset does not exist", target))
    return errors, warnings, outline, words, todos


def main():
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("draft", help="Markdown or MDX file to lint")
    parser.add_argument("--lang", default="en",
                        help="article language code; compound and phrase checks run for en only")
    parser.add_argument("--allow", default="",
                        help="comma separated hyphenated names to accept, e.g. 'GPT-5.5,Wi-Fi'")
    parser.add_argument("--assets-root", default=None,
                        help="directory that root-relative asset paths (/images/x.webp) resolve against")
    args = parser.parse_args()

    allowed = {item.strip() for item in args.allow.split(",") if item.strip()}
    errors, warnings, outline, words, todos = lint(args.draft, args.lang.lower(), allowed,
                                                   args.assets_root)

    minutes = max(1, round(words / WORDS_PER_MINUTE))
    print(f"article: {args.draft} ({words} words, about {minutes} min read)")
    print("outline:")
    for level, title in outline:
        print(f"  {'  ' * max(0, level - 1)}{'#' * level} {title}")
    if not outline:
        print("  (no headings)")
    print(f"todo markers: {todos}")

    for label, items in (("errors", errors), ("warnings", warnings)):
        print(f"{label} ({len(items)}):")
        for number, kind, detail in items:
            print(f"  L{number} {kind}: {detail}")

    sys.exit(1 if errors else 0)


if __name__ == "__main__":
    main()
