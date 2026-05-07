# Requirements Document

## Introduction

This document specifies requirements for improving the Fuse AI model management platform's directory management, CLI design, and overall architecture. After analyzing the codebase, several gaps have been identified:

1. **Directory Confusion**: Two `.fuse` directories (global and project-level) cause confusion
2. **CLI Verbosity**: Commands are too long and don't follow modern CLI best practices
3. **Missing Essential Features**: No context analysis command, no info/status command, no simplified aliases
4. **Inconsistent Command Structure**: Mix of verbose and short commands without clear patterns
5. **Missing Production-Grade Features**: No shell completions, no command aliases, limited help system

This spec addresses these gaps by:
- Renaming global directory to `~/.fuse_cli` for clarity
- Adding a `fuse read` command for project context analysis (similar to Claude's init)
- Restructuring CLI commands to be more concise and intuitive
- Adding essential production-grade CLI features

## Glossary

- **Global Fuse Directory**: The `.fuse_cli` directory located in the user's home directory (`~/.fuse_cli`) that stores user-level configuration, downloaded models, cache, and global settings
- **Project Fuse Directory**: The `.fuse` directory located in a project repository (`./.fuse`) that stores project-specific data such as specs, workflows, and reports
- **Fuse System**: The AI model management platform that manages both directory types
- **Config Path**: The file path to the configuration file (config.toml or config.yaml)
- **Init Command**: The `fuse init` command that initializes configuration in a repository

## Requirements

### Requirement 1

**User Story:** As a developer, I want clear separation between global and project-specific Fuse directories, so that I understand where different types of data are stored

#### Acceptance Criteria

1. THE Fuse System SHALL store global configuration, models, and cache in the Global Fuse Directory at `~/.fuse_cli`
2. THE Fuse System SHALL store project-specific specs, workflows, and reports in the Project Fuse Directory at `./.fuse`
3. THE Fuse System SHALL create the Global Fuse Directory automatically on first run or when running any command that requires configuration
4. THE Fuse System SHALL create the Project Fuse Directory only when the Init Command is executed within a project repository
5. THE Fuse System SHALL NOT create duplicate configuration files in both directories

### Requirement 2

**User Story:** As a developer, I want the `fuse init` command to only manage project-specific configuration, so that it doesn't interfere with my global settings

#### Acceptance Criteria

1. WHEN the Init Command is executed, THE Fuse System SHALL create or update the Project Fuse Directory at `./.fuse`
2. WHEN the Init Command is executed, THE Fuse System SHALL create a project-specific config file at `./.fuse/config.toml` if the user opts for project-level configuration
3. WHEN the Init Command is executed, THE Fuse System SHALL provide an option to use global configuration instead of creating a project-specific config
4. THE Fuse System SHALL load project-specific configuration with higher priority than global configuration when both exist
5. WHEN no project-specific config exists, THE Fuse System SHALL fall back to the global configuration in the Global Fuse Directory

### Requirement 3

**User Story:** As a developer, I want clear documentation about which directory stores what data, so that I can manage my Fuse installation effectively

#### Acceptance Criteria

1. THE Fuse System SHALL display the Config Path being used when running `fuse config` command
2. WHEN the Init Command completes, THE Fuse System SHALL display which directories were created and their purposes
3. THE Fuse System SHALL provide a `fuse info` or `fuse status` command that displays both Global Fuse Directory and Project Fuse Directory paths
4. THE Fuse System SHALL include help text in the Init Command that explains the difference between global and project-specific configuration
5. THE Fuse System SHALL update the `.gitignore` file to exclude appropriate Project Fuse Directory contents when initializing in a git repository

### Requirement 4

**User Story:** As a developer, I want consistent directory structure across global and project directories, so that I can easily navigate and understand the organization

#### Acceptance Criteria

1. THE Fuse System SHALL organize the Global Fuse Directory with subdirectories: `models/`, `cache/`, `config.toml`, and example config files
2. THE Fuse System SHALL organize the Project Fuse Directory with subdirectories: `specs/`, `report/`, and optionally `config.toml`
3. THE Fuse System SHALL create subdirectories within `report/` for each feature: `compatibility/`, `scan/`, `inspect/`, `validation/`
4. THE Fuse System SHALL document the directory structure in README or help documentation
5. THE Fuse System SHALL create directories on-demand when features are first used rather than creating all directories upfront

### Requirement 5

**User Story:** As a developer, I want to migrate between global and project-specific configuration easily, so that I can reorganize my setup as my needs change

#### Acceptance Criteria

1. THE Fuse System SHALL provide a command to copy global configuration to project-specific configuration
2. THE Fuse System SHALL provide a command to remove project-specific configuration and fall back to global configuration
3. WHEN copying configuration, THE Fuse System SHALL preserve all settings including feature flags and custom paths
4. THE Fuse System SHALL validate configuration after migration to ensure correctness
5. THE Fuse System SHALL provide clear feedback about which configuration is active after migration operations

### Requirement 6

**User Story:** As a developer, I want a `fuse read` command that analyzes my project documentation and codebase to build context, so that I can quickly understand and work with my project similar to how Claude's init command works

#### Acceptance Criteria

1. WHEN the `fuse read` command is executed in a directory, THE Fuse System SHALL scan for documentation files including README, CONTRIBUTING, ARCHITECTURE, and other markdown files
2. WHEN the `fuse read` command is executed, THE Fuse System SHALL analyze the git repository structure if present, including commit history and branch information
3. THE Fuse System SHALL extract and summarize key information such as project purpose, architecture, dependencies, and setup instructions
4. THE Fuse System SHALL store the analyzed context in the Project Fuse Directory at `./.fuse/context.json` for future reference
5. WHEN the `fuse read` command completes, THE Fuse System SHALL display a summary of the project including technology stack, main components, and entry points

### Requirement 7

**User Story:** As a developer, I want concise and intuitive CLI commands following modern CLI design principles, so that I can work efficiently without typing verbose commands

#### Acceptance Criteria

1. THE Fuse System SHALL provide short command aliases for frequently used operations (e.g., `ls` for `list`, `rm` for `remove`)
2. THE Fuse System SHALL group related commands under logical subcommands (e.g., `model pull`, `model list` instead of top-level commands)
3. THE Fuse System SHALL use consistent flag naming across all commands (e.g., `-o` for output, `-f` for format, `-v` for verbose)
4. THE Fuse System SHALL provide both short (`-v`) and long (`--verbose`) flag options for all flags
5. THE Fuse System SHALL limit command depth to maximum 2 levels (e.g., `fuse model pull` not `fuse model source pull`)

### Requirement 8

**User Story:** As a developer, I want production-grade CLI features, so that I can integrate Fuse seamlessly into my development workflow

#### Acceptance Criteria

1. THE Fuse System SHALL provide shell completion scripts for bash, zsh, fish, and PowerShell
2. THE Fuse System SHALL provide a `fuse info` or `fuse status` command that displays system status, active configuration, and directory paths
3. THE Fuse System SHALL support environment variable configuration for common settings (e.g., `FUSE_CONFIG`, `FUSE_LOG_LEVEL`)
4. THE Fuse System SHALL provide a `--help` flag that displays contextual help with examples for each command
5. THE Fuse System SHALL support command output in multiple formats (text, JSON, YAML) via a global `--format` flag

### Requirement 9

**User Story:** As a developer, I want simplified command structure for common operations, so that I can perform tasks with minimal typing

#### Acceptance Criteria

1. THE Fuse System SHALL provide a `fuse get <model>` command as an alias for `fuse pull <model>`
2. THE Fuse System SHALL provide a `fuse ls` command as an alias for `fuse list`
3. THE Fuse System SHALL allow omitting subcommands for common operations (e.g., `fuse <model>` to run a model)
4. THE Fuse System SHALL provide a `fuse version` command that displays version information and build details
5. THE Fuse System SHALL support command chaining via `--and` flag for sequential operations

### Requirement 10

**User Story:** As a developer, I want better error messages and help system, so that I can quickly resolve issues and learn command usage

#### Acceptance Criteria

1. WHEN a command fails, THE Fuse System SHALL display the error message with suggested fixes and relevant documentation links
2. WHEN an invalid command is entered, THE Fuse System SHALL suggest similar valid commands using fuzzy matching
3. THE Fuse System SHALL provide a `fuse help <command>` that displays detailed help with usage examples
4. THE Fuse System SHALL display command examples in help text for all commands
5. WHEN a required argument is missing, THE Fuse System SHALL display the expected argument format and examples
