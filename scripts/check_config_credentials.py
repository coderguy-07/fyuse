#!/usr/bin/env python3
"""
Check configuration files for exposed credentials and sensitive information.
"""

import re
import sys
from pathlib import Path
from typing import List, Tuple, Dict

# Credential patterns to detect
CREDENTIAL_PATTERNS = {
    'AWS Access Key': r'AKIA[0-9A-Z]{16}',
    'AWS Secret Key': r'[A-Za-z0-9/+=]{40}',
    'GitHub Token': r'gh[pousr]_[A-Za-z0-9]{36,}',
    'Slack Token': r'xox[baprs]-[0-9]{10,12}-[0-9]{10,12}-[A-Za-z0-9]{24,32}',
    'Hugging Face Token': r'hf_[A-Za-z0-9]{20,}',
    'JWT Token': r'eyJ[A-Za-z0-9-_=]+\.eyJ[A-Za-z0-9-_=]+\.[A-Za-z0-9-_.+/=]*',
    'Private Key': r'-----BEGIN\s+(?:RSA\s+)?PRIVATE\s+KEY-----',
    'Database URL': r'(?:postgres|mysql|mongodb|redis)://[^:]+:[^@]+@',
}

# Suspicious value patterns
SUSPICIOUS_PATTERNS = {
    'Long Base64': r'[A-Za-z0-9+/]{40,}={0,2}',
    'Hex String': r'[0-9a-fA-F]{32,}',
    'UUID': r'[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}',
}

def check_config_credentials(filepath: Path) -> List[Tuple[int, str, str]]:
    """
    Check configuration file for credentials.
    
    Returns:
        List of (line_number, credential_type, masked_value) tuples
    """
    findings = []
    
    try:
        with open(filepath, 'r', encoding='utf-8') as f:
            content = f.read()
            lines = content.split('\n')
            
            for line_num, line in enumerate(lines, 1):
                # Skip comments
                if line.strip().startswith('#'):
                    continue
                
                # Check for credential patterns
                for cred_type, pattern in CREDENTIAL_PATTERNS.items():
                    matches = re.finditer(pattern, line)
                    for match in matches:
                        value = match.group(0)
                        masked = value[:10] + '***' if len(value) > 10 else '***'
                        findings.append((line_num, cred_type, masked))
                
                # Check for suspicious patterns in values
                if '=' in line or ':' in line:
                    # Extract value part
                    if '=' in line:
                        parts = line.split('=', 1)
                    else:
                        parts = line.split(':', 1)
                    
                    if len(parts) == 2:
                        key = parts[0].strip()
                        value = parts[1].strip().strip('"\'')
                        
                        # Skip if value is a placeholder or env var
                        if value.startswith('$') or value.startswith('${'):
                            continue
                        if value.upper() in ['YOUR_API_KEY_HERE', 'CHANGEME', 'PLACEHOLDER']:
                            continue
                        
                        # Check suspicious patterns
                        for pattern_name, pattern in SUSPICIOUS_PATTERNS.items():
                            if re.fullmatch(pattern, value):
                                # Check if key suggests this is sensitive
                                sensitive_keys = ['key', 'secret', 'token', 'password', 'credential']
                                if any(sk in key.lower() for sk in sensitive_keys):
                                    masked = value[:10] + '***' if len(value) > 10 else '***'
                                    findings.append((line_num, f'Suspicious {pattern_name}', masked))
    
    except Exception as e:
        print(f"Error reading {filepath}: {e}", file=sys.stderr)
    
    return findings

def main():
    """Main function to check config files for credentials."""
    if len(sys.argv) < 2:
        print("Usage: check_config_credentials.py <config_file1> [config_file2] ...", file=sys.stderr)
        sys.exit(0)
    
    files_to_check = [Path(f) for f in sys.argv[1:]]
    all_findings = []
    
    for filepath in files_to_check:
        if not filepath.exists():
            continue
        
        # Skip example configs
        if 'example' in filepath.name.lower():
            continue
        
        findings = check_config_credentials(filepath)
        if findings:
            all_findings.append((filepath, findings))
    
    # Report findings
    if all_findings:
        print("\n" + "="*80)
        print("🚨 CREDENTIALS DETECTED IN CONFIG FILES - COMMIT BLOCKED")
        print("="*80 + "\n")
        
        for filepath, findings in all_findings:
            print(f"📄 {filepath}")
            for line_num, cred_type, masked_value in findings:
                print(f"   Line {line_num}: {cred_type}")
                print(f"   → {masked_value}")
            print()
        
        print("="*80)
        print("CRITICAL: Configuration files contain credentials!")
        print("\nImmediate actions:")
        print("  1. Remove all credentials from config files")
        print("  2. Use environment variables instead")
        print("  3. Add config files to .gitignore")
        print("  4. Rotate any exposed credentials immediately")
        print("\nExample secure configuration:")
        print("  [auth]")
        print("  api_key = \"${FUSE_API_KEY}\"")
        print("  secret = \"${FUSE_SECRET}\"")
        print("="*80 + "\n")
        
        sys.exit(1)
    
    sys.exit(0)

if __name__ == '__main__':
    main()
