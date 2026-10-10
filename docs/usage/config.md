---
icon: lucide/monitor-cog
---

This is an example config. Please note, we're not pointing out defaults everywhere yet, it is a WIP.

The fork also supports `initial-color` (one-based palette number) and `copy-on-save` (`none`, `image` or `file-path`) under `[general]`. Invalid palette numbers fall back to the first color. These options are configured in TOML.

Saving creates missing output directories and copies the successfully saved result before any configured early exit. Cancelled or failed saves skip `copy-on-save`. `copy-on-save = "file-path"` copies the resolved saved path, while `image` copies the edited PNG. This also applies to Save As.

```
--8<-- "config.toml"
```
