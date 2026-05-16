use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskMode {
    Plan,
    Execute,
    Stop,
}

impl TaskMode {
    pub fn parse(input: &str) -> Option<TaskMode> {
        let lower = input.to_lowercase();
        if lower.contains("план") || lower.contains("plan") { Some(TaskMode::Plan) }
        else if lower.contains("выпол") || lower.contains("execute") || lower.contains("задач") { Some(TaskMode::Execute) }
        else if lower.contains("стоп") || lower.contains("stop") { Some(TaskMode::Stop) }
        else { None }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Executor {
    pub agent_id: String,
    pub node_id: String,
    pub role: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskResources {
    pub bridges: Vec<String>,
    pub databases: Vec<String>,
    pub mcp_tools: Vec<String>,
    pub storage: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub mode: TaskMode,
    pub created_by: String,
    pub assigned_to: Option<String>,
    pub assigned_node: Option<String>,
    pub group: Option<String>,
    pub executors: Vec<Executor>,
    pub resources: TaskResources,
    pub created_at: String,
}

pub struct TaskManager {
    tasks: Arc<Mutex<HashMap<String, Task>>>,
    next_id: u64,
}

impl TaskManager {
    pub fn new() -> Self {
        TaskManager {
            tasks: Arc::new(Mutex::new(HashMap::new())),
            next_id: 0,
        }
    }

    pub async fn create(&mut self, title: &str, desc: &str, created_by: &str, group: Option<&str>) -> Task {
        self.next_id += 1;
        let id = format!("task-{:04}", self.next_id);
        let task = Task {
            id: id.clone(),
            title: title.to_string(),
            description: desc.to_string(),
            status: "open".into(),
            mode: TaskMode::Plan,
            created_by: created_by.to_string(),
            assigned_to: None,
            assigned_node: None,
            group: group.map(|g| g.to_string()),
            executors: Vec::new(),
            resources: TaskResources::default(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        self.tasks.lock().await.insert(id, task.clone());
        info!("Task created: {} — {} [mode: Plan]", task.id, task.title);
        task
    }

    /// Assign an agent to this task
    pub async fn assign_agent(&self, task_id: &str, agent_id: &str, node_id: &str, role: &str) -> Option<Task> {
        let mut tasks = self.tasks.lock().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.executors.push(Executor {
                agent_id: agent_id.to_string(),
                node_id: node_id.to_string(),
                role: role.to_string(),
                status: "assigned".into(),
            });
            task.assigned_to = Some(agent_id.to_string());
            task.assigned_node = Some(node_id.to_string());
            task.status = "assigned".into();
            info!("Task {}: agent {} assigned as {}", task_id, agent_id, role);
            Some(task.clone())
        } else { None }
    }

    pub async fn set_mode(&self, task_id: &str, mode: TaskMode) -> Option<Task> {
        let mut tasks = self.tasks.lock().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.mode = mode;
            info!("Task {} mode: {:?}", task_id, mode);
            Some(task.clone())
        } else { None }
    }

    pub async fn bind_resource(&self, task_id: &str, res_type: &str, name: &str) -> Option<Task> {
        let mut tasks = self.tasks.lock().await;
        if let Some(task) = tasks.get_mut(task_id) {
            match res_type {
                "bridge" => task.resources.bridges.push(name.to_string()),
                "db" | "database" => task.resources.databases.push(name.to_string()),
                "mcp" => task.resources.mcp_tools.push(name.to_string()),
                "storage" => task.resources.storage = Some(name.to_string()),
                _ => {}
            }
            info!("Task {}: {} bound {}", task_id, res_type, name);
            Some(task.clone())
        } else { None }
    }

    pub async fn complete(&self, task_id: &str) -> Option<Task> {
        let mut tasks = self.tasks.lock().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.status = "done".into();
            Some(task.clone())
        } else { None }
    }

    pub async fn list(&self) -> Vec<Task> {
        let tasks = self.tasks.lock().await;
        let mut list: Vec<Task> = tasks.values().cloned().collect();
        list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        list
    }

    pub async fn list_by_group(&self, group: &str) -> Vec<Task> {
        let tasks = self.tasks.lock().await;
        let mut list: Vec<Task> = tasks.values()
            .filter(|t| t.group.as_deref() == Some(group))
            .cloned().collect();
        list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        list
    }

    pub async fn get(&self, id: &str) -> Option<Task> {
        self.tasks.lock().await.get(id).cloned()
    }
}
