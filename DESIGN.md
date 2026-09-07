# Siegu Design System Specification

This document serves as the absolute source of truth for the visual style of interactive elements in the Siegu application.

## 1. Core Palette

- **Base Background:** `#fafafa` (Standard Light Theme)
- **Interactive Background (Buttons/Fields):** `#000000` (Pure Black)
- **Interactive Text:** `#ffffff` (Pure White)
- **Icon Container:** Grey Circle (using Zinc-800 or similar)
- **Secondary Text:** `#52525b` (Zinc-600)

## 2. Global Button Specification (`.siegu-btn`)

Every button in the app (Primary, Secondary, and Destructive) must follow the same visual language:

- **Background:** `#000000` !important
- **Text Color:** `#ffffff` !important
- **Border:** None
- **Border Radius:** `16px`
- **Hover Effect:** `transform: scale(1.02) translateY(-1px)`
- **Active Effect:** `transform: scale(0.98)`
- **Transition:** `all 0.2s cubic-bezier(0.4, 0, 0.2, 1)`

## 3. Icon Treatment & Secondary Actions

- **Icons:** Every button should ideally contain an icon centered within a circular background (`rgba(255, 255, 255, 0.15)`).
- **Secondary Buttons:** Actions like "Cancel", "Close", or "Back" must no longer use transparent or grey styles. They must be black buttons.
- **Destructive Buttons:** Actions like "Delete" or "Remove" must use the black button style. Error/Red text should be used sparingly for confirmation text, but not the button background itself.
- **Structure:** Follow the onboarding "Sync" / "Join" pattern: [Icon Circle] + [Text Label].

## 4. Form Elements (Selects, Checkboxes, Switches)

- **Select Boxes / Fields:** Must use the black background with white text.
- **Checkboxes/Switches:** Must use black as the primary active color with white accents.
- **Border Radius:** Standardized at `16px` for consistency with buttons.

## 5. Animation Principles

- All interactions must have immediate tactile feedback (scaling).
- Transitions must be smooth (`0.2s`) and use standard easing.
