# Privacy policy

BrightPause is a break reminder for Windows. It collects nothing about you and sends nothing anywhere. This policy covers the Microsoft Store version and the version built from this repository.

## What stays on your PC

BrightPause stores these files on your PC only:

- `settings.json`: your choices (break times, sound, theme, language, shortcuts).
- `history.jsonl`: one line per break with its time, kind and whether you took, postponed or skipped it. This feeds the "today" and week counts in settings.
- `events.log`: one line each time a break starts, waits or ends, with the reason (for example a full-screen app, and that app's file name). It helps explain a missed or late break. It stays on your PC, and BrightPause keeps it under about 0.5 MB.
- Your background picture, if you choose one.

The version from this repository keeps them in `%APPDATA%\io.github.jordanalbayrak.brightpause\`. The Store version keeps them in its own folder under `%LOCALAPPDATA%\Packages\`, and uninstalling it deletes them. You can delete either folder at any time.

## Microphone

With "Wait during calls" on, BrightPause asks Windows whether another app is recording from a microphone, so a break does not start in the middle of a call. It never records, opens or listens to the microphone itself, and it learns nothing about the other app except that it is recording.

## Network

- The Microsoft Store version makes no network connections. The Store delivers its updates.
- The version from this repository connects to GitHub only to check for updates, when you press "Check for updates" or turn on automatic updates (off by default).
- A source link on a break screen opens in your default browser when you click it.

There are no accounts, ads, analytics, crash reports or telemetry, and nothing is shared with anyone.

## Children

BrightPause collects no personal information from anyone, including children.

## Changes and contact

Changes to this policy are made in this file, and its history shows every version. Questions: open an issue at https://github.com/JordanAlbayrak/BrightPause/issues.

Last updated: 19 November 2026.
