import hashlib
import io
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from PIL import Image, ImageOps
from image_raster_cache import ImageRasterCache
from render_frame import image_layer


def encoded(color=(20, 80, 150, 180), format='PNG', orientation=None):
    image = Image.new('RGBA', (9, 7), color)
    image.putpixel((0, 0), (255, 0, 0, 255))
    if format == 'JPEG': image = image.convert('RGB')
    stream = io.BytesIO()
    exif = Image.Exif()
    if orientation: exif[274] = orientation
    image.save(stream, format=format, exif=exif)
    return stream.getvalue()


class ImageRasterCacheTest(unittest.TestCase):
    def test_formats_orientation_resize_and_caller_isolation(self):
        for format in ['PNG', 'JPEG', 'WEBP']:
            data = encoded(format=format, orientation=6)
            cache = ImageRasterCache()
            with Image.open(io.BytesIO(data)) as source:
                expected = ImageOps.exif_transpose(source).convert('RGBA').resize(
                    (5, 3), Image.Resampling.LANCZOS).tobytes()
            with cache.decode(data, (5, 3)) as first:
                self.assertEqual(first.tobytes(), expected)
                first.putpixel((0, 0), (0, 0, 0, 0))
            with patch('image_raster_cache.Image.open', side_effect=AssertionError('decoded again')):
                with cache.decode(data, (5, 3)) as second:
                    self.assertEqual(second.tobytes(), expected)

    def test_changed_bytes_budget_lru_and_invalid_input(self):
        cache = ImageRasterCache(max_bytes=32, max_entries=2)
        data = encoded()
        cache.decode(data, (2, 2)).close()
        first = next(iter(cache.entries.values()))
        cache.decode(data, (1, 4)).close()
        cache.decode(data, (2, 2)).close()
        cache.decode(encoded(color=(0, 255, 0, 255)), (2, 2)).close()
        self.assertIn(first, cache.entries.values())
        self.assertEqual((len(cache.entries), cache.bytes), (2, 32))
        cache.decode(data, (3, 3)).close()  # Too large to retain.
        self.assertEqual((len(cache.entries), cache.bytes), (2, 32))
        for data, target in [(b'broken', (2, 2)), (data, (0, 2)), (data, (True, 2)),
                             (data, (32768, 32768)), (encoded(format='GIF'), (2, 2))]:
            with self.assertRaises((ValueError, OSError)):
                cache.decode(data, target)

    def test_animation_rejected(self):
        stream = io.BytesIO()
        first = Image.new('RGBA', (2, 2), 'red')
        first.save(stream, format='PNG', save_all=True,
                   append_images=[Image.new('RGBA', (2, 2), 'blue')], duration=100)
        with self.assertRaisesRegex(ValueError, 'animation'):
            ImageRasterCache().decode(stream.getvalue(), (2, 2))

    def test_entry_limit_closes_evicted_rasters_and_zero_disables_retention(self):
        data = encoded()
        cache = ImageRasterCache(max_bytes=1024, max_entries=1)
        cache.decode(data, (2, 2)).close()
        old = next(iter(cache.entries.values()))
        cache.decode(data, (3, 3)).close()
        with self.assertRaises(ValueError): old.getpixel((0, 0))
        self.assertEqual((len(cache.entries), cache.bytes), (1, 36))
        disabled = ImageRasterCache(max_entries=0)
        disabled.decode(data, (2, 2)).close()
        self.assertEqual((len(disabled.entries), disabled.bytes), (0, 0))

    def test_render_still_checks_original_bytes_before_cache_hit(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)/'source.png'; path.write_bytes(encoded())
            resources = {'a': dict(path=str(path), sha256=hashlib.sha256(path.read_bytes()).hexdigest())}
            primitive = dict(asset_id='a')
            transform = dict(x=0, y=0, width=9, height=7)
            image_layer(Image.new('RGBA', (9, 7)), primitive, transform, resources)
            path.write_bytes(encoded(color=(0, 255, 0, 255)))
            with self.assertRaisesRegex(ValueError, 'hash mismatch'):
                image_layer(Image.new('RGBA', (9, 7)), primitive, transform, resources)
            resources['a']['sha256'] = hashlib.sha256(path.read_bytes()).hexdigest()
            result = Image.new('RGBA', (9, 7))
            image_layer(result, primitive, transform, resources)
            self.assertEqual(result.getpixel((8, 6)), (0, 255, 0, 255))


if __name__ == '__main__':
    unittest.main()
