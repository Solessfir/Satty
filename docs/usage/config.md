---
icon: lucide/monitor-cog
---

--8<-- "README.md:config-search"

## Styles

--8<-- "README.md:styles"

[config.toml]: #example-config

## Example config

This is an example config. Please note, we're not pointing out defaults everywhere yet, it is a WIP.

Ctrl+Shift+C exports the current edited image to a unique private directory under `XDG_RUNTIME_DIR` (or the system temporary directory when unset) and copies the PNG path. This follows `early-exit = ["copy"]`; failed exports leave the editor open. The file remains after Satty exits, and the session runtime directory is cleared on reboot. Ctrl+C still copies the image, and Ctrl+Alt+C still copies the last saved file's path. Rebind the export action using `"<Shift><Control>c" = "copy-image-path-to-clipboard"` under `[keybinds]`, or use it in configurable action lists.

```toml title="config.toml" linenums="1"
--8<-- "config.toml"
```
