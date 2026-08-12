"""The open-session args schema is driven by `BridgeConfig` (schemars).

Over MCP there is no dedicated `/sessions/open-args-schema` endpoint; the
same schema is what the adapter publishes as the virtual `open_session`
tool's `inputSchema` (ACT-MCP §4.1, `get-open-session-args-schema`).
"""


async def test_open_session_args_schema(client):
    tools = await client.list_tools()
    open_tool = next(t for t in tools if t.name == "open_session")
    assert open_tool.inputSchema["type"] == "object"
    assert "spec_url" in open_tool.inputSchema["properties"]
