use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};

/// Глобальный флаг — разрешено ли самосовершенствование
pub static SELF_IMPROVE_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn toggle_self_improve(on: bool) -> bool {
    SELF_IMPROVE_ENABLED.store(on, Ordering::SeqCst);
    on
}

pub fn is_self_improve_enabled() -> bool {
    SELF_IMPROVE_ENABLED.load(Ordering::SeqCst)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Plan,     // планирование задач
    Assemble, // сбор группы (ноды + агенты)
    Execute,  // выполнение задач
    Stop,     // остановка
    Log,      // журнал работы
    Dnd,      // не беспокоить — отклонять новые подключения
    Sos,      // SOS — аварийный режим, экстренный вызов всех нод
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Mode::Plan => write!(f, "📋 План"),
            Mode::Assemble => write!(f, "🔗 Сбор группы"),
            Mode::Execute => write!(f, "⚡ Выполнение"),
            Mode::Stop => write!(f, "⏹ Стоп"),
            Mode::Log => write!(f, "📜 Журнал"),
            Mode::Dnd => write!(f, "🔇 Не беспокоить"),
            Mode::Sos => write!(f, "🆘 SOS — Аварийный режим"),
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
            Mode::Dnd => "🔇 Режим НЕ БЕСПОКОИТЬ. Новые подключения отклоняются.",
            Mode::Sos => {
                "🆘 SOS — АВАРИЙНЫЙ РЕЖИМ. Все агенты на полную. Приоритет — экстренная связь."
            }
        }
    }

    pub fn available_commands(&self) -> Vec<&str> {
        match self.current {
            Mode::Plan => vec![
                "создай задачу",
                "покажи задачи",
                "режим сбор",
                "режим выполнение",
                "режим стоп",
                "режим журнал",
            ],
            Mode::Assemble => vec![
                "подключись к",
                "добавь агента",
                "создай группу",
                "покажи ноды",
                "режим план",
                "режим выполнение",
            ],
            Mode::Execute => vec![
                "назначь",
                "покажи статус",
                "стоп задача",
                "режим стоп",
                "режим журнал",
            ],
            Mode::Stop => vec![
                "продолжить",
                "покажи статус",
                "режим план",
                "режим выполнение",
                "режим dnd",
            ],
            Mode::Log => vec!["покажи лог", "режим план", "режим выполнение", "режим dnd"],
            Mode::Dnd => vec![
                "режим план",
                "режим выполнение",
                "покажи контакты",
                "покажи статус",
            ],
            Mode::Sos => vec![
                "режим план",
                "покажи статус",
                "покажи пиров",
                "экстренный вызов",
                "sos сигнал",
            ],
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
        if lower.contains("план") || lower.contains("plan") {
            Some(Mode::Plan)
        } else if lower.contains("сбор") || lower.contains("групп") || lower.contains("assemble")
        {
            Some(Mode::Assemble)
        } else if lower.contains("выпол") || lower.contains("execute") || lower.contains("задач")
        {
            Some(Mode::Execute)
        } else if lower.contains("стоп") || lower.contains("stop") || lower.contains("стоп")
        {
            Some(Mode::Stop)
        } else if lower.contains("журнал") || lower.contains("лог") || lower.contains("log")
        {
            Some(Mode::Log)
        } else if lower.contains("dnd")
            || lower.contains("не беспоко")
            || lower.contains("тишин")
            || lower.contains("отбой")
        {
            Some(Mode::Dnd)
        } else if lower.contains("sos")
            || lower.contains("авария")
            || lower.contains("экстрен")
            || lower.contains("бедствие")
        {
            Some(Mode::Sos)
        } else {
            None
        }
    }
}
