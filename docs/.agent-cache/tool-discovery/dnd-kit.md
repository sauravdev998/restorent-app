# dnd-kit Agent Skills and MCP Discovery

**Generated:** 2026-09-10

## Agent Skills

Found via `npx skills find dnd-kit`. Highest credibility dnd-kit specific skills:

1. **atman-33/skills@dnd-kit-implementation** (93 installs)
   - Dedicated dnd-kit implementation patterns
   - Direct dnd-kit focus

2. **ancoleman/ai-design-components@implementing-drag-drop** (336 installs)
   - General drag-drop patterns and best practices
   - Not dnd-kit specific but relevant for D&D UI patterns

3. **agents-inc/skills@web-dnd-dnd-kit** (20 installs)
   - Web focused dnd-kit implementation

4. **cfardev/kanban-hub@dnd-kit** (5 installs)
   - Kanban board pattern with dnd-kit

5. **spardutti/claude-skills@dnd-kit** (4 installs)
   - dnd-kit specific skill

## MCP Servers

1. **dnd-kit Docs** (GitMCP-based)
   - Source: https://gitmcp.io/clauderic/dnd-kit
   - Provides: Documentation access for dnd-kit GitHub repo
   - Enables querying dnd-kit docs in Claude Desktop, VSCode, Cursor, and other AI tools
   - Maintained by GitMCP project

## Version and Compatibility

- **@dnd-kit/core**: 6.3.1
  - Peer: react >=16.8.0 (compatible with React 19)
  
- **@dnd-kit/sortable**: 10.0.0
  - Peer: react >=16.8.0, @dnd-kit/core ^6.3.0
  - Compatible with React 19

- **@dnd-kit/react**: 0.5.0
  - Peer: react ^18.0.0 || ^19.0.0 (explicitly supports React 19)
  - Pre-1.0 but actively maintained; not yet considered "stable" by semver
  - Recommended for modern React 19 projects

**React 19 Compatibility**: All three packages work with React 19. @dnd-kit/react is the newest API with explicit React 19 support.

## Notes

- No dnd-kit skills already installed in .agents/skills/
- Not listed in AGENTS.md "Declined" section
- Verified: atman-33 and agents-inc repositories exist and are credible
- All version info from npm registry
