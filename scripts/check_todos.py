#!/usr/bin/env python3
"""
Check for TODO/FIXME comments in production code.
"""

import re
import sys
from pathlib import Path
from typing import List, Tuple

# Patterns to detect
TODO_PATTERNS = [
    (r'//\s*TODO:', 'TODO'),
    (r'//\s*FIXME:', 'FIXME'),
    (r'//\s*XXX:', 'XXX'),
    (r'//\s*HACK:', 'HACK'),
    (r'//\s*BUG:', 'BUG'),
    (r'//\s*NOTE:', 'NOTE'),
]

def check_file(filepath: Path) -> List[Tuple[int, str, str]]:
    """
    Check a file for TODO/FIXME comments.
    
    Returns:
        List of (line_number, comment_type, comment_text) tuples
    """
    findings = []
    
    try:
        with open(filepath, 'r', encoding='utf-8', errors='ignore') as f:
            for line_num, line in enumerate(f, 1):
                for pattern, comment_type in TODO_PATTERNS:
                    match = re.search(pattern, line, re.IGNORECASE)
                    if match:
                        # Extract the comment text
                        comment_text = line[match.end():].strip()
                        if len(comment_text) > 60:
                            comment_text = comment_text[:60] + '...'
                        findings.append((line_num, comment_type, comment_text))
    
    except Exception as e:
        print(f"Error reading {filepath}: {e}", file=sys.stderr)
    
    return findings

def main():
    """Main function to check files for TODO comments."""
    if len(sys.argv) < 2:
        print("Usage: check_todos.py <file1> [file2] ...", file=sys.stderr)
        sys.exit(0)
    
    files_to_check = [Path(f) for f in sys.argv[1:]]
    all_findings = []
    
    for filepath in files_to_check:
        if not filepath.exists():
            continue
        
        # Skip test files
        if 'test' in str(filepath).lower():
            continue
        
        findings = check_file(filepath)
        if findings:
            all_findings.append((filepath, findings))
    
    # Report findings
    if all_findings:
        print("\n" + "="*80)
        print("⚠️  TODO/FIXME COMMENTS FOUND")
        print("="*80 + "\n")
        
        for filepath, findings in all_findings:
            print(f"📄 {filepath}")
            for line_num, comment_type, comment_text in findings:
                print(f"   Line {line_num}: {comment_type} - {comment_text}")
            print()
        
        print("="*80)
        print("Please resolve TODO/FIXME comments before pushing to production.")
        print("If these are intentional, consider:")
        print("  - Creating GitHub issues and referencing them")
        print("  - Moving to documentation")
        print("  - Using #[allow(todo)] for intentional TODOs")
        print("="*80 + "\n")
        
        # Warning only, don't block commit
        sys.exit(0)
    
    sys.exit(0)

if __name__ == '__main__':
    main()
