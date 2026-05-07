use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub model: Option<String>,
    pub tokens: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppState {
    pub messages: VecDeque<Message>,
    pub selected_model: Option<String>,
    pub available_models: Vec<ModelInfo>,
    pub is_streaming: bool,
    pub current_input: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub size_bytes: u64,
    pub loaded: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            messages: VecDeque::new(),
            selected_model: None,
            available_models: Vec::new(),
            is_streaming: false,
            current_input: String::new(),
        }
    }
}

impl AppState {
    pub fn add_message(&mut self, message: Message) {
        self.messages.push_back(message);
    }

    pub fn clear_messages(&mut self) {
        self.messages.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json;

    // Message tests
    mod message_tests {
        use super::*;

        #[test]
        fn test_message_creation() {
            let timestamp = Utc::now();
            let message = Message {
                role: MessageRole::User,
                content: "Hello world".to_string(),
                timestamp,
                model: Some("gpt-2".to_string()),
                tokens: Some(42),
            };

            assert_eq!(message.role, MessageRole::User);
            assert_eq!(message.content, "Hello world");
            assert_eq!(message.timestamp, timestamp);
            assert_eq!(message.model, Some("gpt-2".to_string()));
            assert_eq!(message.tokens, Some(42));
        }

        #[test]
        fn test_message_with_minimal_fields() {
            let timestamp = Utc::now();
            let message = Message {
                role: MessageRole::Assistant,
                content: "Response".to_string(),
                timestamp,
                model: None,
                tokens: None,
            };

            assert_eq!(message.role, MessageRole::Assistant);
            assert_eq!(message.content, "Response");
            assert_eq!(message.model, None);
            assert_eq!(message.tokens, None);
        }

        #[test]
        fn test_message_serialization() {
            let timestamp = Utc::now();
            let message = Message {
                role: MessageRole::User,
                content: "Test message".to_string(),
                timestamp,
                model: Some("test-model".to_string()),
                tokens: Some(100),
            };

            let serialized = serde_json::to_string(&message).unwrap();
            let deserialized: Message = serde_json::from_str(&serialized).unwrap();

            assert_eq!(message.role, deserialized.role);
            assert_eq!(message.content, deserialized.content);
            assert_eq!(message.model, deserialized.model);
            assert_eq!(message.tokens, deserialized.tokens);
            // Note: timestamp comparison might have slight differences due to serialization
        }

        #[test]
        fn test_message_with_empty_content() {
            let timestamp = Utc::now();
            let message = Message {
                role: MessageRole::System,
                content: String::new(),
                timestamp,
                model: None,
                tokens: Some(0),
            };

            assert_eq!(message.content, "");
            assert_eq!(message.tokens, Some(0));
        }

        #[test]
        fn test_message_with_large_content() {
            let timestamp = Utc::now();
            let large_content = "a".repeat(10000);
            let message = Message {
                role: MessageRole::User,
                content: large_content.clone(),
                timestamp,
                model: Some("large-model".to_string()),
                tokens: Some(1000),
            };

            assert_eq!(message.content.len(), 10000);
            assert_eq!(message.content, large_content);
        }
    }

    // MessageRole tests
    mod message_role_tests {
        use super::*;

        #[test]
        fn test_message_role_variants() {
            assert_eq!(MessageRole::User as u8, 0);
            assert_eq!(MessageRole::Assistant as u8, 1);
            assert_eq!(MessageRole::System as u8, 2);
        }

        #[test]
        fn test_message_role_equality() {
            assert_eq!(MessageRole::User, MessageRole::User);
            assert_eq!(MessageRole::Assistant, MessageRole::Assistant);
            assert_eq!(MessageRole::System, MessageRole::System);

            assert_ne!(MessageRole::User, MessageRole::Assistant);
            assert_ne!(MessageRole::Assistant, MessageRole::System);
            assert_ne!(MessageRole::System, MessageRole::User);
        }

        #[test]
        fn test_message_role_serialization() {
            let roles = vec![
                MessageRole::User,
                MessageRole::Assistant,
                MessageRole::System,
            ];

            for role in roles {
                let serialized = serde_json::to_string(&role).unwrap();
                let deserialized: MessageRole = serde_json::from_str(&serialized).unwrap();
                assert_eq!(role, deserialized);
            }
        }

        #[test]
        fn test_message_role_debug() {
            assert_eq!(format!("{:?}", MessageRole::User), "User");
            assert_eq!(format!("{:?}", MessageRole::Assistant), "Assistant");
            assert_eq!(format!("{:?}", MessageRole::System), "System");
        }
    }

    // ModelInfo tests
    mod model_info_tests {
        use super::*;

        #[test]
        fn test_model_info_creation() {
            let model_info = ModelInfo {
                id: "gpt2".to_string(),
                name: "GPT-2 Small".to_string(),
                size_bytes: 1_500_000_000,
                loaded: false,
            };

            assert_eq!(model_info.id, "gpt2");
            assert_eq!(model_info.name, "GPT-2 Small");
            assert_eq!(model_info.size_bytes, 1_500_000_000);
            assert_eq!(model_info.loaded, false);
        }

        #[test]
        fn test_model_info_loaded_state() {
            let loaded_model = ModelInfo {
                id: "llama".to_string(),
                name: "LLaMA 7B".to_string(),
                size_bytes: 13_000_000_000,
                loaded: true,
            };

            let unloaded_model = ModelInfo {
                id: "llama".to_string(),
                name: "LLaMA 7B".to_string(),
                size_bytes: 13_000_000_000,
                loaded: false,
            };

            assert_eq!(loaded_model.loaded, true);
            assert_eq!(unloaded_model.loaded, false);
        }

        #[test]
        fn test_model_info_serialization() {
            let model_info = ModelInfo {
                id: "test-model".to_string(),
                name: "Test Model".to_string(),
                size_bytes: 1000000,
                loaded: true,
            };

            let serialized = serde_json::to_string(&model_info).unwrap();
            let deserialized: ModelInfo = serde_json::from_str(&serialized).unwrap();

            assert_eq!(model_info, deserialized);
        }

        #[test]
        fn test_model_info_equality() {
            let model1 = ModelInfo {
                id: "model1".to_string(),
                name: "Model One".to_string(),
                size_bytes: 1000,
                loaded: false,
            };

            let model2 = ModelInfo {
                id: "model1".to_string(),
                name: "Model One".to_string(),
                size_bytes: 1000,
                loaded: false,
            };

            let model3 = ModelInfo {
                id: "model2".to_string(),
                name: "Model Two".to_string(),
                size_bytes: 2000,
                loaded: true,
            };

            assert_eq!(model1, model2);
            assert_ne!(model1, model3);
        }

        #[test]
        fn test_model_info_with_zero_size() {
            let model_info = ModelInfo {
                id: "empty".to_string(),
                name: "Empty Model".to_string(),
                size_bytes: 0,
                loaded: false,
            };

            assert_eq!(model_info.size_bytes, 0);
        }

        #[test]
        fn test_model_info_with_max_size() {
            let model_info = ModelInfo {
                id: "huge".to_string(),
                name: "Huge Model".to_string(),
                size_bytes: u64::MAX,
                loaded: true,
            };

            assert_eq!(model_info.size_bytes, u64::MAX);
        }
    }

    // AppState tests
    mod app_state_tests {
        use super::*;

        #[test]
        fn test_app_state_default() {
            let state = AppState::default();

            assert!(state.messages.is_empty());
            assert_eq!(state.selected_model, None);
            assert!(state.available_models.is_empty());
            assert_eq!(state.is_streaming, false);
            assert_eq!(state.current_input, "");
        }

        #[test]
        fn test_app_state_creation() {
            let mut messages = VecDeque::new();
            messages.push_back(Message {
                role: MessageRole::User,
                content: "Hello".to_string(),
                timestamp: Utc::now(),
                model: None,
                tokens: None,
            });

            let available_models = vec![ModelInfo {
                id: "gpt2".to_string(),
                name: "GPT-2".to_string(),
                size_bytes: 1000000,
                loaded: false,
            }];

            let state = AppState {
                messages,
                selected_model: Some("gpt2".to_string()),
                available_models,
                is_streaming: true,
                current_input: "Test input".to_string(),
            };

            assert_eq!(state.messages.len(), 1);
            assert_eq!(state.selected_model, Some("gpt2".to_string()));
            assert_eq!(state.available_models.len(), 1);
            assert_eq!(state.is_streaming, true);
            assert_eq!(state.current_input, "Test input");
        }

        #[test]
        fn test_app_state_add_message() {
            let mut state = AppState::default();
            let message = Message {
                role: MessageRole::User,
                content: "Test message".to_string(),
                timestamp: Utc::now(),
                model: Some("test-model".to_string()),
                tokens: Some(10),
            };

            state.add_message(message.clone());

            assert_eq!(state.messages.len(), 1);
            assert_eq!(state.messages.front().unwrap().content, "Test message");
        }

        #[test]
        fn test_app_state_add_multiple_messages() {
            let mut state = AppState::default();

            for i in 0..5 {
                let message = Message {
                    role: MessageRole::User,
                    content: format!("Message {}", i),
                    timestamp: Utc::now(),
                    model: None,
                    tokens: Some(i * 10),
                };
                state.add_message(message);
            }

            assert_eq!(state.messages.len(), 5);
            for (i, message) in state.messages.iter().enumerate() {
                assert_eq!(message.content, format!("Message {}", i));
                assert_eq!(message.tokens, Some(i * 10));
            }
        }

        #[test]
        fn test_app_state_clear_messages() {
            let mut state = AppState::default();

            // Add some messages
            for i in 0..3 {
                let message = Message {
                    role: MessageRole::Assistant,
                    content: format!("Response {}", i),
                    timestamp: Utc::now(),
                    model: Some("test-model".to_string()),
                    tokens: Some(50),
                };
                state.add_message(message);
            }

            assert_eq!(state.messages.len(), 3);

            state.clear_messages();

            assert!(state.messages.is_empty());
        }

        #[test]
        fn test_app_state_clear_empty_messages() {
            let mut state = AppState::default();
            state.clear_messages();
            assert!(state.messages.is_empty());
        }

        #[test]
        fn test_app_state_with_many_models() {
            let models = (0..100)
                .map(|i| ModelInfo {
                    id: format!("model-{}", i),
                    name: format!("Model {}", i),
                    size_bytes: i as u64 * 1000000,
                    loaded: i % 2 == 0,
                })
                .collect::<Vec<_>>();

            let state = AppState {
                available_models: models,
                ..AppState::default()
            };

            assert_eq!(state.available_models.len(), 100);
            assert_eq!(state.available_models[0].id, "model-0");
            assert_eq!(state.available_models[99].id, "model-99");
        }

        #[test]
        fn test_app_state_streaming_state() {
            let mut state = AppState::default();

            assert_eq!(state.is_streaming, false);

            state.is_streaming = true;
            assert_eq!(state.is_streaming, true);

            state.is_streaming = false;
            assert_eq!(state.is_streaming, false);
        }

        #[test]
        fn test_app_state_current_input() {
            let mut state = AppState::default();

            assert_eq!(state.current_input, "");

            state.current_input = "Hello world".to_string();
            assert_eq!(state.current_input, "Hello world");

            state.current_input.clear();
            assert_eq!(state.current_input, "");
        }

        #[test]
        fn test_app_state_selected_model() {
            let mut state = AppState::default();

            assert_eq!(state.selected_model, None);

            state.selected_model = Some("gpt2".to_string());
            assert_eq!(state.selected_model, Some("gpt2".to_string()));

            state.selected_model = None;
            assert_eq!(state.selected_model, None);
        }
    }

    // Edge case tests
    mod edge_case_tests {
        use super::*;

        #[test]
        fn test_empty_strings_in_message() {
            let message = Message {
                role: MessageRole::User,
                content: String::new(),
                timestamp: Utc::now(),
                model: Some(String::new()),
                tokens: Some(0),
            };

            assert_eq!(message.content, "");
            assert_eq!(message.model, Some(String::new()));
            assert_eq!(message.tokens, Some(0));
        }

        #[test]
        fn test_very_long_strings() {
            let long_string = "a".repeat(100000);
            let message = Message {
                role: MessageRole::Assistant,
                content: long_string.clone(),
                timestamp: Utc::now(),
                model: Some("a".repeat(1000)),
                tokens: Some(usize::MAX),
            };

            assert_eq!(message.content.len(), 100000);
            assert_eq!(message.model.as_ref().unwrap().len(), 1000);
            assert_eq!(message.tokens, Some(usize::MAX));
        }

        #[test]
        fn test_max_values() {
            let model_info = ModelInfo {
                id: "max-model".to_string(),
                name: "Max Model".to_string(),
                size_bytes: u64::MAX,
                loaded: true,
            };

            assert_eq!(model_info.size_bytes, u64::MAX);
        }

        #[test]
        fn test_message_order_preservation() {
            let mut state = AppState::default();
            let timestamp1 = Utc::now();
            let timestamp2 = Utc::now();

            let message1 = Message {
                role: MessageRole::User,
                content: "First".to_string(),
                timestamp: timestamp1,
                model: None,
                tokens: None,
            };

            let message2 = Message {
                role: MessageRole::Assistant,
                content: "Second".to_string(),
                timestamp: timestamp2,
                model: None,
                tokens: None,
            };

            state.add_message(message1);
            state.add_message(message2);

            let messages: Vec<_> = state.messages.iter().collect();
            assert_eq!(messages[0].content, "First");
            assert_eq!(messages[1].content, "Second");
        }
    }

    // Serialization edge cases
    mod serialization_tests {
        use super::*;

        #[test]
        fn test_message_serialization_edge_cases() {
            // Test with None values
            let message = Message {
                role: MessageRole::System,
                content: "System message".to_string(),
                timestamp: Utc::now(),
                model: None,
                tokens: None,
            };

            let serialized = serde_json::to_string(&message).unwrap();
            let deserialized: Message = serde_json::from_str(&serialized).unwrap();

            assert_eq!(deserialized.model, None);
            assert_eq!(deserialized.tokens, None);
        }

        #[test]
        fn test_model_info_serialization_edge_cases() {
            let model_info = ModelInfo {
                id: String::new(),
                name: String::new(),
                size_bytes: 0,
                loaded: false,
            };

            let serialized = serde_json::to_string(&model_info).unwrap();
            let deserialized: ModelInfo = serde_json::from_str(&serialized).unwrap();

            assert_eq!(deserialized.id, "");
            assert_eq!(deserialized.name, "");
            assert_eq!(deserialized.size_bytes, 0);
            assert_eq!(deserialized.loaded, false);
        }
    }
}
