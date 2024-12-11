-- Example: Hello World Lua Plugin for Ando
--
-- This plugin adds a custom header to every response
-- and logs the request method + URI.
--
-- Configuration:
--   {
--     "header_name": "X-Hello",
--     "header_value": "World"
--   }

local plugin = {}

-- Plugin metadata
ando.plugin.define({
    name = "hello-world",
    version = "0.1.0",
    priority = 100,
    description = "A simple hello world plugin"
})

-- Rewrite phase: log the incoming request
function plugin.rewrite(config, ctx)
    local method = ando.request.get_method()
    local uri = ando.request.get_uri()
    ando.log.info("Hello World plugin: " .. method .. " " .. uri)
end

-- Access phase: check for a custom header
function plugin.access(config, ctx)
    local blocked = ando.request.get_header("x-block-me")
    if blocked then
        ando.response.exit(403, '{"error": "Blocked by hello-world plugin"}')
        return
    end
end

-- Header filter phase: add custom response header
function plugin.header_filter(config, ctx)
    local header_name = config.header_name or "X-Hello"
    local header_value = config.header_value or "World"
    ando.response.set_header(header_name, header_value)
end

-- Log phase: log the response status
function plugin.log(config, ctx)
    ando.log.info("Hello World plugin: request completed")
end

return plugin
