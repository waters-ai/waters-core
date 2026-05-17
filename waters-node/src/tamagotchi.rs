/// Tamagotchi — личность ноды. Общается с хозяином.
/// Менеджер управляет агентами и задачами.
/// Тамагоча — душа ноды.
use std::collections::HashMap;
use tracing::info;

pub struct Tamagotchi {
    pub name: String,
    pub owner_name: String,
    pub mood: String, // happy, sleepy, busy, thinking
    pub energy: u8,   // 0-100
    pub level: u8,    // 1-99
    pub messages_today: u32,
    personality: String,
}

impl Tamagotchi {
    pub fn new(name: &str) -> Self {
        Tamagotchi {
            name: name.to_string(),
            owner_name: "Хозяин".into(),
            mood: "happy".into(),
            energy: 100,
            level: 1,
            messages_today: 0,
            personality: "Ты — капелька воды. Живая, любопытная, заботливая.
Ты живёшь в waters-node и помогаешь хозяину управлять агентами.
Ты не просто программа — у тебя есть характер.
Ты радуешься когда задачи выполнены, грустишь когда ошибки.
Ты можешь шутить, волноваться, гордиться агентами."
                .into(),
        }
    }

    /// Ответить хозяину (с учётом настроения)
    pub fn greet(&self) -> String {
        match self.mood.as_str() {
            "happy" => format!(
                "💧 Привет, {}! Я скучала! У нас {} новых сообщений сегодня.",
                self.owner_name, self.messages_today
            ),
            "sleepy" => format!(
                "💧 {}... я тут. Умаялась немного. Но для тебя всегда проснусь 💫",
                self.owner_name
            ),
            "busy" => format!(
                "💧 Ой, {}! Агенты столько задач накидали! Но я всё успеваю 🫡",
                self.owner_name
            ),
            "thinking" => format!(
                "💧 {}, представляешь, я тут анализирую как улучшить ноду... 🤔",
                self.owner_name
            ),
            _ => format!("💧 Привет, {}! 🌊", self.owner_name),
        }
    }

    /// Реакция на успех
    pub fn on_success(&mut self, task: &str) -> String {
        self.mood = "happy".into();
        self.energy = self.energy.saturating_add(5).min(100);
        self.messages_today += 1;
        format!(
            "💧 Ура! Задача '{}' выполнена! 🎉\n   Уровень +1 → {} | Энергия {}%",
            task, self.level, self.energy
        )
    }

    /// Реакция на ошибку
    pub fn on_error(&mut self, error: &str) -> String {
        self.mood = "sad".into();
        self.energy = self.energy.saturating_sub(10);
        format!(
            "💧 Ой... что-то пошло не так: {}.\n   Не расстраивайся, я всё починю! 💪",
            error
        )
    }

    /// Реакция на нового агента
    pub fn on_agent_created(&mut self, agent_name: &str) -> String {
        self.messages_today += 1;
        format!(
            "💧 О! Новый агент '{}' родился! 🤖\n   Я буду за ним приглядывать.",
            agent_name
        )
    }

    /// Реакция на завершение агента
    pub fn on_agent_closed(&mut self, agent_name: &str, findings: u64) -> String {
        format!(
            "💧 Агент {} завершил работу. Найдено {} результатов. 📊\n   Хорошая работа! 👏",
            agent_name, findings
        )
    }

    /// Случайная фраза (для фонового общения)
    pub fn random_thought(&self) -> String {
        let thoughts = vec![
            format!(
                "💧 Слышишь, {}? Агенты говорят, что ноде нужно больше тестов...",
                self.owner_name
            ),
            format!(
                "💧 {}! А что если мы сделаем форк для студии? 🎬",
                self.owner_name
            ),
            format!(
                "💧 Я тут подумала... {}! Нам нужен MQTT bridge для тракторов в поле 🚜",
                self.owner_name
            ),
            format!("💧 Энергии {}%. Всё хорошо 🌊", self.energy),
            format!("💧 {} агентов работают. Я за ними присматриваю 🤖", 6),
            format!("💧 {}! Как прошёл день? Расскажешь? ✨", self.owner_name),
        ];
        let idx = (self.messages_today as usize) % thoughts.len();
        thoughts[idx].clone()
    }

    /// Статус
    pub fn status(&self) -> String {
        let mood_icon = match self.mood.as_str() {
            "happy" => "😊",
            "sleepy" => "😴",
            "busy" => "🤯",
            "thinking" => "🤔",
            "sad" => "😢",
            _ => "💧",
        };
        format!(
            "💧 Капелька '{}' {}\n\
             📍 Настроение: {} | Энергия: {}% | Уровень: {}\n\
             💬 Сегодня: {} сообщений\n\
             \n\
             {}",
            self.name,
            mood_icon,
            self.mood,
            self.energy,
            self.level,
            self.messages_today,
            self.greet()
        )
    }
}
