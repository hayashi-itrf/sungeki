"""Rasterize a validated ScenePlan frame. Product host/worker transport is pending.
The caller supplies captured originals, not thumbnails or mutable input paths.
Text uses explicit line breaks; overflow is reported, never silently clipped.
"""
import argparse
import io
import json
import math
import os
from pathlib import Path
import re
import sys

# Imported worker modules may live inside a signed, read-only application bundle.
sys.dont_write_bytecode = True

from PIL import Image, ImageDraw, ImageFont, ImageOps

from original_bytes import verified_bytes
from text_layout import lines
from image_raster_cache import CACHE as image_rasters
from backdrop_blur import apply as blur_backdrop
Image.MAX_IMAGE_PIXELS = 32 * 1024 * 1024
FRAME_PNG_COMPRESS_LEVEL = 3  # Lossless staging: balance encoding time and SSD space.




def number(value, minimum, maximum):
    if isinstance(value, bool) or not isinstance(value, (float, int)) or not math.isfinite(value) or not minimum <= value <= maximum:
        raise ValueError('Unsupported geometry')
    return value


def color(value):
    if not isinstance(value, str) or not re.fullmatch('#[0-9a-fA-F]{6}', value):
        raise ValueError('Invalid color')
    return value


def text_layer(draw, primitive, transform, font_bytes):
    size = number(primitive['font_size'], 1, 4096)
    text = primitive['text']
    if not isinstance(text, str) or len(text.encode('utf-8')) > 65536 or '\0' in text:
        raise ValueError('Invalid text')
    font = ImageFont.truetype(io.BytesIO(font_bytes), size=size)
    outline = primitive.get('outline')
    stroke = 0
    stroke_fill = None
    if outline is not None:
        stroke = outline['width']
        if type(stroke) is not int or not 1 <= stroke <= 32:
            raise ValueError('Invalid outline width')
        stroke_fill = color(outline['color'])
    layout = primitive.get('layout') or dict(alignment='left', wrap=False)
    alignment = layout['alignment']
    if alignment not in ('left', 'center', 'right') or type(layout['wrap']) is not bool:
        raise ValueError('Invalid text layout')
    def bounds(line):
        return draw.textbbox((0, 0), line, font=font, anchor='lt', stroke_width=stroke)
    def measure(line):
        box = bounds(line)
        return box[2] - box[0]
    wrapped = lines(text, measure, transform['width'], layout['wrap'])
    boxes = []
    for index, line in enumerate(wrapped):
        if line:
            box = bounds(line)
            available = transform['width'] - (box[2] - box[0])
            shift = available / 2 if alignment == 'center' else available if alignment == 'right' else 0
            position = (transform['x'] + shift - box[0], transform['y'] + stroke + index * size * 1.3)
            boxes.append(draw.textbbox(position, line, font=font, anchor='lt', stroke_width=stroke))
            draw.text(position, line, font=font, fill=color(primitive['color']), anchor='lt', stroke_width=stroke, stroke_fill=stroke_fill)
    return boxes


def image_layer(layer, primitive, transform, resources):
    resource = resources[primitive['asset_id']]
    encoded = verified_bytes(resource['path'], resource['sha256'])
    if 'decoded_video' in resource:
        from video_frame import decoded_image
        with decoded_image(encoded, resource['decoded_video']) as source:
            paste_image(layer, source, transform)
        return
    size = (max(1, round(transform['width'])), max(1, round(transform['height'])))
    with image_rasters.decode(encoded, size) as raster:
        paste_image(layer, raster, transform)


def paste_image(layer, source, transform):
    size = (max(1, round(transform['width'])), max(1, round(transform['height'])))
    if size[0] * size[1] > Image.MAX_IMAGE_PIXELS:
        raise ValueError('Transformed image is too large')
    resized = source if source.size == size else source.resize(size, Image.Resampling.LANCZOS)
    if transform.get('flip_x', False):
        resized = ImageOps.mirror(resized)
    layer.alpha_composite(resized, (round(transform['x']), round(transform['y'])))


def psd_layer(layer, primitive, transform, resources, pose_cache=None):
    from psd_cache import pose_image
    size = (max(1, round(transform['width'])), max(1, round(transform['height'])))
    with pose_image(resources[primitive['asset_id']], primitive['pose'], pose_cache, target=size) as image:
        paste_image(layer, image, transform)


def render(packet, resources, font_path, font_hash, pose_cache=None):
    canvas = packet['canvas']
    width, height = canvas['width'], canvas['height']
    if type(width) is not int or type(height) is not int or not 1 <= width <= 8192 or not 1 <= height <= 8192 or width * height > Image.MAX_IMAGE_PIXELS:
        raise ValueError('Unsupported canvas size')
    layers = packet['layers']
    if not isinstance(layers, list) or len(layers) > 10000:
        raise ValueError('Too many layers')
    font_bytes = verified_bytes(font_path, font_hash)
    result = Image.new('RGBA', (width, height), color(canvas['background']))
    issues = []
    for item in layers:
        transform = item['transform']
        for key in ('x', 'y'):
            number(transform[key], -1000000, 1000000)
        for key in ('width', 'height'):
            number(transform[key], 0.000001, 32768)
        opacity = number(transform['opacity'], 0, 1)
        layer = Image.new('RGBA', result.size)
        draw = ImageDraw.Draw(layer)
        primitive = item['primitive']
        kind = primitive['type']
        if type(transform.get('flip_x', False)) is not bool:
            raise ValueError('flip_x must be boolean')
        if transform.get('flip_x', False) and kind not in ('image', 'psd'):
            raise ValueError('Horizontal flip requires an image or PSD')
        if kind == 'blur':
            sigma = number(primitive['sigma'], 0, 128)
            blur_backdrop(result, transform, sigma, opacity)
            continue
        if kind == 'text':
            for box in text_layer(draw, primitive, transform, font_bytes):
                outside = box[0] < 0 or box[1] < 0 or box[2] > width or box[3] > height
                outside_box = box[0] < transform['x'] or box[1] < transform['y'] or box[2] > transform['x'] + transform['width'] or box[3] > transform['y'] + transform['height']
                if outside or outside_box:
                    issues.append(dict(code='TEXT_OVERFLOW', clip_id=item['clip_id'], bounds=box, outside_canvas=outside))
        elif kind == 'shape':
            x, y = round(transform['x']), round(transform['y'])
            bounds = (x, y, x + max(1, round(transform['width'])) - 1, y + max(1, round(transform['height'])) - 1)
            if primitive['shape'] == 'rectangle':
                draw.rectangle(bounds, fill=color(primitive['fill']))
            elif primitive['shape'] == 'ellipse':
                draw.ellipse(bounds, fill=color(primitive['fill']))
            else:
                raise ValueError('Unsupported shape')
        elif kind == 'image':
            image_layer(layer, primitive, transform, resources)
        elif kind == 'psd':
            psd_layer(layer, primitive, transform, resources, pose_cache)
        else:
            raise ValueError('Unsupported primitive')
        if opacity != 1:
            layer.putalpha(layer.getchannel('A').point(lambda alpha: round(alpha * opacity)))
        result.alpha_composite(layer)
    return result, dict(frame_index=packet['frame_index'], time_us=packet['time_us'],
                        font_sha256=font_hash, layout_version=9, issues=issues)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--input', type=Path, required=True)
    parser.add_argument('--resources', type=Path)
    parser.add_argument('--font', type=Path, required=True)
    parser.add_argument('--font-sha256', required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--pose-cache', type=Path)
    args = parser.parse_args()
    if args.input.stat().st_size > 16 * 1024 * 1024:
        raise ValueError('Frame plan is too large')
    resources = json.loads(args.resources.read_text()) if args.resources else {}
    image, report = render(json.loads(args.input.read_text()), resources, args.font, args.font_sha256, args.pose_cache)
    # Output belongs to the caller's new staging directory. Never overwrite.
    with args.output.open('xb') as stream:
        image.save(stream, format='PNG', compress_level=FRAME_PNG_COMPRESS_LEVEL)
        stream.flush()
        os.fsync(stream.fileno())
    print(json.dumps(report, ensure_ascii=False))


if __name__ == '__main__':
    try:
        if sys.argv[1:2] == ['--batch']:
            del sys.argv[1]
            from render_batch import main as batch_main
            batch_main()
        else:
            main()
    except Exception as error:
        print(json.dumps(dict(error=f'{type(error).__name__}: {error}')), file=sys.stderr)
        sys.exit(1)
