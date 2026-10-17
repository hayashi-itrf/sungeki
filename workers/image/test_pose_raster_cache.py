import io
import unittest
from PIL import Image
from pose_raster_cache import PoseRasterCache


def encoded(size=(9, 7), color=(20, 80, 150, 180), mode='RGBA'):
    image = Image.new(mode, size, color)
    stream = io.BytesIO()
    image.save(stream, format='PNG')
    return stream.getvalue()


class RasterCacheTest(unittest.TestCase):
    def test_exact_resize_and_caller_mutations_are_isolated(self):
        data = encoded()
        cache = PoseRasterCache()
        with Image.open(io.BytesIO(data)) as source:
            expected = source.resize((5, 3), Image.Resampling.LANCZOS).tobytes()
        first = cache.decode(data, (9, 7), (5, 3))
        self.assertEqual(first.tobytes(), expected)
        first.putpixel((0, 0), (255, 0, 0, 0)); first.close()
        with cache.decode(data, (9, 7), (5, 3)) as second:
            self.assertEqual(second.tobytes(), expected)
        self.assertEqual((len(cache.entries), cache.bytes), (1, 60))

    def test_changed_payload_dimensions_or_mode_cannot_hit(self):
        cache = PoseRasterCache()
        data = encoded(); cache.decode(data, (9, 7), (5, 3)).close()
        changed = encoded(color=(0, 255, 0, 255))
        with cache.decode(changed, (9, 7), (5, 3)) as result:
            self.assertEqual(result.getpixel((0, 0)), (0, 255, 0, 255))
        for bad, expected in [(b'broken', (9, 7)), (data, (8, 7)), (encoded(color='red', mode='RGB'), (9, 7))]:
            with self.assertRaises((ValueError, OSError)):
                cache.decode(bad, expected, (5, 3))

    def test_lru_byte_budget_entry_limit_and_large_bypass(self):
        cache = PoseRasterCache(max_bytes=32, max_entries=2)
        data = encoded()
        cache.decode(data, (9, 7), (2, 2)).close()
        first = next(iter(cache.entries.values()))
        cache.decode(data, (9, 7), (1, 4)).close()
        cache.decode(data, (9, 7), (2, 2)).close()  # Keep first recently used.
        cache.decode(data, (9, 7), (4, 1)).close()
        self.assertIn(first, cache.entries.values())
        self.assertEqual(cache.bytes, 32)
        cache.decode(data, (9, 7), (3, 3)).close()  # Larger than budget: no retention.
        self.assertEqual((len(cache.entries), cache.bytes), (2, 32))
        tiny = PoseRasterCache(max_bytes=1000, max_entries=1)
        tiny.decode(data, (9, 7), (1, 1)).close()
        old = next(iter(tiny.entries.values()))
        tiny.decode(data, (9, 7), (2, 2)).close()
        with self.assertRaises(ValueError): old.getpixel((0, 0))
        self.assertEqual((len(tiny.entries), tiny.bytes), (1, 16))

    def test_invalid_sizes_are_rejected_before_decode(self):
        for target in [(0, 2), (True, 2), (1.5, 2), (32768, 32768)]:
            with self.assertRaises(ValueError):
                PoseRasterCache().decode(encoded(), (9, 7), target)


if __name__ == '__main__':
    unittest.main()
