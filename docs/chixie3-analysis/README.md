# ChiXie3 analysis evidence

Evidence behind [../chixie3-features.md](../chixie3-features.md), produced 2026-11-07 from ChiXie3
(炽写) `alpha.3.0.310`.

| File | What |
|---|---|
| `A_device_shell.md` | Static findings: devices, connection, firmware, jobs, G-code, 百宝箱, main window, home page, AI, upgrade, endpoints ("A @hex") |
| `B_templates_plotting.md` | Static findings: page templates, plotting editor, settings dialog ("B @hex") |
| `C_writing.md` | Static findings: writing editor, typesetting, fonts, effects, output, import/export, limits ("C @hex") |
| `ui-walk.md` | Live UI walk: exact labels, defaults, menus and dialogs ("UI") |
| `ui-labels.md` | Qt Designer form labels: every user-visible string and control name of each `ui.UI_*` form module, in form order (tag `[ui-form]`); no defaults or ranges |
| `tools/` | Scripts used to extract the evidence (below) |

`@hex` offsets point into `start.exe` of that exact build. The raw string dumps are not kept in the
repo (≈ 50 MB); regenerate them:

```
uv run python -I tools/strings.py "%LOCALAPPDATA%\Programs\ChiXie3\start.exe" start_strings.txt
uv run python -I tools/qual.py start_strings.txt classes.txt          # Class.method qualnames by class
uv run python -I tools/slice.py start_strings.txt app_strings.txt 9441b78 957b2bf   # app modules only
uv run python -I tools/ui_labels.py start_strings.txt ui-labels.md    # Designer form labels per ui.UI_* module
```

Nuitka keeps each module's constants together: identifiers, UI strings, docstrings and qualnames
(`u<Class>.<method>`). The app's own modules sit between the `colorthief` and `cryptography`
constants in this build (offsets `0x9441b78`–`0x957b2bf`). The Qt Designer forms (`ui.UI_*`) sit in a
separate constant region (around `0xa01…`–`0xa04…`), mined by `tools/ui_labels.py` into `ui-labels.md`.
Numeric literals (spin-box ranges, defaults) and combo items filled in code are still not recoverable
this way, so the live UI walk fills those in.

**Live UI walk.** ChiXie3 asks for admin rights, but it runs fine un-elevated with
`__COMPAT_LAYER=RunAsInvoker` set, so no UAC prompt is needed:

```
$env:__COMPAT_LAYER = 'RunAsInvoker'; Start-Process "$env:LOCALAPPDATA\Programs\ChiXie3\start.exe"
```

UI Automation names are empty (the widgets paint their own text), so `tools/step.ps1` clicks by
coordinates in a 1600-wide preview of the primary screen and saves a screenshot after each step. It
refuses to send input unless a ChiXie3 window is in front. Ribbon groups expand in place, which
shifts later click targets, so re-shoot the ribbon before clicking further. Screenshots are not kept
in the repo because they show account details.
