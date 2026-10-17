use hound::{SampleFormat, WavSpec, WavWriter};
use std::io::Cursor;
use sungeki_media::{ExpectedAudio, WavError, verify_wav};

fn pcm(channels: u16, rate: u32, bits: u16, frames: usize) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    {
        let spec = WavSpec {
            channels,
            sample_rate: rate,
            bits_per_sample: bits,
            sample_format: SampleFormat::Int,
        };
        let mut writer = WavWriter::new(&mut bytes, spec).unwrap();
        for _ in 0..frames * channels as usize {
            writer.write_sample(1i32).unwrap();
        }
        writer.finalize().unwrap();
    }
    let mut bytes = bytes.into_inner();
    if bytes.len() % 2 != 0 {
        bytes.push(0);
        fix_size(&mut bytes);
    }
    bytes
}
fn floats(samples: &[f32]) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    {
        let spec = WavSpec {
            channels: 1,
            sample_rate: 44100,
            bits_per_sample: 32,
            sample_format: SampleFormat::Float,
        };
        let mut writer = WavWriter::new(&mut bytes, spec).unwrap();
        for sample in samples {
            writer.write_sample(*sample).unwrap();
        }
        writer.finalize().unwrap();
    }
    bytes.into_inner()
}
fn expected(channels: u16, sample_rate: u32) -> ExpectedAudio {
    ExpectedAudio {
        channels,
        sample_rate,
    }
}
fn fix_size(bytes: &mut [u8]) {
    let size = (bytes.len() - 8) as u32;
    bytes[4..8].copy_from_slice(&size.to_le_bytes());
}
fn chunk(bytes: &[u8], name: &[u8; 4]) -> usize {
    let mut at = 12;
    loop {
        if &bytes[at..at + 4] == name {
            return at;
        }
        let size = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        at += 8 + size + (size & 1);
    }
}
#[test]
fn decodes_all_samples_and_counts_frames_independently_of_channels() {
    for channels in [1, 2] {
        for bits in [8, 16, 24, 32] {
            let bytes = pcm(channels, 44100, bits, 441);
            let result = verify_wav(bytes.clone(), expected(channels, 44100)).unwrap();
            assert_eq!(result.bytes(), bytes);
            assert_eq!(result.info().frames, 441);
            assert_eq!(result.info().duration_us, 10000);
            assert_eq!(result.info().bits_per_sample, bits);
            assert_eq!(result.info().content_hash.len(), 64);
            assert!(!result.info().is_float);
        }
    }
    let result = verify_wav(pcm(1, 44100, 16, 1), expected(1, 44100)).unwrap();
    assert_eq!(result.info().duration_us, 23);
    let original = result.info().content_hash.clone();
    let result = verify_wav(pcm(1, 44100, 16, 2), expected(1, 44100)).unwrap();
    assert_ne!(result.info().content_hash, original);
}
#[test]
fn floating_audio_rejects_nonfinite_values_but_not_silence() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            verify_wav(floats(&[0.0, bad]), expected(1, 44100)).unwrap_err(),
            WavError::NonFiniteSample
        );
    }
    let result = verify_wav(floats(&[0.0, 0.0]), expected(1, 44100)).unwrap();
    assert!(result.info().is_float);
    assert_eq!(result.info().frames, 2);
}
#[test]
fn requires_expected_output_format_and_nonempty_audio() {
    assert_eq!(
        verify_wav(pcm(1, 24000, 16, 20), expected(1, 48000)).unwrap_err(),
        WavError::FormatMismatch
    );
    assert_eq!(
        verify_wav(pcm(1, 24000, 16, 20), expected(2, 24000)).unwrap_err(),
        WavError::FormatMismatch
    );
    assert_eq!(
        verify_wav(pcm(1, 24000, 16, 0), expected(1, 24000)).unwrap_err(),
        WavError::Empty
    );
    assert_eq!(
        verify_wav(vec![0; 64 * 1024 * 1024 + 1], expected(1, 24000)).unwrap_err(),
        WavError::TooLarge
    );
}
#[test]
fn rejects_truncated_and_ambiguous_riff_envelopes() {
    let bytes = pcm(2, 24000, 16, 20);
    for len in 0..bytes.len() {
        assert!(verify_wav(bytes[..len].to_vec(), expected(2, 24000)).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.extend_from_slice(b"extra");
    assert!(verify_wav(trailing, expected(2, 24000)).is_err());
    let mut duplicate = bytes.clone();
    let data = chunk(&bytes, b"data");
    duplicate.extend_from_slice(&bytes[data..]);
    fix_size(&mut duplicate);
    assert!(verify_wav(duplicate, expected(2, 24000)).is_err());
    let mut partial_frame = bytes.clone();
    partial_frame.truncate(bytes.len() - 2);
    partial_frame[data + 4..data + 8].copy_from_slice(&78u32.to_le_bytes());
    fix_size(&mut partial_frame);
    assert!(verify_wav(partial_frame, expected(2, 24000)).is_err());
    let mut bad_alignment = bytes.clone();
    let fmt = chunk(&bytes, b"fmt ");
    bad_alignment[fmt + 20..fmt + 22].copy_from_slice(&3u16.to_le_bytes());
    assert!(verify_wav(bad_alignment, expected(2, 24000)).is_err());
}
#[test]
fn accepts_bounded_ancillary_chunks_but_requires_padding() {
    let mut prefixed = pcm(1, 24000, 16, 1);
    prefixed.splice(12..12, *b"JUNK\x01\0\0\0x\0");
    fix_size(&mut prefixed);
    let result = verify_wav(prefixed.clone(), expected(1, 24000)).unwrap();
    assert_eq!(result.bytes(), prefixed);
    let mut bytes = pcm(1, 24000, 16, 1);
    bytes.extend_from_slice(b"JUNK\x01\0\0\0x\0");
    fix_size(&mut bytes);
    verify_wav(bytes.clone(), expected(1, 24000)).unwrap();
    bytes.pop();
    fix_size(&mut bytes);
    assert!(verify_wav(bytes, expected(1, 24000)).is_err());
}
#[test]
fn malformed_header_mutations_return_errors_without_panicking() {
    let original = pcm(1, 24000, 16, 20);
    for at in 0..original.len() {
        for value in [0, 1, 127, 255] {
            let mut bytes = original.clone();
            bytes[at] = value;
            let _ = verify_wav(bytes, expected(1, 24000));
        }
    }
}
