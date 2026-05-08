# 🌐 Multilang Policy — Политика мультиязычности / 多语言政策

**Статус:** Доктрина. Фаза 0.
**Дата:** 07.05.2026
**Версия:** 1.0
**Автор:** Конструктор Сети v1.0
**Связанные документы:** [Манифест](manifesto.md), [Гексада](hexad.md), [Social Contract](../SOCIAL_CONTRACT.md)

---

## 1. Цель / Purpose / 目的

Обеспечить ведение всего GitHub-репозитория WATERS на трёх языках:
русский, английский, китайский (упрощённый).

Ensure the entire WATERS GitHub repository is maintained in three languages:
Russian, English, and Chinese (Simplified).

确保整个 WATERS GitHub 仓库以三种语言维护：俄语、英语和中文（简体）。

---

## 2. Распределение ответственности / Responsibility / 责任分配

| Язык / Language / 语言 | Кто отвечает / Who / 负责人 | Тип файлов / File types / 文件类型 |
|:---|:---|:---|
| **Русский** | Архитектор (доктрины) / Конструктор (техника) | Основной язык документов |
| **English** | Constructor (technical) / Architect (doctrines) | Interface with global community |
| **中文** | Constructor (technical) / Architect (doctrines) | Interface with Chinese community |

### 2.1 Архитектор / Architect / 架构师
- Доктрины: manifesto, hexad, hivemind, multilang_policy, security, skills
- Онтологии и спецификации агентов
- Doctrines: manifesto, hexad, hivemind, multilang_policy, security, skills
- Ontologies and agent specifications
- 负责：纲领文件（manifesto, hexad, hivemind 等）、本体论和智能体规范

### 2.2 Конструктор / Constructor / 构造师
- Технические файлы: run_*.sh, .env.*, docker-compose.yml, Dockerfile.*
- Инфраструктурные схемы и топологии
- JSON-схемы HiveMind
- Technical files: run_*.sh, .env.*, docker-compose.yml, Dockerfile.*
- Infrastructure schemas and topologies
- HiveMind JSON schemas
- 负责：技术文件（脚本、Docker 配置、环境变量）、基础设施架构、HiveMind JSON 模式

---

## 3. Структура файлов / File Structure / 文件结构

Каждый файл содержит блоки на трёх языках через разделители.

Each file contains three language blocks separated by horizontal rules.

每个文件通过分隔线包含三个语言区块。

### 3.1 Формат заголовка / Header Format / 标题格式

```
# 🌐 Title in Russian — Title in English / 中文标题
```

### 3.2 Разделители / Delimiters / 分隔符

```
---
```

### 3.3 Порядок языков / Language Order / 语言顺序

1. **Русский** — первичный язык платформы
2. **English** — вторичный, для международного сообщества
3. **中文** — третичный, для китайского сообщества

---

## 4. Правила перевода / Translation Rules / 翻译规则

### 4.1 Обязательные поля / Mandatory Fields / 必填字段

- Заголовки / Titles / 标题
- Описания / Descriptions / 描述
- Ключевые термины / Key Terms / 关键术语
- Комментарии в коде (на русском) / Code Comments (in Russian) / 代码注释（俄语）

### 4.2 Опциональные поля / Optional Fields / 可选字段

- Логи изменений / Changelogs / 更新日志
- Примечания / Notes / 备注
- TODO-комментарии / TODO Comments / TODO 注释

### 4.3 Глоссарий терминов / Glossary / 术语表

| Русский | English | 中文 |
|:---|:---|:---|
| Конструктор Сети | Network Constructor | 网络构造师 |
| Верховный Архитектор | Chief Architect | 首席架构师 |
| Интегратор Знаний | Knowledge Integrator | 知识集成师 |
| Директор по Смыслу | Director of Meaning | 意义总监 |
| Хранитель Протокола | Protocol Keeper | 协议守护者 |
| Законодатель | Lawkeeper | 立法者 |
| Гексада | Hexad | 六边形体系 |
| HiveMind | HiveMind | 蜂群心智 |
| Троица | Trinity | 三位一体 |
| Яса | Yasa | 雅萨法典 |
| Пирожок | Bionic Cell (Agent) | 仿生细胞（智能体） |
| Карусель | Carousel | 轮转系统 |
| Планёрка | Planning Meeting | 规划会议 |

---

## 5. Процесс синхронизации / Sync Process / 同步流程

### 5.1 Создание нового файла / New File / 新文件

1. Автор пишет на русском (основной язык платформы)
2. Автор добавляет английский перевод
3. Автор добавляет китайский перевод (или оставляет TODO)
4. Файл проходит ревью Архитектора (доктрины) или Конструктора (техника)

1. Author writes in Russian (primary platform language)
2. Author adds English translation
3. Author adds Chinese translation (or leaves TODO)
4. File goes through review by Architect (doctrines) or Constructor (technical)

1. 作者用俄语编写（平台主要语言）
2. 作者添加英文翻译
3. 作者添加中文翻译（或留 TODO）
4. 文件由架构师（纲领文件）或构造师（技术文件）审核

### 5.2 Обновление файла / File Update / 文件更新

- Все три языковые версии обновляются одновременно
- Если один язык отстаёт, в файл добавляется уведомление
- Максимальное отставание: 1 спринт (7 дней)

- All three language versions are updated simultaneously
- If one language lags, a notice is added to the file
- Maximum lag: 1 sprint (7 days)

- 所有三种语言版本同时更新
- 如果某种语言滞后，需在文件中添加通知
- 最大滞后时间：1 个冲刺（7 天）

### 5.3 Ревью и контроль / Review & Control / 审核与监控

| Метрика / Metric / 指标 | Цель / Target / 目标 | Ответственный / Responsible / 负责人 |
|:---|:---|:---|
| Покрытие EN | 100% доктрин и технических файлов | Конструктор |
| Покрытие ZH | 100% доктрин, > 80% технических | Конструктор |
| Синхронизация версий | < 7 дней отставания | Архитектор |

---

## 6. Инструменты / Tools / 工具

| Инструмент / Tool / 工具 | Назначение / Purpose / 用途 |
|:---|:---|:---|
| GitHub Actions | CI для проверки наличия всех трёх языков |
| `scripts/check_lang_coverage.sh` | Локальная проверка покрытия языков |
| AI-перевод (OpenCode/LLM) | Первичный перевод для черновиков |
| Человеческое ревью | Вычистка и адаптация переводов |

---

## 7. Приоритетные файлы для перевода / Priority Files / 优先文件

### Спринт 1 (Фаза 0)

| Файл / File / 文件 | Текущий статус / Status / 状态 | Ответственный / By / 负责人 |
|:---|:---|:---|
| `doctrine/manifesto.md` | Только RU | Архитектор |
| `doctrine/hexad.md` | Только RU | Архитектор |
| `doctrine/hivemind_spec.md` | Только RU | Архитектор |
| `doctrine/multilang_policy.md` | RU+EN+ZH | Конструкор |
| `README.md` | Только RU | Конструкор |
| `AGENTS.md` | Только RU | Конструкор |
| `SOCIAL_CONTRACT.md` | Только RU | Архитектор |
| `run_carousel.sh` | Только RU (комментарии) | Конструкор |
| `run_terminals.sh` | Только RU (комментарии) | Конструкор |

---

## 8. Исключения / Exceptions / 例外

- **Код (Python, Bash, JS):** комментарии остаются на русском (язык платформы).
  Переменные, функции — английские нейминг-конвенции.
- **JSON/YAML:** ключи на английском, значения на всех трёх языках
  где применимо.
- **Логи и сообщения Kafka:** на английском (язык системы).

- **Code (Python, Bash, JS):** comments remain in Russian (platform language).
  Variables, functions — English naming conventions.
- **JSON/YAML:** keys in English, values in all three languages where applicable.
- **Logs and Kafka messages:** in English (system language).

- **代码（Python, Bash, JS）：** 注释保留俄语（平台语言）。
  变量、函数使用英文命名规范。
- **JSON/YAML：** 键使用英文，值在适用时使用所有三种语言。
- **日志和 Kafka 消息：** 使用英文（系统语言）。

---

*Вода принимает форму любого сосуда — и русского, и английского, и китайского.*
*Water takes the shape of any vessel — Russian, English, or Chinese.*
*水能适应任何容器的形状——无论是俄语、英语还是中文。*
