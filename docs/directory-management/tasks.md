# Implementation Plan

- [-] 1. Implement DirectoryManager for path resolution
  - Create DirectoryManager struct with global and project directory paths
  - Implement directory detection and creation logic
  - Add configuration priority resolution (project > global > env > defaults)
  - Implement path resolution methods for models, cache, logs, and reports
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 4.1, 4.2, 4.5_

- [ ] 2. Implement directory migration from ~/.fuse to ~/.fuse_cli
- [ ] 2.1 Create migration detection logic
  - Check if old ~/.fuse directory exists
  - Check if new ~/.fuse_cli directory already exists
  - Determine if migration is needed
  - _Requirements: 1.1, 5.1_

- [ ] 2.2 Implement migration prompt and execution
  - Display migration prompt with clear explanation
  - Implement directory rename operation
  - Update configuration file paths if needed
  - Create migration log for troubleshooting
  - _Requirements: 1.1, 5.3, 5.4, 5.5_

- [ ] 2.3 Add migration command for manual migration
  - Implement `fuse migrate` command
  - Add --from and --to flags
  - Add --dry-run flag to preview changes
  - Display migration summary
  - _Requirements: 5.1, 5.2, 5.5_

- [ ] 3. Update configuration loading to use DirectoryManager
  - Modify FuseConfig::load_or_default() to use DirectoryManager
  - Implement configuration priority resolution
  - Add environment variable override support (FUSE_CONFIG, FUSE_LOG_LEVEL, etc.)
  - Update all hardcoded ~/.fuse references to use DirectoryManager
  - _Requirements: 1.5, 2.2, 2.4, 8.3_

- [ ] 4. Implement project-specific configuration support
- [ ] 4.1 Add project config detection in init command
  - Modify init command to ask about project vs global config
  - Create ./.fuse directory structure when project config is chosen
  - Add --project flag to force project-level configuration
  - _Requirements: 2.1, 2.2, 2.3_

- [ ] 4.2 Implement config migration commands
  - Add `fuse config copy-to-project` command
  - Add `fuse config use-global` command to remove project config
  - Validate configuration after migration
  - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5_

- [ ] 5. Implement ContextAnalyzer for project analysis
- [ ] 5.1 Create ContextAnalyzer struct and ProjectContext data model
  - Define ProjectContext struct with all fields
  - Define DocumentSummary and GitInfo structs
  - Implement serialization/deserialization
  - _Requirements: 6.4_

- [ ] 5.2 Implement documentation scanner
  - Scan for README, CONTRIBUTING, ARCHITECTURE, and other .md files
  - Parse markdown files and extract titles and sections
  - Generate summaries for each document
  - Store document metadata in ProjectContext
  - _Requirements: 6.1, 6.3_

- [ ] 5.3 Implement dependency analyzer
  - Detect package manifest files (Cargo.toml, package.json, pyproject.toml, go.mod, etc.)
  - Parse manifest files to extract dependencies
  - Identify tech stack from dependencies and file extensions
  - Store dependency information in ProjectContext
  - _Requirements: 6.3_

- [ ] 5.4 Implement git repository analyzer
  - Check if directory is a git repository
  - Extract remote URL, current branch, and commit count
  - Get contributor list from git log
  - Store git information in ProjectContext
  - _Requirements: 6.2_

- [ ] 5.5 Implement entry point detection
  - Identify main entry points (main.rs, index.js, __main__.py, etc.)
  - Detect build configuration files
  - Store entry point paths in ProjectContext
  - _Requirements: 6.3, 6.5_

- [ ] 5.6 Implement context storage and retrieval
  - Save ProjectContext to ./.fuse/context.json
  - Implement load_context() to read existing context
  - Add timestamp to track when context was last analyzed
  - _Requirements: 6.4_

- [ ] 6. Implement `fuse read` command
  - Create read command handler
  - Integrate ContextAnalyzer
  - Display analysis progress with spinner
  - Show formatted summary after analysis
  - Add --refresh flag to force re-analysis
  - Add --format flag for output format (text, json, yaml)
  - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5_

- [ ] 7. Implement `fuse status` command
- [ ] 7.1 Create SystemStatus struct and data collection
  - Collect version information
  - Get configuration path and directory paths
  - Count installed models
  - Calculate cache size
  - List active feature flags
  - _Requirements: 3.3, 8.2_

- [ ] 7.2 Implement status display formatting
  - Create formatted text output
  - Add JSON output format
  - Add YAML output format
  - Display with colors and icons
  - _Requirements: 3.3, 8.2, 8.5_

- [ ] 8. Restructure CLI commands with modern design
- [ ] 8.1 Create new command structure with subcommands
  - Group model operations under `model` subcommand
  - Group layer operations under `layer` subcommand
  - Group compatibility operations under `compat` subcommand
  - Maintain flat structure for common operations
  - _Requirements: 7.2, 7.5, 9.3_

- [ ] 8.2 Implement command alias system
  - Create AliasResolver to map short commands to full commands
  - Add aliases: get→model pull, ls→model list, rm→model rm, info→model info
  - Add comp→compat check alias
  - Resolve aliases before command execution
  - _Requirements: 7.1, 9.1, 9.2_

- [ ] 8.3 Update all command handlers to use new structure
  - Update existing handlers to work with new command structure
  - Ensure backward compatibility with deprecation warnings
  - Update help text for all commands
  - _Requirements: 7.2, 7.5_

- [ ] 9. Implement consistent flag naming across commands
  - Standardize -o/--output for output file paths
  - Standardize -f/--format for output formats
  - Standardize -v/--verbose for verbose output
  - Standardize -y/--yes for confirmation skipping
  - Add both short and long forms for all flags
  - _Requirements: 7.3, 7.4_

- [ ] 10. Implement shell completion generation
- [ ] 10.1 Add completion generation using clap_complete
  - Add clap_complete dependency
  - Implement completion generation for bash
  - Implement completion generation for zsh
  - Implement completion generation for fish
  - Implement completion generation for PowerShell
  - _Requirements: 8.1_

- [ ] 10.2 Create `fuse completion` command
  - Add completion subcommand with shell selection
  - Output completion script to stdout
  - Add --install flag to auto-install completions
  - Detect user's shell automatically
  - _Requirements: 8.1_

- [ ] 11. Enhance help system with examples
- [ ] 11.1 Add usage examples to all commands
  - Add examples to help text using clap's after_help
  - Include common use cases for each command
  - Show flag combinations in examples
  - _Requirements: 10.3, 10.4_

- [ ] 11.2 Implement fuzzy command suggestion
  - Add strsim or similar crate for string similarity
  - Suggest similar commands when invalid command is entered
  - Show top 3 most similar commands
  - _Requirements: 10.2_

- [ ] 11.3 Improve error messages with suggestions
  - Add contextual suggestions to error messages
  - Include documentation links in errors
  - Show example commands for common errors
  - _Requirements: 10.1, 10.5_

- [ ] 12. Add environment variable support
  - Document supported environment variables (FUSE_CONFIG, FUSE_LOG_LEVEL, FUSE_MODELS_DIR, etc.)
  - Implement environment variable parsing in FuseConfig
  - Apply environment overrides after loading config file
  - Add env var display in `fuse status` command
  - _Requirements: 8.3_

- [ ] 13. Implement global output format flag
  - Add --format flag to root CLI (global flag)
  - Support text, json, and yaml formats
  - Implement OutputFormatter trait for consistent formatting
  - Update all commands to respect global format flag
  - _Requirements: 8.5_

- [ ] 14. Add `fuse version` command
  - Display version number from Cargo.toml
  - Show build information (commit hash, build date)
  - Display Rust version used for build
  - Add --short flag for version number only
  - _Requirements: 9.4_

- [ ] 15. Update .gitignore handling in init command
  - Check if .gitignore exists in project
  - Add .fuse/ entries to .gitignore if not present
  - Exclude .fuse/context.json, .fuse/report/, .fuse/vibe/
  - Keep .fuse/config.toml tracked (optional, ask user)
  - _Requirements: 3.5_

- [ ] 16. Update documentation and examples
- [ ] 16.1 Update README with new command structure
  - Document new directory structure (~/.fuse_cli vs ./.fuse)
  - Update all command examples to use new structure
  - Add migration guide for existing users
  - Document command aliases
  - _Requirements: 3.4, 4.4_

- [ ] 16.2 Update CLI_USAGE_EXAMPLES.md
  - Update all examples to use new commands
  - Add examples for new commands (read, status, version)
  - Add examples for command aliases
  - Add shell completion examples
  - _Requirements: 3.4_

- [ ] 16.3 Update CONFIG_README.md
  - Update directory paths to ~/.fuse_cli
  - Document project-level configuration
  - Document configuration priority
  - Document environment variables
  - _Requirements: 3.1, 3.4_

- [ ] 17. Update existing code references
  - Search and replace all hardcoded ~/.fuse paths
  - Update all path construction to use DirectoryManager
  - Update logging paths to use DirectoryManager
  - Update report generation paths to use DirectoryManager
  - _Requirements: 1.1, 4.1, 4.2_

- [ ] 18. Add backward compatibility layer
  - Detect old command usage and show deprecation warnings
  - Support both old and new command structures temporarily
  - Add migration timeline to deprecation warnings
  - _Requirements: 7.2_

- [ ] 19. Write unit tests for new components
  - Test DirectoryManager path resolution
  - Test configuration priority resolution
  - Test ContextAnalyzer with sample projects
  - Test AliasResolver command mapping
  - Test migration logic
  - _Requirements: All_

- [ ] 20. Write integration tests
  - Test end-to-end directory migration
  - Test `fuse read` command with real project
  - Test configuration loading with different priorities
  - Test command aliases end-to-end
  - Test shell completion generation
  - _Requirements: All_

- [ ] 21. Final integration and testing
  - Test all commands with new directory structure
  - Verify migration works correctly
  - Test on different operating systems
  - Verify shell completions work
  - Test environment variable overrides
  - _Requirements: All_
