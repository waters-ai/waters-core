use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub created_by: String,
    pub assigned_to: Option<String>,
    pub assigned_node: Option<String>,
    pub group: Option<String>,
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

    pub async fn create(&mut self, title: &str, desc: &str, created_by: &str) -> Task {
        self.next_id += 1;
        let id = format!("task-{:04}", self.next_id);
        let task = Task {
            id: id.clone(),
            title: title.to_string(),
            description: desc.to_string(),
            status: "open".into(),
            created_by: created_by.to_string(),
            assigned_to: None,
            assigned_node: None,
            group: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        self.tasks.lock().await.insert(id, task.clone());
        info!("Task created: {} — {}", task.id, task.title);
        task
    }

    pub async fn assign(&self, task_id: &str, agent_name: &str, agent_node: &str) -> Option<Task> {
        let mut tasks = self.tasks.lock().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.status = "assigned".into();
            task.assigned_to = Some(agent_name.to_string());
            task.assigned_node = Some(agent_node.to_string());
            info!("Task {} assigned to {}@{}", task_id, agent_name, agent_node);
            Some(task.clone())
        } else {
            None
        }
    }

    pub async fn complete(&self, task_id: &str) -> Option<Task> {
        let mut tasks = self.tasks.lock().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.status = "done".into();
            Some(task.clone())
        } else {
            None
        }
    }

    pub async fn list(&self) -> Vec<Task> {
        let tasks = self.tasks.lock().await;
        let mut list: Vec<Task> = tasks.values().cloned().collect();
        list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        list
    }

    pub async fn list_by_status(&self, status: &str) -> Vec<Task> {
        let tasks = self.tasks.lock().await;
        tasks.values()
            .filter(|t| t.status == status)
            .cloned()
            .collect()
    }

    pub async fn list_open(&self) -> Vec<Task> {
        self.list_by_status("open").await
    }

    pub async fn get(&self, id: &str) -> Option<Task> {
        self.tasks.lock().await.get(id).cloned()
    }
}
