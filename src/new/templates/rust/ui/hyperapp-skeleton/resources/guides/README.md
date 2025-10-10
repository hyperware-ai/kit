# 📖 Hyperware Development Guides for AI Models

This directory contains comprehensive guides designed specifically for AI models building Hyperware applications using the skeleton app as a starting point.

## 🎯 Guide Navigation

### For Quick Reference
- **[00-QUICK-REFERENCE.md](./00-QUICK-REFERENCE.md)** - Critical rules, build commands, and common fixes
  - Use when: You need a quick reminder of syntax or requirements
  - Contains: Cheat sheet of must-follow rules and patterns

### For Implementation
- **[01-COMMON-PATTERNS.md](./01-COMMON-PATTERNS.md)** - Copy-paste recipes for common scenarios
  - Use when: Building specific features (CRUD, P2P, state management)
  - Contains: Complete code patterns ready to adapt

### For Problem Solving
- **[02-TROUBLESHOOTING.md](./02-TROUBLESHOOTING.md)** - Comprehensive error diagnosis and fixes
  - Use when: Something isn't working as expected
  - Contains: Error messages, root causes, and solutions

### For Data Design
- **[03-WIT-TYPES-DATA-MODELING.md](./03-WIT-TYPES-DATA-MODELING.md)** - Type system constraints and workarounds
  - Use when: Designing data structures or getting WIT errors
  - Contains: Type compatibility matrix, patterns, and escape hatches

### For P2P Features
- **[04-P2P-PATTERNS.md](./04-P2P-PATTERNS.md)** - Node-to-node communication patterns
  - Use when: Building collaborative or distributed features
  - Contains: Complete P2P examples from basic to advanced

### For Frontend Development
- **[05-UI-FRONTEND-GUIDE.md](./05-UI-FRONTEND-GUIDE.md)** - React/TypeScript UI development
  - Use when: Building the user interface
  - Contains: Component patterns, state management, API integration

### For Quality Assurance
- **[06-TESTING-DEBUGGING.md](./06-TESTING-DEBUGGING.md)** - Testing strategies and debug techniques
  - Use when: Testing your app or tracking down bugs
  - Contains: Debug helpers, test patterns, performance monitoring

### For Complete Examples
- **[07-COMPLETE-EXAMPLES.md](./07-COMPLETE-EXAMPLES.md)** - Full working app examples
  - Use when: You need to see how everything fits together
  - Contains: Todo app, notepad, KV store, file sharing examples

### For Deployment & Installation
- **[08-MANIFEST-AND-DEPLOYMENT.md](./08-MANIFEST-AND-DEPLOYMENT.md)** - Understanding manifest.json
  - Use when: Getting "failed to open file manifest.json" errors
  - Contains: manifest.json structure, auto-generation, customization

### For System Permissions
- **[09-CAPABILITIES-GUIDE.md](./09-CAPABILITIES-GUIDE.md)** - Capabilities reference
  - Use when: Unsure what capabilities to request or getting permission errors
  - Contains: All system capabilities, security patterns, common errors

### For Database Development
- **[10-SQLITE-API-GUIDE.md](./10-SQLITE-API-GUIDE.md)** - Comprehensive SQLite guide
  - Use when: Building apps that need persistent data storage
  - Contains: Setup, basic operations, advanced patterns, complete examples

### For Real-time Communication
- **[11-REALTIME-COMMUNICATION-PATTERNS.md](./11-REALTIME-COMMUNICATION-PATTERNS.md)** - WebSocket & streaming patterns
  - Use when: Building voice/video chat, collaborative editing, or real-time features
  - Contains: WebSocket protocols, audio streaming, state sync, performance patterns

## 🔍 How to Use These Guides

### Quick Start Path (For Beginners)
If you're new to Hyperware development, follow this path:
1. **[00-QUICK-REFERENCE](./00-QUICK-REFERENCE.md)** - Learn the critical rules (10 min)
2. **[08-MANIFEST-AND-DEPLOYMENT](./08-MANIFEST-AND-DEPLOYMENT.md)** - Understand how apps work (5 min)
3. **[09-CAPABILITIES-GUIDE](./09-CAPABILITIES-GUIDE.md)** - Know what you can access (10 min)
4. **[01-COMMON-PATTERNS](./01-COMMON-PATTERNS.md)** - Copy patterns for your features (reference as needed)
5. **[07-COMPLETE-EXAMPLES](./07-COMPLETE-EXAMPLES.md)** - See full working apps (study one similar to yours)

### Building a New App
1. Start with **QUICK-REFERENCE** to understand the rules
2. Read **MANIFEST-AND-DEPLOYMENT** to understand how apps are installed
3. Review **CAPABILITIES-GUIDE** to know what your app can access
4. Check **COMPLETE-EXAMPLES** for a similar app pattern
5. Use **COMMON-PATTERNS** for specific features
6. Refer to **WIT-TYPES** for data modeling
7. Check **TROUBLESHOOTING** when errors occur

### Adding P2P Features
1. Read **P2P-PATTERNS** thoroughly
2. Start with basic request-response pattern
3. Test with multiple nodes early
4. Use **TESTING-DEBUGGING** for P2P test scenarios

### Fixing Errors
1. Check **TROUBLESHOOTING** first
2. Look for your error message
3. Follow the diagnostic steps
4. Use **TESTING-DEBUGGING** if needed

## 🚨 Most Common Mistakes to Avoid

1. **Missing `_request_body` parameter** in HTTP endpoints
   - Solution: See QUICK-REFERENCE section 1

2. **Missing `/our.js` script** in HTML
   - Solution: See QUICK-REFERENCE section 2

3. **Using object format** instead of tuples for parameters
   - Solution: See QUICK-REFERENCE section 3

4. **No timeout** on remote requests
   - Solution: See QUICK-REFERENCE section 4

5. **Using HashMap** or other unsupported types
   - Solution: See WIT-TYPES guide

## 📊 Decision Trees

### "What guide should I read?"
```
Need to fix an error? → TROUBLESHOOTING
Need to implement a feature? → COMMON-PATTERNS
Need to understand P2P? → P2P-PATTERNS
Need to design data types? → WIT-TYPES
Need UI help? → UI-FRONTEND
Need quick syntax reminder? → QUICK-REFERENCE
Want to see full examples? → COMPLETE-EXAMPLES
Getting manifest.json errors? → MANIFEST-AND-DEPLOYMENT
Need to know what capabilities? → CAPABILITIES-GUIDE
Building real-time features? → REALTIME-COMMUNICATION-PATTERNS
```

### "My build is failing"
```
Check TROUBLESHOOTING Section 1: Build Errors
↓
Still failing? Check QUICK-REFERENCE for common fixes
↓
Type-related? Check WIT-TYPES guide
↓
Import issues? Check TROUBLESHOOTING "ambiguous imports"
```

### "My P2P calls aren't working"
```
Check P2P-PATTERNS for correct syntax
↓
Check TROUBLESHOOTING Section 3: P2P Issues
↓
Add debug logging (see TESTING-DEBUGGING)
↓
Test with local nodes first
```

## 💡 Pro Tips for AI Models

1. **Always validate** your understanding by checking the examples
2. **Copy working patterns** rather than creating from scratch
3. **Test incrementally** - don't write everything before testing
4. **Use type-safe patterns** - avoid `any` types
5. **Handle errors gracefully** - users need feedback
6. **Design for offline** - nodes can disconnect
7. **Keep state minimal** - less to sync and debug

## 🔗 External Resources

- The `example-apps` folder contains working applications
- The skeleton app is your starting template
- Build with `kit b --hyperapp`
- Test with `kit s`

## 📝 Guide Maintenance

These guides are based on observed patterns and common issues. Key sources:
- Working example apps (especially samchat)
- Common build errors and their fixes
- P2P communication patterns that work
- Type system limitations and workarounds

When in doubt, refer to the working examples in `example-apps/` folder.

---

Remember: The skeleton app is designed to compile and run immediately. Start there and modify incrementally, testing at each step.