---
name: A3S Test
description: A visible evidence path for autonomous interface testing.
colors:
  control-blue: "#1456f0"
  control-blue-deep: "#0f46c7"
  control-blue-soft: "#e8efff"
  canvas: "#f5f7fb"
  paper: "#eaf0f8"
  panel: "#ffffff"
  panel-strong: "#0d1728"
  line: "#dce3ec"
  line-strong: "#cbd4e0"
  ink: "#142033"
  slate: "#526078"
  faint: "#748095"
  evidence-green: "#16845b"
  review-violet: "#5b57d9"
typography:
  display:
    fontFamily: "Geist Variable, Avenir Next, HarmonyOS Sans SC, PingFang SC, Microsoft YaHei, ui-sans-serif, system-ui, sans-serif"
    fontSize: "clamp(50px, 5.4vw, 76px)"
    fontWeight: 740
    lineHeight: 1.01
    letterSpacing: "-0.04em"
  headline:
    fontFamily: "Geist Variable, Avenir Next, HarmonyOS Sans SC, PingFang SC, Microsoft YaHei, ui-sans-serif, system-ui, sans-serif"
    fontSize: "clamp(38px, 4vw, 56px)"
    fontWeight: 720
    lineHeight: 1.06
    letterSpacing: "-0.035em"
  title:
    fontFamily: "Geist Variable, Avenir Next, HarmonyOS Sans SC, PingFang SC, Microsoft YaHei, ui-sans-serif, system-ui, sans-serif"
    fontSize: "20px"
    fontWeight: 720
    lineHeight: 1.25
    letterSpacing: "-0.025em"
  body:
    fontFamily: "Geist Variable, Avenir Next, HarmonyOS Sans SC, PingFang SC, Microsoft YaHei, ui-sans-serif, system-ui, sans-serif"
    fontSize: "16px"
    fontWeight: 400
    lineHeight: 1.7
  label:
    fontFamily: "Geist Mono Variable, SFMono-Regular, Cascadia Code, Menlo, Consolas, monospace"
    fontSize: "11px"
    fontWeight: 700
    lineHeight: 1.4
    letterSpacing: "0.04em"
rounded:
  control: "9px"
  row: "10px"
  surface: "14px"
  shell: "18px"
  pill: "999px"
spacing:
  xs: "8px"
  sm: "12px"
  md: "16px"
  lg: "24px"
  xl: "32px"
  section: "96px"
components:
  button-primary:
    backgroundColor: "{colors.control-blue}"
    textColor: "{colors.panel}"
    rounded: "{rounded.control}"
    padding: "0 20px"
    height: "48px"
  button-secondary:
    backgroundColor: "{colors.panel}"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    padding: "0 20px"
    height: "48px"
  surface:
    backgroundColor: "{colors.panel}"
    textColor: "{colors.ink}"
    rounded: "{rounded.surface}"
    padding: "24px"
  code-field:
    backgroundColor: "{colors.panel-strong}"
    textColor: "{colors.panel}"
    rounded: "{rounded.control}"
    padding: "18px"
---

# Design System: A3S Test

## Overview

**Creative North Star: "The Visible Evidence Path"**

A3S Test inherits the current A3S Cloud visual language and turns it toward interface understanding. A bright cool canvas, one electric-blue field, precise product UI, and a visible path from rendered page to typed action to evidence make the testing mechanism understandable before the visitor reads implementation detail.

The site is confident and technical without becoming a generic dark developer-tool page. Real Test Kit behavior is the first proof. Documentation stays quiet, light, and highly readable while the homepage uses larger fields and product-scale interface demonstrations.

**Key Characteristics:**

- White and pale-blue operating surfaces with one decisive electric-blue field per viewport.
- A real Test Kit-powered page experience as the primary visual proof.
- Cool hairlines, compact controls, restrained blue-tinted elevation, and generous section spacing.
- Product prose in the humanist sans; monospace only for commands, identifiers, coordinates, and evidence.
- Complete Chinese and English layouts with structural mobile reflow.

## Colors

The palette uses A3S Control Blue for action and wayfinding, cool neutrals for documentation, Evidence Green for verified outcomes, and Review Violet for human authorization.

**The One Blue Field Rule.** One major blue region may carry a viewport. Everywhere else, blue is reserved for actions, selection, and wayfinding.

**The Semantic Color Rule.** Green and violet communicate evidence and review authority; they do not decorate neutral content.

## Typography

**Display Font:** Geist Variable with Chinese and system sans fallbacks

**Body Font:** Geist Variable with Chinese and system sans fallbacks

**Label/Mono Font:** Geist Mono Variable with platform monospace fallbacks

**Character:** One compact humanist family keeps the A3S family resemblance. Weight and scale establish authority; monospace appears only where the product exposes machine-readable truth.

- **Display:** Product name and first-viewport promise only.
- **Headline:** Major narrative transitions and final calls to action.
- **Title:** Interface modules, documentation sections, and capability headings.
- **Body:** Explanations held near 65 to 75 characters per line.
- **Label:** Commands, node refs, revisions, coordinates, and evidence identifiers.

**The Data Voice Rule.** Monospace communicates data and protocol state, never general product personality.

## Layout

The site uses a 72px navigation bar and a 1440px maximum product canvas. The first viewport is an evidence runway: the product promise and actions lead into a real Test Kit surface, live context, and review state. Major sections alternate between quiet white explanation and one stronger blue or pale-blue operating field.

At 1100px, wide evidence modules compact. At 900px, two-column scenes stack. At 768px, navigation becomes a modal, touch targets remain at least 44px, and every evidence or install control uses the full width. Hidden drawers and sidebars leave both the accessibility tree and keyboard order.

## Elevation & Depth

The system is near-flat. Cool hairlines establish most structure. Soft blue-tinted shadows are reserved for the interactive surface shell, the Test Kit overlay, menus, and other elements that genuinely sit above the page.

**The Hairline Before Shadow Rule.** Structural regions use a cool boundary; only floating or overlapping interfaces receive visible elevation.

## Shapes

Controls use 8px to 9px corners, rows use 10px, primary surfaces use 12px to 14px, and interface shells may use 18px. Pills are limited to compact status and version controls. The recurring silhouette is a rectangular product canvas with one rounded blue field, not a wall of floating cards.

## Components

### Buttons

- Primary actions use Control Blue, white text, a 48px target, and a 9px corner.
- Secondary actions use a white field, cool border, and the same footprint.
- Hover lifts by no more than 2px; focus uses a visible blue ring; active translates by 1px.

### Navigation

Navigation uses a translucent white bar, compact labels, a small A3S mark, and blue active state. Language and version selectors remain directly reachable on desktop and inside the mobile navigation dialog.

### Live Surface Frame

The signature surface shows a plausible product page wrapped by `A3STestKit` and `A3STestBoundary`. It exposes current revision, semantic target, geometry, and evidence state, then enables the real `A3SReviewOverlay` on explicit user request. Demo submissions remain local and say so.

### Install Field

macOS, Linux, and Windows share one dark command field with accessible tabs, visible package version, a copy result state, and version-pinned commands on historical routes.

## Do's and Don'ts

### Do:

- **Do** demonstrate rendered semantics, revision binding, coordinates, human review, and evidence with real product behavior.
- **Do** keep Chinese and English content, routes, labels, and accessible names in parity.
- **Do** reserve the strongest visual field for the page's current proof or action.
- **Do** keep documentation reading surfaces quiet and code samples on one approved dark field.

### Don't:

- **Don't** recreate the former dark-green homepage inside the new A3S family system.
- **Don't** invent repair success, customer evidence, benchmark results, or unsupported platform availability.
- **Don't** use hidden off-canvas navigation that remains keyboard reachable.
- **Don't** replace the real Test Kit experience with a static illustration or fake overlay.
