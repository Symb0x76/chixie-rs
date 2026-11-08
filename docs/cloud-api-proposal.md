# chixie-rs cloud API proposal (templates and parameter sync)

Status: proposal for the backend team, 2026-11-08. Background: ChiXie3 has no working template
library backend, and its only sync call is `config/font_config` (4 font ids + 2 numbers, see
[server-api.md](server-api.md)). chixie-rs needs a template library and a full settings profile.
The plan is in [reimplementation-plan.md](reimplementation-plan.md) (M7 templates, M10 sync).

Conventions follow the existing API: base `https://chixie.com.cn/client_api/`, JSON, envelope
`{status, msg, data, pagination}`, auth header `token: <access_token>` from `user/login`.
New paths live under `chixie-rs/` so they can't collide with ChiXie3. All content is stored in
common formats (decision: no home-made formats): templates as **SVG**, the profile as JSON that
mirrors the client's **TOML** profile.

## Templates

A template is one SVG file (page drawn at real size in mm; template parameters in `data-*`
attributes on the root element). The server treats it as an opaque file plus listing metadata.

| Call | Request | Response `data` |
|---|---|---|
| `GET chixie-rs/templates` | query: `category` (`write` / `draw` / `laser`), `tag` (人物角色 … 其他模板, optional), `sort` (`newest` / `favorites` / `likes`), `q` (search), `scope` (`online` / `mine` / `favorites`), `page` (1-based), `per_page` (default 9) | `[{id, name, category, tags[], kind (note / grid / table / drawing), paper_mm:[w,h], author, favorites, likes, is_favorite, is_liked, updated_at, sha256, size}]`, with `pagination {page, per_page, total}` |
| `GET chixie-rs/templates/{id}/file` | — | the SVG (`image/svg+xml`), `ETag` = sha256 |
| `POST chixie-rs/templates` | multipart: `file` (SVG), `name`, `category`, `tags` (JSON array) | the new listing item (`scope=mine`) |
| `PUT chixie-rs/templates/{id}` | same as POST, owner only | the updated item |
| `DELETE chixie-rs/templates/{id}` | owner only | `null` |
| `POST` / `DELETE chixie-rs/templates/{id}/favorite` | — | `{favorites, is_favorite}` |
| `POST` / `DELETE chixie-rs/templates/{id}/like` | — | `{likes, is_liked}` |

Notes:
- Previews are rendered by the client from the SVG, so no thumbnail service is needed.
- `tags` is a real JSON array (not a JSON string, unlike the font listing).
- Server-side validation: SVG ≤ 2 MB, no scripts or external references.

## Parameter sync (profile)

The profile is the client's settings that should follow the user between machines: font roles,
handwriting style, deformation, page defaults, active template id. Device profiles stay local.

| Call | Request | Response `data` |
|---|---|---|
| `GET chixie-rs/profile` | — | `{revision, updated_at, profile}` (`profile` = JSON object, schema versioned by `profile.version`), or `null` if never saved |
| `PUT chixie-rs/profile` | `{base_revision, profile}` | `{revision, updated_at}`; **409** with the current `{revision, updated_at, profile}` if `base_revision` is stale |

- Conflict handling is in the client (使用云端 / 使用本机 / 查看差异), driven by the 409.
- For ChiXie3 compatibility the client also keeps `config/font_config` in step: it writes the four
  default-font ids and the deformation level whenever they change (body shape still to be
  confirmed; see server-api.md).

## Fonts

Unchanged: the existing font endpoints (`font/get_online_fonts`, `font/get_collected_fonts`,
`font/collect_a_font`, `font/get_font_access`). The client converts downloaded fonts to SVG fonts
locally.
