#!/usr/bin/env python3
"""
Check file permissions to ensure sensitive files are not world-readable.
"""

import os
import stat
import sys
from pathlib import Path
from typing import List, Tuple

# Files that should have restricted permissions
SENSITIVE_FILE_PATTERNS = [
    '*.key',
    '*.pem',
    '*.p12',
    '*.pfx',
    '*_rsa',
    '*_dsa',
    '*_ecdsa',
    '*_ed25519',
    'config.toml',
    'config.yaml',
    'secrets.*',
    'credentials.*',
    '.env',
    '.env.*',
]

def check_file_permissions(filepath: Path) -> List[Tuple[str, str, str]]:
    """
    Check if file has appropriate permissions.
    
    Returns:
        List of (filepath, current_perms, issue) tuples
    """
    findings = []
    
    try:
        # Get file stats
        file_stat = os.stat(filepath)
        mode = file_stat.st_mode
        
        # Check if file is world-readable
        if mode & stat.S_IROTH:
            current_perms = oct(stat.S_IMODE(mode))
            findings.append((
                str(filepath),
                current_perms,
                "File is world-readable"
            ))
        
        # Check if file is world-writable
        if mode & stat.S_IWOTH:
            current_perms = oct(stat.S_IMODE(mode))
            findings.append((
                str(filepath),
                current_perms,
                "File is world-writable"
            ))
        
        # Check if file is group-writable for sensitive files
        if mode & stat.S_IWGRP:
            for pattern in SENSITIVE_FILE_PATTERNS:
                if filepath.match(pattern):
                    current_perms = oct(stat.S_IMODE(mode))
                    findings.append((
                        str(filepath),
                        current_perms,
                        "Sensitive file is group-writable"
                    ))
                    break
    
    except Exception as e:
        print(f"Error checking {filepath}: {e}", file=sys.stderr)
    
    return findings

def main():
    """Main function to check file permissions."""
    if len(sys.argv) < 2:
        print("Usage: check_file_permissions.py <file1> [file2] ...", file=sys.stderr)
        sys.exit(0)
    
    files_to_check = [Path(f) for f in sys.argv[1:]]
    all_findings = []
    
    for filepath in files_to_check:
        if not filepath.exists():
            continue
        
        findings = check_file_permissions(filepath)
        if findings:
            all_findings.extend(findings)
    
    # Report findings
    if all_findings:
        print("\n" + "="*80)
        print("⚠️  INSECURE FILE PERMISSIONS DETECTED")
        print("="*80 + "\n")
        
        for filepath, perms, issue in all_findings:
            print(f"📄 {filepath}")
            print(f"   Current permissions: {perms}")
            print(f"   Issue: {issue}")
            print()
        
        print("="*80)
        print("Please fix file permissions:")
        print("  chmod 600 <file>  # For private keys and secrets")
        print("  chmod 644 <file>  # For regular files")
        print("  chmod 755 <file>  # For executables")
        print("="*80 + "\n")
        
        sys.exit(1)
    
    sys.exit(0)

if __name__ == '__main__':
    main()
