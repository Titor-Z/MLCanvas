#!/usr/bin/env python3
"""生成应用图标源图 app-icon.png（1024×1024 RGBA）：Apple 网格玻璃砖 + 品牌渐变「聿」字标。

PIL 负责超采样绘制与抗锯齿，产物交给 `tauri icon` 生成全平台资产。
用法：npm run icon
"""
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFilter, ImageFont

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "app-icon.png"

SIZE = 1024
SS = 4  # 超采样倍数
N = SIZE * SS
PLATE = 824 * SS
RADIUS = 185 * SS
P0 = (N - PLATE) // 2

G0, G1, G2 = (0x5B, 0x7C, 0xFA), (0x8B, 0x5C, 0xF6), (0x22, 0xD3, 0xEE)


def gradient(w: int, h: int, stops, horizontal: float = 0.65, vertical: float = 0.45) -> Image.Image:
    """左上→右下多段线性渐变（三段）。"""
    img = Image.new("RGB", (w, h))
    data = bytearray()
    for y in range(h):
        for x in range(w):
            t = min(1.0, (x / w) * horizontal + (y / h) * vertical)
            if t < 0.5:
                a, b, u = stops[0], stops[1], t * 2
            else:
                a, b, u = stops[1], stops[2], (t - 0.5) * 2
            data += bytes(round(a[i] + (b[i] - a[i]) * u) for i in range(3))
    img.frombytes(bytes(data))
    return img


def main() -> None:
    canvas = Image.new("RGBA", (N, N), (0, 0, 0, 0))

    # 1) 砖体轮廓
    plate_mask = Image.new("L", (N, N), 0)
    ImageDraw.Draw(plate_mask).rounded_rectangle(
        (P0, P0, P0 + PLATE, P0 + PLATE), radius=RADIUS, fill=255
    )

    # 2) 外侧柔和投影
    shadow = Image.new("L", (N, N), 0)
    ImageDraw.Draw(shadow).rounded_rectangle(
        (P0, P0 + 10 * SS, P0 + PLATE, P0 + PLATE + 10 * SS), radius=RADIUS, fill=76
    )
    shadow = shadow.filter(ImageFilter.GaussianBlur(24 * SS))
    canvas.alpha_composite(
        Image.merge("RGBA", (Image.new("L", (N, N), 10), Image.new("L", (N, N), 12), Image.new("L", (N, N), 28), shadow))
    )

    # 3) 玻璃砖：白 → 冷灰的竖向渐变
    top, bottom = 255, 232
    band = Image.new("L", (1, PLATE))
    for y in range(PLATE):
        t = (y / PLATE) ** 2
        band.putpixel((0, y), round(top + (bottom - top) * t))
    band = band.resize((PLATE, PLATE))
    plate = Image.merge(
        "RGBA",
        (band, ImageChops.add(band, Image.new("L", (PLATE, PLATE), 4)), ImageChops.add(band, Image.new("L", (PLATE, PLATE), 14)),
         Image.new("L", (PLATE, PLATE), 255)),
    )
    layer = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    layer.paste(plate, (P0, P0))
    canvas.alpha_composite(Image.composite(layer, Image.new("RGBA", (N, N), (0, 0, 0, 0)), plate_mask))

    # 4) 描边内高光（玻璃厚度感）
    edge = Image.new("L", (N, N), 0)
    ImageDraw.Draw(edge).rounded_rectangle(
        (P0 + 2 * SS, P0 + 2 * SS, P0 + PLATE - 2 * SS, P0 + PLATE - 2 * SS),
        radius=RADIUS - 2 * SS, outline=255, width=3 * SS,
    )
    edge = ImageChops.multiply(edge, plate_mask).filter(ImageFilter.GaussianBlur(SS))
    edge = edge.point(lambda v: v * 3 // 5)
    canvas.alpha_composite(
        Image.merge("RGBA", (Image.new("L", (N, N), 255), Image.new("L", (N, N), 255), Image.new("L", (N, N), 255), edge))
    )

    # 5) 「聿」字标：PingFang SC Semibold 字面 + 品牌渐变
    font = ImageFont.truetype("/System/Library/Fonts/PingFang.ttc", 540 * SS, index=8)
    glyph = Image.new("L", (N, N), 0)
    gd = ImageDraw.Draw(glyph)
    box = gd.textbbox((0, 0), "聿", font=font)
    gd.text(
        (
            (N - (box[2] - box[0])) // 2 - box[0],
            (N - (box[3] - box[1])) // 2 - box[1] - 8 * SS,
        ),
        "聿",
        font=font,
        fill=255,
    )

    color = gradient(N, N, (G0, G1, G2)).convert("RGBA")
    color.putalpha(glyph)
    canvas.alpha_composite(color)

    canvas.resize((SIZE, SIZE), Image.Resampling.LANCZOS).save(OUT)
    print(f"written: {OUT}")


if __name__ == "__main__":
    main()
