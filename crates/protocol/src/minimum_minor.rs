#[path = "minimum_minor_edits.rs"]
mod edits;
#[path = "minimum_minor_recent.rs"]
mod recent;
#[path = "minimum_minor_relink.rs"]
mod relink;
use crate::SessionRequest;
impl SessionRequest {
    pub fn minimum_protocol_minor(&self) -> u16 {
        if matches!(self, Self::GetRuntimeEnvironment {}) {
            return 160;
        }
        let recipe = match self {
            Self::BuildVideo {
                request: crate::BuildVideoRequest::Start { args },
            } => Some(&args.recipe),
            Self::Presets {
                request: crate::PresetRequest::Save { preset, .. },
            } => match &preset.content {
                sungeki_project::PresetContent::Template { recipe } => Some(recipe),
                _ => None,
            },
            _ => None,
        };
        if recipe.is_some_and(|r| r.presentation.as_ref().is_some_and(|p| p.fitting.is_some())) {
            return 159;
        }
        if recipe.is_some_and(|r| {
            r.bgm.as_ref().is_some_and(|b| b.playback.fade_in_us.0 > 0)
                && r.presentation
                    .as_ref()
                    .is_some_and(|p| p.bgm_speech_envelope.is_some())
        }) {
            return 158;
        }
        if recipe.is_some_and(|r| r.roles.iter().any(|role| role.preset_id == "psd-custom-v1")) {
            return 157;
        }
        if recipe.is_some_and(|r| {
            r.presentation
                .as_ref()
                .is_some_and(|p| p.structure.is_some())
        }) {
            return 156;
        }
        if recipe.is_some_and(|r| {
            r.scenes
                .iter()
                .any(|s| s.lines.iter().any(|l| l.psd_states.is_some()))
        }) {
            return 155;
        }
        let presentation = match self {
            Self::BuildVideo {
                request: crate::BuildVideoRequest::Start { args },
            } => args.recipe.presentation.as_ref(),
            Self::Presets {
                request: crate::PresetRequest::Save { preset, .. },
            } => match &preset.content {
                sungeki_project::PresetContent::Template { recipe } => recipe.presentation.as_ref(),
                _ => None,
            },
            _ => None,
        };
        if presentation.is_some_and(|p| p.opening.is_some()) {
            return 154;
        }
        if presentation.is_some_and(|p| p.slots.is_some() || p.ending.is_some()) {
            return 153;
        }
        if matches!(self, Self::BuildVideo {
            request: crate::BuildVideoRequest::Start { args }
        } if args.recipe.presentation.is_some())
            || matches!(self, Self::Presets {
                request: crate::PresetRequest::Save { preset, .. }
            } if matches!(&preset.content, sungeki_project::PresetContent::Template { recipe }
                if recipe.presentation.is_some()))
        {
            return 152;
        }
        if matches!(
            self,
            Self::GetRuntimeSettings | Self::SaveRuntimeSettings { .. }
        ) {
            return 151;
        }
        if edits::minimum(self) == 149 {
            return 149;
        }
        if matches!(
            self,
            Self::GetLocalSpeechCatalog { .. } | Self::GetLocalSpeechPolicy { .. }
        ) {
            return 149;
        }
        if matches!(self, Self::ReadVrmAsset { .. })
            || crate::minimum_minor_vrm::pose_required(self)
        {
            return 148;
        }
        if matches!(self, Self::ReadEditingAudio { .. }) {
            return 146;
        }
        if matches!(self, Self::ParseMarkdownScript { .. }) {
            return 144;
        }
        if matches!(
            self,
            Self::Collection {
                request: crate::CollectionRequest::Get { .. }
            }
        ) || matches!(self, Self::Collection { request: crate::CollectionRequest::Start { args } } if args.wait_ms.is_some())
        {
            return 143;
        }
        if matches!(
            self,
            Self::Proxy {
                request: crate::ProxyRequest::Get { .. }
            }
        ) || matches!(self, Self::Proxy { request: crate::ProxyRequest::Start { args } } if args.wait_ms.is_some())
        {
            return 142;
        }
        if matches!(
            self,
            Self::LoudnessJob {
                request: crate::LoudnessJobRequest::Get { .. }
            }
        ) || matches!(self, Self::LoudnessJob { request: crate::LoudnessJobRequest::Start { args } } if args.wait_ms.is_some())
        {
            return 141;
        }
        if matches!(
            self,
            Self::AudioPreview {
                request: crate::AudioPreviewRequest::Get { .. }
            }
        ) || matches!(self, Self::AudioPreview { request: crate::AudioPreviewRequest::Start { args } } if args.wait_ms.is_some())
        {
            return 140;
        }
        if matches!(self, Self::RenderJob { request: crate::RenderJobRequest::Start { args } | crate::RenderJobRequest::Preview { args } } if args.wait_ms.is_some())
        {
            return 137;
        }
        if matches!(self, Self::BuildVideo { request: crate::BuildVideoRequest::Start { args } } if args.wait_ms.is_some())
        {
            return 136;
        }
        if matches!(
            self,
            Self::BuildVideo {
                request: crate::BuildVideoRequest::Get { .. }
            }
        ) {
            return 135;
        }
        if matches!(self, Self::InstantiatePreset { .. }) {
            return 129;
        }
        let commands = match self {
            Self::Edit { edit, .. } => match &edit.action {
                crate::EditAction::Apply { commands } => Some(commands),
                _ => None,
            },
            Self::PreviewEdit { commands, .. } => Some(commands),
            _ => None,
        };
        if commands.is_some_and(|commands| {
            commands
                .iter()
                .any(|c| matches!(c, sungeki_editor::Command::Ripple { .. }))
        }) {
            return 134;
        }
        if commands.is_some_and(|commands| sungeki_editor::has_batch_references(commands)) {
            return 132;
        }
        if commands.is_some_and(|commands| {
            commands
                .iter()
                .any(|c| matches!(c, sungeki_editor::Command::ResizeLayout { .. }))
        }) {
            return 131;
        }
        if commands.is_some_and(|commands| {
            commands
                .iter()
                .any(|c| matches!(c, sungeki_editor::Command::ApplyVoicePreset { .. }))
        }) {
            return 130;
        }
        if commands.is_some_and(|commands| {
            commands
                .iter()
                .any(|c| matches!(c, sungeki_editor::Command::ApplyCharacterPreset { .. }))
        }) {
            return 129;
        }
        if matches!(self, Self::Presets { .. }) {
            return 127;
        }
        if matches!(
            self,
            Self::BuildVideo {
                request: crate::BuildVideoRequest::ReadReuseSource { .. }
            }
        ) {
            return 126;
        }
        if matches!(
            self,
            Self::BuildVideo {
                request: crate::BuildVideoRequest::ReconcileAudio { .. }
            }
        ) {
            return 124;
        }
        if matches!(self, Self::BuildVideo { request: crate::BuildVideoRequest::Start { args } } if args.reuse_from.is_some())
        {
            return 123;
        }
        if relink::required(self) {
            return 120;
        }
        if let Some(minor) = recent::minimum(self) {
            return minor;
        }
        if let Self::StartReview {
            sampling: Some(options),
            ..
        } = self
        {
            if options.text_safe_margin_percent.is_some() {
                return 76;
            }
        }
        if matches!(
            self,
            Self::StartReview {
                sampling: Some(_),
                ..
            }
        ) {
            return 75;
        }
        if matches!(self, Self::ReadReviewImage { .. }) {
            return 74;
        }
        if matches!(self, Self::ExportSubtitles { .. }) {
            return 63;
        }
        // Every production response can embed the representative-image report.
        if matches!(self, Self::BuildVideo { .. }) {
            return 73;
        }
        if matches!(
            self,
            Self::FramePreview {
                request: crate::FramePreviewRequest::Read { .. }
                    | crate::FramePreviewRequest::Publish { .. }
            }
        ) {
            return 72;
        }
        if matches!(self, Self::FramePreview { .. }) {
            return 71;
        }
        if matches!(self, Self::AudioPreview { .. }) {
            return 44;
        }
        if let Self::ProbeAudio { path } = self {
            return if std::path::Path::new(path)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("mp3"))
            {
                60
            } else {
                41
            };
        }
        if matches!(self, Self::ProposeSpeechGain { .. }) {
            return 39;
        }
        if matches!(self, Self::LoudnessJob { .. }) {
            return 38;
        }
        if matches!(self, Self::AnalyzeAudio { .. }) {
            return 36;
        }
        if matches!(
            self,
            Self::RenderJob {
                request: crate::RenderJobRequest::Publish { .. }
            }
        ) {
            return 35;
        }
        if matches!(self, Self::PsdPreview { .. }) {
            return 34;
        }
        if matches!(self, Self::ProbePsd { .. }) {
            return 30;
        }
        if matches!(self, Self::RenderJob { .. }) {
            return 29;
        }
        if matches!(self, Self::GetVoicevoxSpeaker { .. }) {
            return 24;
        }
        if matches!(
            self,
            Self::DiscoverVoicevox { .. } | Self::GetVoicevoxCatalog { .. }
        ) {
            return 23;
        }
        if matches!(self, Self::PreviewPronunciation { .. }) {
            return 21;
        }
        if matches!(
            self,
            Self::StartReview { .. }
                | Self::GetReviewJob { .. }
                | Self::CancelReviewJob { .. }
                | Self::ListReviewJobs { .. }
        ) {
            return 73;
        }
        edits::minimum(self)
    }
}
