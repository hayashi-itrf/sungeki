use crate::*;

impl BuildRecipe {
    pub(crate) fn validate_psd_states(
        &self,
        line: &BuildLine,
        project: &Project,
    ) -> Result<(), ValidationError> {
        if line.pose.is_some()
            && line
                .psd_states
                .as_ref()
                .is_some_and(|states| states.contains_key(&line.role))
        {
            return Err(ValidationError::new(
                "話者のposeとpsd_statesは併用できません".into(),
            ));
        }
        for (name, state) in line.psd_states.iter().flatten() {
            let role =
                self.roles.iter().find(|r| &r.role == name).ok_or_else(|| {
                    ValidationError::new("PSD演出に未登録の役名があります".into())
                })?;
            let BuildStanding::Psd {
                asset_id, flip_x, ..
            } = &role.standing
            else {
                return Err(ValidationError::new(
                    "PSD演出はPSDの出演者だけに指定できます".into(),
                ));
            };
            // Reuse canonical pose hierarchy, source hash, character and both
            // mouth-option validations, including hidden parent combinations.
            BuildStanding::Psd {
                asset_id: asset_id.clone(),
                pose: state.pose.clone(),
                speech: state.speech.clone(),
                flip_x: *flip_x,
            }
            .validate_character(project, &role.character.id)?;
        }
        Ok(())
    }
}
