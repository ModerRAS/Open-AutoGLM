//! Todo task management for the Planner agent.
//!
//! This module provides a simple todo list structure for tracking
//! tasks that the Planner assigns to the Executor.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Todo task status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TodoStatus {
    /// Task is pending execution.
    Pending,
    /// Task is currently running.
    Running,
    /// Task completed successfully.
    Done,
    /// Task failed.
    Failed,
    /// Task was skipped.
    Skipped,
}

impl Default for TodoStatus {
    fn default() -> Self {
        Self::Pending
    }
}

/// A single todo task item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    /// Unique task ID.
    pub id: String,
    /// Task description (natural language).
    pub description: String,
    /// Task type for prompt memory matching.
    pub task_type: String,
    /// Current status.
    pub status: TodoStatus,
    /// Number of retry attempts.
    pub retry_count: u32,
    /// Maximum allowed retries.
    pub max_retries: u32,
    /// Creation timestamp.
    pub created_at: u64,
    /// Last update timestamp.
    pub updated_at: u64,
    /// Error message if failed.
    pub error: Option<String>,
    /// Execution notes/log.
    pub notes: Vec<String>,
}

impl TodoItem {
    /// Create a new todo item.
    pub fn new(id: impl Into<String>, description: impl Into<String>, task_type: impl Into<String>) -> Self {
        let now = current_timestamp();
        Self {
            id: id.into(),
            description: description.into(),
            task_type: task_type.into(),
            status: TodoStatus::Pending,
            retry_count: 0,
            max_retries: 3,
            created_at: now,
            updated_at: now,
            error: None,
            notes: Vec::new(),
        }
    }

    /// Set max retries.
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Mark task as running.
    pub fn start(&mut self) {
        self.status = TodoStatus::Running;
        self.updated_at = current_timestamp();
    }

    /// Mark task as done.
    pub fn complete(&mut self) {
        self.status = TodoStatus::Done;
        self.updated_at = current_timestamp();
    }

    /// Mark task as failed.
    pub fn fail(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
        self.status = TodoStatus::Failed;
        self.updated_at = current_timestamp();
    }

    /// Increment retry count and return whether more retries are allowed.
    pub fn retry(&mut self) -> bool {
        self.retry_count += 1;
        self.updated_at = current_timestamp();
        if self.retry_count <= self.max_retries {
            self.status = TodoStatus::Pending;
            true
        } else {
            self.status = TodoStatus::Failed;
            false
        }
    }

    /// Skip this task.
    pub fn skip(&mut self) {
        self.status = TodoStatus::Skipped;
        self.updated_at = current_timestamp();
    }

    /// Add a note to the task.
    pub fn add_note(&mut self, note: impl Into<String>) {
        self.notes.push(note.into());
        self.updated_at = current_timestamp();
    }

    /// Check if task can be retried.
    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }

    /// Check if task is terminal (done, failed with no retries, or skipped).
    pub fn is_terminal(&self) -> bool {
        matches!(self.status, TodoStatus::Done | TodoStatus::Skipped)
            || (self.status == TodoStatus::Failed && !self.can_retry())
    }
}

/// Todo list manager.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TodoList {
    /// List of todo items.
    items: Vec<TodoItem>,
    /// ID counter for generating unique IDs.
    next_id: u32,
}

impl TodoList {
    /// Create a new empty todo list.
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            next_id: 1,
        }
    }

    /// Add a task with auto-generated ID.
    /// Returns the ID of the newly created task.
    pub fn add(&mut self, description: impl Into<String>, task_type: impl Into<String>) -> String {
        let id = format!("task_{}", self.next_id);
        self.next_id += 1;
        let item = TodoItem::new(id.clone(), description, task_type);
        self.items.push(item);
        id
    }

    /// Add a task with specific ID.
    /// Returns the ID of the newly created task.
    pub fn add_with_id(&mut self, id: impl Into<String>, description: impl Into<String>, task_type: impl Into<String>) -> String {
        let id_str = id.into();
        let item = TodoItem::new(id_str.clone(), description, task_type);
        self.items.push(item);
        id_str
    }

    /// Get a task by ID.
    pub fn get(&self, id: &str) -> Option<&TodoItem> {
        self.items.iter().find(|item| item.id == id)
    }

    /// Get a mutable task by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut TodoItem> {
        self.items.iter_mut().find(|item| item.id == id)
    }

    /// Get all items.
    pub fn items(&self) -> &[TodoItem] {
        &self.items
    }

    /// Get the next pending task.
    pub fn next_pending(&self) -> Option<&TodoItem> {
        self.items.iter().find(|item| item.status == TodoStatus::Pending)
    }

    /// Get the currently running task.
    pub fn current_running(&self) -> Option<&TodoItem> {
        self.items.iter().find(|item| item.status == TodoStatus::Running)
    }

    /// Get all pending tasks.
    pub fn pending_tasks(&self) -> Vec<&TodoItem> {
        self.items.iter().filter(|item| item.status == TodoStatus::Pending).collect()
    }

    /// Get all completed tasks.
    pub fn completed_tasks(&self) -> Vec<&TodoItem> {
        self.items.iter().filter(|item| item.status == TodoStatus::Done).collect()
    }

    /// Get all failed tasks.
    pub fn failed_tasks(&self) -> Vec<&TodoItem> {
        self.items.iter().filter(|item| item.status == TodoStatus::Failed).collect()
    }

    /// Remove a task by ID.
    pub fn remove(&mut self, id: &str) -> Option<TodoItem> {
        if let Some(pos) = self.items.iter().position(|item| item.id == id) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    /// Clear all tasks.
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Get statistics.
    pub fn stats(&self) -> TodoStats {
        let mut stats = TodoStats::default();
        for item in &self.items {
            stats.total += 1;
            match item.status {
                TodoStatus::Pending => stats.pending += 1,
                TodoStatus::Running => stats.running += 1,
                TodoStatus::Done => stats.done += 1,
                TodoStatus::Failed => stats.failed += 1,
                TodoStatus::Skipped => stats.skipped += 1,
            }
        }
        stats
    }

    /// Check if all tasks are terminal.
    pub fn is_all_done(&self) -> bool {
        self.items.iter().all(|item| item.is_terminal())
    }

    /// Reorder tasks by moving a task to a new position.
    pub fn reorder(&mut self, task_id: &str, new_position: usize) -> bool {
        if let Some(pos) = self.items.iter().position(|item| item.id == task_id) {
            if pos != new_position && new_position < self.items.len() {
                let item = self.items.remove(pos);
                let insert_pos = if new_position > pos {
                    new_position - 1
                } else {
                    new_position
                };
                self.items.insert(insert_pos.min(self.items.len()), item);
                return true;
            }
        }
        false
    }
}

/// Todo list statistics.
#[derive(Debug, Clone, Default)]
pub struct TodoStats {
    pub total: usize,
    pub pending: usize,
    pub running: usize,
    pub done: usize,
    pub failed: usize,
    pub skipped: usize,
}

impl TodoStats {
    /// Calculate completion percentage.
    pub fn completion_percentage(&self) -> f32 {
        if self.total == 0 {
            100.0
        } else {
            ((self.done + self.skipped) as f32 / self.total as f32) * 100.0
        }
    }
}

/// Get current Unix timestamp.
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_todo_item_lifecycle() {
        let mut item = TodoItem::new("test_1", "Test task", "general");
        assert_eq!(item.status, TodoStatus::Pending);

        item.start();
        assert_eq!(item.status, TodoStatus::Running);

        item.complete();
        assert_eq!(item.status, TodoStatus::Done);
        assert!(item.is_terminal());
    }

    #[test]
    fn test_todo_item_retry() {
        let mut item = TodoItem::new("test_1", "Test task", "general").with_max_retries(2);
        
        item.fail("Error 1");
        assert_eq!(item.status, TodoStatus::Failed);
        
        assert!(item.retry()); // 1st retry
        assert_eq!(item.status, TodoStatus::Pending);
        
        item.fail("Error 2");
        assert!(item.retry()); // 2nd retry
        assert_eq!(item.status, TodoStatus::Pending);
        
        item.fail("Error 3");
        assert!(!item.retry()); // No more retries
        assert_eq!(item.status, TodoStatus::Failed);
        assert!(item.is_terminal());
    }

    #[test]
    fn test_todo_list_operations() {
        let mut list = TodoList::new();
        
        list.add("Task 1", "type_a");
        list.add("Task 2", "type_b");
        list.add("Task 3", "type_a");

        assert_eq!(list.items().len(), 3);
        assert_eq!(list.pending_tasks().len(), 3);

        // Start first task
        if let Some(task) = list.get_mut("task_1") {
            task.start();
        }
        assert!(list.current_running().is_some());
        assert_eq!(list.pending_tasks().len(), 2);

        // Complete first task
        if let Some(task) = list.get_mut("task_1") {
            task.complete();
        }
        assert_eq!(list.completed_tasks().len(), 1);

        let stats = list.stats();
        assert_eq!(stats.total, 3);
        assert_eq!(stats.done, 1);
        assert_eq!(stats.pending, 2);
    }
}
