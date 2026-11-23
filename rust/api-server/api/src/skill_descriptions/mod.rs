pub(crate) mod bitcraft;
mod definitions;
pub(crate) use definitions::SkillDefinitions;

const SKILL_ORDER: &[&str] = &[
    "Forestry", "Carpentry", "Farming", "Foraging", "Tailoring", "Mining", "Smithing", "Masonry",
    "Scholar", "Hunting", "Leatherworking", "Fishing", "Cooking", "Construction", "Slayer",
    "Merchanting", "Sailing", "Taming",
];

pub(crate) fn skill_sort_key(name: &str) -> (usize, &str) {
    let index = SKILL_ORDER.iter().position(|&skill| skill == name);
    (index.unwrap_or(SKILL_ORDER.len()), name)
}
