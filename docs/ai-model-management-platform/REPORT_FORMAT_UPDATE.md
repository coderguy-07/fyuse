# Report Format Enhancement - Specification Update

## Overview

Updated the Fuse specification to support multiple output formats for all report-generating commands, with a standardized report directory structure.

## Changes Made

### 1. Requirements Document Updates

**Requirement 9: Model Compatibility Checking** - Enhanced with:
- Support for 4 output formats: ASCII table (default), JSON, HTML, and Markdown
- Format flags: `--json`, `--html`, `--md`
- Output file flag: `-o <filename>`
- Default report location: `.fuse/report/compatibility/<report>.<extension>`
- Automatic directory creation
- Format-specific features:
  - **ASCII Table**: Terminal output with comfy-table formatting
  - **JSON**: Structured data for programmatic access
  - **HTML**: Interactive reports with charts and visualizations
  - **Markdown**: Formatted tables for documentation

### 2. Design Document Updates

**Added Section 12: Compatibility Checker Service**
- Defined `CompatibilityChecker` trait
- Created `CompatibilityReport` struct with scoring and recommendations
- Defined `ReportFormat` enum
- Documented report generation details for each format

**Added Section 13: Report Generation System**
- Created unified `ReportGenerator` trait
- Defined `ReportData` trait for format conversion
- Documented report directory structure
- Added HTML report template
- Listed required dependencies:
  - `comfy-table`: ASCII table formatting
  - `serde_json`: JSON serialization
  - `plotters` or `charming`: Chart generation
  - `pulldown-cmark`: Markdown generation

**Report Directory Structure**:
```
.fuse/
└── report/
    ├── compatibility/
    │   ├── 2024-01-15_10-30-45.json
    │   ├── 2024-01-15_10-30-45.html
    │   └── 2024-01-15_10-30-45.md
    ├── scan/
    │   ├── model1_2024-01-15.html
    │   └── model1_2024-01-15.json
    ├── inspect/
    │   └── model1_layers.json
    └── validation/
        └── model1_validation.html
```

### 3. Tasks Document Updates

**Task 14: Model Compatibility Checker** - Expanded from 4 to 8 subtasks:
- 14.1: Added format flags and -o flag to CLI command
- 14.2: Compatibility scoring (unchanged)
- 14.3: NEW - Report generation infrastructure
- 14.4: NEW - ASCII table report format implementation
- 14.5: NEW - JSON report format implementation
- 14.6: NEW - HTML report format implementation
- 14.7: NEW - Markdown report format implementation
- 14.8: Enhanced testing to cover all formats

**Task 13: Layer Manipulation Service** - Enhanced:
- Added format flags to layer inspect command
- Added report generation for inspection results
- Save reports to `.fuse/report/inspect/`

**Task 16: Vulnerability Scanner** - Enhanced:
- Added format flags to scan command
- Generate reports in all 4 formats
- Save to `.fuse/report/scan/` by default
- Support custom output paths

## Command Examples

### Compatibility Check

```bash
# Default ASCII table output to terminal
fuse comp check model1 model2

# Generate JSON report with default location
fuse comp check model1 model2 --json

# Generate HTML report with custom location
fuse comp check model1 model2 --html -o /path/to/report.html

# Generate Markdown report
fuse comp check model1 model2 --md

# Multiple formats at once
fuse comp check model1 model2 --json --html --md
```

### Vulnerability Scan

```bash
# Default ASCII table output
fuse scan model1

# Generate HTML report
fuse scan model1 --html

# Custom output location
fuse scan model1 --json -o security-audit.json
```

### Layer Inspection

```bash
# Default format to terminal
fuse layer inspect model1

# Wide format with JSON export
fuse layer inspect model1 -o wide --json

# HTML report with visualizations
fuse layer inspect model1 --html
```

## Implementation Notes

### Report Format Features

1. **ASCII Table** (Default):
   - Uses `comfy-table` crate
   - Colored output for terminal
   - Compact, readable format
   - No file output unless `-o` specified

2. **JSON**:
   - Complete structured data
   - All raw metrics and metadata
   - Programmatic access friendly
   - Timestamp and version info

3. **HTML**:
   - Interactive visualizations
   - Embedded CSS (standalone)
   - Charts using `plotters` or `charming`
   - Responsive design
   - Print-friendly

4. **Markdown**:
   - GitHub/GitLab compatible
   - Formatted tables
   - Code blocks for technical details
   - Easy to include in documentation

### File Naming Convention

- **Timestamp format**: `YYYY-MM-DD_HH-MM-SS`
- **Compatibility**: `<timestamp>.<ext>`
- **Scan**: `<model>_<timestamp>.<ext>`
- **Inspect**: `<model>_layers.<ext>` or `<model>.<ext>`

### Directory Management

- Automatic creation of `.fuse/report/<feature>/` directories
- Graceful handling of permission errors
- Cleanup of old reports (configurable retention)

## Benefits

1. **Flexibility**: Users can choose the format that best suits their needs
2. **Automation**: JSON format enables CI/CD integration
3. **Documentation**: Markdown format for easy inclusion in docs
4. **Presentation**: HTML format for stakeholder reports
5. **Consistency**: Unified approach across all report-generating commands
6. **Discoverability**: Centralized report location in `.fuse/report/`

## Future Enhancements

1. **PDF Export**: Add PDF generation using `printpdf` or `headless_chrome`
2. **Report Comparison**: Compare multiple reports over time
3. **Report Aggregation**: Combine multiple reports into dashboards
4. **Custom Templates**: Allow users to provide custom HTML/Markdown templates
5. **Report Retention**: Automatic cleanup of old reports based on config
6. **Report Indexing**: Search across all generated reports

## Dependencies to Add

```toml
[dependencies]
# Report generation
comfy-table = "7.1"           # ASCII table formatting
plotters = "0.3"              # Chart generation for HTML
# OR
charming = "0.3"              # Alternative chart library
pulldown-cmark = "0.9"        # Markdown parsing/generation

# Optional for PDF
printpdf = "0.7"              # PDF generation
```

## Testing Requirements

Each report format must be tested for:
- Correct data serialization
- Proper file creation and naming
- Directory creation
- Custom output path handling
- Error handling (permissions, disk space)
- Format-specific features (charts, tables, etc.)

## Documentation Requirements

- Update CLI help text with format flags
- Add examples to README.md
- Create report format comparison guide
- Document report directory structure
- Provide sample reports in docs/examples/
