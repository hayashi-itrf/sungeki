use crate::SessionRequest;
pub(super) fn minimum(request: &SessionRequest) -> Option<u16> {
    if matches!(
        request,
        SessionRequest::RenderJob {
            request: crate::RenderJobRequest::Frame { .. }
        }
    ) {
        return Some(122);
    }
    if matches!(
        request,
        SessionRequest::RenderJob {
            request: crate::RenderJobRequest::Get { .. }
        }
    ) {
        return Some(121);
    }
    if matches!(request, SessionRequest::BuildVideo { request: crate::BuildVideoRequest::Start { args } }
        if args.recipe.bgm.as_ref().is_some_and(|b| b.volume_point_reference_duration_us.is_some()))
    {
        return Some(115);
    }
    if matches!(request, SessionRequest::BuildVideo { request: crate::BuildVideoRequest::Start { args } }
        if args.recipe.scenes.iter().any(|s| s.diagrams.iter().any(|d| d.fade.is_some())))
    {
        return Some(114);
    }
    if matches!(request, SessionRequest::BuildVideo { request: crate::BuildVideoRequest::Start { args } }
        if args.recipe.scenes.iter().any(|s| s.diagrams.iter().any(|d| d.motion.is_some())))
    {
        return Some(113);
    }
    if matches!(request, SessionRequest::BuildVideo { request: crate::BuildVideoRequest::Start { args } }
        if args.recipe.scenes.iter().any(|s| s.fade.is_some()))
    {
        return Some(112);
    }
    if matches!(
        request,
        SessionRequest::RenderJob {
            request: crate::RenderJobRequest::Preview { .. }
        }
    ) {
        return Some(111);
    }
    if matches!(
        request,
        SessionRequest::RenderJob {
            request: crate::RenderJobRequest::Read { .. }
        }
    ) {
        return Some(110);
    }
    None
}
