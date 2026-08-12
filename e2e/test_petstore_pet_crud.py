"""Swagger Petstore 3.0 — public OpenAPI spec, exercised over the
session-based bridge. Pet resource lifecycle: find, login, add, update,
get-by-id, delete. One session backs the whole flow (`session`/
`session_meta` fixtures — see conftest.py), matching the original hurl
file's single `session_id` reused across every request.
"""

import re


async def test_pet_crud_and_login(client, session, session_meta):
    # Session ids are guest-generated (`alloc_session_id`, src/lib.rs):
    # `format!("openapi_{id}")`. hurl's `matches` is an unanchored search,
    # not a full match (verified against hurl 8.0.1) — re.search here, kept
    # for consistency even though this particular pattern would also pass
    # under fullmatch.
    assert re.search(r"openapi_\d+", session)

    # --- GET with query parameter ---
    found = await client.call_tool(
        "find_pets_by_status", {"status": "sold", "_meta": session_meta},
    )
    assert len(found.content) >= 1
    assert found.content[0].meta["dev.actcore/mime-type"] == "application/json"

    login = await client.call_tool(
        "login_user", {"username": "test", "password": "test", "_meta": session_meta},
    )
    assert "Logged in" in login.content[0].text

    # --- POST with request body ---
    added = await client.call_tool("add_pet", {
        "id": 99887, "name": "TestDog", "status": "available",
        "photoUrls": ["http://example.com/dog.jpg"],
        "_meta": session_meta,
    })
    assert "TestDog" in added.content[0].text
    assert "99887" in added.content[0].text

    # --- PUT with request body ---
    updated = await client.call_tool("update_pet", {
        "id": 99887, "name": "TestDogUpdated", "status": "sold",
        "photoUrls": ["http://example.com/dog2.jpg"],
        "_meta": session_meta,
    })
    assert "TestDogUpdated" in updated.content[0].text
    assert "sold" in updated.content[0].text

    # --- GET with path parameter ---
    got = await client.call_tool("get_pet_by_id", {"petId": 99887, "_meta": session_meta})
    assert "TestDogUpdated" in got.content[0].text

    # --- DELETE with path parameter ---
    await client.call_tool("delete_pet", {"petId": 99887, "_meta": session_meta})
