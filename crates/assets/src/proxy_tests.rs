use super::*;
use sungeki_project::{
    MediaRatio, MediaStreamClock, VideoContainer, VideoMetadata, VideoStreamInfo,
};

fn source() -> VideoProbe {
    VideoProbe {
        sha256: "a".repeat(64),
        byte_length: 4096,
        metadata: VideoMetadata {
            container: VideoContainer::Mov,
            audio: vec![],
            video: VideoStreamInfo {
                index: 0,
                codec: "h264".into(),
                width: 320,
                height: 180,
                sample_aspect_ratio: None,
                clock: MediaStreamClock {
                    start_pts: Some(0),
                    duration_ts: Some(1000),
                    time_base: MediaRatio {
                        numerator: 1,
                        denominator: 1000,
                    },
                },
                average_frame_rate: None,
                nominal_frame_rate: None,
                frame_count: None,
                rotation_degrees: None,
                display_matrix: None,
                legacy_rotate_tag: None,
            },
        },
    }
}

#[test]
fn key_separates_original_stream_conversion_and_version() {
    let source = source();
    let key = ProxyKey::new(&source, b"encoder-v1;640;rgba;no-audio;exact-pts").unwrap();
    assert_eq!(
        key,
        ProxyKey::new(&source, b"encoder-v1;640;rgba;no-audio;exact-pts").unwrap()
    );
    for edited in [
        VideoProbe {
            sha256: "b".repeat(64),
            ..source.clone()
        },
        VideoProbe {
            byte_length: 4097,
            ..source.clone()
        },
        {
            let mut p = source.clone();
            p.metadata.video.index = 1;
            p
        },
    ] {
        assert_ne!(
            key,
            ProxyKey::new(&edited, b"encoder-v1;640;rgba;no-audio;exact-pts").unwrap()
        );
    }
    assert_ne!(
        key,
        ProxyKey::new(&source, b"encoder-v2;640;rgba;no-audio;exact-pts").unwrap()
    );
    assert_ne!(
        key,
        ProxyKey::versioned(&source, b"encoder-v1;640;rgba;no-audio;exact-pts", 2).unwrap()
    );
    assert_eq!(key.as_str().len(), 64);
    assert!(key.as_str().bytes().all(|b| b.is_ascii_hexdigit()));
    assert!(ProxyKey::new(&source, b"").is_err());
    assert!(ProxyKey::new(&source, &vec![0; 65537]).is_err());
    assert!(
        ProxyKey::new(
            &VideoProbe {
                sha256: "../original".into(),
                ..source
            },
            b"x"
        )
        .is_err()
    );
}

#[test]
fn bounded_index_eviction_replacement_and_byte_identity() {
    let source = source();
    let a = ProxyKey::new(&source, b"a").unwrap();
    let b = ProxyKey::new(&source, b"b").unwrap();
    let c = ProxyKey::new(&source, b"c").unwrap();
    let candidate = |key| ProxyCandidate::new(key, &"b".repeat(64), 100).unwrap();
    let mut index = ProxyIndex::new(2).unwrap();
    index.insert(candidate(a.clone()));
    index.insert(candidate(b.clone()));
    assert!(index.candidate(&a).unwrap().matches(&"B".repeat(64), 100));
    index.insert(candidate(c.clone()));
    assert!(index.candidate(&b).is_none());
    index.insert(ProxyCandidate::new(a.clone(), &"c".repeat(64), 101).unwrap());
    let entry = index.candidate(&a).unwrap();
    assert!(!entry.matches(&"b".repeat(64), 100));
    assert!(!entry.matches(&"c".repeat(64), 100));
    assert!(entry.matches(&"c".repeat(64), 101));
    index.remove(&a);
    assert!(index.candidate(&a).is_none());
    assert!(index.candidate(&c).is_some());
    assert!(ProxyIndex::new(0).is_err());
    assert!(ProxyCandidate::new(a, "bad", 100).is_err());
}
