# Guide Updates Summary

This document summarizes the changes made to improve the Hyperware guides.

## Files Changed

### 1. Moved and Renumbered SQLite Guide
- **Action**: Moved `/resources/guides/11-SQLITE-API-GUIDE.md` to `/hyperapp-skeleton/resources/guides/10-SQLITE-API-GUIDE.md`
- **Reason**: Guide was in wrong location, now consistent with other guides
- **Note**: Renumbered from 11 to 10 after deleting the redundant Guide 10

### 2. Deleted Redundant Guide
- **Action**: Deleted `10-MANIFEST-CONFIGURATION.md`
- **Reason**: Content was redundant with Guide 08, merged key information into Guide 08

### 3. Updated Guide 08 (MANIFEST-AND-DEPLOYMENT.md)
- Added build process flow diagram
- Added metadata.json example
- Added skeleton app configuration section from deleted Guide 10
- Improved explanation of manifest.json generation

### 4. Updated Guide 09 (CAPABILITIES-GUIDE.md)
- Added critical warning that SQLite requires BOTH sqlite AND vfs capabilities
- Updated SQLite code examples to match actual API
- Added reference to SQLite guide
- Added note about network capability format variations

### 5. Updated Guide 01 (COMMON-PATTERNS.md)
- Added new Timer Patterns section with examples:
  - Basic one-time timer
  - Recurring timer pattern
  - Delayed operations
  - Timeout pattern
  - Debounce pattern
- Added standard error handling pattern
- Updated table of contents

### 6. Updated Guide 00 (QUICK-REFERENCE.md)
- Added reference to Troubleshooting guide for `_request_body` details
- Standardized import pattern with `serde_json::json!`
- Improved import requirements section

### 7. Updated Guide README.md
- Added "Quick Start Path" section for beginners
- Added SQLite guide to the navigation
- Improved guide organization

### 8. Updated Guide 04 (P2P-PATTERNS.md)
- Added important notes about publisher consistency across nodes
- Clarified ProcessId format requirements
- Added comments about metadata.json relationship

### 9. Updated Guide 03 (WIT-TYPES-DATA-MODELING.md)
- Added "See Also" section with cross-references
- Links to Troubleshooting, Common Patterns, and Complete Examples

## Key Improvements

1. **Better Organization**: Removed redundancy, consolidated related information
2. **Cross-References**: Added links between related guides
3. **Standardization**: Consistent import patterns and error handling
4. **Missing Content**: Added timer patterns that were previously undocumented
5. **Critical Warnings**: Highlighted SQLite+VFS requirement prominently
6. **Beginner-Friendly**: Added quick start path for new developers

## Notes on Uncertain Information

Some information was marked with notes rather than changed:
- Network capability format (`net:tcp:sys` vs `net:distro:sys`) - added note about variation
- Specific error messages - left as-is since they may have changed
- API signatures not seen in actual code - left unchanged

These guides should now be more accurate, less redundant, and easier to navigate.