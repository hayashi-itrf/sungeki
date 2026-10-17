"""Sungeki adapter for an owned, already loaded Irodori runtime.

The host must attest originals and own the engine lease. This adapter never
loads/downloads models, adopts servers, or edits a Project.
"""
from copy import deepcopy
import math
from pathlib import Path


def fields(value, required, optional=()):
    if not isinstance(value, dict) or not set(required) <= value.keys() or (
            value.keys() - set(required) - set(optional)):
        raise ValueError('missing or unknown Irodori fields')


def text(value, limit, label, controls=False):
    if (not isinstance(value, str) or not value.strip()
            or len(value.encode('utf-8')) > limit or '\0' in value
            or (controls and any(ord(c) < 32 or 127 <= ord(c) <= 159 for c in value))):
        raise ValueError(f'invalid {label}')
    return value


def integer(value, low, high, label):
    if type(value) is not int or not low <= value <= high:
        raise ValueError(f'invalid {label}')
    return value


def registered_voice(value):
    fields(value, ('id', 'name', 'reference', 'caption'), ('path',))
    value = deepcopy(value)
    text(value['id'], 128, 'voice id', True)
    text(value['name'], 1024, 'voice name', True)
    if value['caption'] is not None:
        text(value['caption'], 4096, 'voice caption')
    if value['reference'] not in ('none', 'audio', 'latent'):
        raise ValueError('invalid reference kind')
    if value['reference'] == 'none':
        if value.get('path') is not None:
            raise ValueError('no-reference voice cannot specify a path')
    else:
        path = Path(text(value.get('path'), 4096, 'reference path'))
        if not path.is_absolute() or '..' in path.parts or not path.is_file():
            raise ValueError('reference must be an existing absolute file')
        value['path'] = str(path.resolve(strict=True))
    return value


def token_limit(tokenizer, value, maximum, label):
    if tokenizer is None or type(maximum) is not int or maximum <= 0:
        raise ValueError(f'invalid runtime {label} tokenizer')
    # encode includes BOS and does not truncate, unlike vendor batch_encode.
    if tokenizer.encode(value).numel() > maximum:
        raise ValueError(f'{label} exceeds model token limit; split the speech')


class RuntimeBridge:
    def __init__(self, runtime, checkpoint, voices, request_type=None, normalize=None):
        if request_type is None or normalize is None:
            from irodori_tts.inference_runtime import SamplingRequest
            from irodori_tts.text_normalization import normalize_text
            request_type = SamplingRequest
            normalize = normalize_text
        text(checkpoint, 4096, 'checkpoint', True)
        if not isinstance(voices, list) or not 1 <= len(voices) <= 4096:
            raise ValueError('invalid voice registry')
        self._voices = {}
        for value in voices:
            voice = registered_voice(value)
            if voice['id'] in self._voices:
                raise ValueError('duplicate voice')
            self._voices[voice['id']] = voice
        self._runtime = runtime
        self._request_type = request_type
        self._normalize = normalize
        self._model = {
            'checkpoint': checkpoint,
            'speaker_conditioning': bool(runtime.model_cfg.use_speaker_condition_resolved),
            'caption_conditioning': bool(runtime.model_cfg.use_caption_condition),
            'sample_rate': integer(runtime.codec.sample_rate, 8000, 192000, 'sample rate'),
            # Sungeki's initial adapter accepts mono runtime output only.
            'channels': 1,
        }

    def catalog(self):
        return {'model': deepcopy(self._model), 'voices': [
            {k: deepcopy(v[k]) for k in ('id', 'name', 'reference', 'caption')}
            for _, v in sorted(self._voices.items())]}

    def prepare(self, payload):
        fields(payload, ('text', 'settings'))
        raw = text(payload['text'], 65536, 'speech text')
        if len(raw) > 4096:
            raise ValueError('speech exceeds Sungeki character limit')
        settings = payload['settings']
        fields(settings, ('voice_id', 'caption', 'cfg_scale', 'num_steps', 'seed'))
        voice_id = text(settings['voice_id'], 128, 'voice id', True)
        if voice_id not in self._voices:
            raise ValueError('unknown registered voice')
        voice = self._voices[voice_id]
        caption = settings['caption']
        if caption is not None:
            text(caption, 4096, 'caption')
        else:
            caption = voice['caption']
        cfg = settings['cfg_scale']
        if type(cfg) not in (int, float) or not math.isfinite(cfg) or not 0 <= cfg <= 30:
            raise ValueError('invalid cfg scale')
        steps = integer(settings['num_steps'], 1, 200, 'steps')
        seed = integer(settings['seed'], 0, 2**32 - 1, 'seed')
        if voice['reference'] != 'none' and not self._model['speaker_conditioning']:
            raise ValueError('model does not support reference audio')
        if caption is not None and not self._model['caption_conditioning']:
            raise ValueError('model does not support caption')
        normalized = self._normalize(raw).strip()
        text(normalized, 65536, 'normalized speech')
        token_limit(self._runtime.tokenizer, normalized,
                    self._runtime.default_text_max_len, 'speech')
        if caption is not None:
            token_limit(self._runtime.caption_tokenizer, caption.strip(),
                        self._runtime.default_caption_max_len, 'caption')
        args = dict(text=raw, caption=caption, cfg_scale=float(cfg), num_steps=steps,
                    seed=seed, no_ref=voice['reference'] == 'none', num_candidates=1,
                    decode_mode='sequential')
        if voice['reference'] != 'none':
            path = voice['path']
            if not Path(path).is_file():
                raise ValueError('registered reference is missing')
            args['ref_wav' if voice['reference'] == 'audio' else 'ref_latent'] = path
        return self._request_type(**args)

    def synthesize(self, payload):
        request = self.prepare(payload)
        result = self._runtime.synthesize(request)
        if result.used_seed != request.seed:
            raise ValueError('runtime returned a different seed')
        if result.sample_rate != self._model['sample_rate']:
            raise ValueError('runtime returned a different sample rate')
        if len(result.audio.shape) != 2 or result.audio.shape[0] != 1 or result.audio.shape[1] <= 0:
            raise ValueError('runtime returned empty or non-mono audio')
        return result

    def synthesize_wav(self, payload):
        from audio import wav_bytes
        result = self.synthesize(payload)
        return wav_bytes(result.audio, result.sample_rate)
