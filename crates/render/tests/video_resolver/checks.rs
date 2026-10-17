use std::{collections::BTreeMap, path::Path, sync::atomic::AtomicBool, time::Duration};
use sungeki_media::video_probe::frames::PreparedVideo;
use sungeki_project::{Project, TimeUs};
use sungeki_render::VideoLayerResolver;
use sungeki_scene::{Primitive, ScenePlan};
pub fn check(video: PreparedVideo, project: &Project, root: &Path, ffmpeg: &Path) {
    let plan = ScenePlan::new(project).unwrap();
    let mut layer = plan.layers[0].clone();
    let Primitive::Video { asset_id, .. } = &layer.primitive else {
        panic!()
    };
    let id = asset_id.clone();
    let flag = AtomicBool::new(false);
    let mut resolver = VideoLayerResolver::new(BTreeMap::from([(id.clone(), video)]));
    let requested: Vec<_> = [200_000, 400_000]
        .into_iter()
        .map(|time| {
            let mut request = layer.clone();
            request.primitive = Primitive::Video {
                asset_id: id.clone(),
                source_time_us: TimeUs(time),
            };
            request
        })
        .collect();
    resolver.plan_frames(&requested).unwrap();
    // A worker can deliver every frame and still fail. Tentative pixels must
    // not survive that failure as successful cache entries.
    use std::os::unix::fs::PermissionsExt;
    let failed_worker = root.join("failed-batch-worker");
    std::fs::write(
        &failed_worker,
        "#!/bin/sh\nhead -c 6144 /dev/zero\nexit 1\n",
    )
    .unwrap();
    std::fs::set_permissions(&failed_worker, std::fs::Permissions::from_mode(0o700)).unwrap();
    layer.primitive = requested[0].primitive.clone();
    assert!(
        resolver
            .resolve(
                &[&layer],
                root,
                &failed_worker,
                Duration::from_secs(30),
                &flag
            )
            .is_err()
    );
    assert!(
        resolver
            .resolve(
                &[&layer],
                root,
                Path::new("/missing-worker"),
                Duration::from_secs(30),
                &flag
            )
            .is_err()
    );
    let mut keys = Vec::new();
    let mut previous: Option<std::path::PathBuf> = None;
    for time in [200_000, 300_000, 400_000, 200_000] {
        layer.primitive = Primitive::Video {
            asset_id: id.clone(),
            source_time_us: TimeUs(time),
        };
        let result = resolver
            .resolve(
                &[&layer, &layer],
                root,
                if !keys.is_empty() {
                    Path::new("/missing-cache-hit-must-not-spawn")
                } else {
                    ffmpeg
                },
                Duration::from_secs(30),
                &flag,
            )
            .unwrap();
        assert_eq!(result.layers.len(), 2);
        assert_eq!(result.resources.len(), 1);
        assert_eq!(result.layers[0].transform, layer.transform);
        assert_eq!(result.layers[0].clip_id, layer.clip_id);
        assert_eq!(result.layers[0].scene_id, layer.scene_id);
        let Primitive::Image { asset_id } = &result.layers[0].primitive else {
            panic!()
        };
        let resource = &result.resources[asset_id];
        assert!(resource.path.exists());
        if let Some(path) = previous {
            assert!(!std::path::PathBuf::from(path).exists());
        }
        previous = Some(resource.path.clone());
        keys.push(asset_id.clone());
    }
    assert_eq!(keys[0], keys[1]);
    assert_ne!(keys[0], keys[2]);
    assert_eq!(keys[0], keys[3]);
    resolver
        .resolve(&[], root, ffmpeg, Duration::from_secs(30), &flag)
        .unwrap();
    assert!(!previous.unwrap().exists());
    // The unrequested next source frame must not have been speculatively decoded.
    layer.primitive = Primitive::Video {
        asset_id: id.clone(),
        source_time_us: TimeUs(600_000),
    };
    assert!(
        resolver
            .resolve(
                &[&layer],
                root,
                Path::new("/missing-worker"),
                Duration::from_secs(30),
                &flag
            )
            .is_err()
    );
    layer.primitive = Primitive::Video {
        asset_id: "missing".into(),
        source_time_us: TimeUs(0),
    };
    assert!(
        resolver
            .resolve(&[&layer], root, ffmpeg, Duration::from_secs(30), &flag)
            .is_err()
    );
}
