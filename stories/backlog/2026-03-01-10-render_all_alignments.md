# US-10: Render All Alignments

As a user, I want to see all alignments I've placed on the map at once, not just the currently selected one, so that I can visualize my entire track network.

## Current Behavior
- Only one alignment is rendered at a time (the `current_alignment`)
- When creating a new alignment, the previous one disappears visually

## Expected Behavior
- All alignments should be rendered simultaneously
- The current/selected alignment could be highlighted differently
- Selecting an alignment should just change which one is editable, not which one is visible
