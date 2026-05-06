---
name: openapi-bridge
description: Dynamically exposes OpenAPI endpoints as ACT tools
metadata:
  act: {}
---

# openapi-bridge

Loads an OpenAPI 3.x spec at runtime and exposes each operation as a
local ACT tool. Path/query/header parameters and JSON request bodies
are flattened into a single tool argument schema.

## How sessions work here

This component requires a session. Open one against the API you want
to expose, then thread the returned id into every tool call as
`std:session-id` metadata.

Open-session args:

| field | type | required | description |
| --- | --- | --- | --- |
| `spec_url` | string | yes | URL of the OpenAPI spec (JSON or YAML) |
| `headers` | object | no | Default headers to send with every request — typically auth |

`open-session` fetches and parses the spec eagerly so `list-tools` is
cheap and bad URLs / unparseable specs surface at open time.
`close-session` drops the session; the parsed spec stays cached
across sessions targeting the same `spec_url`.

Without `std:session-id`, `list-tools` returns an empty list and
`call-tool` errors with `std:invalid-args`. Calls referencing a
closed session-id return `std:session-not-found` (HTTP 404).

## Per-call header overrides

Callers can pass extra headers per call by including
`x-act-header-<name>` keys in the metadata. They merge on top of the
session-level `headers`.

## Example

```text
open_session({"spec_url": "https://petstore3.swagger.io/api/v3/openapi.json"})
→ {"id": "openapi_0", "metadata": {}}

list_tools(_meta = {std:session-id: "openapi_0"})
→ [find_pets_by_status, get_pet_by_id, add_pet, ...]

call_tool("find_pets_by_status", {"status": "sold"},
          _meta = {std:session-id: "openapi_0"})
→ JSON array of pets

close_session("openapi_0")
```

## Tool naming

If an operation has `operationId`, it's snake_cased. Otherwise the
bridge synthesises `<method>_<path>`, snake_casing each segment and
prefixing path-parameter segments with `by_` (e.g.
`DELETE /pets/{petId}` → `delete_pets_by_pet_id`).

## Limitations

- One spec per session; a single bridge can host many sessions.
- 30-second per-request timeout, 10 MB response cap.
- `Content-Type: application/json` request bodies only.
