# Contributing

Thanks for helping. BrightPause is small on purpose, so the best changes keep it light, quiet and calm.

## Before you start

- **Bugs:** open an issue with the bug template. Say what you expected, what happened, your Windows version and how many screens you use.
- **Ideas:** open an issue with the feature template before writing code, so we can agree on the shape first.
- **Break tips:** a new tip needs a source anyone can open (a health body, or a peer-reviewed study). Challenges need no source but must be safe for anyone at a desk.

## Ground rules

- **No network calls** unless the user asks for them. No analytics, telemetry or crash reporting.
- **Idle stays light:** nothing new may run between breaks except the tray and the timer.
- **No copied code, art or text** from other break reminders.
- Only assets you made yourself or that are CC0 may be added, with the source noted in the pull request.

## Working on the code

    npm install
    npx tauri dev
    cargo test --manifest-path src-tauri/Cargo.toml

- `src-tauri/src/schedule.rs` is the timer logic, with no I/O, and is fully tested. Put new timing rules there, with a test.
- `src-tauri/src/lib.rs` wires the tray, windows and commands. `dim.rs` draws the side screens without a web view.
- `ui/` is plain HTML, CSS and JavaScript, with no build step and no framework.
- `tools/` holds scripts that measure memory, check network use and take screenshots of a running build.

## Pull requests

- One change per pull request, with a short description of why.
- `cargo test` passes, and you have tried the change in `npx tauri dev`.
- UI changes: add a before and after screenshot, in light and dark.

By contributing, you agree that your work is released under the MIT licence.
