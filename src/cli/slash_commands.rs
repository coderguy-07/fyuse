//! Slash command framework [10.7]
//!
//! Extensible command system for interactive CLI/TUI with:
//! - Command registry with discovery
//! - Argument parsing
//! - Tab completion support
//! - Plugin-provided commands

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A parsed slash command with arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCommand {
    pub name: String,
    pub args: Vec<String>,
    pub raw: String,
}

impl ParsedCommand {
    /// Parse a slash command string (e.g., "/model llama3:7b").
    pub fn parse(input: &str) -> Option<Self> {
        let trimmed = input.trim();
        if !trimmed.starts_with('/') {
            return None;
        }

        let without_slash = &trimmed[1..];
        let parts: Vec<&str> = without_slash.split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }

        Some(Self {
            name: parts[0].to_lowercase(),
            args: parts[1..].iter().map(|s| s.to_string()).collect(),
            raw: trimmed.to_string(),
        })
    }

    /// Get the first argument, if any.
    pub fn first_arg(&self) -> Option<&str> {
        self.args.first().map(|s| s.as_str())
    }

    /// Get all arguments joined as a single string.
    pub fn args_string(&self) -> String {
        self.args.join(" ")
    }
}

/// Definition of a slash command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandDef {
    /// Command name (without leading /).
    pub name: String,
    /// Short description shown in help.
    pub description: String,
    /// Usage pattern (e.g., "/model <name>").
    pub usage: String,
    /// Category for grouping in help.
    pub category: CommandCategory,
    /// Whether this command is built-in or from a plugin.
    pub source: CommandSource,
    /// Aliases (e.g., "/q" for "/quit").
    pub aliases: Vec<String>,
}

/// Command category for help grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandCategory {
    Session,
    Model,
    Display,
    Navigation,
    System,
    Plugin,
}

impl CommandCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Session => "Session",
            Self::Model => "Model",
            Self::Display => "Display",
            Self::Navigation => "Navigation",
            Self::System => "System",
            Self::Plugin => "Plugin",
        }
    }
}

/// Where the command comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandSource {
    BuiltIn,
    Plugin { plugin_name: String },
}

/// Result of executing a command.
#[derive(Debug, Clone)]
pub enum CommandResult {
    /// Command executed successfully with optional message.
    Ok(Option<String>),
    /// Command wants to quit the application.
    Quit,
    /// Command not found.
    NotFound(String),
    /// Command failed with error.
    Error(String),
}

/// Registry of available slash commands.
pub struct CommandRegistry {
    commands: HashMap<String, CommandDef>,
    aliases: HashMap<String, String>,
}

impl CommandRegistry {
    /// Create a new registry with built-in commands.
    pub fn new() -> Self {
        let mut registry = Self {
            commands: HashMap::new(),
            aliases: HashMap::new(),
        };
        registry.register_builtins();
        registry
    }

    /// Register built-in commands.
    fn register_builtins(&mut self) {
        let builtins = vec![
            CommandDef {
                name: "quit".into(),
                description: "Exit Fuse".into(),
                usage: "/quit".into(),
                category: CommandCategory::Session,
                source: CommandSource::BuiltIn,
                aliases: vec!["q".into(), "exit".into()],
            },
            CommandDef {
                name: "clear".into(),
                description: "Clear chat history".into(),
                usage: "/clear".into(),
                category: CommandCategory::Session,
                source: CommandSource::BuiltIn,
                aliases: vec![],
            },
            CommandDef {
                name: "model".into(),
                description: "Switch or show current model".into(),
                usage: "/model [name]".into(),
                category: CommandCategory::Model,
                source: CommandSource::BuiltIn,
                aliases: vec![],
            },
            CommandDef {
                name: "help".into(),
                description: "Show available commands".into(),
                usage: "/help [command]".into(),
                category: CommandCategory::System,
                source: CommandSource::BuiltIn,
                aliases: vec!["?".into()],
            },
            CommandDef {
                name: "theme".into(),
                description: "Toggle dark/light theme".into(),
                usage: "/theme".into(),
                category: CommandCategory::Display,
                source: CommandSource::BuiltIn,
                aliases: vec![],
            },
            CommandDef {
                name: "sidebar".into(),
                description: "Toggle sidebar visibility".into(),
                usage: "/sidebar".into(),
                category: CommandCategory::Display,
                source: CommandSource::BuiltIn,
                aliases: vec![],
            },
            CommandDef {
                name: "export".into(),
                description: "Export conversation to file".into(),
                usage: "/export [filename]".into(),
                category: CommandCategory::Session,
                source: CommandSource::BuiltIn,
                aliases: vec![],
            },
            CommandDef {
                name: "system".into(),
                description: "Set system prompt".into(),
                usage: "/system <prompt>".into(),
                category: CommandCategory::Model,
                source: CommandSource::BuiltIn,
                aliases: vec![],
            },
            CommandDef {
                name: "tokens".into(),
                description: "Toggle token count display".into(),
                usage: "/tokens".into(),
                category: CommandCategory::Display,
                source: CommandSource::BuiltIn,
                aliases: vec![],
            },
            CommandDef {
                name: "timestamps".into(),
                description: "Toggle timestamp display".into(),
                usage: "/timestamps".into(),
                category: CommandCategory::Display,
                source: CommandSource::BuiltIn,
                aliases: vec![],
            },
            CommandDef {
                name: "doctor".into(),
                description: "Run system diagnostics".into(),
                usage: "/doctor".into(),
                category: CommandCategory::System,
                source: CommandSource::BuiltIn,
                aliases: vec![],
            },
            CommandDef {
                name: "status".into(),
                description: "Show session status".into(),
                usage: "/status".into(),
                category: CommandCategory::Session,
                source: CommandSource::BuiltIn,
                aliases: vec![],
            },
        ];

        for cmd in builtins {
            self.register(cmd);
        }
    }

    /// Register a command.
    pub fn register(&mut self, def: CommandDef) {
        for alias in &def.aliases {
            self.aliases.insert(alias.clone(), def.name.clone());
        }
        self.commands.insert(def.name.clone(), def);
    }

    /// Unregister a command by name.
    pub fn unregister(&mut self, name: &str) -> bool {
        if let Some(def) = self.commands.remove(name) {
            for alias in &def.aliases {
                self.aliases.remove(alias);
            }
            true
        } else {
            false
        }
    }

    /// Look up a command by name or alias.
    pub fn get(&self, name: &str) -> Option<&CommandDef> {
        let canonical = self.aliases.get(name).map(|s| s.as_str()).unwrap_or(name);
        self.commands.get(canonical)
    }

    /// Resolve a name/alias to canonical name.
    pub fn resolve_name<'a>(&'a self, name: &str) -> Option<&'a str> {
        if let Some((key, _)) = self.commands.get_key_value(name) {
            Some(key.as_str())
        } else {
            self.aliases.get(name).map(|s| s.as_str())
        }
    }

    /// List all commands.
    pub fn list(&self) -> Vec<&CommandDef> {
        let mut cmds: Vec<&CommandDef> = self.commands.values().collect();
        cmds.sort_by_key(|c| &c.name);
        cmds
    }

    /// List commands by category.
    pub fn list_by_category(&self, category: CommandCategory) -> Vec<&CommandDef> {
        self.commands
            .values()
            .filter(|c| c.category == category)
            .collect()
    }

    /// Tab completion: find commands matching a prefix.
    pub fn complete(&self, prefix: &str) -> Vec<&str> {
        let prefix_lower = prefix.to_lowercase();
        let mut matches: Vec<&str> = self
            .commands
            .keys()
            .filter(|name| name.starts_with(&prefix_lower))
            .map(|s| s.as_str())
            .collect();

        // Also check aliases
        for alias in self.aliases.keys() {
            if alias.starts_with(&prefix_lower) && !matches.contains(&alias.as_str()) {
                matches.push(alias.as_str());
            }
        }

        matches.sort();
        matches
    }

    /// Fuzzy search: find commands matching a query anywhere in name or description.
    pub fn search(&self, query: &str) -> Vec<&CommandDef> {
        let query_lower = query.to_lowercase();
        self.commands
            .values()
            .filter(|cmd| {
                cmd.name.contains(&query_lower)
                    || cmd.description.to_lowercase().contains(&query_lower)
            })
            .collect()
    }

    /// Total number of registered commands.
    pub fn count(&self) -> usize {
        self.commands.len()
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let cmd = ParsedCommand::parse("/quit").unwrap();
        assert_eq!(cmd.name, "quit");
        assert!(cmd.args.is_empty());
    }

    #[test]
    fn test_parse_with_args() {
        let cmd = ParsedCommand::parse("/model llama3:7b").unwrap();
        assert_eq!(cmd.name, "model");
        assert_eq!(cmd.args, vec!["llama3:7b"]);
    }

    #[test]
    fn test_parse_multiple_args() {
        let cmd = ParsedCommand::parse("/system You are a helpful assistant").unwrap();
        assert_eq!(cmd.name, "system");
        assert_eq!(cmd.args.len(), 5);
        assert_eq!(cmd.args_string(), "You are a helpful assistant");
    }

    #[test]
    fn test_parse_no_slash() {
        assert!(ParsedCommand::parse("hello").is_none());
    }

    #[test]
    fn test_parse_empty_slash() {
        assert!(ParsedCommand::parse("/").is_none());
    }

    #[test]
    fn test_parse_with_whitespace() {
        let cmd = ParsedCommand::parse("  /quit  ").unwrap();
        assert_eq!(cmd.name, "quit");
    }

    #[test]
    fn test_parse_case_insensitive() {
        let cmd = ParsedCommand::parse("/QUIT").unwrap();
        assert_eq!(cmd.name, "quit");
    }

    #[test]
    fn test_first_arg() {
        let cmd = ParsedCommand::parse("/model llama3").unwrap();
        assert_eq!(cmd.first_arg(), Some("llama3"));

        let cmd = ParsedCommand::parse("/quit").unwrap();
        assert_eq!(cmd.first_arg(), None);
    }

    #[test]
    fn test_registry_has_builtins() {
        let reg = CommandRegistry::new();
        assert!(reg.count() > 0);
        assert!(reg.get("quit").is_some());
        assert!(reg.get("clear").is_some());
        assert!(reg.get("model").is_some());
        assert!(reg.get("help").is_some());
    }

    #[test]
    fn test_registry_alias_resolution() {
        let reg = CommandRegistry::new();
        // "q" and "exit" are aliases for "quit"
        assert_eq!(reg.resolve_name("q"), Some("quit"));
        assert_eq!(reg.resolve_name("exit"), Some("quit"));
        assert_eq!(reg.resolve_name("?"), Some("help"));
    }

    #[test]
    fn test_registry_get_by_alias() {
        let reg = CommandRegistry::new();
        let cmd = reg.get("q").unwrap();
        assert_eq!(cmd.name, "quit");
    }

    #[test]
    fn test_registry_register_custom() {
        let mut reg = CommandRegistry::new();
        let before = reg.count();
        reg.register(CommandDef {
            name: "custom".into(),
            description: "A custom command".into(),
            usage: "/custom".into(),
            category: CommandCategory::Plugin,
            source: CommandSource::Plugin {
                plugin_name: "my-plugin".into(),
            },
            aliases: vec!["c".into()],
        });
        assert_eq!(reg.count(), before + 1);
        assert!(reg.get("custom").is_some());
        assert!(reg.get("c").is_some());
    }

    #[test]
    fn test_registry_unregister() {
        let mut reg = CommandRegistry::new();
        assert!(reg.unregister("quit"));
        assert!(reg.get("quit").is_none());
        assert!(reg.get("q").is_none()); // Alias also removed
        assert!(!reg.unregister("nonexistent"));
    }

    #[test]
    fn test_tab_completion() {
        let reg = CommandRegistry::new();
        let matches = reg.complete("qu");
        assert!(matches.contains(&"quit"));
    }

    #[test]
    fn test_tab_completion_empty() {
        let reg = CommandRegistry::new();
        let matches = reg.complete("");
        assert!(matches.len() >= 10); // All commands
    }

    #[test]
    fn test_tab_completion_no_match() {
        let reg = CommandRegistry::new();
        let matches = reg.complete("zzz");
        assert!(matches.is_empty());
    }

    #[test]
    fn test_fuzzy_search() {
        let reg = CommandRegistry::new();
        let results = reg.search("chat");
        assert!(results.iter().any(|c| c.name == "clear")); // "Clear chat history"
    }

    #[test]
    fn test_list_by_category() {
        let reg = CommandRegistry::new();
        let session_cmds = reg.list_by_category(CommandCategory::Session);
        assert!(!session_cmds.is_empty());
        assert!(session_cmds
            .iter()
            .all(|c| c.category == CommandCategory::Session));
    }

    #[test]
    fn test_list_sorted() {
        let reg = CommandRegistry::new();
        let list = reg.list();
        for window in list.windows(2) {
            assert!(window[0].name <= window[1].name);
        }
    }

    #[test]
    fn test_command_def_serde() {
        let def = CommandDef {
            name: "test".into(),
            description: "Test command".into(),
            usage: "/test".into(),
            category: CommandCategory::System,
            source: CommandSource::BuiltIn,
            aliases: vec!["t".into()],
        };
        let json = serde_json::to_string(&def).unwrap();
        let back: CommandDef = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "test");
        assert_eq!(back.aliases, vec!["t"]);
    }

    #[test]
    fn test_command_result_variants() {
        let ok = CommandResult::Ok(Some("done".into()));
        assert!(matches!(ok, CommandResult::Ok(Some(_))));

        let quit = CommandResult::Quit;
        assert!(matches!(quit, CommandResult::Quit));

        let not_found = CommandResult::NotFound("foo".into());
        assert!(matches!(not_found, CommandResult::NotFound(_)));
    }

    #[test]
    fn test_category_label() {
        assert_eq!(CommandCategory::Session.label(), "Session");
        assert_eq!(CommandCategory::Plugin.label(), "Plugin");
    }
}
