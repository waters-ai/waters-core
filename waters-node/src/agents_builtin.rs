/// Встроенные TUI-агенты — вшиты в бинарник через include_str!
use crate::skill::{SkillManifest, SkillRegistry, LlmConfig};

pub fn register(skill_reg: &mut SkillRegistry) -> usize {
    let agents = vec![
        ("general", "Универсальный агент — любые задачи, код, анализ",
         include_str!("../agents/tui-code/general/SKILL.md")),
        ("explorer", "Исследователь — поиск информации, чтение",
         include_str!("../agents/tui-code/explorer/SKILL.md")),
        ("planner", "Проектировщик — планирование, декомпозиция задач",
         include_str!("../agents/tui-code/planner/SKILL.md")),
        ("implementer", "Реализатор — пишет код по спецификации",
         include_str!("../agents/tui-code/implementer/SKILL.md")),
        ("reviewer", "Ревьюер — проверяет код, ищет ошибки",
         include_str!("../agents/tui-code/reviewer/SKILL.md")),
        ("verifier", "Верификатор — тесты, проверки, качество",
         include_str!("../agents/tui-code/verifier/SKILL.md")),
        ("custom", "Кастомный агент — настраиваемый под задачу",
         include_str!("../agents/tui-code/custom/SKILL.md")),
    ];

    let mut count = 0;
    for (name, desc, prompt) in &agents {
        let manifest = SkillManifest {
            name: name.to_string(),
            version: "1.0.0".into(),
            description: desc.to_string(),
            author: Some("tui".into()),
            tags: vec!["builtin".into(), "tui".into()],
            dependencies: vec![],
            bridges: vec![],
            bookmarks: vec![],
            category: "agents/builtin".into(),
            role: "assistant".into(),
            llm: LlmConfig::default(),
            tools: vec![],
            output_types: vec![],
            imported_from: Some("builtin".into()),
        };
        skill_reg.create_from_manifest(manifest, prompt);
        count += 1;
    }
    count
}
