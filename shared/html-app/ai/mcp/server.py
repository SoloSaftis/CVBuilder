"""Optional read-only MCP resources; no candidate data or release authority."""
from pathlib import Path
import json
from mcp.server.fastmcp import FastMCP

ROOT = Path(__file__).resolve().parents[1]
server = FastMCP('CVBuilder Read-only Workflow')


@server.tool()
def workflow() -> str:
    """Read the task graph and approval gates."""
    return (ROOT / 'workflow.json').read_text(encoding='utf-8')


@server.tool()
def prompt(role: str) -> str:
    """Read an allowlisted role prompt."""
    roles = [task['role'] for task in json.loads(workflow())['tasks']]
    if role not in roles:
        raise ValueError('Unknown role')
    return (ROOT / 'prompts' / f'{role}.md').read_text(encoding='utf-8')


if __name__ == '__main__':
    server.run(transport='stdio')
