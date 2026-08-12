"""Documents a measured MCP-transport limitation, not a ported assertion.

The original petstore.hurl, after opening a session, asserted
`$.tools count >= 10` plus 7 `contains` checks (find_pets_by_status,
get_inventory, add_pet, get_pet_by_id, login_user, place_order,
delete_order) against `POST /tools {"metadata": {"std:session-id": sid}}` —
ACT-HTTP lets a caller pass per-request metadata straight into `list-tools`.

MCP's `tools/list` has no equivalent channel: unlike `tools/call`, there is
no `arguments` object to inject a `_meta` property into (ACT-MCP §3.2 is
explicitly a `tools/call`-only mechanism), and the transport `_meta` field
(§3.1) is read for `call_tool` but explicitly discarded for `list_tools`
(act-cli/src/rmcp_bridge.rs, `list_tools`: `let _ = context;`). So there is
no way, over MCP, for a client of any library to make `list-tools` reflect a
particular session's resolved operations — confirmed by reading the adapter,
not inferred from a client-side symptom.

This is a `tools/list`-specific gap, not a functional one: every
petstore-derived tool in test_petstore_pet_crud.py / _order_crud.py works
correctly despite never appearing in `list_tools()`, because `call_tool`
never consults the advertised list — it resolves the operation directly from
the session's cached spec. The 8 hurl assertions above are therefore not
portable and are not ported; this test instead pins down the actual
observed behavior, so a future host change that starts honouring per-request
metadata for `list-tools` doesn't leave this file's reasoning stale.
"""


async def test_list_tools_ignores_open_session(client, session):
    tools = await client.list_tools()
    names = {t.name for t in tools}
    assert names == {"open_session", "close_session"}
