# Lettura Design System

> A reading-first visual language for Lettura — your personal reading space.

## Philosophy

**"Paper and ink, refined for the screen."**

Lettura is a tool for deep reading and knowledge management. The design system prioritizes:

1. **Readability first** — Typography and spacing are optimized for long-form text consumption
2. **Warm minimalism** — Clean interfaces with subtle warmth, avoiding cold, clinical aesthetics
3. **Progressive disclosure** — Information and actions surface only when needed
4. **Consistency across surfaces** — Web app, browser extension, and future surfaces share the same visual language

## Color System

### Primitive Colors

| Token | Hex | Usage |
|-------|-----|-------|
| **Indigo 600** | `#4f46e5` | Primary actions, links, active states |
| **Indigo 500** | `#6366f1` | Primary in dark mode (brighter for contrast) |
| **Amber 500** | `#f59e0b` | Stars, bookmarks, highlights — "save this" |
| **Emerald 600** | `#059669` | Success states, confirmations |
| **Rose 600** | `#e11d48` | Destructive actions, errors |
| **Stone 25** | `#fafaf9` | Light mode background (paper-like) |
| **Stone 900** | `#1c1917` | Light mode primary text |
| **Slate 950** | `#020617` | Dark mode background |
| **Slate 100** | `#f1f5f9` | Dark mode primary text |

### Semantic Tokens (CSS Variables)

```css
/* Light mode */
--background:     30 6% 97%   /* stone-25  */
--foreground:     24 10% 10%  /* stone-900 */
--card:           0 0% 100%   /* white     */
--primary:        243 75% 59% /* indigo-600 */
--accent:         30 6% 92%   /* warm gray for hover/selection */
--destructive:    350 89% 60% /* rose-500   */
--success:        160 84% 39% /* emerald-600 */
--muted:          30 6% 90%
--border:         30 6% 88%
```

### Color Usage Rules

- **Primary (Indigo)** — Buttons, links, navigation active states, focus rings
- **Accent (warm gray)** — Generic hover/selection backgrounds. Used by shadcn/ui components.
- **Amber** — Star/bookmark actions only. Do not use for primary actions.
- **Success (Emerald)** — Archive confirmations, "saved" states, positive feedback
- **Destructive (Rose-600)** — Delete, remove, error states. Rose-500 had insufficient contrast with white text.
- **Background hierarchy** — `background` < `card` < `popover`. Each step slightly more elevated.

## Typography

### Font Stack

```css
--font-sans: "Inter", "Noto Sans SC", "PingFang SC", "Hiragino Sans GB",
             "Microsoft YaHei", ui-sans-serif, system-ui, sans-serif;
--font-mono: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace;
```

### Scale

| Token | Size | Line Height | Weight | Letter Spacing | Usage |
|-------|------|-------------|--------|----------------|-------|
| **Display** | 1.5rem (24px) | 1.2 | 700 | -0.025em | Page titles ("未读", "收藏") |
| **Headline** | 1.25rem (20px) | 1.3 | 600 | -0.015em | Section headers |
| **Title** | 1.0625rem (17px) | 1.4 | 600 | -0.01em | Card titles, article headlines |
| **Body** | 0.9375rem (15px) | 1.6 | 400 | 0 | General body text |
| **Caption** | 0.8125rem (13px) | 1.5 | 400 | 0.01em | Metadata, timestamps |
| **Label** | 0.75rem (12px) | 1.4 | 500 | 0.02em | Badges, tags, small labels |

### Rules

- Use `font-feature-settings: "cv02", "cv03", "cv04", "cv11", "tnum"` for better numeral and glyph rendering
- Headlines and titles use negative letter-spacing for tighter, more confident typography
- Body text stays at comfortable 1.6 line-height for Chinese/English mixed content

## Spacing & Shape

### Radius Scale

| Token | Value | Usage |
|-------|-------|-------|
| `--radius-sm` | 0.25rem (4px) | Tags, badges, small pills |
| `--radius-md` | 0.375rem (6px) | Small buttons, inputs |
| `--radius-lg` | 0.625rem (10px) | Default — cards, buttons, modals |
| `--radius-xl` | 1.125rem (18px) | Large cards, containers |
| `--radius-2xl` | 1.625rem (26px) | Hero sections, feature cards |

### Shadows

Keep shadows extremely subtle. We want elevation to feel like stacking paper, not floating glass.

```css
/* Card hover lift */
box-shadow:
  0 1px 2px hsl(var(--foreground) / 0.03),
  0 4px 12px hsl(var(--foreground) / 0.05);

/* Primary button */
box-shadow: 0 1px 2px hsl(var(--primary) / 0.25);
```

### Spacing Philosophy

- Use a **4px base grid** (Tailwind default)
- Card internal padding: `20px` (p-5) standard, `16px` (p-4) compact
- Section gaps: `20px` to `24px`
- Element gaps inside cards: `12px` to `16px`

## Component Patterns

### Card

Cards are the primary content container. They should feel like sheets of paper.

```css
.card-surface {
  background: hsl(var(--card));
  border: 1px solid hsl(var(--border));
  border-radius: var(--radius-xl);
  transition: transform 0.2s ease, box-shadow 0.2s ease;
}
.card-surface:hover {
  transform: translateY(-1px);
  box-shadow: 0 4px 12px hsl(var(--foreground) / 0.05);
}
```

### Button

| Variant | Style |
|---------|-------|
| **Primary** | Indigo background, white text, subtle shadow |
| **Secondary** | Muted background, dark text |
| **Outline** | Transparent bg, border, hover fills |
| **Ghost** | Transparent, hover shows muted bg |
| **Danger** | Rose background or rose text on hover |

Rules:
- Buttons have `border-radius: 10px` (radius-lg)
- Primary buttons have active state: `transform: scale(0.98)`
- Icon-only buttons are square with generous padding

### Input

- Background: transparent or `bg-background`
- Border: `1px solid hsl(var(--border))`
- Focus: `border-color: primary`, `box-shadow: 0 0 0 3px primary/12`
- Height: 40px standard, 36px compact

## Reading Experience (Entry Content)

This is the heart of Lettura. Article content must be optimized for deep reading.

### Text

- Base size: `17px` (`1.0625rem`)
- Line height: `1.8`
- Max reading width: `680px` (enforced by container, not `.entry-content` itself)
- Paragraph spacing: `1.35em` margin-bottom

### Links in Content

- Color: Primary indigo
- Underline with `text-decoration-color: primary/35`
- Hover: underline darkens to `primary/80`, slight opacity reduction
- No jarring color shifts

### Images

- Border radius: `var(--radius)`
- Light mode: no filter
- **Dark mode: `filter: brightness(0.88) contrast(1.05)`** — essential for eye comfort

### Code

- Inline: `bg-muted`, rounded `4px`, `0.875em`
- Blocks: `bg-muted`, `border-radius: var(--radius)`, padding `1.1em 1.3em`
- Font: `font-mono`

### Dark Mode Adjustments

| Element | Light | Dark |
|---------|-------|------|
| Background | `#fafaf9` warm paper | `#020617` deep slate |
| Cards | `#ffffff` | `#0a0f1d` slightly lifted |
| Text | `#1c1917` | `#f1f5f9` |
| Muted text | `#78716c` | `#94a3b8` |
| Borders | `#e7e5e4` | `#1e293b` |
| Primary | `#4f46e5` | `#6366f1` brighter |
| Images | natural | brightness(0.88) |

## Animation & Motion

### Principles

- **Fast**: Most transitions complete in `150ms` to `250ms`
- **Purposeful**: Animations guide attention or confirm actions, never decorate
- **Respectful**: Honor `prefers-reduced-motion`

### Standard Animations

| Name | Duration | Easing | Usage |
|------|----------|--------|-------|
| `fade-in` | 250ms | ease-out | Page transitions |
| `fade-in-up` | 300ms | `cubic-bezier(0.25, 0.46, 0.45, 0.94)` | List items, cards |
| `slide-in-right` | 250ms | `cubic-bezier(0.25, 0.46, 0.45, 0.94)` | Sidebars, panels |
| `scale-in` | 200ms | `cubic-bezier(0.25, 0.46, 0.45, 0.94)` | Modals, dropdowns |

### List Stagger

Lists use staggered entrance with `35ms` increments (up to 10 items).

## Browser Extension

The extension popup shares the same color tokens and typography but adapts to the constrained popup format:

- Width: `360px`
- Padding: `16px`
- Inputs and buttons: `border-radius: 10px`
- All colors map directly to the web app tokens

## Do's and Don'ts

### ✅ Do

- Use `stone` warm grays for surfaces and `slate` for dark mode
- Keep borders subtle (`border-opacity` low)
- Use amber **only** for bookmark/star actions
- Use `accent` (warm gray) for generic hover backgrounds — never replace it with amber
- Maintain generous whitespace around reading content
- Use the typography scale consistently

### ❌ Don't

- Use pure black (`#000`) or pure white (`#fff`) anywhere
- Use amber for primary actions, hover backgrounds, or selection states
- Apply heavy shadows or glassmorphism effects
- Use different border radii within the same component hierarchy
- Animate layout properties (width, height, top, left) — use transform only

## Implementation Notes

### Tailwind v4

This design system is built for Tailwind CSS v4 with the `@theme inline` directive. Custom colors are exposed as CSS variables and mapped in `@theme inline`.

### shadcn/ui

Components are styled using shadcn/ui "New York" style as the base. Override colors by changing CSS variables in `:root` and `.dark`.

### Adding New Colors

When adding a new semantic color:

1. Define the primitive hex in `@theme inline`
2. Add HSL variables to `:root` and `.dark`
3. Map to `@theme inline` with `hsl(var(--name))`
4. Document in this file

---

*Last updated: 2026-05-16*
