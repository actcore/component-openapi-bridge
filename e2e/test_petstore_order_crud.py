"""Store: POST + GET + DELETE order cycle, same session-based bridge as
test_petstore_pet_crud.py. Orders are an independent resource from pets, so
this gets its own session rather than sharing test_petstore_pet_crud.py's.
"""


async def test_order_crud(client, session_meta):
    placed = await client.call_tool("place_order", {
        "id": 77788, "petId": 1, "quantity": 1, "status": "placed", "complete": True,
        "_meta": session_meta,
    })
    assert "77788" in placed.content[0].text
    assert "placed" in placed.content[0].text

    got = await client.call_tool("get_order_by_id", {"orderId": 77788, "_meta": session_meta})
    assert "77788" in got.content[0].text

    await client.call_tool("delete_order", {"orderId": 77788, "_meta": session_meta})
