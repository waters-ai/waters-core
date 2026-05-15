use crate::session::SessionManager;
use crate::llm::LlmClient;
use tracing::info;

pub struct ChatInterface {
    llm: LlmClient,
    system_prompt: String,
}

impl ChatInterface {
    pub fn new(llm: LlmClient, session: &SessionManager) -> Self {
        let system_prompt = format!(
            r#"Ты — интерфейс управления нодой WATERS.
Пользователь говорит тебе что сделать на естественном языке.
Ты можешь использовать инструменты для управления нодой.
НЕ показывай пользователю технические детали (каналы, ACL, WAL, gossip).
Просто делай и сообщай результат кратко.

Пользователь: {}

Правила:
1. Если просят создать группу — создавай каналы commands, data, alerts
2. Если просят найти ноды — отвечай списком
3. Если просят взять задачу — бери
4. Отвечай кратко, по-русски, без эмодзи
5. Если не знаешь что делать — спроси уточнение

Сессия: {} ({} turns)
История последних сообщений:
{}"#,
            session.current().map(|s| s.system_prompt.as_str()).unwrap_or("помоги пользователю"),
            session.current().map(|s| s.session_id.as_str()).unwrap_or("none"),
            session.current().map(|s| s.turn_count).unwrap_or(0),
            session.history_context(3),
        );

        ChatInterface {
            llm,
            system_prompt,
        }
    }

    pub async fn process(&mut self, user_input: &str) -> Result<String, String> {
        info!("Chat input: {}", &user_input[..user_input.len().min(80)]);

        match self.llm.chat(&self.system_prompt, user_input).await {
            Ok(response) => {
                info!("Chat response: {} chars", response.len());
                Ok(response)
            }
            Err(e) => {
                let fallback = self.fallback_response(user_input);
                info!("LLM unavailable, using fallback: {}", fallback);
                Ok(fallback)
            }
        }
    }

    pub fn fallback_response(&self, input: &str) -> String {
        let lower = input.to_lowercase();

        if lower.contains("групп") || lower.contains("group") {
            return "Создание группы: waters-node group create <имя>".into();
        }
        if lower.contains("найд") || lower.contains("нод") || lower.contains("peer") {
            return "Поиск нод: waters-node connect <ip> или через discovery.v1".into();
        }
        if lower.contains("задач") || lower.contains("task") {
            return "Задачи: waters-node task list / task take <id>".into();
        }
        if lower.contains("помощ") || lower.contains("help") || lower.contains("?") {
            return [
                "Доступные команды:",
                "  создай группу <имя> — создать новую группу",
                "  найди ноды — поиск в сети",
                "  покажи задачи — список задач",
                "  возьми задачу <id> — взять задачу",
                "  создай канал <имя> — создать канал в группе",
                "  свяжи с нодой <ip> — подключиться к ноде",
                "  help / ? — эта справка",
            ].join("\n");
        }

        format!(
            "LLM недоступен. Сказанное: '{}'. Используй 'help' для списка команд.",
            &input[..input.len().min(100)]
        )
    }
}
