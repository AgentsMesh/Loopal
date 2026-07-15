import { existsSync, writeFileSync } from 'node:fs'
import { createInterface } from 'node:readline'

const objectSchema = (properties = {}, required = []) => ({
  type: 'object', properties, required, additionalProperties: false,
})
const tools = [{
  name: 'fixture_echo',
  description: 'Echo a deterministic value from the Desktop MCP fixture.',
  inputSchema: objectSchema({ message: { type: 'string' } }, ['message']),
}, {
  name: 'fixture_rich',
  description: 'Return every supported rich MCP content block.',
  inputSchema: objectSchema(),
}, {
  name: 'fixture_error',
  description: 'Return an MCP-level error result.',
  inputSchema: objectSchema(),
}, {
  name: 'fixture_reconnect',
  description: 'Close once, then succeed after automatic reconnect.',
  inputSchema: objectSchema({ marker: { type: 'string' } }, ['marker']),
}, {
  name: 'fixture_slow',
  description: 'Complete late so the runtime can cancel the call.',
  inputSchema: objectSchema(),
}]

function respond(id, result) {
  process.stdout.write(`${JSON.stringify({ jsonrpc: '2.0', id, result })}\n`)
}

function fail(id, code, message) {
  process.stdout.write(`${JSON.stringify({ jsonrpc: '2.0', id, error: { code, message } })}\n`)
}

function callTool(id, params) {
  if (params.name === 'fixture_echo' && typeof params.arguments?.message === 'string') {
    respond(id, { content: [{
      type: 'text', text: `fixture_echo result: ${params.arguments.message}`,
    }], isError: false })
    return
  }
  if (params.name === 'fixture_rich') {
    respond(id, { content: [
      { type: 'text', text: 'fixture rich text' },
      { type: 'image', mimeType: 'image/png', data: 'iVBORw0KGgo=' },
      { type: 'resource', resource: {
        uri: 'fixture://embedded', mimeType: 'text/plain', text: 'fixture embedded body',
      } },
      {
        type: 'resource_link', uri: 'fixture://linked', name: 'fixture-linked',
        description: 'Fixture resource link', mimeType: 'text/plain',
      },
    ], isError: false })
    return
  }
  if (params.name === 'fixture_error') {
    respond(id, {
      content: [{ type: 'text', text: 'fixture MCP error result' }], isError: true,
    })
    return
  }
  if (params.name === 'fixture_reconnect' && typeof params.arguments?.marker === 'string') {
    if (!existsSync(params.arguments.marker)) {
      writeFileSync(params.arguments.marker, 'closed once\n')
      process.exit(17)
    }
    respond(id, { content: [{
      type: 'text', text: 'fixture reconnected after transport close',
    }], isError: false })
    return
  }
  if (params.name === 'fixture_slow') {
    setTimeout(() => respond(id, { content: [{
      type: 'text', text: 'fixture slow late result',
    }], isError: false }), 4_000)
    return
  }
  fail(id, -32602, `Invalid fixture tool call: ${String(params.name)}`)
}

function handle(message) {
  if (!Object.hasOwn(message, 'id')) return
  const { id, method, params = {} } = message
  if (method === 'initialize') {
    respond(id, {
      protocolVersion: params.protocolVersion ?? '2025-06-18',
      capabilities: {
        tools: { listChanged: false }, resources: { listChanged: false },
        prompts: { listChanged: false },
      },
      serverInfo: { name: 'loopal-desktop-fixture', version: '2.0.0' },
      instructions: 'Use the fixture tools, resource, and prompt for Desktop MCP tests.',
    })
    return
  }
  if (method === 'tools/list') return respond(id, { tools })
  if (method === 'tools/call') return callTool(id, params)
  if (method === 'resources/list') return respond(id, { resources: [{
    uri: 'fixture://resource', name: 'fixture-resource',
    description: 'Deterministic fixture resource', mimeType: 'text/plain',
  }] })
  if (method === 'resources/read') return respond(id, { contents: [{
    uri: params.uri, mimeType: 'text/plain', text: 'fixture resource contents',
  }] })
  if (method === 'prompts/list') return respond(id, { prompts: [{
    name: 'fixture_prompt', description: 'Deterministic fixture prompt', arguments: [],
  }] })
  if (method === 'prompts/get') return respond(id, {
    description: 'Deterministic fixture prompt',
    messages: [{ role: 'user', content: { type: 'text', text: 'fixture prompt body' } }],
  })
  if (method === 'ping') return respond(id, {})
  fail(id, -32601, `Unsupported fixture method: ${String(method)}`)
}

createInterface({ input: process.stdin, crlfDelay: Infinity }).on('line', (line) => {
  try { handle(JSON.parse(line)) }
  catch { fail(null, -32700, 'Invalid JSON') }
})
