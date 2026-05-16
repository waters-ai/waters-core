use std::path::PathBuf;
use serde::{Deserialize, Serialize};

pub enum ConvoAction {
    Exit,
    Menu,
    Help,
    ListTasks,
    ListAgents,
    ListGroups,
    ListPeers,
    Report,
    Setup,
    Response(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub name: String,
    pub greeted: bool,
    pub setup_done: bool,
}

impl UserProfile {
    pub fn new(name: &str) -> Self {
        UserProfile {
            name: name.to_string(),
            greeted: true,
            setup_done: false,
        }
    }
}

pub struct Convo {
    pub profile: UserProfile,
    pub step: u32,
}

impl Convo {
    pub fn load(path: &PathBuf) -> Self {
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Ok(profile) = serde_json::from_str::<UserProfile>(&content) {
                    let step = if profile.setup_done { 10 } else { 5 };
                    return Convo { profile, step };
                }
            }
        }
        Convo {
            profile: UserProfile { name: String::new(), greeted: false, setup_done: false },
            step: 0,
        }
    }

    pub fn save(&self, path: &PathBuf) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        if let Ok(json) = serde_json::to_string_pretty(&self.profile) {
            std::fs::write(path, json).ok();
        }
    }

    pub fn greet(&self) -> &'static str {
        "👋 Привет! Я waters-node.\n\nКак тебя зовут?"
    }

    pub fn handle(&mut self, input: &str) -> ConvoAction {
        let lower = input.to_lowercase().trim().to_string();

        if lower.contains("задачи") && (lower.contains("покажи") || lower.contains("список") || lower == "задачи") {
            return ConvoAction::ListTasks;
        }
        if lower.contains("агенты") && (lower.contains("покажи") || lower == "агенты" || lower == "мои агенты") {
            return ConvoAction::ListAgents;
        }
        if lower.contains("группы") && (lower.contains("покажи") || lower == "группы" || lower == "мои группы") {
            return ConvoAction::ListGroups;
        }
        if lower.contains("ноды") || lower.contains("подключен") || lower == "сеть" {
            return ConvoAction::ListPeers;
        }
        if lower.contains("отчёт") || lower.contains("отчет") || lower.contains("статус") {
            return ConvoAction::Report;
        }
        if lower.contains("настрой") || lower.contains("конфиг") {
            return ConvoAction::Setup;
        }

        match self.step {
            0 => {
                let name = self.extract_name(&lower);
                self.profile.name = name.clone();
                self.profile.greeted = true;
                self.step = 1;
                ConvoAction::Response(format!("Приятно познакомиться, {0}! 🌊\n\nЧто хочешь сделать?\n• задачи\n• агенты\n• группы\n• отчёт\n• помощь", name))
            }
            5 => {
                self.step = 10;
                ConvoAction::Response(format!("С возвращением, {0}! 🌊\n• задачи\n• агенты\n• группы\n• помощь", self.profile.name))
            }
            _ => {
                if lower.contains("помощ") || lower == "help" || lower == "?" || lower == "меню" {
                    ConvoAction::Help
                } else if lower.contains("привет") || lower.contains("здрав") || lower.contains("hello") {
                    ConvoAction::Response(format!("Привет, {0}! Чем займёмся?", self.profile.name))
                } else if lower.contains("пока") || lower.contains("до свидан") {
                    ConvoAction::Response(format!("Пока, {0}! напиши exit чтобы выключить", self.profile.name))
                } else if lower.contains("exit") || lower.contains("quit") || lower.contains("выход") {
                    ConvoAction::Exit
                } else {
                    ConvoAction::Response(format!("Не понял, {0}. Попробуй: задачи | агенты | группы | отчёт", self.profile.name))
                }
            }
        }
    }

    fn extract_name(&self, lower: &str) -> String {
        let name = lower.trim()
            .trim_start_matches("меня зовут ")
            .trim_start_matches("я ")
            .trim_start_matches("меня ")
            .trim_start_matches("зовут ")
            .trim_start_matches("my name is ")
            .trim_start_matches("i am ")
            .trim_start_matches("i'm ")
            .to_string();
        let name = if name.len() > 1 && name.len() < 30 { name }
                  else if lower.len() > 1 && lower.len() < 30 { lower.to_string() }
                  else { "друг".to_string() };
        let first = name.chars().next().unwrap_or('д').to_uppercase().to_string();
        let rest = &name[1..];
        format!("{}{}", first, rest)
    }
}
