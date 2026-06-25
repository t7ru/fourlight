# fourlight

A neat little utility zooms with a spotlight, it is inspired by [ZoomIt](https://learn.microsoft.com/en-us/sysinternals/downloads/zoomit), and that one FNaF game, dehehe.

Currently, it only supports Windows.

## Build

```sh
git clone https://github.com/t7ru/fourlight.git
cd fourlight
cargo build --release
```

Run `target/release/fourlight.exe`. The default hotkey is **Ctrl + Alt + Q**, you can configure it in the settings through the tray icon.

## Controls

| Input              | Action                                          |
| ------------------ | ----------------------------------------------- |
| Hotkey             | Toggle zoom on / off                            |
| Mouse wheel        | Zoom in / out                                   |
| F                  | Toggle the flashlight spotlight                 |
| Ctrl + mouse wheel | Adjust spotlight radius (when flashlight is on) |

## Settings

Right-click the tray icon -> Settings.

- **Hotkey**: click the field and press the key combination you want
- **Default zoom**: starting zoom level when you activate
- **Flashlight**: spotlight on zoom, radius, and shadow strength
- **Virtual display for OBS**: see [here](#OBS)

Settings are saved to `%APPDATA%\fourlight\config.toml`.

## OBS

The overlay is excluded from screen capture, so it stays local to you and does not show up in recordings by default.

To record with the overlay, enable **Virtual display for OBS** in settings, then add a **Window Capture** source in OBS and pick the window named **fourlight OBS output**.

Effectively, this virtual display is a direct replacement for Display Capture.

## License

[MIT](./LICENSE)
