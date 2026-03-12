GET {{host}}/info
HTTP 200
[Asserts]
jsonpath "$.name" == "openapi-bridge"
jsonpath "$.version" isString
