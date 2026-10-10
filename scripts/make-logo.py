#!/usr/bin/env python3
"""Builds assets/logo.svg. Run it from the root of the repository.

The logo is a box of tools in pixels on a 12px grid, drawn here rather than
taken from a font or a picture, so the file needs nothing on the machine of
the viewer. scripts/make-banner.py draws the banner with the name.
"""

# Teal through blue to violet.
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


def gradient(name, width, height):
    """One gradient across the whole image, and not one for each rect."""
    stops = "".join(f'<stop offset="{o}" stop-color="{c}"/>' for o, c in STOPS)
    return (
        f'<linearGradient id="{name}" gradientUnits="userSpaceOnUse" '
        f'x1="0" y1="0" x2="{width}" y2="{height}">{stops}</linearGradient>'
    )


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
    with open("assets/logo.svg", "w") as f:
        f.write(logo())
