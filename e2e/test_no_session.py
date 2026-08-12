"""Without std:session-id, list-tools returns an empty list from the guest's
own perspective, and call-tool errors with std:invalid-args — the original
hurl file's contract, exercised over ACT-HTTP.
"""


async def test_no_session_list_tools_shows_only_virtual_tools(client):
    """The original hurl asserted `$.tools count == 0` over ACT-HTTP, which
    has no synthesised session tools. Measured (not assumed) over MCP: the
    guest's own `list-tools` still returns nothing (src/lib.rs,
    `extract_session_id` -> None -> `tools: []`), but the adapter
    additionally synthesises `open_session`/`close_session` for any
    session-provider component (ACT-MCP §4.1) independent of session state
    — so the true count here is 2, not 0.
    """
    tools = await client.list_tools()
    names = {t.name for t in tools}
    assert names == {"open_session", "close_session"}


async def test_no_session_call_is_invalid_args(client):
    result = await client.call_tool("anything", {}, raise_on_error=False)
    assert result.is_error
    assert result.meta.get("dev.actcore/error-kind") == "std:invalid-args"
    assert "std:session-id" in result.content[0].text
