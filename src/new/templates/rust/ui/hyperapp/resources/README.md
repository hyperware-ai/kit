# 📚 Hyperware Skeleton App Resources

This directory contains all the resources needed to transform the skeleton app into any type of Hyperware application.

## 📖 Development Guides

The [`guides/`](./guides/) directory contains comprehensive documentation for building Hyperware apps:

- **[Quick Reference](./guides/00-QUICK-REFERENCE.md)** - Essential rules and syntax
- **[Common Patterns](./guides/01-COMMON-PATTERNS.md)** - Ready-to-use code recipes  
- **[Troubleshooting](./guides/02-TROUBLESHOOTING.md)** - Fix common errors
- **[WIT Types Guide](./guides/03-WIT-TYPES-DATA-MODELING.md)** - Data modeling constraints
- **[P2P Patterns](./guides/04-P2P-PATTERNS.md)** - Node-to-node communication
- **[Frontend Guide](./guides/05-UI-FRONTEND-GUIDE.md)** - React/TypeScript development
- **[Testing Guide](./guides/06-TESTING-DEBUGGING.md)** - Debug and test strategies
- **[Complete Examples](./guides/07-COMPLETE-EXAMPLES.md)** - Full working apps
- **[Manifest & Deployment](./guides/08-MANIFEST-AND-DEPLOYMENT.md)** - Understanding manifest.json
- **[Capabilities Guide](./guides/09-CAPABILITIES-GUIDE.md)** - System permissions reference

See the [Guides README](./guides/README.md) for detailed navigation help.

## 💡 Example App Ideas

The [`example-apps/TODO.md`](./example-apps/TODO.md) file contains 12+ app ideas ranging from basic to advanced:

- Todo lists and notepads
- P2P chat and file sharing  
- Collaborative tools
- Games and marketplaces
- System utilities

Each idea includes implementation notes and key concepts to demonstrate.

## 🎯 How to Use These Resources

### Starting a New App
1. Copy the skeleton app
2. Read the Quick Reference guide
3. Find a similar example in Complete Examples
4. Use Common Patterns for specific features

### When You're Stuck
1. Check Troubleshooting for your error
2. Verify all requirements in Quick Reference
3. Look for working patterns in Complete Examples
4. Test with simpler code first

### For Specific Features
- **State Management** → Common Patterns section 1
- **P2P Communication** → P2P Patterns guide
- **File Handling** → Common Patterns section 4
- **UI Development** → Frontend Guide

## 🔑 Key Principles

1. **Start Simple** - Get basic functionality working first
2. **Test Incrementally** - Don't write everything before testing
3. **Follow Patterns** - Use proven patterns from the guides
4. **Handle Errors** - Always provide user feedback
5. **Design for P2P** - Remember there's no central server

## 📝 Quick Reminders

### Must-Have Requirements
- `_request_body: String` parameter on ALL HTTP endpoints
- `<script src="/our.js"></script>` in your HTML
- Tuple format `[p1, p2]` for multi-parameter calls
- `.expects_response(30)` on remote requests

### Common Fixes
- **Build errors** → Usually missing requirements above
- **Type errors** → Use JSON strings for complex types
- **P2P failures** → Check node names and ProcessId format
- **UI issues** → Verify /our.js is included
- **manifest.json missing** → Run `kit b --hyperapp` to generate it
- **Capability errors** → Check Capabilities Guide for required permissions

## 🚀 Next Steps

1. Review the skeleton app's heavily commented `lib.rs`
2. Pick an example from Complete Examples to study
3. Start modifying the skeleton incrementally
4. Test with multiple nodes for P2P features

Remember: The skeleton app is designed to compile and run immediately. Build on that working foundation!