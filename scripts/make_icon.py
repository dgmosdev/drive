"""Uygulama ikonunu üretir: assets/icon-1024.png ve assets/app-icon.png (256px).

macOS Big Sur ikon ızgarası: 1024px tuval, ortada 824px yuvarlatılmış kare,
altında yumuşak gölge. Zemin DGMOS'un siyah-beyaz kimliğine uygun koyu degrade;
ortada dünya işareti, sağ altta yükleyiciyi anlatan mavi (#009EF7) yükleme rozeti.

Kullanım: python3 -I scripts/make_icon.py <frontend/public> <assets>
"""
import sys
from PIL import Image, ImageDraw, ImageFilter

public, out = sys.argv[1], sys.argv[2]
S = 1024
BODY = 824
OFF = (S - BODY) // 2
RADIUS = 185
PRIMARY = (0, 158, 247)


def rounded_mask(size, box, radius):
    m = Image.new("L", size, 0)
    ImageDraw.Draw(m).rounded_rectangle(box, radius=radius, fill=255)
    return m


def globe_mask(diameter):
    """Frontend'deki dünya simgesi: koyu kısımlar opak (okyanus), kıtalar saydam."""
    src = Image.open(f"{public}/media/logos/default-small-transparent.png").convert("RGBA")
    r, g, b, a = src.split()
    lum = Image.merge("RGB", (r, g, b)).convert("L")
    alpha = Image.composite(Image.eval(lum, lambda v: 255 - v), Image.new("L", src.size, 0), a)
    alpha = alpha.crop(alpha.getbbox())
    return alpha.resize((diameter, diameter), Image.LANCZOS)


icon = Image.new("RGBA", (S, S), (0, 0, 0, 0))
box = (OFF, OFF, OFF + BODY, OFF + BODY)

# gölge
shadow = Image.new("RGBA", (S, S), (0, 0, 0, 0))
ImageDraw.Draw(shadow).rounded_rectangle((OFF, OFF + 12, OFF + BODY, OFF + BODY + 12), radius=RADIUS, fill=(0, 0, 0, 90))
icon.alpha_composite(shadow.filter(ImageFilter.GaussianBlur(18)))

# zemin: üstten alta #2B2F45 → #0F1120
grad = Image.new("RGBA", (S, S))
top, bottom = (43, 47, 69), (15, 17, 32)
px = grad.load()
for y in range(S):
    t = (y - OFF) / BODY
    t = min(max(t, 0.0), 1.0)
    c = tuple(round(top[i] + (bottom[i] - top[i]) * t) for i in range(3))
    for x in range(S):
        px[x, y] = (*c, 255)
icon.paste(grad, (0, 0), rounded_mask((S, S), box, RADIUS))

# iç kenar parlaması
rim = Image.new("RGBA", (S, S), (0, 0, 0, 0))
ImageDraw.Draw(rim).rounded_rectangle(box, radius=RADIUS, outline=(255, 255, 255, 34), width=3)
icon.alpha_composite(rim)

# dünya
d = 470
g = globe_mask(d)
gx, gy = (S - d) // 2, (S - d) // 2 - 10
white = Image.new("RGBA", (d, d), (255, 255, 255, 255))
icon.paste(white, (gx, gy), g)

# yükleme rozeti
br = 132
cx, cy = gx + d - 40, gy + d - 40
badge = Image.new("RGBA", (S, S), (0, 0, 0, 0))
bd = ImageDraw.Draw(badge)
bd.ellipse((cx - br - 14, cy - br - 14, cx + br + 14, cy + br + 14), fill=(15, 17, 32, 255))
bd.ellipse((cx - br, cy - br, cx + br, cy + br), fill=(*PRIMARY, 255))
w = 26
bd.line((cx, cy + 60, cx, cy - 58), fill="white", width=w)
bd.line((cx - 52, cy - 8, cx, cy - 62), fill="white", width=w, joint="curve")
bd.line((cx + 52, cy - 8, cx, cy - 62), fill="white", width=w, joint="curve")
for px_, py_ in ((cx, cy - 62), (cx - 52, cy - 8), (cx + 52, cy - 8), (cx, cy + 60)):
    bd.ellipse((px_ - w / 2, py_ - w / 2, px_ + w / 2, py_ + w / 2), fill="white")
icon.alpha_composite(badge)

icon.save(f"{out}/icon-1024.png", optimize=True)
icon.resize((256, 256), Image.LANCZOS).save(f"{out}/app-icon.png", optimize=True)
print("ok")
