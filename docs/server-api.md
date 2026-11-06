# ChiXie3 backend API (recovered from client)

Source: Nuitka constant pools (`core.Constant`, `core.Util.NetworkUtil`, `core.User.LoginDialog`,
`core.Util.FUtil`). Names/paths are exact; request/response *shapes* are inferred — **VERIFY**
against backend docs or a traffic capture before relying on them.

## Transport (`core.Util.NetworkUtil.Network`)

- Base: env `ServerUrl` / `WebsiteUrl` (default `https://chixie.com.cn/`) + `client_api/` + path.
- `requests` get/post, headers carry `token` (from login). Query built as `{}={}` pairs.
- Separate `post_for_chixie_esp32` for device HTTP.

## Auth

`POST client_api/user/login` — fields `account`, `password`, `platform` (`Constant.Platform` = `windows`).

Response fields read by `LoginDialog.save_gv_data`:
`token`, `backup_token`, `username`, `email`, `phone`, `created_at`, `role_code`,
`async_operation`, `upgrade_date`, `period_of_validity`, `expiry_date`.

Roles (`UserRoleCode`): `ordinary_user`, `vip_user`, `super_vip_user`, `super_user`, `writing_robot_seller`.
Also `user/logout`, `user/heartbeat` (periodic, `HeartbeatInterval`), `user/collect_logout_info`,
`user/collect_missing_characters`.

Client stores `[LoginConfig] Account/Password/RememberPassword/AutomaticLogin` in `config.ini`.

## Verified by capture (2026-11-06, own account)

All POST, JSON bodies, header `token: <access_token>`, `Content-Type: application/json`.
Base host `https://chixie.com.cn/client_api/`. Envelope: `{status, msg, data, pagination}`.

- `user/login` req `{account, password, platform:"windows"}` →
  `data {token, access_token, refresh_token, expires_in, backup_token, username, phone,
  role_code, role, permissions:[draw,laser,write,mobile_app], expiry_date, period_of_validity, async_operation}`.
- `update/check_update` req `{major_version, minor_version, fixed_version, alphabet_version, soft_type}`.
- `font/get_collected_fonts` req `{page_num}` → `data:[{id, font_name, character_number,
  default_load_size, mixing_ratio, type_name, tags, font_preview, …}]`.
- `font/get_font_access` req `{font_id, token}` → `data`: a **Fernet token string** holding the
  whole font (one call per font; 6–13 MB decrypted).
- `user/heartbeat` req `{}` → `data:"ok"`. `ai/get_ai_account` req `{ai_type}` → xunfei keys.

### Font payload format (decrypted)

Fernet key = login `backup_token` (44-char urlsafe-b64) **with its last two characters
swapped** (`data.backup_token`). `Fernet(key).decrypt(data)` → UTF-8 JSON:

```
{ "<char>": [advance_width, stroke, stroke, ...], ... }   # ~6900 chars per CJK font
stroke = [[x0,x1,...], [y0,y1,...]]    # polyline, one pen-down pass
```

Coordinates are a small float grid (cell ≈ 0..7 units, y downward); `advance_width` in the
same units. Pen goes down at a stroke's first point, up at its last. `mixing_ratio` /
`default_load_size` from the font listing scale and blend glyphs. Captured raw font JSON
(user's licensed data, with a live session token) stays out of the repo.

## Fonts (endpoint names)

| Path | Use |
|---|---|
| `font/get_main_fonts_info` | default fonts (`MainFont`, `MainEnglishFont`, `MainNumberFont`, `MainSymbolFont` IDs) |
| `font/get_online_fonts`, `font/get_collected_fonts`, `font/get_recorded_fonts` | listings (`page_num`) |
| `font/collect_a_font`, `font/get_daily_free_font`, `font/get_font_access` | access |
| `font/get_character_access` (`GetCharacterPath`) | **glyph strokes** for `font_id` + `characters` |
| `font/download_font` | whole-font download |
| `font/get_standard_br` | ? |
| `config/font_config` | get/set user font config (`CharacterDeformation`, `deform_range`) |

Glyph payload (`FUtil.DownloadFont`): response → `Fernet(BackupToken).decrypt()` → `json.loads`
→ `{char: route}` → `add_text_path_to_font`. Route geometry format **VERIFY**.

Local templates (`.cx`) are pickled then Fernet-encrypted with a client constant `TEMPLATE_KEY`.

## Other

`update/check_update`, `update/compare_file_differences`, `pay/*`, `invitation/*`,
`config/query_exclusive_config_file`, `config/query_config_download_url`,
`config/download_exclusive_config_file`, `file_parsing/dxf`,
`file_parsing/image-to-table-template[-2]`, `text_processing/text_block`, `ai/get_ai_account`,
`font/admin_tune_fonts`.
