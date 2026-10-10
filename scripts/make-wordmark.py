#!/usr/bin/env python3
"""Builds assets/wordmark.svg and assets/logo.svg. Run it from the root of the
repository.

The wordmark is the name in a 5x7 pixel face on a 12px grid, after a prompt.
The logo is a box of tools in the same pixels. The letters are drawn here
rather than set in a font, so the files need no font on the machine of the
viewer and carry no font licence.
"""

CELL = 12
COLS, ROWS = 5, 7
ADVANCE = (COLS + 1) * CELL

FACE = {
    'A': ("01110", "10001", "10001", "11111", "10001", "10001", "10001"),
    'L': ("10000", "10000", "10000", "10000", "10000", "10000", "11111"),
    'T': ("11111", "00100", "00100", "00100", "00100", "00100", "00100"),
    'H': ("10001", "10001", "10001", "11111", "10001", "10001", "10001"),
    'E': ("11111", "10000", "10000", "11110", "10000", "10000", "11111"),
    'O': ("01110", "10001", "10001", "10001", "10001", "10001", "01110"),
    'S': ("01111", "10000", "10000", "01110", "00001", "00001", "11110"),
}

CHEVRON = ((0, 0), (1, 1), (2, 2), (3, 3), (4, 2), (5, 1), (6, 0))

# Teal through blue to violet. The shift runs along the word.
STOPS = (("0", "#14b8a6"), ("0.45", "#38bdf8"), ("0.8", "#818cf8"), ("1", "#a78bfa"))


def runs(cells):
    """Merge each row of cells into one rect for each run."""
    rows = {}
    for r, c in cells:
        rows.setdefault(r, set()).add(c)
    out = []
    for r in sorted(rows):
        cols = sorted(rows[r])
        start = prev = cols[0]
        for c in cols[1:] + [None]:
            if c is not None and c == prev + 1:
                prev = c
                continue
            out.append((r, start, prev - start + 1))
            if c is not None:
                start = prev = c
    return out


def glyph(char, x0, y0):
    rects = []
    cells = [(r, c) for r, row in enumerate(FACE[char]) for c, bit in enumerate(row) if bit == "1"]
    for r, c, n in runs(cells):
        rects.append(f'<rect x="{x0 + c * CELL}" y="{y0 + r * CELL}" width="{n * CELL}" height="{CELL}"/>')
    return rects


def gradient(name, width, height):
    """One gradient across the whole image, and not one for each rect."""
    stops = "".join(f'<stop offset="{o}" stop-color="{c}"/>' for o, c in STOPS)
    return (
        f'<linearGradient id="{name}" gradientUnits="userSpaceOnUse" '
        f'x1="0" y1="0" x2="{width}" y2="{height}">{stops}</linearGradient>'
    )


def wordmark():
    word = "ALLTHETOOLS"
    pad = 24
    x = pad
    y0 = (180 - ROWS * CELL) // 2
    rects = []
    for r, c, n in runs(CHEVRON):
        rects.append(f'<rect x="{x + c * CELL}" y="{y0 + r * CELL}" width="{n * CELL}" height="{CELL}"/>')
    x += 6 * CELL
    for ch in word:
        rects.extend(glyph(ch, x, y0))
        x += ADVANCE
    width = x - CELL + pad
    body = "\n    ".join(rects)
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} 180" width="{width}" height="180" role="img" aria-label="AllTheTools">
  <title>AllTheTools</title>
  <!-- scripts/make-wordmark.py writes this file. Change the script. -->
  <defs>{gradient("ink", width, 180)}</defs>
  <g fill="url(#ink)">
    {body}
  </g>
</svg>
'''


def logo():
    # A box of tools: a lid with a handle, a body, and a latch, on a dark
    # tile with round corners.
    art = (
        "....................",
        "....................",
        ".......######.......",
        ".......#....#.......",
        "...##############...",
        "...#............#...",
        "...##############...",
        "...#.....##.....#...",
        "...#.....##.....#...",
        "...#............#...",
        "...#............#...",
        "...#............#...",
        "...##############...",
        "....................",
        "....................",
        "....................",
    )
    cell = 12
    off = (256 - len(art[0]) * cell) // 2
    offy = (256 - len(art) * cell) // 2 + 12
    cells = [(r, c) for r, row in enumerate(art) for c, ch in enumerate(row) if ch == "#"]
    rects = "\n    ".join(
        f'<rect x="{off + c * cell}" y="{offy + r * cell}" width="{n * cell}" height="{cell}"/>'
        for r, c, n in runs(cells)
    )
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" width="256" height="256" role="img" aria-label="AllTheTools">
  <title>AllTheTools</title>
  <!-- scripts/make-wordmark.py writes this file. Change the script. -->
  <defs>
    <linearGradient id="tile" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#111827"/>
      <stop offset="1" stop-color="#030712"/>
    </linearGradient>
    {gradient("ink", 256, 256)}
  </defs>
  <rect width="256" height="256" rx="48" fill="url(#tile)"/>
  <g fill="url(#ink)">
    {rects}
  </g>
</svg>
'''


if __name__ == "__main__":
    with open("assets/wordmark.svg", "w") as f:
        f.write(wordmark())
    with open("assets/logo.svg", "w") as f:
        f.write(logo())
