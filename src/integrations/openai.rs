use anyhow::Result;
use async_openai::{
    types::{ChatCompletionRequestMessage, CreateChatCompletionRequestArgs},
    Client,
};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::thinking::sequential::{SequentialThinking, Thought};

const DEFAULT_MODEL: &str = "gpt-4o";

/// OpenAI client configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIConfig {
    /// API Key (if not provided, will use OPENAI_API_KEY env var)
    pub api_key: Option<String>,
    /// Organization ID (if not provided, will use OPENAI_ORG_ID env var)
    pub org_id: Option<String>,
    /// Model to use
    pub model: String,
    /// Max tokens to generate
    pub max_tokens: Option<i32>,
    /// Temperature
    pub temperature: Option<f32>,
}

impl Default for OpenAIConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            org_id: None,
            model: DEFAULT_MODEL.to_string(),
            max_tokens: Some(2048),
            temperature: Some(0.7),
        }
    }
}

/// OpenAI client wrapper
#[derive(Debug, Clone)]
pub struct OpenAIClient {
    client: Client<async_openai::config::OpenAIConfig>,
    config: OpenAIConfig,
}

impl OpenAIClient {
    /// Create a new OpenAI client
    pub fn new(config: Option<OpenAIConfig>) -> Result<Self> {
        let config = config.unwrap_or_default();

        // Create OpenAI config
        let mut openai_config = async_openai::config::OpenAIConfig::new();

        // Set API key if provided
        if let Some(api_key) = &config.api_key {
            openai_config = openai_config.with_api_key(api_key);
        }

        // Set org ID if provided
        if let Some(org_id) = &config.org_id {
            openai_config = openai_config.with_org_id(org_id);
        }

        // Create client with config
        let client = Client::with_config(openai_config);

        Ok(Self { client, config })
    }

    /// Execute a prompt against the OpenAI API
    pub async fn execute_prompt(&self, prompt: &str) -> Result<String> {
        debug!("Executing prompt against OpenAI: {}", prompt);

        // Criar vetor de mensagens usando a API do async-openai
        let message = ChatCompletionRequestMessage::User(
            async_openai::types::ChatCompletionRequestUserMessage {
                content: async_openai::types::ChatCompletionRequestUserMessageContent::Text(
                    prompt.to_string(),
                ),
                name: None,
            },
        );

        let messages = vec![message];

        let mut request = CreateChatCompletionRequestArgs::default()
            .model(&self.config.model)
            .messages(messages)
            .build()?;

        if let Some(max_tokens) = self.config.max_tokens {
            #[allow(deprecated)]
            {
                request.max_tokens = Some(max_tokens as u32);
            }
        }

        // Adicionar temperature se definido
        if let Some(temp) = self.config.temperature {
            request.temperature = Some(temp);
        }

        let response = self.client.chat().create(request).await?;

        if let Some(choice) = response.choices.first() {
            if let Some(content) = &choice.message.content {
                return Ok(content.clone());
            }
        }

        Err(anyhow::anyhow!("No response from OpenAI"))
    }

    /// Execute a chat conversation against the OpenAI API
    pub async fn execute_chat(
        &self,
        messages: Vec<ChatCompletionRequestMessage>,
    ) -> Result<String> {
        debug!(
            "Executing chat against OpenAI with {} messages",
            messages.len()
        );

        let mut request = CreateChatCompletionRequestArgs::default()
            .model(&self.config.model)
            .messages(messages)
            .build()?;

        if let Some(max_tokens) = self.config.max_tokens {
            #[allow(deprecated)]
            {
                request.max_tokens = Some(max_tokens as u32);
            }
        }

        if let Some(temp) = self.config.temperature {
            request.temperature = Some(temp);
        }

        let response = self.client.chat().create(request).await?;

        if let Some(choice) = response.choices.first() {
            if let Some(content) = &choice.message.content {
                return Ok(content.clone());
            }
        }

        Err(anyhow::anyhow!("No response from OpenAI"))
    }
}

/// Integration of OpenAI with sequential thinking
pub struct OpenAIThinking {
    client: OpenAIClient,
    thinking: SequentialThinking,
    system_prompt: String,
}

impl OpenAIThinking {
    /// Create a new OpenAI thinking integration
    pub fn new(client: OpenAIClient, system_prompt: Option<String>) -> Self {
        let default_system_prompt = r#"You are an AI assistant that thinks step by step to solve complex problems.
For each step in your thinking process, articulate your thoughts clearly and concisely.
If you realize a mistake in your previous thinking, you can revise it.
Your goal is to reach the best possible solution through careful sequential thinking."#;

        let system_prompt = system_prompt.unwrap_or_else(|| default_system_prompt.to_string());

        Self {
            client,
            thinking: SequentialThinking::new(),
            system_prompt,
        }
    }

    /// Process a query with sequential thinking
    #[allow(deprecated)]
    pub async fn process_query(&mut self, query: &str, total_steps: usize) -> Result<String> {
        // Initialize the conversation with system and user prompts
        let mut messages = vec![
            ChatCompletionRequestMessage::System(
                async_openai::types::ChatCompletionRequestSystemMessage {
                    content: async_openai::types::ChatCompletionRequestSystemMessageContent::Text(
                        self.system_prompt.clone()
                    ),
                    name: None,
                }
            ),
            ChatCompletionRequestMessage::User(
                async_openai::types::ChatCompletionRequestUserMessage {
                    content: async_openai::types::ChatCompletionRequestUserMessageContent::Text(
                        format!("Question: {}\n\nThink step-by-step to solve this problem. Start your first step with 'Step 1:'", query)
                    ),
                    name: None,
                    // Remove role property as it's not in the struct anymore
                }
            )
        ];

        // First thought
        let response = self.client.execute_chat(messages.clone()).await?;

        // Add first thought to the thinking process
        let thought = Thought {
            content: response.clone(),
            thought_number: 1,
            total_thoughts: total_steps,
            next_thought_needed: total_steps > 1,
            is_revision: false,
            revises_thought: None,
            branch_from_thought: None,
            branch_id: None,
            needs_more_thoughts: false,
        };

        self.thinking.add_thought(thought)?;

        // Add assistant's first thought to the conversation
        messages.push(ChatCompletionRequestMessage::Assistant(
            async_openai::types::ChatCompletionRequestAssistantMessage {
                content: Some(
                    async_openai::types::ChatCompletionRequestAssistantMessageContent::Text(
                        response,
                    ),
                ),
                name: None,
                tool_calls: None,
                audio: None,
                #[allow(deprecated)]
                function_call: None,
                refusal: None,
            },
        ));

        // Process remaining thoughts
        for step in 2..=total_steps {
            // Add user prompt for next step
            messages.push(ChatCompletionRequestMessage::User(
                async_openai::types::ChatCompletionRequestUserMessage {
                    content: async_openai::types::ChatCompletionRequestUserMessageContent::Text(
                        format!("Continue your thinking process. What is step {}?", step),
                    ),
                    name: None,
                    // Remove role property as it's not in the struct anymore
                },
            ));

            // Get response for this step
            let response = self.client.execute_chat(messages.clone()).await?;

            // Add thought to the thinking process
            let thought = Thought {
                content: response.clone(),
                thought_number: step,
                total_thoughts: total_steps,
                next_thought_needed: step < total_steps,
                is_revision: false,
                revises_thought: None,
                branch_from_thought: None,
                branch_id: None,
                needs_more_thoughts: false,
            };

            self.thinking.add_thought(thought)?;

            // Add assistant's thought to the conversation
            messages.push(ChatCompletionRequestMessage::Assistant(
                async_openai::types::ChatCompletionRequestAssistantMessage {
                    content: Some(
                        async_openai::types::ChatCompletionRequestAssistantMessageContent::Text(
                            response,
                        ),
                    ),
                    name: None,
                    tool_calls: None,
                    audio: None,
                    #[allow(deprecated)]
                    function_call: None,
                    refusal: None,
                },
            ));
        }

        // Finally, ask for a conclusion
        messages.push(ChatCompletionRequestMessage::User(
            async_openai::types::ChatCompletionRequestUserMessage {
                content: async_openai::types::ChatCompletionRequestUserMessageContent::Text(
                    "Based on your step-by-step thinking, what is your final answer or conclusion?"
                        .to_string(),
                ),
                name: None,
                // Remove role property as it's not in the struct anymore
            },
        ));

        let conclusion = self.client.execute_chat(messages).await?;

        // Get a summary of all the thinking
        let thinking_summary = self.thinking.get_summary();

        // Return both the thinking process and the conclusion
        Ok(format!(
            "# Thinking Process\n\n{}\n\n# Conclusion\n\n{}",
            thinking_summary, conclusion
        ))
    }

    /// Process a query with an option to revise previous thinking
    pub async fn process_query_with_revisions(
        &mut self,
        query: &str,
        initial_steps: usize,
        max_revisions: usize,
    ) -> Result<String> {
        // Basic thinking process
        let mut result = self.process_query(query, initial_steps).await?;

        // Ask if any revisions are needed
        let revision_prompt = format!(
            "You have completed your initial thinking process:\n\n{}\n\nDo you want to revise any of your previous thoughts? If yes, specify which thought(s) needs revision and why. If no, respond with 'No revisions needed.'",
            self.thinking.get_summary()
        );

        let revision_response = self.client.execute_prompt(&revision_prompt).await?;

        // Check if revisions are needed
        if !revision_response.contains("No revisions needed") && max_revisions > 0 {
            // Add the revision request to result
            result.push_str("\n\n# Revision Request\n\n");
            result.push_str(&revision_response);

            // Extract which thought to revise (this is a simplistic approach)
            // A more robust implementation would parse the response more carefully
            let thought_number = extract_thought_number(&revision_response).unwrap_or(1);

            // Create a revision prompt
            let revision_content_prompt = format!(
                "Please provide a revised version of thought #{}. Make sure to address the issues you identified.",
                thought_number
            );

            let revised_thought = self.client.execute_prompt(&revision_content_prompt).await?;

            // Add the revised thought
            let thought = Thought {
                content: revised_thought.clone(),
                thought_number: self.thinking.get_thoughts().len() + 1,
                total_thoughts: initial_steps,
                next_thought_needed: false,
                is_revision: true,
                revises_thought: Some(thought_number),
                branch_from_thought: None,
                branch_id: None,
                needs_more_thoughts: false,
            };

            self.thinking.add_thought(thought)?;

            // Add the revision to result
            result.push_str("\n\n# Revised Thought\n\n");
            result.push_str(&revised_thought);

            // Process further revisions recursively if needed
            if max_revisions > 1 {
                // This would be implemented to allow for multiple rounds of revisions
                // For simplicity, we'll just note that additional revisions would go here
                result.push_str("\n\n(Additional revisions would be processed here if needed)");
            }
        } else {
            // No revisions needed
            result.push_str("\n\n# Revision Check\n\n");
            result.push_str(&revision_response);
        }

        Ok(result)
    }
}

/// Utility function to extract thought number from a revision request
fn extract_thought_number(text: &str) -> Option<usize> {
    // Look for patterns like "Thought #2", "Step 2", etc.
    for pattern in ["thought #", "thought number ", "step ", "thought "] {
        if let Some(idx) = text.to_lowercase().find(pattern) {
            let start = idx + pattern.len();
            let end = start + 2.min(text.len() - start);
            let number_text = &text[start..end];
            if let Ok(num) = number_text.trim().parse::<usize>() {
                return Some(num);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[tokio::test]
    async fn test_openai_client() {
        // Skip if no API key is set
        if env::var("OPENAI_API_KEY").is_err() {
            println!("Skipping OpenAI test: No API key");
            return;
        }

        let client = OpenAIClient::new(None).unwrap();
        let response = client.execute_prompt("Say hello").await.unwrap();

        assert!(!response.is_empty());
        assert!(response.len() > 5);
    }
}
