# Knowledge Engine Git Hooks

These git hooks are automatically installed by the Knowledge Engine extension to provide continuous learning and optimization.

## Hooks Included

- **pre-commit**: Analyzes staged changes and learns patterns
- **post-commit**: Records successful patterns
- **pre-push**: Runs optimization before pushing
- **post-merge**: Learns from merged changes  
- **commit-msg**: Learns from commit message patterns

## Installation

These hooks are installed automatically when the Knowledge Engine extension activates.

To manually install:
```bash
git config core.hooksPath .githooks
```

## Features

- 🧠 Automatic pattern learning from code changes
- 📊 Success pattern recording
- 🔧 Pre-push optimization
- 🐛 Bug fix pattern detection
- ✨ Feature pattern detection

## Disabling

To disable automation, update VS Code settings:
```json
{
  "knoEng.automation.gitHooks": false
}
```
