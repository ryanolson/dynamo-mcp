#!/bin/bash

# Test the MCP server with a single request and exit

echo '{"jsonrpc":"2.0","method":"initialize","id":1,"params":{}}' | \
    timeout 2 ./target/release/dynamo_mcp 2>/dev/null | \
    head -n1 | \
    jq '.result.serverInfo'