/// Мультиязычная поддержка — переводы для агентов и уведомлений

#[derive(Debug, Clone)]
pub struct Lang {
    pub code: &'static str,
    pub name: &'static str,
}

pub const LANGUAGES: &[Lang] = &[
    Lang {
        code: "ru",
        name: "Русский",
    },
    Lang {
        code: "en",
        name: "English",
    },
    Lang {
        code: "zh",
        name: "中文",
    },
];

pub fn t(lang: &str, key: &str) -> String {
    match lang {
        "ru" => ru(key),
        "en" => en(key),
        "zh" => zh(key),
        _ => en(key),
    }
}

fn ru(key: &str) -> String {
    match key {
        "node_started" => "✅ Нода запущена".into(),
        "node_stopped" => "⏹ Нода остановлена".into(),
        "peer_connected" => "🔗 Пир подключился".into(),
        "peer_disconnected" => "🔗 Пир отключился".into(),
        "agent_created" => "🤖 Агент создан".into(),
        "agent_closed" => "🤖 Агент завершён".into(),
        "agent_error" => "❌ Ошибка агента".into(),
        "acl_blocked" => "🔒 ACL заблокировал сообщение".into(),
        "dnd_mode" => "🔇 Режим DND — новые подключения отклоняются".into(),
        "sos_alert" => "🆘 SOS — экстренный вызов!".into(),
        "skill_evolved" => "🧬 Скил эволюционировал".into(),
        "push_unknown" => "🔔 Событие".into(),
        _ => key.to_string(),
    }
}

fn en(key: &str) -> String {
    match key {
        "node_started" => "✅ Node started".into(),
        "node_stopped" => "⏹ Node stopped".into(),
        "peer_connected" => "🔗 Peer connected".into(),
        "peer_disconnected" => "🔗 Peer disconnected".into(),
        "agent_created" => "🤖 Agent created".into(),
        "agent_closed" => "🤖 Agent finished".into(),
        "agent_error" => "❌ Agent error".into(),
        "acl_blocked" => "🔒 ACL blocked message".into(),
        "dnd_mode" => "🔇 DND mode — new connections rejected".into(),
        "sos_alert" => "🆘 SOS — emergency call!".into(),
        "skill_evolved" => "🧬 Skill evolved".into(),
        "push_unknown" => "🔔 Event".into(),
        _ => key.to_string(),
    }
}

fn zh(key: &str) -> String {
    match key {
        "node_started" => "✅ 节点已启动".into(),
        "node_stopped" => "⏹ 节点已停止".into(),
        "peer_connected" => "🔗 对等已连接".into(),
        "peer_disconnected" => "🔗 对等已断开".into(),
        "agent_created" => "🤖 智能体已创建".into(),
        "agent_closed" => "🤖 智能体已完成".into(),
        "agent_error" => "❌ 智能体错误".into(),
        "acl_blocked" => "🔒 ACL 已阻止消息".into(),
        "dnd_mode" => "🔇 DND 模式 — 拒绝新连接".into(),
        "sos_alert" => "🆘 SOS — 紧急呼叫！".into(),
        "skill_evolved" => "🧬 技能已进化".into(),
        "push_unknown" => "🔔 事件".into(),
        _ => key.to_string(),
    }
}

/// Код языка по умолчанию (из ENV или "ru")
pub fn default_lang() -> String {
    std::env::var("WATERS_LANG").unwrap_or_else(|_| "ru".into())
}
