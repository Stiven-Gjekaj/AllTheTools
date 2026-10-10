#!/usr/bin/env python3
"""Builds assets/banner.svg. Run it from the root of the repository.

The banner is the name in the figlet font ANSI Shadow, slanted for italic,
with a gradient across it. Each character becomes shapes: a full block is a
rect, and a double line of the shadow is two strokes. So the file needs no
font on the machine of the viewer, and each viewer sees the same banner.
"""

# The output of `figlet -f "ANSI Shadow" AllTheTools`.
ART = r"""
 █████╗ ██╗     ██╗  ████████╗██╗  ██╗███████╗████████╗ ██████╗  ██████╗ ██╗     ███████╗
██╔══██╗██║     ██║  ╚══██╔══╝██║  ██║██╔════╝╚══██╔══╝██╔═══██╗██╔═══██╗██║     ██╔════╝
███████║██║     ██║     ██║   ███████║█████╗     ██║   ██║   ██║██║   ██║██║     ███████╗
██╔══██║██║     ██║     ██║   ██╔══██║██╔══╝     ██║   ██║   ██║██║   ██║██║     ╚════██║
██║  ██║███████╗███████╗██║   ██║  ██║███████╗   ██║   ╚██████╔╝╚██████╔╝███████╗███████║
╚═╝  ╚═╝╚══════╝╚══════╝╚═╝   ╚═╝  ╚═╝╚══════╝   ╚═╝    ╚═════╝  ╚═════╝ ╚══════╝╚══════╝
""".strip("\n").splitlines()

W, H = 10, 20  # one character cell, in the shape of a cell of a terminal
GAP = 2.6  # half the distance between the two strokes of a double line
SLANT = -14  # degrees: the italic
PAD = 24

STOPS = (
    ("0", "#2dd4bf"),
    ("0.3", "#38bdf8"),
    ("0.6", "#818cf8"),
    ("0.85", "#c084fc"),
    ("1", "#f472b6"),
)


def lines_for(ch, x, y):
    """The polylines of one character of a double line, as lists of points."""
    cx, cy, d = x + W / 2, y + H / 2, GAP
    right, bottom = x + W, y + H
    shapes = {
        "═": [[(x, cy - d), (right, cy - d)], [(x, cy + d), (right, cy + d)]],
        "║": [[(cx - d, y), (cx - d, bottom)], [(cx + d, y), (cx + d, bottom)]],
        "╗": [[(x, cy - d), (cx + d, cy - d), (cx + d, bottom)],
              [(x, cy + d), (cx - d, cy + d), (cx - d, bottom)]],
        "╔": [[(right, cy - d), (cx - d, cy - d), (cx - d, bottom)],
              [(right, cy + d), (cx + d, cy + d), (cx + d, bottom)]],
        "╝": [[(x, cy + d), (cx + d, cy + d), (cx + d, y)],
              [(x, cy - d), (cx - d, cy - d), (cx - d, y)]],
        "╚": [[(right, cy + d), (cx - d, cy + d), (cx - d, y)],
              [(right, cy - d), (cx + d, cy - d), (cx + d, y)]],
    }
    return shapes.get(ch, [])


def banner():
    cols = max(len(line) for line in ART)
    width_art, height_art = cols * W, len(ART) * H
    # The slant moves the top to the right, so the art starts further left.
    import math

    shift = math.tan(math.radians(-SLANT)) * height_art
    width = int(width_art + shift + 2 * PAD)
    height = height_art + 2 * PAD

    blocks, strokes = [], []
    for r, line in enumerate(ART):
        c = 0
        while c < len(line):
            if line[c] == "█":
                start = c
                while c < len(line) and line[c] == "█":
                    c += 1
                blocks.append(
                    f'<rect x="{start * W}" y="{r * H}" width="{(c - start) * W}" height="{H}"/>'
                )
                continue
            for points in lines_for(line[c], c * W, r * H):
                text = " ".join(f"{px:g},{py:g}" for px, py in points)
                strokes.append(f'<polyline points="{text}"/>')
            c += 1

    stops = "".join(f'<stop offset="{o}" stop-color="{col}"/>' for o, col in STOPS)
    body_blocks = "\n      ".join(blocks)
    body_strokes = "\n      ".join(strokes)
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" width="{width}" height="{height}" role="img" aria-label="AllTheTools">
  <title>AllTheTools</title>
  <!-- scripts/make-banner.py writes this file. Change the script. -->
  <defs>
    <linearGradient id="ink" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="{width_art}" y2="{height_art}">{stops}</linearGradient>
    <filter id="glow" x="-10%" y="-30%" width="120%" height="160%">
      <feGaussianBlur stdDeviation="5" result="blur"/>
      <feMerge><feMergeNode in="blur"/><feMergeNode in="SourceGraphic"/></feMerge>
    </filter>
  </defs>
  <g transform="translate({PAD + shift:.1f} {PAD}) skewX({SLANT})">
    <g fill="none" stroke="url(#ink)" stroke-width="1.6" stroke-linejoin="miter" opacity="0.55">
      {body_strokes}
    </g>
    <g fill="url(#ink)" filter="url(#glow)">
      {body_blocks}
    </g>
  </g>
</svg>
'''


if __name__ == "__main__":
    with open("assets/banner.svg", "w") as f:
        f.write(banner())
