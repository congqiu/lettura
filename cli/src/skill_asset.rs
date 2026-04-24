use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../skills"]
#[include = "*.md"]
pub struct SkillAssets;
