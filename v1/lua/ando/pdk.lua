-- Ando PDK — Lua Plugin Development Kit
-- This module is automatically loaded into every Lua VM.
--
-- Usage in plugins:
--   local ando = require("ando.pdk")
--   ando.request.get_method()
--   ando.response.exit(403, '{"error": "forbidden"}')
--
-- The `ando` global is already available without requiring.

local pdk = {}

-- Re-export the global ando PDK
pdk.request = ando.request
pdk.response = ando.response
pdk.log = ando.log
pdk.ctx = ando.ctx
pdk.json = ando.json
pdk.plugin = ando.plugin

return pdk
