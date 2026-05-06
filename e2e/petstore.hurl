# Swagger Petstore 3.0 — public OpenAPI spec, exercised over the
# session-based bridge.

# open-session args schema (driven by BridgeConfig / schemars).
POST {{baseurl}}/sessions/open-args-schema
{}
HTTP 200
[Asserts]
jsonpath "$.type" == "object"
jsonpath "$.properties.spec_url" exists

# Open a session for the petstore spec. open-session pre-fetches and
# parses the spec, so connect / parse failures surface here.
POST {{baseurl}}/sessions
{"arguments": {"spec_url": "{{petstore_spec}}"}}
HTTP 201
[Asserts]
jsonpath "$.id" matches "openapi_\\d+"
[Captures]
session_id: jsonpath "$.id"

# list-tools — bridge resolves the cached spec, returns operation tools.
POST {{baseurl}}/tools
{"metadata": {"std:session-id": "{{session_id}}"}}
HTTP 200
[Asserts]
jsonpath "$.tools" count >= 10
jsonpath "$.tools[*].name" contains "find_pets_by_status"
jsonpath "$.tools[*].name" contains "get_inventory"
jsonpath "$.tools[*].name" contains "add_pet"
jsonpath "$.tools[*].name" contains "get_pet_by_id"
jsonpath "$.tools[*].name" contains "login_user"
jsonpath "$.tools[*].name" contains "place_order"
jsonpath "$.tools[*].name" contains "delete_order"

# --- GET with query parameter ---

POST {{baseurl}}/tools/find_pets_by_status
{"arguments": {"status": "sold"}, "metadata": {"std:session-id": "{{session_id}}"}}
HTTP 200
[Asserts]
jsonpath "$.content" count >= 1
jsonpath "$.content[0].mime_type" == "application/json"

POST {{baseurl}}/tools/login_user
{"arguments": {"username": "test", "password": "test"}, "metadata": {"std:session-id": "{{session_id}}"}}
HTTP 200
[Asserts]
jsonpath "$.content[0].data" contains "Logged in"

# --- POST with request body ---

POST {{baseurl}}/tools/add_pet
{"arguments": {"id": 99887, "name": "TestDog", "status": "available", "photoUrls": ["http://example.com/dog.jpg"]}, "metadata": {"std:session-id": "{{session_id}}"}}
HTTP 200
[Asserts]
jsonpath "$.content[0].data" contains "TestDog"
jsonpath "$.content[0].data" contains "99887"

# --- PUT with request body ---

POST {{baseurl}}/tools/update_pet
{"arguments": {"id": 99887, "name": "TestDogUpdated", "status": "sold", "photoUrls": ["http://example.com/dog2.jpg"]}, "metadata": {"std:session-id": "{{session_id}}"}}
HTTP 200
[Asserts]
jsonpath "$.content[0].data" contains "TestDogUpdated"
jsonpath "$.content[0].data" contains "sold"

# --- GET with path parameter ---

POST {{baseurl}}/tools/get_pet_by_id
{"arguments": {"petId": 99887}, "metadata": {"std:session-id": "{{session_id}}"}}
HTTP 200
[Asserts]
jsonpath "$.content[0].data" contains "TestDogUpdated"

# --- DELETE with path parameter ---

POST {{baseurl}}/tools/delete_pet
{"arguments": {"petId": 99887}, "metadata": {"std:session-id": "{{session_id}}"}}
HTTP 200

# --- Store: POST + GET + DELETE cycle ---

POST {{baseurl}}/tools/place_order
{"arguments": {"id": 77788, "petId": 1, "quantity": 1, "status": "placed", "complete": true}, "metadata": {"std:session-id": "{{session_id}}"}}
HTTP 200
[Asserts]
jsonpath "$.content[0].data" contains "77788"
jsonpath "$.content[0].data" contains "placed"

POST {{baseurl}}/tools/get_order_by_id
{"arguments": {"orderId": 77788}, "metadata": {"std:session-id": "{{session_id}}"}}
HTTP 200
[Asserts]
jsonpath "$.content[0].data" contains "77788"

POST {{baseurl}}/tools/delete_order
{"arguments": {"orderId": 77788}, "metadata": {"std:session-id": "{{session_id}}"}}
HTTP 200

# --- Error cases ---

# Non-existent tool returns 404 (std:not-found).
POST {{baseurl}}/tools/doesNotExist
{"arguments": {}, "metadata": {"std:session-id": "{{session_id}}"}}
HTTP 404
[Asserts]
jsonpath "$.error.kind" == "std:not-found"

# Close the session.
DELETE {{baseurl}}/sessions/{{session_id}}
HTTP 204

# After close, calls referencing the id surface std:session-not-found (404).
POST {{baseurl}}/tools/find_pets_by_status
{"arguments": {"status": "sold"}, "metadata": {"std:session-id": "{{session_id}}"}}
HTTP 404
[Asserts]
jsonpath "$.error.kind" == "std:session-not-found"
