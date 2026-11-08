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
- `font/get_online_fonts` (full catalog, **1-based page, 9/page**) req
  `{filter_tags:["chinese","english","Japanese","handwritten","standard","formula_symbol","scribble"],
  sort_rule:"collections", page_num:1, search_name:""}` → same `FontInfo` shape as collected.
  Omitting the tags/sort/search fields or using page 0 → HTTP 400.
- Listing item fields (verified 2026-11-07): `id, font_name, character_number, collection_count,
  content_preview (null so far), font_description, font_preview, permission, tags, type_name
  ("中文6900"), username, created_at`. `tags` is a JSON array **encoded as a string**
  (`"[\"中文字体\", \"手写字体\"]"`). `font_preview` is a public, no-auth PNG
  `https://chixie.oss-cn-shanghai.aliyuncs.com/font_preview/font_<id>.png`: the font name written in
  the font, black on opaque white, ~6600×2250, ~400 KB. Some ids have none (404).
- `font/get_character_access` (strokes for chosen characters) → HTTP 404 `Not Found` now; whole-font
  download is the only glyph source.
- `user/heartbeat` req `{}` → `data:"ok"`. `ai/get_ai_account` req `{ai_type}` → xunfei keys.

### Font payload format (decrypted, verified)

**Key:** Fernet key = login `backup_token` (44-char urlsafe-b64) with its **last two
characters swapped**.

**Blob base64:** the `data` string is a Fernet token with **2 junk characters appended**,
so its length mod 4 is 2.
- When the token has `=` padding, the junk follows the `=` run. Python's lenient
  `urlsafe_b64decode` stops at the padding; a strict decoder must cut at the end of the `=` run.
- When the token needs no padding (its byte length is divisible by 3), the junk sits directly
  after the data and there is no `=` at all. Fonts 4, 11, 18, 144 and 2417 are like this, which
  was found on 2026-11-07. The decoder then tries every cut that leaves a multiple of 4
  characters and lets the HMAC pick the right one.
- Code: src/font.rs `token_candidates`.
- `chixie font-check --all` downloads every collected font and reports the result.

**Fernet timestamp:** decode must ignore the token timestamp — the server clock runs ahead
of the client, so a strict future-timestamp check rejects every token. We decode Fernet
manually (HMAC-SHA256 verify + AES-128-CBC/PKCS7, no time check); src/font.rs `fernet_decrypt`.

**Glyph JSON** (`~6900` chars per CJK font):
```
{ "<char>": [advance_width, X, Y], ... }
```
`X` and `Y` are parallel coordinate containers, NOT strokes:
- flat number arrays → one pen-down polyline: points = zip(X, Y);
- arrays-of-arrays → one stroke per inner array: stroke i = zip(X[i], Y[i]).

Coordinates are a small float grid (cell ≈ 0..7.7 units, **y downward**); `advance_width`
in the same units. Handwriting fonts (手写字体) draw a whole character as few/one continuous
stroke. A third sub-array sometimes present in older captures is unused. `mixing_ratio` /
`default_load_size` from the font listing scale/blend glyphs. Raw font JSON and live tokens
stay out of the repo.

Verified end to end: `write "你好A7"` → legible handwriting pen paths (X 0..51mm at scale 3).

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
