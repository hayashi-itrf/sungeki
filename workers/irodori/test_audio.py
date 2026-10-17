from pathlib import Path
import shutil
import struct
import subprocess
import unittest
from unittest.mock import patch
import numpy as np
from audio import HEADER_BYTES, MAX_WAV_BYTES, wav_bytes


class Tensor:
    def __init__(self, values):
        self.samples = np.asarray(values)
        self.shape = self.samples.shape
    def detach(self):
        return self
    def float(self):
        self.samples = self.samples.astype(np.float32)
        return self
    def cpu(self):
        return self
    def numpy(self):
        return self.samples


class AudioTests(unittest.TestCase):
    def test_riff_structure_and_samples_preserved(self):
        values = [[-1.25, -1., -.125, 0., .125, 1., 1.25]]
        audio = wav_bytes(Tensor(values), 48000)
        self.assertEqual(audio[:4], b'RIFF')
        self.assertEqual(struct.unpack_from('<I', audio, 4)[0], len(audio) - 8)
        self.assertEqual(audio[8:16], b'WAVEfmt ')
        self.assertEqual(struct.unpack_from('<IHHIIHHH', audio, 16),
                         (18, 3, 1, 48000, 192000, 4, 32, 0))
        self.assertEqual(audio[38:42], b'fact')
        self.assertEqual(struct.unpack_from('<II', audio, 42), (4, 7))
        self.assertEqual(audio[50:54], b'data')
        self.assertEqual(struct.unpack_from('<I', audio, 54)[0], 28)
        np.testing.assert_array_equal(np.frombuffer(audio[HEADER_BYTES:], dtype='<f4'), values[0])

    def test_invalid_samples_and_shapes(self):
        for values in ([[float('nan')]], [[float('inf')]], [[-float('inf')]],
                       [[1e300]], [[]], [[0.], [0.]], [0., 1.]):
            with self.subTest(values=values), np.errstate(over='ignore'):
                with self.assertRaises(ValueError):
                    wav_bytes(Tensor(values), 48000)
        for rate in (True, 0, 192001, 48000.):
            with self.assertRaises(ValueError):
                wav_bytes(Tensor([[0.]]), rate)

    def test_size_checked_before_tensor_transfer(self):
        class Oversized:
            shape = (1, MAX_WAV_BYTES // 4)
            def detach(self):
                raise AssertionError('must reject before allocating or copying')
        with self.assertRaisesRegex(ValueError, '64 MiB'):
            wav_bytes(Oversized(), 48000)
        with patch('audio.MAX_WAV_BYTES', HEADER_BYTES + 4):
            self.assertEqual(len(wav_bytes(Tensor([[0.]]), 8000)), HEADER_BYTES + 4)
            with self.assertRaises(ValueError):
                wav_bytes(Tensor([[0., 1.]]), 8000)

    def test_noncontiguous_and_big_endian_samples(self):
        tensor = Tensor(np.array([[0., 9., 1., 9., -1.]], dtype='>f4')[:, ::2])
        audio = wav_bytes(tensor, 24000)
        self.assertEqual(audio[HEADER_BYTES:], struct.pack('<fff', 0., 1., -1.))

    def test_ffmpeg_decodes_without_clipping_or_truncation(self):
        ffmpeg = shutil.which('ffmpeg')
        if ffmpeg is None:
            self.skipTest('ffmpeg unavailable')
        samples = np.array([[-1.25, -.5, 0., .5, 1.25]], dtype=np.float32)
        result = subprocess.run([ffmpeg, '-v', 'error', '-i', 'pipe:0',
            '-f', 'f32le', '-acodec', 'pcm_f32le', 'pipe:1'],
            input=wav_bytes(Tensor(samples), 48000), capture_output=True, check=True, timeout=15)
        np.testing.assert_array_equal(np.frombuffer(result.stdout, dtype='<f4'), samples[0])

    def test_shared_rust_fixture_and_bridge_output(self):
        from test_bridge import Runtime
        from bridge import RuntimeBridge
        from types import SimpleNamespace as NS
        runtime = Runtime()
        runtime.result.audio = Tensor([[-1.25, -.5, 0., .5, 1.25]])
        bridge = RuntimeBridge(runtime, 'model', [dict(
            id='none', name='参照なし', reference='none', caption=None)], NS, lambda s: s)
        output = bridge.synthesize_wav(dict(text='検査', settings=dict(
            voice_id='none', caption=None, cfg_scale=3., num_steps=40, seed=42)))
        fixture = Path(__file__).resolve().parents[2] / 'tests/fixtures/irodori-float.wav'
        self.assertEqual(output, fixture.read_bytes())

    def test_cpu_tensor_conversion(self):
        import torch
        tensor = torch.tensor([[0., .25, -.25]], requires_grad=True)
        result = wav_bytes(tensor, 48000)
        self.assertEqual(result[HEADER_BYTES:], struct.pack('<fff', 0., .25, -.25))
        self.assertTrue(tensor.requires_grad)


if __name__ == '__main__':
    unittest.main()
