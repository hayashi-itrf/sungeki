use sungeki_project::*;
pub(super) fn bgm_playback(
    bgm: &BuildBgm,
    duration: TimeUs,
) -> Result<AudioPlayback, ValidationError> {
    let mut playback = bgm.playback.clone();
    if let Some(reference) = bgm.volume_point_reference_duration_us {
        if reference.0 == 0 {
            return Err(ValidationError::new(
                "BGM音量ポイントの基準時間は0より大きい値が必要です".into(),
            ));
        }
        for point in &mut playback.volume_points {
            let scaled = (u128::from(point.at_us.0) * u128::from(duration.0)
                + u128::from(reference.0) / 2)
                / u128::from(reference.0);
            point.at_us =
                TimeUs(u64::try_from(scaled).map_err(|_| {
                    ValidationError::new("BGM音量ポイントの時刻が範囲外です".into())
                })?);
        }
        if playback
            .volume_points
            .windows(2)
            .any(|p| p[0].at_us >= p[1].at_us)
        {
            return Err(ValidationError::new(
                "BGMが短いため音量ポイントの時刻が重なります。ポイントを減らしてください".into(),
            ));
        }
    }
    Ok(playback)
}
