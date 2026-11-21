# Visual Regression Testing Guide

## Overview

Visual regression testing automatically captures and compares UI screenshots to detect unintended visual changes. This guide explains how to set up, run, and maintain visual regression tests for the Stellar Nebula Nomad frontend.

## Quick Start

### Local Setup

1. Install dependencies in frontend directory:

```bash
cd frontend
npm install
```

2. Add visual testing scripts to `package.json`:

```json
{
  "scripts": {
    "storybook": "storybook dev -p 6006",
    "build-storybook": "storybook build",
    "test:visual": "playwright test --config=visual-regression.config.ts",
    "test:visual:baseline": "playwright test --config=visual-regression.config.ts --update-snapshots",
    "test:a11y": "axe --selenium"
  }
}
```

3. Build Storybook:

```bash
npm run build-storybook
```

4. Run visual tests:

```bash
npm run test:visual
```

### Docker Setup

For consistent testing environments:

```bash
docker build -f Dockerfile.visual-tests -t stellar-visual-tests .
docker run --rm stellar-visual-tests
```

## CI/CD Integration

### GitHub Actions Workflow

The `.github/workflows/visual-regression.yml` workflow:

1. **Builds Storybook** - Creates static site with all components
2. **Captures Screenshots** - Takes baseline images on main branch
3. **Compares on PR** - Detects visual changes in pull requests
4. **Accessibility Scan** - Checks WCAG compliance
5. **Comments on PR** - Reports results directly in PR

### How It Works

```
PR Created
    ↓
Build Storybook
    ↓
Capture Current Screenshots
    ↓
Compare to Baseline (from main)
    ↓
Generate Diffs (if changes)
    ↓
Comment Results on PR
    ↓
Review Results
```

## Component Story Setup

### Writing Stories for Visual Testing

Create `.stories.tsx` files for components:

```typescript
// Button.stories.tsx
import { Button } from './Button';

export default {
  title: 'Components/Button',
  component: Button,
  tags: ['autodocs'],
};

export const Primary = {
  args: {
    variant: 'primary',
    label: 'Click me',
  },
};

export const Secondary = {
  args: {
    variant: 'secondary',
    label: 'Cancel',
  },
};

export const Loading = {
  args: {
    variant: 'primary',
    loading: true,
    label: 'Loading...',
  },
};
```

### Viewports to Test

Visual tests capture screenshots in multiple viewports:

| Viewport | Width | Height | Use Case |
|----------|-------|--------|----------|
| Desktop | 1280px | 720px | Main browser |
| Tablet | 768px | 1024px | iPad-like devices |
| Mobile | 375px | 812px | iPhone-like devices |

## Baseline Management

### Capturing Baselines

Initial baseline capture (run once on main):

```bash
npm run test:visual:baseline
```

This creates baseline images in `frontend/visual-regression-baselines/`.

### Updating Baselines

When intentional UI changes are made:

1. Verify changes are correct
2. Update baselines:

```bash
npm run test:visual:baseline
```

3. Review diffs:

```bash
git diff frontend/visual-regression-baselines/
```

4. Commit changes:

```bash
git add frontend/visual-regression-baselines/
git commit -m "chore: update visual baselines for component changes"
```

### Baseline Storage

```
frontend/
  visual-regression-baselines/
    Components-Button-Primary-desktop.png
    Components-Button-Primary-mobile.png
    Components-Button-Secondary-tablet.png
    ...
```

## Interpreting Results

### No Visual Changes

```
✓ All visual tests passed
  0 differences detected
  47 stories compared
```

No action needed.

### Visual Changes Detected

```
✗ Visual regression detected
  2 differences found
  - Components/Button/Primary (desktop, tablet)
  - Components/Navigation/Header (mobile)

Review diffs in visual-regression-diffs/
```

**What to do**:
1. Check `visual-regression-diffs/` for highlighted changes
2. If intentional, approve and update baselines
3. If unintended, revert the CSS/component changes

### New Components

```
⚠ New baselines created for:
  - Components/Modal (3 stories)
  - Components/Toast (5 stories)

Please review and commit baselines.
```

New components create new baselines. Review them before committing.

## Accessibility Testing

### WCAG Compliance

Automated accessibility checks run alongside visual tests:

```bash
npm run test:a11y
```

Checks for:
- Color contrast ratios
- Missing alt text
- Keyboard navigation
- ARIA attributes
- Semantic HTML

### Manual Accessibility Review

For each PR with UI changes:

1. Run the app in browser
2. Test keyboard navigation (Tab through all components)
3. Test screen reader (use axe DevTools)
4. Test mobile accessibility
5. Check responsive behavior

## Troubleshooting

### "Screenshots don't match baseline"

**Likely causes**:
1. CSS changes (intentional or unintended)
2. Font rendering differences across OS
3. Dynamic content (use fixed test data)

**Solutions**:
1. If intentional, update baselines
2. If unintended, check recent CSS changes
3. Use `isolation: 'isolate'` CSS for consistent rendering

### Flaky Visual Tests

**Symptoms**: Same test passes/fails inconsistently

**Causes**:
1. Animations not waited for
2. API calls incomplete
3. Timing-dependent rendering

**Fixes**:
1. Add `waitFor` conditions in test setup
2. Mock API responses with fixed data
3. Disable animations in test mode:

```css
@media (prefers-reduced-motion: reduce) {
  * {
    animation: none !important;
    transition: none !important;
  }
}
```

### Viewport Size Issues

**Problem**: Components render differently at breakpoints

**Solution**: Test at standard breakpoints:

```json
"viewport": {
  "mobile": { "width": 375, "height": 812 },
  "tablet": { "width": 768, "height": 1024 },
  "desktop": { "width": 1280, "height": 720 }
}
```

## Best Practices

### Do's ✓

- ✓ Test primary and alternate states (hover, focus, disabled, loading)
- ✓ Test responsive breakpoints
- ✓ Use consistent test data (avoid random values)
- ✓ Update baselines only for intentional changes
- ✓ Review all visual diffs before committing
- ✓ Disable animations and transitions for consistency
- ✓ Test accessibility alongside visual tests

### Don'ts ✗

- ✗ Use dynamic dates/times in component stories
- ✗ Update baselines without reviewing changes
- ✗ Ignore accessibility failures
- ✗ Mix visual changes with logic changes in one PR
- ✗ Use `--update-snapshots` on every test run
- ✗ Skip visual tests in CI
- ✗ Test implementation details instead of visual output

## Configuration Files

### `frontend/.storybook/main-visual.ts`

Storybook config for visual testing:
- Loads all `.stories.tsx` files
- Enables a11y addon
- Sets up visual regression mode

### `frontend/.storybook/visual-regression-config.json`

Visual test runner config:
- Viewport definitions
- Comparison thresholds (1% default)
- Concurrency settings
- Baseline/diff directories

### `.github/workflows/visual-regression.yml`

CI workflow:
- Builds Storybook
- Runs visual tests
- Captures baselines on main
- Accessibility scanning

## Related Commands

```bash
# Run visual tests with verbose output
npm run test:visual -- --debug

# Update specific story baseline
npm run test:visual -- Components/Button --update-snapshots

# Run tests in headed mode (see browser)
npm run test:visual -- --headed

# Run with specific viewport only
npm run test:visual -- --grep "@desktop"

# Generate HTML report
npm run test:visual -- --reporter=html
```

## Related Documentation

- [Storybook Documentation](https://storybook.js.org/)
- [Playwright Visual Comparison](https://playwright.dev/docs/test-snapshots)
- [WCAG 2.1 Standards](https://www.w3.org/WAI/WCAG21/quickref/)
- [CSS Best Practices for Testing](./FRONTEND_TESTING.md)
