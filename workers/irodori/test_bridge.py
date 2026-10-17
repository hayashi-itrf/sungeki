import copy
from pathlib import Path
from types import SimpleNamespace as NS
import tempfile
import unittest
from bridge import RuntimeBridge


class Tokenizer:
    def encode(self, value):
        return NS(numel=lambda: len(value) + 1)


class Runtime:
    def __init__(self):
        self.model_cfg = NS(use_speaker_condition_resolved=True, use_caption_condition=True)
        self.codec = NS(sample_rate=48000)
        self.tokenizer = self.caption_tokenizer = Tokenizer()
        self.default_text_max_len = self.default_caption_max_len = 16
        self.requests = []
        self.result = NS(used_seed=42, sample_rate=48000, audio=NS(shape=(1, 4800)))

    def synthesize(self, request):
        self.requests.append(request)
        return self.result


class BridgeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name) / 'reference.wav'
        self.path.write_bytes(b'fixture; not a real waveform')
        self.voices = [dict(id='registered', name='声', reference='audio',
                            caption='落ち着いた声', path=str(self.path.resolve()))]
        self.runtime = Runtime()
        self.payload = dict(text='こんにちは', settings=dict(
            voice_id='registered', caption=None, cfg_scale=3., num_steps=40, seed=42))

    def bridge(self):
        return RuntimeBridge(self.runtime, 'model', self.voices, NS, lambda s: s)

    def test_mapping_and_catalog_ownership(self):
        bridge = self.bridge()
        self.voices[0]['caption'] = '変更'
        self.assertEqual(bridge.catalog()['voices'][0]['caption'], '落ち着いた声')
        catalog = bridge.catalog()
        catalog['model']['sample_rate'] = 1
        self.assertEqual(bridge.catalog()['model']['sample_rate'], 48000)
        self.assertNotIn('path', catalog['voices'][0])
        result = bridge.synthesize(self.payload)
        self.assertIs(result, self.runtime.result)
        request = self.runtime.requests[0]
        self.assertEqual(request.text, 'こんにちは')
        self.assertEqual(request.caption, '落ち着いた声')
        self.assertEqual(request.ref_wav, str(self.path.resolve()))
        self.assertEqual((request.seed, request.num_steps, request.cfg_scale), (42, 40, 3.))
        self.assertFalse(request.no_ref)
        self.assertEqual(request.num_candidates, 1)

    def test_explicit_caption_latent_and_no_reference(self):
        self.payload['settings']['caption'] = '明るい声'
        self.voices[0]['reference'] = 'latent'
        req = self.bridge().prepare(self.payload)
        self.assertEqual(req.ref_latent, str(self.path.resolve()))
        self.assertEqual(req.caption, '明るい声')
        self.voices[0].update(reference='none', path=None)
        req = self.bridge().prepare(self.payload)
        self.assertTrue(req.no_ref)
        self.assertFalse(hasattr(req, 'ref_wav'))
        self.assertFalse(hasattr(req, 'ref_latent'))

    def test_unsupported_conditions_and_bad_parameters_never_synthesize(self):
        invalid = [dict(seed=-1), dict(seed=True), dict(seed=2**32), dict(num_steps=0),
                   dict(cfg_scale=float('nan')), dict(cfg_scale=True), dict(caption=' '),
                   dict(voice_id='missing'), dict(ref_audio_path=str(self.path.resolve()))]
        bridge = self.bridge()
        for update in invalid:
            with self.subTest(update=update):
                payload = copy.deepcopy(self.payload)
                payload['settings'].update(update)
                with self.assertRaises(ValueError):
                    bridge.synthesize(payload)
        self.runtime.model_cfg.use_speaker_condition_resolved = False
        with self.assertRaises(ValueError):
            self.bridge().synthesize(self.payload)
        self.runtime.model_cfg.use_speaker_condition_resolved = True
        self.runtime.model_cfg.use_caption_condition = False
        with self.assertRaises(ValueError):
            self.bridge().synthesize(self.payload)
        self.assertEqual(self.runtime.requests, [])

    def test_token_limit_includes_bos_and_never_truncates(self):
        bridge = self.bridge()
        self.payload['text'] = 'あ' * 15
        bridge.prepare(self.payload)
        self.payload['text'] += 'あ'
        with self.assertRaisesRegex(ValueError, 'speech exceeds model token limit'):
            bridge.synthesize(self.payload)
        self.payload['text'] = '短文'
        self.payload['settings']['caption'] = 'あ' * 16
        with self.assertRaisesRegex(ValueError, 'caption exceeds model token limit'):
            bridge.synthesize(self.payload)
        self.assertEqual(self.runtime.requests, [])

    def test_registry_and_missing_original(self):
        self.voices.append(copy.deepcopy(self.voices[0]))
        with self.assertRaises(ValueError):
            self.bridge()
        self.voices.pop()
        bridge = self.bridge()
        self.path.unlink()
        with self.assertRaises(ValueError):
            bridge.synthesize(self.payload)
        self.assertEqual(self.runtime.requests, [])

    def test_result_mismatch_is_not_accepted(self):
        for key, value in [('used_seed', 43), ('sample_rate', 24000),
                           ('audio', NS(shape=(2, 4800))), ('audio', NS(shape=(1, 0)))]:
            with self.subTest(key=key, value=value):
                original = getattr(self.runtime.result, key)
                setattr(self.runtime.result, key, value)
                with self.assertRaises(ValueError):
                    self.bridge().synthesize(self.payload)
                setattr(self.runtime.result, key, original)


if __name__ == '__main__':
    unittest.main()
