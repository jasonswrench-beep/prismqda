import assert from 'node:assert/strict';
import { test } from 'node:test';
import { assistantArgs, assistantConfig } from './assistant';

test('a session that reads names the project and the permission, and one that proposes also says whose coding it would become', () => {
  assert.deepEqual(assistantArgs('C:\\Work\\Study.pqda', 'read', 'Jo'), ['mcp', 'C:\\Work\\Study.pqda', '--permission', 'read']);
  assert.deepEqual(assistantArgs('C:\\Work\\Study.pqda', 'propose', '  Jo Reader '), ['mcp', 'C:\\Work\\Study.pqda', '--permission', 'propose', '--actor', 'Jo Reader']);
});

test('the settings for an assistant application are JSON with one server that starts the program', () => {
  const parsed = JSON.parse(assistantConfig('C:\\Program Files\\PrismQDA\\PrismQDA.exe', 'C:\\Work\\Study.pqda', 'read', ''));
  assert.deepEqual(parsed, { mcpServers: { prismqda: { command: 'C:\\Program Files\\PrismQDA\\PrismQDA.exe', args: ['mcp', 'C:\\Work\\Study.pqda', '--permission', 'read'] } } });
});
