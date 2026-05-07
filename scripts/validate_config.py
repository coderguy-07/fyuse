#!/usr/bin/env python3
"""
Validate configuration files to ensure they don't contain sensitive data.
"""

import re
import sys
from pathlib import Path
from typing import List, Tuple

# Sensitive keys that should not have real values in committed configs
SENSITIVE_KEYS = [
    'api_key',
    'api_secret',
    'access_token',
    'auth_token',
    'password',
    'passwd',
    'pwd',
    'secret',
    'private_key',
    'client_secret',
    'aws_access_key_id',
    'aws_secret_access_key',
    'database_url',
    'connection_string',
    'jwt_secret',
    'encryption_key',
]

# Allowed placeholder values
ALLOWED_PLACEHOLDERS = [
    '',
    'YOUR_API_KEY_HERE',
    'YOUR_SECRET_HERE',
    '<API_KEY>',
    '<SECRET>',
    '${API_KEY}',
    '${SECRET}',
    'changeme',
    'placeholder',
    'example',
    'test',
    'dummy',
]

def check_config_file(filepath: Path) -> List[Tuple[int, str, str]]:
    """
    Check configuration file for sensitive data.
    
    Returns:
        List of (line_number, key, value) tuples with potential issues
    """
    findings = []
    
    try:
        with open(filepath, 'r', encoding='utf-8') as f:
            for line_num, line in enumerate(f, 1):
                # Skip comments
                if line.strip().startswith('#'):
                    continue
                
                # Check for sensitive keys
                for sensitive_key in SENSITIVE_KEYS:
                    # TOML format: key = "value"
                    toml_pattern = rf'{sensitive_key}\s*=\s*["\']([^"\']+)["\']'
                    # YAML format: key: value
                    yaml_pattern = rf'{sensitive_key}:\s*["\']?([^"\'\n]+)["\']?'
                    
                    for pattern in [toml_pattern, yaml_pattern]:
                        match = re.search(pattern, line, re.IGNORECASE)
                        if match:
                            value = match.group(1).strip()
                            
                            # Check if value is a placeholder
                            is_placeholder = False
                            for placeholder in ALLOWED_PLACEHOLDERS:
                                if value.lower() == placeholder.lower():
                                    is_placeholder = True
                                    break
                            
                            # Check if value looks like an environment variable
                            if value.startswith('$') or value.startswith('${'):
                                is_placeholder = True
                            
                            # If not a placeholder and has substantial length, flag it
                            if not is_placeholder and len(value) > 5:
                                findings.append((line_num, sensitive_key, value[:20] + '...'))
    
    except Exception as e:
        print(f"Error reading {filepath}: {e}", file=sys.stderr)
    
    return findings

def main():
    """Main function to validate configuration files."""
    if len(sys.argv) < 2:
        print("Usage: validate_config.py <config_file1> [config_file2] ...", file=sys.stderr)
        sys.exit(0)
    
    files_to_check = [Path(f) for f in sys.argv[1:]]
    all_findings = []
    
    for filepath in files_to_check:
        if not filepath.exists():
            continue
        
        # Skip example configs
        if 'example' in filepath.name.lower():
            continue
        
        findings = check_config_file(filepath)
        if findings:
            all_findings.append((filepath, findings))
    
    # Report findings
    if all_findings:
        print("\n" + "="*80)
        print("⚠️  SENSITIVE DATA IN CONFIG FILES - COMMIT BLOCKED")
        print("="*80 + "\n")
        
        for filepath, findings in all_findings:
            print(f"📄 {filepath}")
            for line_num, key, value in findings:
                print(f"   Line {line_num}: {key} = {value}")
            print()
        
        print("="*80)
        print("Configuration files should not contain real credentials.")
        print("Please use one of these approaches:")
        print("  1. Use environment variables: ${API_KEY}")
        print("  2. Use placeholders: YOUR_API_KEY_HERE")
        print("  3. Add to .gitignore and use local config files")
        print("  4. Use secret management tools")
        print("\nExample:")
        print("  api_key = \"${FUSE_API_KEY}\"  # ✓ Good")
        print("  api_key = \"sk-abc123...\"      # ✗ Bad")
        print("="*80 + "\n")
        
        sys.exit(1)
    
    sys.exit(0)

if __name__ == '__main__':
    main()
