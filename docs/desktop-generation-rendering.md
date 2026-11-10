# Generation and live preview on the desktop

## Goal

Each interactive request generates one floor and shows how it is built on the desktop. Generation stays off the UI thread, which receives small geometry events so it can update the map without waiting for the final result.

## Implemented flow

`stage_with_progress` forwards events for each attempt: attempt started, grid ready, rooms created, and corridor segments added or changed. The generator publishes the topology changes the algorithm chooses. Simulations used only to compute the maximum β do not appear in the preview.

The desktop receives these events over an mpsc channel; the worker thread calls `egui::Context::request_repaint` after each batch. A `work_id` discards late messages from an earlier request. When validation asks for a new attempt, the preview is cleared and shows that attempt's construction. At the end, the validated map replaces the partial state and enables inspection, overlays, pan and zoom.

The preview is drawn with egui's `Painter`. A fast generation (under 120 ms) swaps the map in one step, without flicker; a slow one shows the floor being built. During generation it shows the grid, rooms and connections; after validation it also draws doors, junctions, labels and the selected overlays. Pan, zoom and theme update the drawing directly in the interface, without serializing SVG.

## Responsibilities

- **Generator:** runs on a worker thread and publishes geometry changes while building each attempt.
- **Interface:** applies the events of the current attempt and redraws the map; the main thread stays available for interaction.
- **Map:** draws the current state of the map; the map computation still runs on the CPU, in the generator.
- **Validation:** decides which attempt becomes the final map. Drawings of discarded attempts are replaced when the next attempt starts.

## Measurements

The panel records the total time of `stage`, including attempts and validation. Drawing cost happens during construction and is not added to the generator time. To compare algorithm performance, use the duration of `stage`; to judge the experience, check that the interface shows partial updates and keeps responding during larger generations.

## Criteria

- An interactive request generates a single floor, with internal validation attempts when needed.
- The interface receives room and corridor events before the final result.
- The map shows connections as the map graph is built.
- A new attempt clears the events of the previous attempt.
- Messages from old requests do not replace the current map.
- The validated result replaces the partial preview and offers all existing inspections and overlays.
- The multi-map benchmark stays separate from the interactive single-floor path.
