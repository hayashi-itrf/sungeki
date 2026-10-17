use sungeki_editor::*;
use sungeki_project::*;
pub fn id(n: u32) -> String {
    format!("70000000-0000-4000-8000-{n:012x}")
}
pub fn fixture() -> (Project, BuildRecipe, BuildScriptIds) {
    let mut recipe: BuildRecipe = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    let mut second = recipe.scenes[0].clone();
    second.name = "第二場面".into();
    second.lines[0].subtitle_text.clear();
    second.lines[0].spoken_text = Some("  字幕なしのセリフ\n".into());
    recipe.scenes.push(second);
    (
        Project::empty(id(1), "新規".into()),
        recipe,
        BuildScriptIds {
            track_id: id(2),
            scene_ids: vec![id(3), id(4)],
            speech_ids: (10..14).map(id).collect(),
        },
    )
}
