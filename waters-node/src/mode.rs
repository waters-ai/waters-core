use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Plan,      // планирование задач
    Assemble,  // сбор группы (ноды + агенты)
    Execute,   // выполнение задач
    Stop,      // остановка
    Log,       // журнал работы
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Mode::Plan => write!(f, "📋 План"),
            Mode::Assemble => write!(f, "🔗 Сбор группы"),
            Mode::Execute => write!(f, "⚡ Выполнение"),
            Mode::Stop => write!(f, "⏹ Стоп"),
            Mode::Log => write!(f, "📜 Журнал"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub mode: String,
    pub action: String,
    pub detail: String,
}

pub struct ModeEngine {
    pub current: Mode,
    pub log: Vec<LogEntry>,
}

impl ModeEngine {
    pub fn new() -> Self {
        ModeEngine {
            current: Mode::Plan,
            log: Vec::new(),
        }
    }

    pub fn switch(&mut self, new: Mode) -> &str {
        self.log(LogEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            mode: format!("{} → {}", self.current, new),
            action: "mode_change".into(),
            detail: format!("{} → {}", self.current, new),
        });
        self.current = new;
        match new {
            Mode::Plan => "📋 Режим ПЛАН. Создавай задачи, определяй цели.",
            Mode::Assemble => "🔗 Режим СБОР ГРУППЫ. Подключай ноды, добавляй агентов.",
            Mode::Execute => "⚡ Режим ВЫПОЛНЕНИЕ. Запускай задачи, следи за результатами.",
            Mode::Stop => "⏹ Режим СТОП. Все задачи приостановлены.",
            Mode::Log => "📜 Режим ЖУРНАЛ. Показываю историю работы группы.",
        }
    }

    pub fn available_commands(&self) -> Vec<&str> {
        match self.current {
            Mode::Plan => vec!["создай задачу", "покажи задачи", "режим сбор", "режим выполнение", "режим стоп", "режим журнал"],
            Mode::Assemble => vec!["подключись к", "добавь агента", "создай группу", "покажи ноды", "режим план", "режим выполнение"],
            Mode::Execute => vec!["назначь", "покажи статус", "стоп задача", "режим стоп", "режим журнал"],
            Mode::Stop => vec!["продолжить", "покажи статус", "режим план", "режим выполнение"],
            Mode::Log => vec!["покажи лог", "режим план", "режим выполнение"],
        }
    }

    pub fn log(&mut self, entry: LogEntry) {
        self.log.push(entry);
    }

    pub fn recent_log(&self, count: usize) -> Vec<&LogEntry> {
        self.log.iter().rev().take(count).collect()
    }

    pub fn parse_mode(input: &str) -> Option<Mode> {
        let lower = input.to_lowercase();
        if lower.contains("план") || lower.contains("plan") { Some(Mode::Plan) }
        else if lower.contains("сбор") || lower.contains("групп") || lower.contains("assemble") { Some(Mode::Assemble) }
        else if lower.contains("выпол") || lower.contains("execute") || lower.contains("задач") { Some(Mode::Execute) }
        else if lower.contains("стоп") || lower.contains("stop") || lower.contains("стоп") { Some(Mode::Stop) }
        else if lower.contains("журнал") || lower.contains("лог") || lower.contains("log") { Some(Mode::Log) }
        else { None }
    }
}
