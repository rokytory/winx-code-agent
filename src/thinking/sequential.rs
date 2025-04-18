use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;

/// Represents a single thought in a sequential thinking process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thought {
    /// The content of the thought
    pub content: String,
    /// The thought number in the sequence
    pub thought_number: usize,
    /// Total thoughts expected (can be adjusted)
    pub total_thoughts: usize,
    /// Whether another thought is needed
    pub next_thought_needed: bool,
    /// Whether this thought is a revision of a previous thought
    pub is_revision: bool,
    /// The thought number being revised (if any)
    pub revises_thought: Option<usize>,
    /// The thought number this branches from (if any)
    pub branch_from_thought: Option<usize>,
    /// Branch identifier (if any)
    pub branch_id: Option<String>,
    /// Whether more thoughts are needed after this one
    pub needs_more_thoughts: bool,
}

/// The sequential thinking process manager
#[derive(Debug, Clone)]
pub struct SequentialThinking {
    /// All thoughts in the process
    thoughts: HashMap<usize, Thought>,
    /// Current branch being followed
    current_branch: Option<String>,
    /// Latest thought number
    latest_thought_number: usize,
}

impl SequentialThinking {
    /// Create a new sequential thinking process
    pub fn new() -> Self {
        Self {
            thoughts: HashMap::new(),
            current_branch: None,
            latest_thought_number: 0,
        }
    }

    /// Add a new thought to the process
    pub fn add_thought(&mut self, thought: Thought) -> Result<()> {
        debug!(
            "Adding thought #{}: {}",
            thought.thought_number, thought.content
        );

        // Validate the thought
        if thought.thought_number == 0 {
            return Err(anyhow::anyhow!("Thought number must be greater than 0"));
        }

        if let Some(revises) = thought.revises_thought {
            if !self.thoughts.contains_key(&revises) {
                return Err(anyhow::anyhow!(
                    "Cannot revise nonexistent thought #{}",
                    revises
                ));
            }
        }

        if let Some(branch_from) = thought.branch_from_thought {
            if !self.thoughts.contains_key(&branch_from) {
                return Err(anyhow::anyhow!(
                    "Cannot branch from nonexistent thought #{}",
                    branch_from
                ));
            }
        }

        // Update current branch if needed
        if let Some(branch_id) = &thought.branch_id {
            self.current_branch = Some(branch_id.clone());
        }

        // Update latest thought number
        self.latest_thought_number = self.latest_thought_number.max(thought.thought_number);

        // Store the thought
        self.thoughts.insert(thought.thought_number, thought);

        Ok(())
    }

    /// Get all thoughts in the process
    pub fn get_thoughts(&self) -> Vec<&Thought> {
        let mut thoughts = self.thoughts.values().collect::<Vec<_>>();
        thoughts.sort_by_key(|t| t.thought_number);
        thoughts
    }

    /// Get the main branch of thoughts
    pub fn get_main_branch(&self) -> Vec<&Thought> {
        // Start with all thoughts in numerical order
        let mut thoughts = self.get_thoughts();

        // Filter out revised thoughts
        let revised_thoughts = thoughts
            .iter()
            .filter_map(|t| {
                if t.is_revision {
                    t.revises_thought
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        thoughts.retain(|t| !revised_thoughts.contains(&t.thought_number));

        // If there's a current branch, filter for that branch
        if let Some(branch) = &self.current_branch {
            let branch_thoughts = thoughts
                .iter()
                .filter(|t| t.branch_id.as_ref() == Some(branch))
                .cloned()
                .collect::<Vec<_>>();

            if !branch_thoughts.is_empty() {
                return branch_thoughts;
            }
        }

        thoughts
    }

    /// Get a specific thought by number
    pub fn get_thought(&self, thought_number: usize) -> Option<&Thought> {
        self.thoughts.get(&thought_number)
    }

    /// Check if another thought is needed
    pub fn needs_next_thought(&self) -> bool {
        // If there are no thoughts, we need one
        if self.thoughts.is_empty() {
            return true;
        }

        // Get the latest thought in the main branch
        match self.get_main_branch().last() {
            Some(thought) => thought.next_thought_needed || thought.needs_more_thoughts,
            None => true, // If no main branch, we need a thought
        }
    }

    /// Get a summary of the thinking process
    pub fn get_summary(&self) -> String {
        let main_branch = self.get_main_branch();

        if main_branch.is_empty() {
            return "No thoughts recorded yet.".to_string();
        }

        let mut summary = String::new();

        for thought in main_branch {
            let prefix = if thought.is_revision {
                format!(
                    "Thought #{} (revises #{})",
                    thought.thought_number,
                    thought.revises_thought.unwrap_or(0)
                )
            } else {
                format!("Thought #{}", thought.thought_number)
            };

            summary.push_str(&format!("{}: {}\n\n", prefix, thought.content));
        }

        summary
    }
}

// Implement Default separately to avoid conflicts with methods
impl Default for SequentialThinking {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sequential_thinking() {
        let mut thinking = SequentialThinking::new();

        // Add initial thought
        let thought1 = Thought {
            content: "First thought".to_string(),
            thought_number: 1,
            total_thoughts: 3,
            next_thought_needed: true,
            is_revision: false,
            revises_thought: None,
            branch_from_thought: None,
            branch_id: None,
            needs_more_thoughts: false,
        };

        thinking.add_thought(thought1.clone()).unwrap();

        assert_eq!(thinking.get_thoughts().len(), 1);
        assert_eq!(thinking.needs_next_thought(), true);

        // Add second thought
        let thought2 = Thought {
            content: "Second thought".to_string(),
            thought_number: 2,
            total_thoughts: 3,
            next_thought_needed: true,
            is_revision: false,
            revises_thought: None,
            branch_from_thought: None,
            branch_id: None,
            needs_more_thoughts: false,
        };

        thinking.add_thought(thought2.clone()).unwrap();

        assert_eq!(thinking.get_thoughts().len(), 2);
        assert_eq!(thinking.needs_next_thought(), true);

        // Add revised second thought
        let thought2_revised = Thought {
            content: "Revised second thought".to_string(),
            thought_number: 3,
            total_thoughts: 3,
            next_thought_needed: true,
            is_revision: true,
            revises_thought: Some(2),
            branch_from_thought: None,
            branch_id: None,
            needs_more_thoughts: false,
        };

        thinking.add_thought(thought2_revised.clone()).unwrap();

        assert_eq!(thinking.get_thoughts().len(), 3);
        assert_eq!(thinking.get_main_branch().len(), 2); // First thought and revised second

        // Complete the thinking process
        let thought3 = Thought {
            content: "Final thought".to_string(),
            thought_number: 4,
            total_thoughts: 3,
            next_thought_needed: false,
            is_revision: false,
            revises_thought: None,
            branch_from_thought: None,
            branch_id: None,
            needs_more_thoughts: false,
        };

        thinking.add_thought(thought3.clone()).unwrap();

        assert_eq!(thinking.get_thoughts().len(), 4);
        assert_eq!(thinking.needs_next_thought(), false);

        // Check the summary
        let summary = thinking.get_summary();
        assert!(summary.contains("Thought #1"));
        assert!(summary.contains("Thought #3 (revises #2)"));
        assert!(summary.contains("Thought #4"));
    }

    #[test]
    fn test_branching() {
        let mut thinking = SequentialThinking::new();

        // Add initial thoughts
        let thought1 = Thought {
            content: "First thought".to_string(),
            thought_number: 1,
            total_thoughts: 4,
            next_thought_needed: true,
            is_revision: false,
            revises_thought: None,
            branch_from_thought: None,
            branch_id: None,
            needs_more_thoughts: false,
        };

        thinking.add_thought(thought1.clone()).unwrap();

        let thought2 = Thought {
            content: "Second thought".to_string(),
            thought_number: 2,
            total_thoughts: 4,
            next_thought_needed: true,
            is_revision: false,
            revises_thought: None,
            branch_from_thought: None,
            branch_id: None,
            needs_more_thoughts: false,
        };

        thinking.add_thought(thought2.clone()).unwrap();

        // Add a branch
        let branch_thought = Thought {
            content: "Branch thought".to_string(),
            thought_number: 3,
            total_thoughts: 4,
            next_thought_needed: true,
            is_revision: false,
            revises_thought: None,
            branch_from_thought: Some(2),
            branch_id: Some("alternative".to_string()),
            needs_more_thoughts: false,
        };

        thinking.add_thought(branch_thought.clone()).unwrap();

        // Complete the branch
        let branch_final = Thought {
            content: "Branch final".to_string(),
            thought_number: 4,
            total_thoughts: 4,
            next_thought_needed: false,
            is_revision: false,
            revises_thought: None,
            branch_from_thought: None,
            branch_id: Some("alternative".to_string()),
            needs_more_thoughts: false,
        };

        thinking.add_thought(branch_final.clone()).unwrap();

        // Check that we get the branch in the main branch now
        let main = thinking.get_main_branch();
        assert_eq!(main.len(), 4);
        assert_eq!(main[2].content, "Branch thought");
        assert_eq!(main[3].content, "Branch final");
    }
}