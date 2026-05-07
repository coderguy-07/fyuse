#!/usr/bin/env python3
"""
Check for sensitive patterns in code that should not be committed.
"""

import re
import sys
from pathlib import Path
from typing import List, Tuple

# Sensitive patterns to detect
SENSITIVE_PATTERNS = [
    # API Keys and Tokens
    (r'api[_-]?key\s*=\s*["\']([^"\']+)["\']', 'API Key'),
    (r'api[_-]?secret\s*=\s*["\']([^"\']+)["\']', 'API Secret'),
    (r'access[_-]?token\s*=\s*["\']([^"\']+)["\']', 'Access Token'),
    (r'auth[_-]?token\s*=\s*["\']([^"\']+)["\']', 'Auth Token'),
    (r'bearer\s+[A-Za-z0-9\-._~+/]+=*', 'Bearer Token'),
    
    # AWS Credentials
    (r'AKIA[0-9A-Z]{16}', 'AWS Access Key'),
    (r'aws[_-]?secret[_-]?access[_-]?key\s*=\s*["\']([^"\']+)["\']', 'AWS Secret Key'),
    
    # Private Keys
    (r'-----BEGIN\s+(?:RSA\s+)?PRIVATE\s+KEY-----', 'Private Key'),
    (r'-----BEGIN\s+OPENSSH\s+PRIVATE\s+KEY-----', 'SSH Private Key'),
    
    # Database Credentials
    (r'postgres://[^:]+:[^@]+@', 'PostgreSQL Connection String'),
    (r'mysql://[^:]+:[^@]+@', 'MySQL Connection String'),
    (r'mongodb://[^:]+:[^@]+@', 'MongoDB Connection String'),
    (r'redis://[^:]+:[^@]+@', 'Redis Connection String'),
    
    # Generic Passwords
    (r'password\s*=\s*["\'](?!<|{|\$|test|example|dummy|placeholder)([^"\']{8,})["\']', 'Password'),
    (r'passwd\s*=\s*["\'](?!<|{|\$|test|example|dummy|placeholder)([^"\']{8,})["\']', 'Password'),
    (r'pwd\s*=\s*["\'](?!<|{|\$|test|example|dummy|placeholder)([^"\']{8,})["\']', 'Password'),
    
    # JWT Tokens
    (r'eyJ[A-Za-z0-9-_=]+\.eyJ[A-Za-z0-9-_=]+\.[A-Za-z0-9-_.+/=]*', 'JWT Token'),
    
    # Slack Tokens
    (r'xox[baprs]-[0-9]{10,12}-[0-9]{10,12}-[A-Za-z0-9]{24,32}', 'Slack Token'),
    
    # GitHub Tokens
    (r'gh[pousr]_[A-Za-z0-9]{36,}', 'GitHub Token'),
    
    # Hugging Face Tokens
    (r'hf_[A-Za-z0-9]{20,}', 'Hugging Face Token'),
    
    # Generic Secrets
    (r'secret\s*=\s*["\'](?!<|{|\$|test|example|dummy|placeholder)([^"\']{16,})["\']', 'Secret'),
    
    # Email addresses (in certain contexts)
    (r'[a-zA-Z0-9._%+-]+@(?!example\.com|test\.com|localhost)[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}', 'Email Address'),
]

# Patterns to exclude (false positives)
EXCLUDE_PATTERNS = [
    r'example\.com',
    r'test\.com',
    r'localhost',
    r'127\.0\.0\.1',
    r'0\.0\.0\.0',
    r'<[A-Z_]+>',  # Template variables
    r'\$\{[^}]+\}',  # Environment variables
    r'TODO',
    r'FIXME',
    r'XXX',
]

def is_excluded(line: str) -> bool:
    """Check if line should be excluded from scanning."""
    for pattern in EXCLUDE_PATTERNS:
        if re.search(pattern, line, re.IGNORECASE):
            return True
    return False

def check_file(filepath: Path) -> List[Tuple[int, str, str]]:
    """
    Check a file for sensitive patterns.
    
    Returns:
        List of (line_number, pattern_name, matched_text) tuples
    """
    findings = []
    
    try:
        with open(filepath, 'r', encoding='utf-8', errors='ignore') as f:
            for line_num, line in enumerate(f, 1):
                # Skip excluded lines
                if is_excluded(line):
                    continue
                
                # Check each pattern
                for pattern, name in SENSITIVE_PATTERNS:
                    matches = re.finditer(pattern, line, re.IGNORECASE)
                    for match in matches:
                        # Mask the sensitive data
                        matched_text = match.group(0)
                        if len(matched_text) > 20:
                            masked = matched_text[:10] + '...' + matched_text[-5:]
                        else:
                            masked = matched_text[:5] + '***'
                        
                        findings.append((line_num, name, masked))
    
    except Exception as e:
        print(f"Error reading {filepath}: {e}", file=sys.stderr)
    
    return findings

def main():
    """Main function to check files for sensitive patterns."""
    if len(sys.argv) < 2:
        print("Usage: check_sensitive_patterns.py <file1> [file2] ...", file=sys.stderr)
        sys.exit(0)
    
    files_to_check = [Path(f) for f in sys.argv[1:]]
    all_findings = []
    
    for filepath in files_to_check:
        if not filepath.exists():
            continue
        
        findings = check_file(filepath)
        if findings:
            all_findings.append((filepath, findings))
    
    # Report findings
    if all_findings:
        print("\n" + "="*80)
        print("⚠️  SENSITIVE DATA DETECTED - COMMIT BLOCKED")
        print("="*80 + "\n")
        
        for filepath, findings in all_findings:
            print(f"📄 {filepath}")
            for line_num, pattern_name, masked_text in findings:
                print(f"   Line {line_num}: {pattern_name}")
                print(f"   → {masked_text}")
            print()
        
        print("="*80)
        print("Please remove sensitive data before committing.")
        print("Consider using:")
        print("  - Environment variables")
        print("  - Configuration files (added to .gitignore)")
        print("  - Secret management tools (Vault, AWS Secrets Manager)")
        print("="*80 + "\n")
        
        sys.exit(1)
    
    sys.exit(0)

if __name__ == '__main__':
    main()
