// What a person puts in an assistant application's settings to connect it to a project (decision 203). The program does the work; this only
// writes down the command the application should start, in the form most such applications read.

export type AssistantPermission = 'read' | 'propose';

/** The arguments the assistant application passes to the program: `mcp`, the project, the permission, and for a session that proposes, whose coding it would become. */
export function assistantArgs(projectPath: string, permission: AssistantPermission, actor: string): string[] {
  const args = ['mcp', projectPath, '--permission', permission];
  if (permission === 'propose') args.push('--actor', actor.trim());
  return args;
}

/** The settings of an assistant application, as the JSON most of them read (a list of servers, each a command and its arguments). */
export function assistantConfig(command: string, projectPath: string, permission: AssistantPermission, actor: string): string {
  return JSON.stringify({ mcpServers: { prismqda: { command, args: assistantArgs(projectPath, permission, actor) } } }, null, 2);
}
