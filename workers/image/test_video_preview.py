import base64
import hashlib
import io
from pathlib import Path
import tempfile
import unittest
from PIL import Image
from video_preview import preview


class VideoPreview(unittest.TestCase):
    def test_leading_gap_is_transparent_not_previous_pixels(self):
        result = preview(None)
        image = Image.open(io.BytesIO(base64.b64decode(result['png_base64'])))
        self.assertEqual(image.size, (1, 1))
        self.assertEqual(image.getpixel((0, 0)), (0, 0, 0, 0))

    def test_bounded_preview_and_hash_verified_raw_input(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root)/'frame.rgba'
            raw = bytes([50, 100, 150, 255])*2048*4
            path.write_bytes(raw)
            resource = dict(path=str(path), sha256=hashlib.sha256(raw).hexdigest(),
                            decoded_video=dict(width=2048, height=4, pts=-10))
            result = preview(resource)
            self.assertEqual((result['width'], result['height']), (1024, 2))
            image = Image.open(io.BytesIO(base64.b64decode(result['png_base64'])))
            self.assertEqual(image.getpixel((0, 0)), (50, 100, 150, 255))
            path.write_bytes(bytes(len(raw)))
            with self.assertRaises(ValueError):
                preview(resource)


if __name__ == '__main__':
    unittest.main()
