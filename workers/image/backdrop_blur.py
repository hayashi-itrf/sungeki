"""Blur the accumulated opaque canvas, clipped to the effect's rectangle.
Sampling includes surrounding pixels; the crop must happen after filtering.
Later layers are composited normally and therefore stay sharp.
"""
from PIL import Image, ImageFilter


def apply(canvas, transform, sigma, opacity):
    if sigma == 0 or opacity == 0:
        return
    x, y = round(transform['x']), round(transform['y'])
    right = x + max(1, round(transform['width']))
    bottom = y + max(1, round(transform['height']))
    box = (max(0, x), max(0, y), min(canvas.width, right), min(canvas.height, bottom))
    if box[0] >= box[2] or box[1] >= box[3]:
        return
    blurred = canvas.filter(ImageFilter.GaussianBlur(sigma)).crop(box)
    if opacity != 1:
        blurred = Image.blend(canvas.crop(box), blurred, opacity)
    canvas.paste(blurred, box)
