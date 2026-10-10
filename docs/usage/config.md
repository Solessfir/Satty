---
icon: lucide/monitor-cog
---

This is an example config. Please note, we're not pointing out defaults everywhere yet, it is a WIP.

The fork also supports `initial-color` (one-based palette number) and `copy-on-save` (`none`, `image` or `file-path`) under `[general]`. Invalid palette numbers fall back to the first color. These options are configured in TOML.

Saving creates missing output directories and copies the successfully saved result before any configured early exit. Cancelled or failed saves skip `copy-on-save`. `copy-on-save = "file-path"` copies the resolved saved path, while `image` copies the edited PNG. This also applies to Save As.

Ctrl+Shift+C exports the current edited image to a unique private directory under `XDG_RUNTIME_DIR` (or the system temporary directory when unset) and copies the PNG path. This follows `early-exit = ["copy"]`; failed exports leave the editor open. The file remains after Satty exits, and the session runtime directory is cleared on reboot. Ctrl+C still copies the image, and Ctrl+Alt+C still copies the last saved file's path. Rebind the export action using `"<Shift><Control>c" = "copy-image-path-to-clipboard"` under `[keybinds]`, or use it in configurable action lists.

```
--8<-- "config.toml"
```
