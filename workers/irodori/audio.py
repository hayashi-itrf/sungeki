"""Preserve Irodori floating samples without implicit gain or peak clipping."""
import struct

MAX_WAV_BYTES = 64 * 1024 * 1024  # common media verifier's bound
HEADER_BYTES = 58  # RIFF + fmt(18) + fact(4) + data header


def wav_bytes(audio, sample_rate):
    """Convert a mono tensor to little-endian IEEE float32 WAV, in memory.

The model may produce samples outside [-1, 1]. Preserve them for Sungeki's
explicit mixing/master stage. Nonfinite, empty and oversized output is invalid.
"""
    if type(sample_rate) is not int or not 8000 <= sample_rate <= 192000:
        raise ValueError('invalid output sample rate')
    shape = tuple(audio.shape)
    if len(shape) != 2 or shape[0] != 1 or shape[1] <= 0:
        raise ValueError('expected nonempty mono audio tensor')
    frames = shape[1]
    if frames * 4 + HEADER_BYTES > MAX_WAV_BYTES:
        raise ValueError('output WAV exceeds 64 MiB')
    # Import only for actual conversion; metadata tests need no ML environment.
    import numpy as np
    samples = audio.detach().float().cpu().numpy()
    if samples.shape != shape or not np.isfinite(samples).all():
        raise ValueError('invalid or nonfinite output samples')
    payload = np.asarray(samples, dtype='<f4', order='C').tobytes(order='C')
    if len(payload) != frames * 4:
        raise ValueError('unexpected sample count')
    header = b'RIFF' + struct.pack('<I', len(payload) + HEADER_BYTES - 8) + b'WAVE'
    header += b'fmt ' + struct.pack('<IHHIIHHH', 18, 3, 1, sample_rate,
                                  sample_rate * 4, 4, 32, 0)
    header += b'fact' + struct.pack('<II', 4, frames)
    header += b'data' + struct.pack('<I', len(payload))
    return header + payload
