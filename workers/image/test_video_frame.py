import hashlib
import tempfile
import unittest
from pathlib import Path
from PIL import Image
from video_frame import decoded_image
from render_frame import image_layer


class VideoFrameTest(unittest.TestCase):
    def test_compositor_preserves_alpha_flip_and_position(self):
        pixels = bytes([255, 0, 0, 255, 0, 0, 255, 128])
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'frame.rgba'
            path.write_bytes(pixels)
            resource = dict(path=str(path), sha256=hashlib.sha256(pixels).hexdigest(),
                            decoded_video=dict(width=2, height=1, pts=-3000))
            canvas = Image.new('RGBA', (4, 2), 'white')
            image_layer(canvas, dict(asset_id='video'),
                        dict(x=1, y=1, width=2, height=1, flip_x=True), {'video': resource})
            self.assertEqual(canvas.getpixel((0, 1)), (255, 255, 255, 255))
            self.assertEqual(canvas.getpixel((1, 1)), (127, 127, 255, 255))
            self.assertEqual(canvas.getpixel((2, 1)), (255, 0, 0, 255))
            path.write_bytes(bytes(8))
            with self.assertRaisesRegex(ValueError, 'hash mismatch'):
                image_layer(canvas, dict(asset_id='video'), {}, {'video': resource})

    def test_invalid_metadata_and_partial_frame_rejected(self):
        for metadata in [dict(width=True, height=1, pts=0), dict(width=1, height=1, pts=0),
                         dict(width=20000, height=1, pts=0), dict(width=1, height=1, pts='0'),
                         dict(width=1, height=1, pts=0, extra=True)]:
            with self.assertRaises(ValueError):
                decoded_image(b'123', metadata)
