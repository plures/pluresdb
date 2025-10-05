# Changelog

## [Unreleased]

- _No changes yet_

## [1.0.1] - 2025-10-03 — Core Security Hardening

### Changed

- Added payload sanitization before persistence to strip prototype pollution vectors and coerce injected functions into safe string placeholders.
- Hardened `GunDB#get` responses with sanitized clones, ensuring consumer code receives benign `toString` implementations and no inherited attacker-controlled state.
- Expanded the security regression suite so the type-confusion prevention scenario now exercises the sanitization path and passes under `npm run verify` (51 tests green).

## [Unreleased] - Phase 1 UI Completion ✅

**Phase 1 is now COMPLETE!** All planned UI foundation and UX polish items have been implemented.

## [Unreleased] - Phase 1 Part 2: Accessibility & Validation

### Added - UI Foundation & UX Polish ✅

- **Accessibility Enhancements**
  - Keyboard navigation with arrow keys, Enter/Space for selection across all panels
  - Comprehensive ARIA labels, roles, and landmark regions throughout the UI
  - Screen reader support with sr-only class and aria-live regions for dynamic content
  - Semantic HTML structure with proper heading hierarchy
- **Node List Improvements**
  - Sort controls for ID and Type with visual indicators (↑/↓)
  - Enhanced keyboard navigation (ArrowUp/ArrowDown to navigate, Enter/Space to select)
  - Proper listbox/option ARIA roles for better assistive technology support
- **Editor Enhancements**
  - Copy-as-cURL button to generate curl commands for API calls
  - Revert changes button with change tracking (disabled when no changes)
  - Visual indication of unsaved changes
  - Tooltips on all editor action buttons
- **Search Panel Improvements**
  - Keyboard navigation for search results (Enter/Space to select)
  - Live result count announcement for screen readers
  - Proper ARIA labels for search input and results

- **Settings Panel Improvements**
  - Live save status announcement for screen readers
  - Descriptive help text for all configuration fields
  - Enhanced ARIA descriptions for all inputs

- **Main Navigation**
  - Proper menubar role with aria-current for active view indication
  - Enhanced dark mode toggle with descriptive aria-label

### Changed

- Reorganized editor toolbar into two rows (formatting actions + node actions)
- Delete button now has outline style to differentiate destructive action
- Improved reactive text handling to preserve unsaved changes when navigating

### Technical

- Built with Svelte 4, CodeMirror 6, Vite 5
- All components now follow WCAG 2.1 accessibility guidelines
- Proper separation of concerns with dedicated stores and components

### Added - Accessibility & Validation ✅

- **WCAG AA Color Contrast**
  - GitHub-inspired color palette with verified 4.5:1 minimum contrast ratios
  - Enhanced primary colors: #0969da (light) / #58a6ff (dark)
  - Improved muted text colors for better readability
  - Enhanced focus indicators (2px outline with offset)
  - Semantic colors for success/error/warning states
  - Accessible disabled states with proper opacity

- **Inline JSON Schema Validation**
  - Real-time validation in CodeMirror as you type
  - Inline error markers with CodeMirror's linter system
  - JSON syntax validation with position-aware error messages
  - Schema validation warnings when schema is provided
  - Automatic revalidation when schema or content changes
  - Clear error messages showing path and validation issue

### Technical Improvements

- Added @codemirror/lint package for inline diagnostics
- Integrated Ajv JSON Schema validator with CodeMirror linter
- Custom CSS variables for WCAG AA compliant color system
- Reactive schema updates trigger editor reconfiguration

### Phase 1 Status: ✅ COMPLETE

All Phase 1 deliverables have been implemented and tested. The UI now has:

- Comprehensive accessibility (keyboard nav, ARIA, WCAG AA colors)
- Real-time JSON Schema validation
- Professional data explorer with all planned features

## Previous Releases

See git history for earlier changes.
