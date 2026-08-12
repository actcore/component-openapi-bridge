import json


async def test_unknown_operation_is_not_found(client, session_meta, expect_error):
    """A tool name with no matching operation in the spec is a not-found
    caller error, not an internal one.
    """
    await expect_error(client, "doesNotExist", {"_meta": session_meta}, "std:not-found")


async def test_call_after_close_is_session_not_found(client, petstore_spec_url, expect_error):
    """After close, calls referencing the id surface std:session-not-found.

    Opens and closes its own session directly (not the `session` fixture,
    which closes on teardown) so the close happens on the test's own
    schedule, before the assertion it's testing for.
    """
    opened = await client.call_tool("open_session", {"spec_url": petstore_spec_url})
    sid = json.loads(opened.content[0].text)["id"]

    await client.call_tool("close_session", {"session_id": sid})

    await expect_error(
        client, "find_pets_by_status",
        {"status": "sold", "_meta": {"std:session-id": sid}},
        "std:session-not-found",
    )
