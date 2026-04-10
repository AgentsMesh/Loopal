"""Claude Code comparison adapter for Terminal-Bench.

Same BENCHMARK_SUFFIX as Loopal adapter, routed through the same API proxy,
so the only variable is the agent harness itself (Claude Code vs Loopal).
"""

import inspect
import os
import shlex
from pathlib import Path

from terminal_bench.agents.installed_agents.claude_code.claude_code_agent import (
    ClaudeCodeAgent,
)
from terminal_bench.terminal.models import TerminalCommand

# Resolve the directory of the ORIGINAL ClaudeCodeAgent class.
# _get_templated_script_path uses inspect.getfile(self.__class__) which would
# resolve to THIS file (the subclass). We need to point to the original.
_ORIGINAL_AGENT_DIR = Path(inspect.getfile(ClaudeCodeAgent)).parent


class ClaudeCodeCompareAgent(ClaudeCodeAgent):
    """Claude Code with same BENCHMARK_SUFFIX and API proxy as Loopal."""

    # Identical to LoopalAgent.BENCHMARK_SUFFIX
    BENCHMARK_SUFFIX = (
        "\n\n---\n"
        "IMPORTANT — follow this workflow IN ORDER:\n"
        "1. FIRST, find and read ALL test/verification files (tests/, "
        "test_*.py, run-tests.sh). Extract the exact expected output: "
        "field names, format strings, file paths, config locations, "
        "error codes. These are the ground truth — do not guess.\n"
        "2. Implement the solution based on what the tests expect.\n"
        "3. Verify with objective commands: "
        "`python3 -c \"print(repr(open('f').read()))\"` for file content, "
        "`ls -la` for permissions, `curl` for endpoints. One pass.\n"
        "4. If tests exist, run them. If any fail, read the assertion, "
        "fix, re-run.\n\n"
        "Rules:\n"
        "- Non-interactive. Do NOT use AskUser. Use best judgment.\n"
        "- chmod +x any script files immediately after creation.\n"
        "- Background services: use nohup/tmux so they survive after "
        "your session.\n"
        "- Only modify files directly related to the task. "
        "git diff --name-only to confirm scope.\n"
        "- If blocked, try alternatives instead of giving up.\n"
    )

    @property
    def _install_agent_script_path(self) -> Path:
        """Point to the original Claude Code setup template, not our subclass dir."""
        from terminal_bench.utils.template_utils import render_setup_script
        import tempfile

        template_path = _ORIGINAL_AGENT_DIR / "claude-code-setup.sh.j2"
        variables = self._get_template_variables()
        script_content = render_setup_script(template_path, variables)

        temp_file = tempfile.NamedTemporaryFile(
            mode="w", suffix=".sh", delete=False
        )
        temp_file.write(script_content)
        temp_file.close()
        os.chmod(temp_file.name, 0o755)
        return Path(temp_file.name)

    @property
    def _env(self) -> dict[str, str]:
        api_key = os.environ.get("ANTHROPIC_API_KEY") or os.environ.get(
            "ANTHROPIC_AUTH_TOKEN", ""
        )
        if not api_key:
            raise EnvironmentError(
                "ANTHROPIC_API_KEY or ANTHROPIC_AUTH_TOKEN is required."
            )

        env = {
            "ANTHROPIC_API_KEY": api_key,
            "FORCE_AUTO_BACKGROUND_TASKS": "1",
            "ENABLE_BACKGROUND_TASKS": "1",
        }

        # Route through Docker-accessible proxy
        if os.environ.get("ANTHROPIC_BASE_URL_DOCKER"):
            env["ANTHROPIC_BASE_URL"] = os.environ["ANTHROPIC_BASE_URL_DOCKER"]
        elif os.environ.get("ANTHROPIC_BASE_URL"):
            import re

            base_url = re.sub(
                r"(https?://)(?:localhost|127\.0\.0\.1|192\.168\.\d+\.\d+|"
                r"10\.\d+\.\d+\.\d+|172\.(?:1[6-9]|2\d|3[01])\.\d+\.\d+)",
                r"\1host.docker.internal",
                os.environ["ANTHROPIC_BASE_URL"],
            )
            env["ANTHROPIC_BASE_URL"] = base_url

        if self._model_name:
            env["ANTHROPIC_MODEL"] = self._model_name.removeprefix("anthropic/")
        elif "ANTHROPIC_MODEL" in os.environ:
            env["ANTHROPIC_MODEL"] = os.environ["ANTHROPIC_MODEL"]

        return env

    def _run_agent_commands(self, instruction: str) -> list[TerminalCommand]:
        full_instruction = instruction + self.BENCHMARK_SUFFIX
        escaped_instruction = shlex.quote(full_instruction)
        return [
            TerminalCommand(
                command=f"claude --verbose --output-format stream-json "
                f"-p {escaped_instruction} --allowedTools "
                f"{' '.join(self.ALLOWED_TOOLS)}",
                min_timeout_sec=0.0,
                max_timeout_sec=float("inf"),
                block=True,
                append_enter=True,
            ),
        ]
