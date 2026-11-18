# Security

Please report security problems privately, through GitHub's **Report a vulnerability** button on the Security tab of this repository, not in a public issue.

Include what you found, how to reproduce it, and which version you used. You will get a reply within a week.

BrightPause has no server and no accounts. The parts most worth a look are the update check (`src-tauri/src/update.rs`), which verifies each update against the public key in `tauri.conf.json`, and the commands the windows can call (`src-tauri/src/lib.rs`).
