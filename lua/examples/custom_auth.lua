-- Example: Custom Auth Lua Plugin for Ando
--
-- This plugin implements custom authentication logic:
-- - Checks for an X-Custom-Token header
-- - Validates the token against a list of allowed tokens
-- - Sets consumer info in the context
--
-- Configuration:
--   {
--     "tokens": {
--       "token-abc-123": "user-alice",
--       "token-def-456": "user-bob"
--     },
--     "header": "X-Custom-Token"
--   }

local plugin = {}

ando.plugin.define({
    name = "custom-auth",
    version = "0.1.0",
    priority = 2500,
    description = "Custom token-based authentication"
})

function plugin.access(config, ctx)
    local header_name = config.header or "X-Custom-Token"
    local token = ando.request.get_header(header_name)

    if not token then
        ando.response.exit(401, ando.json.encode({
            error = "Missing authentication token",
            status = 401
        }))
        return
    end

    local tokens = config.tokens or {}
    local consumer = tokens[token]

    if not consumer then
        ando.log.warn("Invalid token attempted: " .. string.sub(token, 1, 8) .. "...")
        ando.response.exit(401, ando.json.encode({
            error = "Invalid authentication token",
            status = 401
        }))
        return
    end

    -- Set consumer in context for downstream plugins
    ando.ctx.set("consumer", consumer)
    ando.ctx.set("auth_method", "custom-token")

    ando.log.info("Authenticated consumer: " .. consumer)
end

function plugin.log(config, ctx)
    local consumer = ando.ctx.get("consumer")
    if consumer then
        ando.log.info("Request by consumer: " .. tostring(consumer))
    end
end

return plugin
