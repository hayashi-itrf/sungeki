import hashlib
import tempfile
import unittest
import unicodedata
from pathlib import Path

from PIL import Image
from render_frame import render, verified_bytes


class RenderFrameTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.font = next(path for path in Path('/System/Library/Fonts').glob('*.ttc')
                        if unicodedata.normalize('NFC', path.name) == 'ヒラギノ角ゴシック W3.ttc')
        cls.font_hash = hashlib.sha256(cls.font.read_bytes()).hexdigest()

    def packet(self, layers, width=100, height=100):
        return dict(canvas=dict(width=width, height=height, background='#ffffff'),
                    layers=layers, frame_index=0, time_us=0)

    def layer(self, primitive, **transform):
        return dict(clip_id='test', primitive=primitive,
                    transform=dict(dict(x=0, y=0, width=100, height=100, opacity=1), **transform))

    def draw(self, packet, resources=None):
        return render(packet, resources or {}, self.font, self.font_hash)

    def test_image_flip_preserves_position_alpha_and_crops_after_mirroring(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'asymmetric.png'
            source = Image.new('RGBA', (3, 1))
            source.putdata([(255, 0, 0, 255), (0, 255, 0, 128), (0, 0, 255, 255)])
            source.save(path)
            resources = {'a': dict(path=str(path), sha256=hashlib.sha256(path.read_bytes()).hexdigest())}
            primitive = dict(type='image', asset_id='a')
            image, _ = self.draw(self.packet([self.layer(primitive, x=1, y=2, width=3, height=1, flip_x=True)]), resources)
            self.assertEqual(image.getpixel((0, 2)), (255, 255, 255, 255))
            self.assertEqual(image.getpixel((1, 2)), (0, 0, 255, 255))
            self.assertEqual(image.getpixel((2, 2)), (127, 255, 127, 255))
            self.assertEqual(image.getpixel((3, 2)), (255, 0, 0, 255))
            cropped, _ = self.draw(self.packet([self.layer(primitive, x=-1, y=0, width=3, height=1, flip_x=True)]), resources)
            self.assertEqual(cropped.getpixel((1, 0)), (255, 0, 0, 255))
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), resources['a']['sha256'])

    def test_flip_cannot_silently_apply_to_text_or_accept_non_boolean(self):
        for flag in (True, 'true', 1):
            item = self.layer(dict(type='text', text='字幕', font_size=24, color='#000000'), flip_x=flag)
            with self.assertRaises(ValueError):
                self.draw(self.packet([item]))

    def test_order_opacity_and_shape_extent(self):
        red = self.layer(dict(type='shape', shape='rectangle', fill='#ff0000'))
        blue = self.layer(dict(type='shape', shape='rectangle', fill='#0000ff'), width=50, opacity=.5)
        image, report = self.draw(self.packet([red, blue]))
        self.assertEqual(image.getpixel((49, 30)), (127, 0, 128, 255))
        self.assertEqual(image.getpixel((50, 30)), (255, 0, 0, 255))
        self.assertEqual(report['issues'], [])
        inverse, _ = self.draw(self.packet([blue, red]))
        self.assertEqual(inverse.getpixel((30, 30)), (255, 0, 0, 255))
        ellipse = self.layer(dict(type='shape', shape='ellipse', fill='#000000'))
        image, _ = self.draw(self.packet([ellipse]))
        self.assertEqual(image.getpixel((0, 0)), (255, 255, 255, 255))
        self.assertEqual(image.getpixel((50, 50)), (0, 0, 0, 255))

    def test_japanese_multiline_and_overflow_are_reported(self):
        text = self.layer(dict(type='text', text='字幕\n\n二行空け', font_size=24, color='#000000'))
        image, report = self.draw(self.packet([text]))
        self.assertLess(image.crop((0, 0, 100, 25)).convert('L').getextrema()[0], 128)
        # Blank lines retain a full line advance.
        self.assertEqual(image.crop((0, 30, 100, 50)).getextrema(), ((255, 255),) * 4)
        self.assertEqual(report['issues'], [])
        text['transform']['width'] = 10
        _, report = self.draw(self.packet([text]))
        self.assertTrue(report['issues'])
        self.assertTrue(all(issue['code'] == 'TEXT_OVERFLOW' for issue in report['issues']))

    def test_outline_is_visible_and_included_in_overflow(self):
        primitive = dict(type='text', text='字幕', font_size=24, color='#ffffff',
                         outline=dict(width=3, color='#000000'))
        layer = self.layer(primitive, x=10, y=10, width=80, height=70)
        image, report = self.draw(self.packet([layer]))
        self.assertEqual(report['issues'], [])
        self.assertLess(image.convert('L').getextrema()[0], 20)
        primitive['outline'] = None
        plain, _ = self.draw(self.packet([layer]))
        self.assertEqual(plain.convert('L').getextrema(), (255, 255))
        primitive['outline'] = dict(width=3, color='#000000')
        layer['transform']['width'] = 48
        _, report = self.draw(self.packet([layer]))
        self.assertTrue(report['issues'])
        for width in [0, 33, 1.5, True]:
            primitive['outline']['width'] = width
            with self.assertRaisesRegex(ValueError, 'outline'):
                self.draw(self.packet([layer]))

    def test_original_image_hash_alpha_and_exif_orientation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            path = root / 'original.png'
            source = Image.new('RGBA', (2, 1), (255, 0, 0, 128))
            source.putpixel((1, 0), (0, 0, 255, 255))
            exif = Image.Exif(); exif[274] = 6
            source.save(path, exif=exif)
            resources = {'asset': dict(path=str(path), sha256=hashlib.sha256(path.read_bytes()).hexdigest())}
            layer = self.layer(dict(type='image', asset_id='asset'), width=1, height=2)
            image, _ = self.draw(self.packet([layer]), resources)
            self.assertEqual(image.getpixel((0, 0)), (255, 127, 127, 255))
            self.assertEqual(image.getpixel((0, 1)), (0, 0, 255, 255))
            path.write_bytes(b'changed')
            with self.assertRaisesRegex(ValueError, 'hash mismatch'):
                self.draw(self.packet([layer]), resources)
            link = root / 'link.png'; link.symlink_to(path)
            with self.assertRaises(OSError):
                verified_bytes(link, resources['asset']['sha256'])

    def test_unknown_primitives_fonts_and_large_geometry_fail(self):
        with self.assertRaisesRegex(ValueError, 'Unsupported primitive'):
            self.draw(self.packet([self.layer(dict(type='video'))]))
        with self.assertRaisesRegex(ValueError, 'hash mismatch'):
            render(self.packet([]), {}, self.font, '0' * 64)
        with self.assertRaisesRegex(ValueError, 'canvas'):
            self.draw(self.packet([], width=100000))
        with self.assertRaisesRegex(ValueError, 'geometry'):
            self.draw(self.packet([self.layer(dict(type='shape'), width=float('inf'))]))


if __name__ == '__main__':
    unittest.main()
