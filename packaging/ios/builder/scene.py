"""Bakes Resources/scene.png, Horizon Mods' background: the launcher's pixel sunset (glass look,
launcher/src/scene.rs) in its colours, drawn small for a phone: banded sky, stars, a sun cut in
slices, clouds, three ridges with square trees. The app scales it by whole texels.

    python3 packaging/ios/builder/scene.py
"""
import math
import os
import random

from PIL import Image

SKY = [0x0a0919, 0x0f0c28, 0x151136, 0x1d1444, 0x281850, 0x361b5b, 0x481e66, 0x5d236e, 0x752873,
       0x902f76, 0xab3a75, 0xc64a71, 0xde5f6a, 0xef7c63, 0xf89d62]
SUN = [0xfff5d2, 0xffe4a0, 0xffca78, 0xffac5f, 0xff8c58, 0xfb6c62, 0xee5277]
HALO, FOG, RIM = 0xff8d6a, 0xe0697c, 0xffb27c
CLOUD_HIGH, CLOUD_LOW, CLOUD_TOP = 0x5f2c7c, 0xc95985, 0xf3a6bd
STAR_COOL, STAR_WARM = 0xd2c8ff, 0xfff3e4
RIDGES = [(0x3f1f5c, 0xc45a7f), (0x25123e, 0x7a3068), (0x100a1f, 0x2c1640)]
TREES = [["..###..", ".#####.", "#######", "#######", ".#####.", "...#...", "...#..."],
         ["..#..", "..#..", ".###.", "..#..", ".###.", "#####", "..#..", "..#.."],
         [".###.", "#####", "#####", "..#..", "..#.."]]
W, H = 200, 434
HORIZON = int(H * 0.60)
SUN_X, SUN_R = int(W * 0.68), 26


def rgb(c):
    return (c >> 16) & 255, (c >> 8) & 255, c & 255


def mix(a, b, t):
    a, b = rgb(a), rgb(b)
    return tuple(int(x + (y - x) * t) for x, y in zip(a, b))


def hexof(t):
    return t[0] << 16 | t[1] << 8 | t[2]


def sky_at(y):
    f = min(max(y / HORIZON, 0), 1) ** 2.3 * (len(SKY) - 3)
    i = int(f)
    return mix(SKY[i], SKY[min(i + 1, len(SKY) - 1)], f - i)


def noise(seed):
    r = random.Random(seed)
    pts = [r.random() for _ in range(64)]

    def at(x):
        i = int(math.floor(x)) % 63
        t = x - math.floor(x)
        t = t * t * (3 - 2 * t)
        return pts[i] + (pts[i + 1] - pts[i]) * t
    return at


def main():
    rand = random.Random(7)
    img = Image.new("RGB", (W, H))
    px = img.load()
    # Sky, with the sun's halo warming it near the horizon.
    for y in range(H):
        base = sky_at(y)
        for x in range(W):
            d = math.hypot(x - SUN_X, (y - (HORIZON - 4)) * 1.4)
            glow = max(0, 1 - d / 110) ** 2 * 0.45
            px[x, y] = tuple(int(c + (h - c) * glow) for c, h in zip(base, rgb(HALO)))
    # Stars in the upper sky.
    for _ in range(170):
        x = int(rand.random() * W)
        y = int(2 + rand.random() ** 1.8 * HORIZON * 0.7)
        b = 0.8 + rand.random() * 0.2 if rand.random() < 0.12 else 0.3 + rand.random() * 0.4
        star = STAR_COOL if rand.random() < 0.35 else STAR_WARM
        px[x, y] = mix(hexof(px[x, y]), star, b)
        if rand.random() < 0.06 and 0 < x < W - 1 and 0 < y:
            for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                px[x + dx, y + dy] = mix(hexof(px[x + dx, y + dy]), star, b * 0.45)
    # The sun, in horizontal slices, its lower part cut by gaps like the mark.
    cy = HORIZON - 6
    for y in range(cy - SUN_R, cy + SUN_R + 1):
        dy = y - cy
        k = (dy + SUN_R) / (2 * SUN_R)
        if dy > 4 and (dy // 3) % 3 == 0:
            continue
        half = int(math.sqrt(max(0, SUN_R * SUN_R - dy * dy)))
        col = rgb(SUN[min(int(k * len(SUN)), len(SUN) - 1)])
        for x in range(SUN_X - half, SUN_X + half + 1):
            if 0 <= x < W and 0 <= y < H:
                px[x, y] = col
    # Clouds: banded strips, lit along the top.
    for i in range(7):
        y = int(HORIZON - 8 - rand.random() * HORIZON * 0.45)
        cw = int(30 + rand.random() * 70)
        ch = 2 + int(rand.random() * 4)
        x0 = int(rand.random() * (W + cw)) - cw // 2
        t = min(max((y - HORIZON * 0.45) / (HORIZON * 0.55), 0), 1)
        body = mix(hexof(sky_at(y)), hexof(mix(CLOUD_HIGH, CLOUD_LOW, t * t)), 0.55)
        for r in range(ch):
            inset = (ch - 1 - r) * 3 if r < ch - 1 else 0
            inset = int(abs(r - ch / 2) * 4)
            for x in range(x0 + inset, x0 + cw - inset):
                if 0 <= x < W and 0 <= y + r < H:
                    px[x, y + r] = mix(hexof(body), CLOUD_TOP, 0.15 + t * 0.35) if r == 0 else body
    # Three ridges, nearest last, with fog between them and trees on the nearest.
    land = H - HORIZON
    specs = [(11, HORIZON, land * 0.18, 2, 9.0), (23, HORIZON + land * 0.22, land * 0.26, 3, 8.0),
             (37, HORIZON + land * 0.5, land * 0.3, 4, 7.0)]
    for n, ((seed, base, amp, block, feature), (color, rim)) in enumerate(zip(specs, RIDGES)):
        at = noise(seed)
        tops = []
        for x in range(0, W, block):
            d = abs(x - SUN_X) / (W * 0.5)
            env = 0.25 + 0.75 * min(d * d * 1.8, 1)
            v = min(max((at(x / feature) - 0.2) / 0.6, 0), 1)
            top = int(round((base - amp * env * v) / block)) * block
            tops += [top] * block
        tops = tops[:W]
        if n > 0:  # fog over what's behind
            for y in range(int(base - amp), H):
                for x in range(W):
                    px[x, y] = mix(hexof(px[x, y]), FOG, 0.08)
        for x in range(W):
            for y in range(max(tops[x], 0), H):
                px[x, y] = rgb(color)
            if 0 <= tops[x] < H:
                px[x, tops[x]] = mix(color, rim, 0.6 if n == 0 else 0.4)
        if n == 2:
            x = 6 + int(rand.random() * 10)
            while x < W - 8:
                sprite = TREES[int(rand.random() * len(TREES))]
                top = tops[min(x + 3, W - 1)] - len(sprite)
                for r, row in enumerate(sprite):
                    for c, ch in enumerate(row):
                        if ch == "#" and 0 <= x + c < W and 0 <= top + r < H:
                            px[x + c, top + r] = rgb(color)
                x += 12 + int(rand.random() * 26)
    out = os.path.join(os.path.dirname(__file__), "Resources", "scene.png")
    img.save(out)
    img.resize((W * 2, H * 2), Image.NEAREST).save("/tmp/claude-0/scene-preview.png")


main()
