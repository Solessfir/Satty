---
icon: lucide/monitor-cog
---

This is an example config. Please note, we're not pointing out defaults everywhere yet, it is a WIP.

The fork also supports `scroll-image-navigation` under `[general]`. It is configured in TOML.

With wheel navigation enabled, wheel down opens the next supported image and wheel up opens the previous one in filename order. The `next-image` and `previous-image` keybind commands do the same from the keyboard, for example `"Right" = "next-image"` under `[keybinds]`; they are unbound by default. Navigation stops at the folder boundaries, preserves the current image on a load failure, and asks before discarding annotations. Ctrl+wheel still zooms, and touchpad scrolling still pans. Standard input and clipboard images keep normal scrolling.

Saving creates missing output directories.

Ctrl+Shift+C exports the current edited image to a unique private directory under `XDG_RUNTIME_DIR` (or the system temporary directory when unset) and copies the PNG path. This follows `early-exit = ["copy"]`; failed exports leave the editor open. The file remains after Satty exits, and the session runtime directory is cleared on reboot. Ctrl+C still copies the image, and Ctrl+Alt+C still copies the last saved file's path. Rebind the export action using `"<Shift><Control>c" = "copy-image-path-to-clipboard"` under `[keybinds]`, or use it in configurable action lists.

```
--8<-- "config.toml"
```
