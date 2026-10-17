import copy
import builtins
import hashlib
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from PIL import Image
import psd_cache


class PoseCacheTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.source = self.root / 'source'
        self.source.write_bytes(b'captured PSD')
        self.cache = self.root / 'cache'
        self.cache.mkdir()
        digest = hashlib.sha256(self.source.read_bytes()).hexdigest()
        self.info = dict(width=3, height=2)
        self.resource = dict(path=str(self.source), sha256=digest, probe=dict(type='psd', info=self.info))
        self.pose = dict(source_sha256=digest, selections=[dict(group='/1', option='/1/0')])
        self.image = Image.new('RGBA', (3, 2), (20, 40, 80, 128))
        for name, result in [('load', (None, {}, 12)), ('probe', self.info), ('render', self.image)]:
            mock = patch('psd_pose.' + name, return_value=result)
            setattr(self, name, mock.start())
            self.addCleanup(mock.stop)

    def draw(self, pose=None):
        return psd_cache.pose_image(self.resource, pose or self.pose, self.cache)

    def test_hit_keeps_full_pixels_and_other_pose_does_not_hit(self):
        first = self.draw()
        second = self.draw()
        self.assertEqual(first.tobytes(), second.tobytes())
        self.assertEqual(second.size, (3, 2))
        self.assertEqual(self.render.call_count, 1)
        pose = copy.deepcopy(self.pose)
        pose['selections'][0]['option'] = '/1/1'
        self.draw(pose)
        self.assertEqual(self.render.call_count, 2)

    def test_hit_does_not_import_the_psd_parser(self):
        self.draw()
        original_import = builtins.__import__
        def guarded(name, *args, **kwargs):
            if name == 'psd_pose':
                raise AssertionError('cache hit must not initialize the PSD parser')
            return original_import(name, *args, **kwargs)
        with patch('builtins.__import__', side_effect=guarded):
            result = self.draw()
        self.assertEqual(result.tobytes(), self.image.tobytes())
        result.close()

    def test_hit_rejects_changed_original(self):
        self.draw()
        self.source.write_bytes(b'changed PSD')
        with self.assertRaises(ValueError):
            self.draw()
        self.assertEqual(self.render.call_count, 1)

    def test_corrupt_cache_and_incomplete_entry_fail(self):
        self.draw()
        png = next(self.cache.glob('*.png'))
        png.write_bytes(b'broken')
        with self.assertRaises(ValueError):
            self.draw()
        next(self.cache.glob('*.json')).unlink()
        with self.assertRaisesRegex(ValueError, 'Incomplete'):
            self.draw()

    def test_raster_hit_still_rejects_changed_original_and_disk_cache(self):
        self.draw()
        def resized():
            return psd_cache.pose_image(self.resource, self.pose, self.cache, target=(2, 1))
        resized().close(); resized().close()
        self.source.write_bytes(b'changed PSD')
        with self.assertRaises(ValueError): resized()
        self.source.write_bytes(b'captured PSD')
        next(self.cache.glob('*.png')).write_bytes(b'broken PNG')
        with self.assertRaises(ValueError): resized()

    def test_invalid_catalog_never_cached(self):
        self.probe.return_value = dict(width=4, height=2)
        with self.assertRaises(ValueError):
            self.draw()
        self.assertEqual(list(self.cache.iterdir()), [])

    def test_limit_falls_back_to_original_composition(self):
        with patch.object(psd_cache, 'MAX_ENTRIES', 0):
            self.assertEqual(self.draw().tobytes(), self.image.tobytes())
        with patch.object(psd_cache, 'MAX_CACHE_BYTES', 1):
            self.draw()
        self.assertEqual(list(self.cache.iterdir()), [])
        self.assertEqual(self.render.call_count, 2)

    def test_symlink_directory_is_rejected(self):
        link = self.root / 'link'
        link.symlink_to(self.cache, target_is_directory=True)
        with self.assertRaises(ValueError):
            psd_cache.pose_image(self.resource, self.pose, link)


if __name__ == '__main__':
    unittest.main()
