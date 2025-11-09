# Track Geometry Experiments

A playground for visualizing and optimizing railway alignments using Bevy, featuring interactive terrain visualization, draggable control points, and alignment constraint enforcement.

> [!WARNING]  
> This project is a work in progress.
> If something is broken I probably already know about it and I may or may not work on it.

## Overview

This project uses Bevy (a modern ECS-based game engine) to create an interactive 3D visualization tool for railway/road track geometry. It integrates procedural terrain generation with an alignment constraint system, allowing users to interact with control points (pins) on a terrain surface while maintaining geometric constraints.

### Story

It all started in the summer of 2025. I was playing Transport Fever 2, but I was frustrated by the track building tools. I kept trying to find new ways to build the tracks so the curves would look nice and smooth, but I could never get them just right. It was from this frustration that I went down the rabbit hole of curves and track geometry.

## Building and Running

### Development Mode

During development, use the following flags to enable dynamic linking (faster compile times) and Bevy's development tools:

**Build:**

```bash
cargo build --features bevy/dynamic_linking,bevy/bevy_dev_tools
```

**Run:**

```bash
RUST_BACKTRACE=1 RUST_LOG='bevy=info,track_geometry=debug' cargo run --features bevy/dynamic_linking,bevy/bevy_dev_tools
```

The `-F bevy/dynamic_linking` flag enables dynamic linking for faster iteration during development. The `-F bevy/bevy_dev_tools` flag enables Bevy's development tools. `RUST_BACKTRACE=1` enables full backtraces on panic, and `RUST_LOG=info` sets the logging level.

### Release Mode

For optimized builds:

```bash
cargo build --release
cargo run --release
```

### Key Features

#### Terrain Visualization

- Procedural terrain generation using `bevy_procedural_terrain_gen`
- Contour line shader for height visualization (In progress)
- Adjustable contour line settings (interval, color, thickness)
- Terrain settings persistence (saves to `terrain_settings.json`)

#### Alignment editing

- Drag and drop control points to edit the alignment
- Add and remove control points
- Move control points along the alignment
- Delete control points
- Undo and redo
- Save and load alignments

#### Camera Controls

- Pan/orbit camera using `bevy_panorbit_camera`
- Toggle between perspective and orthographic views (press `T`)
- Smooth transitions between camera modes
- Wireframe mode toggle (press `Space`)
