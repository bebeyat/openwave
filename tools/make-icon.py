"""Builds the app icons from assets/branding/openwave-emblem-source.png (white wave on black).

Outputs (run from the repo root: python tools/make-icon.py):
  assets/branding/openwave-icon-512.png  rounded-square app icon (transparent corners)
  assets/branding/openwave.ico           the same at 16-256 px, embedded into the exe
  assets/branding/openwave-emblem.png    the wave alone as an alpha mask (in-app mark, tinted by CSS)
"""
from PIL import Image, ImageChops, ImageDraw, ImageOps

SRC = "assets/branding/openwave-emblem-source.png"
OUT = "assets/branding/"

src = Image.open(SRC).convert("L")
# The wave: everything clearly brighter than the black ground.
mask = src.point(lambda v: 255 if v > 60 else 0)
left, top, right, bottom = mask.getbbox()
cx, cy = (left + right) / 2, (top + bottom) / 2
wave_w = right - left

# Rounded-square icon: the wave fills ~74% of the width, centred.
side = int(wave_w / 0.74)
box = (int(cx - side / 2), int(cy - side / 2), int(cx + side / 2), int(cy + side / 2))
crop = ImageOps.expand(src, border=side, fill=0).crop(tuple(v + side for v in box))
icon = Image.new("RGBA", (1024, 1024), (0, 0, 0, 0))
ground = Image.new("RGBA", (1024, 1024), (14, 14, 16, 255))
white = Image.new("RGBA", (1024, 1024), (255, 255, 255, 255))
art = crop.resize((1024, 1024), Image.LANCZOS)
filled = Image.composite(white, ground, art)
corner = Image.new("L", (1024, 1024), 0)
ImageDraw.Draw(corner).rounded_rectangle((0, 0, 1023, 1023), radius=230, fill=255)
icon.paste(filled, (0, 0), corner)
icon.resize((512, 512), Image.LANCZOS).save(OUT + "openwave-icon-512.png")
icon.save(OUT + "openwave.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])

# In-app mark: the wave only, as alpha (white with luminance as opacity), tight crop.
wave = src.crop((left - 4, top - 4, right + 4, bottom + 4))
h = 128
wave = wave.resize((round(wave.width * h / wave.height), h), Image.LANCZOS)
emblem = Image.new("RGBA", wave.size, (255, 255, 255, 0))
emblem.putalpha(wave)
emblem.save(OUT + "openwave-emblem.png", optimize=True)
print("icon", icon.size, "emblem", emblem.size)
