POST {{baseurl}}/tools
{"metadata": {"spec_url": "https://petstore3.swagger.io/api/v3/openapi.json"}}
HTTP 200
[Asserts]
jsonpath "$.tools" count >= 1
