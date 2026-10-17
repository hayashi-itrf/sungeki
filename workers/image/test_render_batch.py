import json
import tempfile
from pathlib import Path
from PIL import Image
import unittest
import test_render_frame as single
from render_batch import read_json, run


class RenderBatchTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        single.RenderFrameTest.setUpClass()
        cls.font = single.RenderFrameTest.font
        cls.font_hash = single.RenderFrameTest.font_hash
    packet = single.RenderFrameTest.packet
    layer = single.RenderFrameTest.layer
    draw = single.RenderFrameTest.draw

    def test_inputs_reject_links_non_files_and_oversized_json(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root/'input.json'; source.write_text('{}')
            link = root/'link.json'; link.symlink_to(source)
            self.assertEqual(read_json(source, 2), {})
            for path, limit in [(link, 2), (root, 2), (source, 1)]:
                with self.assertRaises((ValueError, OSError)):
                    read_json(path, limit)
            with self.assertRaises(ValueError):
                read_json(Path('relative.json'), 2)

    def test_late_failure_does_not_return_a_success_receipt(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); resources = root/'resources.json'; resources.write_text('{}')
            source = root/'input.json'; source.write_text(json.dumps(self.packet([])))
            frame = dict(input=str(source), resources=str(resources), output='frame-0.png')
            bad = dict(frame, input=str(root/'missing.json'), output='frame-1.png')
            with self.assertRaises(FileNotFoundError):
                run(dict(version=1, frames=[frame,bad]), root, self.font, self.font_hash)
            self.assertTrue((root/'frame-0.png').exists())
            self.assertFalse((root/'frame-1.png').exists())

    def test_batch_pixels_reports_and_output_protection(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); resource = root/'resources.json'; resource.write_text('{}')
            packets = [self.packet([self.layer(dict(type='text', text='確認', font_size=20, color='#234567'))]),
                       self.packet([self.layer(dict(type='shape', shape='ellipse', fill='#abcdef'), opacity=.4)])]
            frames = []
            expected = []
            for i,p in enumerate(packets):
                p.update(frame_index=i, time_us=i*33333)
                source = root/f'input-{i}.json'; source.write_text(json.dumps(p))
                frames.append(dict(input=str(source), resources=str(resource), output=f'frame-{i}.png'))
                expected.append(self.draw(p))
            manifest = dict(version=1, frames=frames)
            result = run(manifest, root, self.font, self.font_hash)
            for i, (pixels, report) in enumerate(expected):
                with Image.open(root/f'frame-{i}.png') as actual:
                    self.assertEqual(actual.tobytes(), pixels.tobytes())
                self.assertEqual(result['frames'][i]['report'], report)
            with self.assertRaisesRegex(ValueError, 'exists'):
                run(manifest, root, self.font, self.font_hash)

    def test_bad_batches_reject_before_creating_outputs(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            frame = dict(input='/unused', resources='/unused', output='frame-0.png')
            for frames in [[], [frame]*33, [frame,frame], [dict(frame,output='../frame-0.png')]]:
                with self.assertRaises(ValueError): run(dict(version=1, frames=frames), root, self.font, self.font_hash)
                self.assertEqual(list(root.iterdir()), [])
            with self.assertRaises(ValueError): run(dict(version=True,frames=[frame]),root,self.font,self.font_hash)
            for manifest in [None, [], dict(version=1, frames=[None])]:
                with self.assertRaises(ValueError):
                    run(manifest, root, self.font, self.font_hash)
