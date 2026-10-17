"""M0 dependency probe; not the production image-worker API."""
import argparse
import hashlib
import io
import json
import platform
import sys

# Entry points must not modify resources inside a signed application bundle.
sys.dont_write_bytecode = True
import unicodedata
from importlib.metadata import version
from pathlib import Path
from uuid import uuid4

from PIL import Image, ImageDraw, ImageFont
from psd_tools import PSDImage
from psd_tools.api.layers import PixelLayer


def verify(output):
    fonts = [path for path in Path('/System/Library/Fonts').glob('*.ttc')
             if unicodedata.normalize('NFC', path.name) == 'ヒラギノ角ゴシック W3.ttc']
    if len(fonts) != 1:
        raise RuntimeError('日本語検証用のヒラギノ角ゴシック W3が見つかりません')
    font = ImageFont.truetype(str(fonts[0]), 36)
    text = '日本語の字幕を確認\nライトテーマが初期設定'
    image = Image.new('RGBA', (640, 180), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    box = draw.multiline_textbbox((24, 20), text, font=font, spacing=12)
    if box[2] >= image.width - 24 or box[3] >= image.height - 20:
        raise RuntimeError('検証字幕が範囲を超えました')
    draw.multiline_text((24, 20), text, font=font, fill=(20, 35, 55, 255), spacing=12)
    image.save(output / 'subtitle.png')
    with Image.open(output / 'subtitle.png') as decoded:
        if decoded.mode != 'RGBA' or decoded.tobytes() != image.tobytes():
            raise RuntimeError('PNGの画素が一致しません')
        if decoded.getchannel('A').getextrema() != (0, 255):
            raise RuntimeError('透明背景と文字がありません')

    psd = PSDImage.new('RGBA', (640, 180))
    layer = PixelLayer.frompil(image, psd, name='Subtitle')
    layer.name = '字幕レイヤー'  # Public setter writes the Unicode name tag.
    psd.save(output / 'fixture.psd')
    parsed = PSDImage.open(output / 'fixture.psd')
    if len(parsed) != 1 or parsed[0].name != '字幕レイヤー':
        raise RuntimeError('PSDレイヤー解析が一致しません')
    restored = parsed[0].topil()
    if restored is None or restored.convert('RGBA').tobytes() != image.tobytes():
        raise RuntimeError('PSDレイヤーの画素が一致しません')
    restored.save(output / 'psd-layer.png')
    try:
        PSDImage.open(io.BytesIO(b'invalid PSD fixture'))
    except (ValueError, OSError):
        pass
    else:
        raise RuntimeError('不正PSDを拒否しませんでした')
    return {'font': str(fonts[0]), 'fontSha256': hashlib.sha256(fonts[0].read_bytes()).hexdigest(),
            'checks': {'pngRoundtrip': 'passed', 'alpha': 'passed', 'psdLayerRoundtrip': 'passed',
                       'invalidPsd': 'passed', 'subtitleBounds': list(box),
                       'complexPsdComposite': 'not_run', 'productionSubtitleLayout': 'not_run'}}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output-root', required=True, type=Path)
    args = parser.parse_args()
    output = args.output_root / str(uuid4())
    output.mkdir(parents=True)
    report = {'python': platform.python_version(), 'executable': sys.executable,
              'packages': {name: version(name) for name in ('Pillow', 'psd-tools', 'numpy', 'attrs', 'typing-extensions')}}
    try:
        report.update(verify(output))
        report['status'] = 'passed'
    except Exception as error:
        report.update(status='failed', error=f'{type(error).__name__}: {error}')
    (output / 'report.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps({'status': report['status'], 'directory': str(output)}, ensure_ascii=False))
    return 0 if report['status'] == 'passed' else 1


if __name__ == '__main__':
    sys.exit(main())
