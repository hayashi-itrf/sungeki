import unittest
from PIL import ImageChops
from test_render_frame import RenderFrameTest


class BackdropBlurTest(unittest.TestCase):
    def setUp(self):
        self.h = RenderFrameTest()
        self.h.setUpClass()
        self.edge = self.h.layer(dict(type='shape', shape='rectangle', fill='#000000'), width=50)
        self.blur = self.h.layer(dict(type='blur', sigma=8), x=30, y=20, width=40, height=60)

    def draw(self, layers):
        image, report = self.h.draw(self.h.packet(layers))
        self.assertEqual(report['issues'], [])
        return image

    def test_region_order_and_surrounding_pixels(self):
        base = self.draw([self.edge])
        result = self.draw([self.edge, self.blur])
        self.assertEqual(ImageChops.difference(base, result).convert('RGB').getbbox(), (30, 20, 70, 80))
        self.assertTrue(0 < result.getpixel((49, 40))[0] < 255)
        # Cropping before filtering would miss this outside black input entirely.
        self.blur['transform'].update(x=50, width=20)
        result = self.draw([self.edge, self.blur])
        self.assertLess(result.getpixel((50, 40))[0], 200)
        cover = self.h.layer(dict(type='shape', shape='rectangle', fill='#ff0000'), x=45, width=10)
        result = self.draw([self.edge, self.blur, cover])
        self.assertEqual(result.getpixel((45, 40)), (255, 0, 0, 255))
        self.assertEqual(result.getpixel((54, 40)), (255, 0, 0, 255))
        self.assertEqual(self.draw([self.blur, self.edge]).tobytes(), base.tobytes())

    def test_zero_fractional_opacity_and_off_canvas(self):
        base = self.draw([self.edge])
        full = self.draw([self.edge, self.blur])
        self.blur['transform']['opacity'] = .5
        half = self.draw([self.edge, self.blur])
        self.assertLessEqual(abs(half.getpixel((49,40))[0] - full.getpixel((49,40))[0]/2), 1)
        self.blur['transform']['opacity'] = 0
        self.assertEqual(self.draw([self.edge, self.blur]).tobytes(), base.tobytes())
        self.blur['transform'].update(opacity=1, x=-1000)
        self.assertEqual(self.draw([self.edge, self.blur]).tobytes(), base.tobytes())
        self.blur['transform']['x'] = 30
        self.blur['primitive']['sigma'] = 0
        self.assertEqual(self.draw([self.edge, self.blur]).tobytes(), base.tobytes())

    def test_invalid_strength_is_rejected(self):
        for value in (-1,129,float('nan'),float('inf'),True):
            self.blur['primitive']['sigma'] = value
            with self.assertRaises(ValueError):
                self.draw([self.blur])
