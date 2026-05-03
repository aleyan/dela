# /// <instructions>
# A self-contained, single-file Python script with no external test files and no pyproject.toml or requirements.txt.
# Execute with `uv run --script dela_coverage_git_refs.py` and add packages using `uv add --script dela_coverage_git_refs.py <deps>`.
# Write AAA-style pytest suites and run them via `uvx pytest dela_coverage_git_refs.py` to cover every public function.
# Run `uvx ruff check --fix dela_coverage_git_refs.py` and ensure the report is clean before completion.
# Run `uvx ruff format dela_coverage_git_refs.py` so whitespace/imports stay normalized.
# Run `uvx ty check dela_coverage_git_refs.py` and resolve any issues it reports.
# Include this commented instruction block in the generated script at the top.
# Keep automated tests inside `dela_coverage_git_refs.py` instead of separate test files.
# These instructions were generated with aleyan.com/projects/script-prompter
# /// </instructions>

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Protocol, TextIO

SCRIPT_PATH = Path(__file__).resolve()
PROJECT_ROOT = SCRIPT_PATH.parent.parent
DEFAULT_JSONL_PATH = SCRIPT_PATH.with_suffix(".jsonl")
DEFAULT_SCRATCH_ROOT = PROJECT_ROOT / "scratch"
DEFAULT_DELA_PATH = PROJECT_ROOT / "target" / "debug" / "dela"
ANSI_ESCAPE_RE = re.compile(r"\x1b\[[0-9;]*m")


class CommandRunner(Protocol):
    def __call__(
        self,
        args: list[str],
        *,
        cwd: Path | None = None,
        input: str | None = None,
        capture_output: bool = False,
        text: bool = False,
        check: bool = False,
    ) -> subprocess.CompletedProcess[str]: ...


@dataclass(frozen=True, slots=True)
class RepoRequest:
    tool: str
    repo: str
    files: tuple[str, ...]


@dataclass(frozen=True, slots=True)
class ScanResult:
    directory: Path
    command: tuple[str, ...]
    returncode: int
    stdout: str
    stderr: str
    parse_errors: tuple[str, ...]
    task_counts_by_runner: dict[str, int]


def parse_repo_slug(repo: str) -> tuple[str, str]:
    parts = [part for part in repo.removesuffix(".git").split("/") if part]
    if len(parts) < 2:
        raise ValueError(f"Unsupported repo value: {repo}")
    return parts[-2], parts[-1]


def checkout_path_for_repo(repo: str, scratch_root: Path) -> Path:
    owner, name = parse_repo_slug(repo)
    return scratch_root / owner / name


def load_repo_requests(jsonl_path: Path) -> list[RepoRequest]:
    grouped_files: dict[tuple[str, str], set[str]] = defaultdict(set)

    for line_number, raw_line in enumerate(
        jsonl_path.read_text(encoding="utf-8").splitlines(),
        start=1,
    ):
        line = raw_line.strip()
        if not line:
            continue

        payload = json.loads(line)
        tool = payload.get("tool")
        repo = payload.get("repo")
        files = payload.get("files")

        if not isinstance(tool, str):
            raise ValueError(f"Line {line_number}: tool must be a string")
        if not isinstance(repo, str):
            raise ValueError(f"Line {line_number}: repo must be a string")
        if not isinstance(files, list) or not all(
            isinstance(item, str) for item in files
        ):
            raise ValueError(f"Line {line_number}: files must be a list of strings")

        for file_path in files:
            grouped_files[(tool, repo)].add(_normalize_repo_file_path(file_path))

    return [
        RepoRequest(tool=tool, repo=repo, files=tuple(sorted(paths)))
        for (tool, repo), paths in sorted(grouped_files.items())
    ]


def ensure_repo_checkout(
    request: RepoRequest,
    scratch_root: Path,
    *,
    runner: CommandRunner = subprocess.run,
) -> Path:
    repo_root = checkout_path_for_repo(request.repo, scratch_root)
    git_dir = repo_root / ".git"

    if repo_root.exists() and not git_dir.exists():
        existing_files = [repo_root / file_path for file_path in request.files]
        if all(path.exists() for path in existing_files):
            return repo_root
        raise RuntimeError(
            f"Refusing to reuse non-git directory without all requested files: {repo_root}"
        )

    repo_root.parent.mkdir(parents=True, exist_ok=True)
    remote_url = _remote_url_for_repo(request.repo)

    if not git_dir.exists():
        _run_checked(
            runner,
            [
                "git",
                "clone",
                "--filter=blob:none",
                "--no-checkout",
                remote_url,
                str(repo_root),
            ],
            context=f"clone {request.repo}",
        )

    _run_checked(
        runner,
        ["git", "-C", str(repo_root), "sparse-checkout", "init", "--no-cone"],
        context=f"initialize sparse checkout for {request.repo}",
    )
    _run_checked(
        runner,
        [
            "git",
            "-C",
            str(repo_root),
            "sparse-checkout",
            "set",
            "--no-cone",
            "--stdin",
        ],
        input="\n".join(request.files) + "\n",
        context=f"set sparse checkout paths for {request.repo}",
    )
    _run_checked(
        runner,
        ["git", "-C", str(repo_root), "checkout"],
        context=f"checkout files for {request.repo}",
    )

    return repo_root


def collect_target_directories(request: RepoRequest, repo_root: Path) -> list[Path]:
    directories = {
        (repo_root / file_path).parent
        for file_path in request.files
        if (repo_root / file_path).exists()
    }
    return sorted(directories)


def resolve_dela_executable(
    project_root: Path,
    *,
    runner: CommandRunner = subprocess.run,
) -> Path:
    _run_checked(
        runner,
        ["cargo", "build", "--quiet"],
        cwd=project_root,
        context="build dela",
    )

    if not DEFAULT_DELA_PATH.exists():
        raise RuntimeError(f"Expected dela binary at {DEFAULT_DELA_PATH}")

    return DEFAULT_DELA_PATH


def run_dela_scan(
    dela_executable: Path,
    directory: Path,
    *,
    runner: CommandRunner = subprocess.run,
) -> ScanResult:
    completed = runner(
        [str(dela_executable), "list", "--verbose"],
        cwd=directory,
        capture_output=True,
        text=True,
        check=False,
    )
    parse_errors = tuple(_extract_parse_errors(completed.stdout, completed.stderr))
    task_counts_by_runner = _extract_task_counts(completed.stdout)

    return ScanResult(
        directory=directory,
        command=(str(dela_executable), "list", "--verbose"),
        returncode=completed.returncode,
        stdout=completed.stdout,
        stderr=completed.stderr,
        parse_errors=parse_errors,
        task_counts_by_runner=task_counts_by_runner,
    )


def render_report(
    results: list[ScanResult],
    missing_files: dict[str, tuple[str, ...]],
    repo_failures: dict[str, str],
    requests: list[RepoRequest],
    scratch_root: Path,
    *,
    stdout: TextIO,
) -> int:
    scanned_directories = len(results)
    parse_failures = [result for result in results if result.parse_errors]
    command_failures = [
        result
        for result in results
        if result.returncode != 0 and not result.parse_errors
    ]
    problem_count = (
        len(parse_failures)
        + len(command_failures)
        + len(missing_files)
        + len(repo_failures)
    )

    _write_line(stdout, f"Scanned {scanned_directories} directories.")

    if repo_failures:
        _write_line(stdout, "")
        _write_line(stdout, "Repository checkout failures:")
        for repo, message in sorted(repo_failures.items()):
            _write_line(stdout, f"- {repo}: {message}")

    if missing_files:
        _write_line(stdout, "")
        _write_line(stdout, "Missing requested files:")
        for repo, paths in sorted(missing_files.items()):
            _write_line(stdout, f"- {repo}")
            for path in paths:
                _write_line(stdout, f"  {path}")

    if parse_failures:
        _write_line(stdout, "")
        _write_line(stdout, "Parse failures:")
        for result in parse_failures:
            _write_line(stdout, f"- {result.directory}")
            for error in result.parse_errors:
                _write_line(stdout, f"  {error}")

    if command_failures:
        _write_line(stdout, "")
        _write_line(stdout, "Command failures:")
        for result in command_failures:
            _write_line(
                stdout,
                f"- {result.directory}: exit {result.returncode} from {' '.join(result.command)}",
            )
            detail = _first_non_empty_line(result.stderr) or _first_non_empty_line(
                result.stdout
            )
            if detail:
                _write_line(stdout, f"  {detail}")

    tool_summary = _summarize_by_tool(
        requests,
        scratch_root=scratch_root,
        results=results,
        missing_files=missing_files,
        repo_failures=repo_failures,
    )
    if tool_summary:
        _write_line(stdout, "")
        _write_line(stdout, "Summary by tool:")
        for summary_line in _format_tool_summary(tool_summary):
            _write_line(
                stdout,
                summary_line,
            )

    if problem_count == 0:
        _write_line(stdout, "")
        _write_line(stdout, "No parse issues detected.")
        return 0

    _write_line(stdout, "")
    _write_line(stdout, f"Detected {problem_count} problem areas.")
    return 1


def main(
    argv: list[str] | None = None,
    *,
    runner: CommandRunner = subprocess.run,
    stdout: TextIO | None = None,
) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Download the referenced task-definition files into scratch/ and "
            "run `dela list --verbose` in each relevant directory."
        )
    )
    parser.add_argument(
        "--jsonl",
        type=Path,
        default=DEFAULT_JSONL_PATH,
        help=f"JSONL file containing repo references (default: {DEFAULT_JSONL_PATH})",
    )
    parser.add_argument(
        "--scratch-root",
        type=Path,
        default=DEFAULT_SCRATCH_ROOT,
        help=f"Checkout root for sparse repos (default: {DEFAULT_SCRATCH_ROOT})",
    )
    parser.add_argument(
        "--dela-bin",
        type=Path,
        default=None,
        help="Path to an already-built dela binary. Defaults to target/debug/dela.",
    )
    parser.add_argument(
        "--limit",
        type=int,
        default=None,
        help="Limit the number of repos processed. Useful while iterating on the script.",
    )
    args = parser.parse_args(argv)

    writer = stdout or sys.stdout
    requests = load_repo_requests(args.jsonl)
    if args.limit is not None:
        requests = requests[: args.limit]

    dela_executable = args.dela_bin or resolve_dela_executable(
        PROJECT_ROOT,
        runner=runner,
    )

    results: list[ScanResult] = []
    missing_files: dict[str, tuple[str, ...]] = {}
    repo_failures: dict[str, str] = {}
    requests_by_repo = _group_requests_by_repo(requests)
    scan_cache: dict[Path, ScanResult] = {}

    for index, (repo, repo_requests) in enumerate(
        sorted(requests_by_repo.items()), start=1
    ):
        _write_line(writer, f"[{index}/{len(requests_by_repo)}] {repo}")
        combined_request = RepoRequest(
            tool="repo",
            repo=repo,
            files=tuple(
                sorted(
                    {
                        file_path
                        for request in repo_requests
                        for file_path in request.files
                    }
                )
            ),
        )
        try:
            repo_root = ensure_repo_checkout(
                combined_request,
                args.scratch_root,
                runner=runner,
            )
        except RuntimeError as error:
            repo_failures[repo] = str(error)
            continue

        missing = tuple(
            sorted(
                file_path
                for file_path in combined_request.files
                if not (repo_root / file_path).exists()
            )
        )
        if missing:
            missing_files[repo] = missing

        requested_directories = sorted(
            {
                directory
                for request in repo_requests
                for directory in collect_target_directories(request, repo_root)
            }
        )
        for directory in requested_directories:
            if directory in scan_cache:
                continue
            scan_result = run_dela_scan(dela_executable, directory, runner=runner)
            scan_cache[directory] = scan_result
            results.append(scan_result)

    return render_report(
        results,
        missing_files,
        repo_failures,
        requests,
        args.scratch_root,
        stdout=writer,
    )


def _normalize_repo_file_path(file_path: str) -> str:
    path = PurePosixPath(file_path)
    if path.is_absolute():
        raise ValueError(f"Absolute file paths are not supported: {file_path}")
    if any(part == ".." for part in path.parts):
        raise ValueError(f"Parent traversal is not supported: {file_path}")
    normalized = path.as_posix()
    if normalized in {"", "."}:
        raise ValueError("Empty file paths are not supported")
    return normalized


def _remote_url_for_repo(repo: str) -> str:
    cleaned = repo.removesuffix(".git")
    if cleaned.startswith(("http://", "https://")):
        return f"{cleaned}.git"
    return f"https://{cleaned}.git"


def _run_checked(
    runner: CommandRunner,
    args: list[str],
    *,
    cwd: Path | None = None,
    input: str | None = None,
    context: str,
) -> subprocess.CompletedProcess[str]:
    completed = runner(
        args,
        cwd=cwd,
        input=input,
        capture_output=True,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        detail = _first_non_empty_line(completed.stderr) or _first_non_empty_line(
            completed.stdout
        )
        if detail:
            raise RuntimeError(f"Failed to {context}: {detail}")
        raise RuntimeError(
            f"Failed to {context}: command exited with {completed.returncode}"
        )
    return completed


def _extract_parse_errors(stdout: str, stderr: str) -> list[str]:
    errors: list[str] = []
    in_errors_section = False

    for raw_line in stdout.splitlines() + stderr.splitlines():
        line = ANSI_ESCAPE_RE.sub("", raw_line).strip()
        if not line:
            continue
        if line == "Errors encountered:":
            in_errors_section = True
            continue
        if in_errors_section and line.startswith("• "):
            errors.append(line.removeprefix("• ").strip())
            continue
        if in_errors_section and not line.startswith("• "):
            in_errors_section = False
        if "Failed to parse " in line and line not in errors:
            errors.append(line)

    return errors


def _extract_task_counts(stdout: str) -> dict[str, int]:
    task_counts: dict[str, int] = defaultdict(int)
    current_runner: str | None = None

    for raw_line in stdout.splitlines():
        line = ANSI_ESCAPE_RE.sub("", raw_line).rstrip()
        stripped = line.strip()

        if not stripped:
            current_runner = None
            continue

        if not line.startswith(" "):
            current_runner = _extract_runner_name_from_header(line)
            continue

        if current_runner is not None and line.startswith("  "):
            task_counts[current_runner] += 1

    return dict(task_counts)


def _first_non_empty_line(text: str) -> str | None:
    for line in text.splitlines():
        stripped = ANSI_ESCAPE_RE.sub("", line).strip()
        if stripped:
            return stripped
    return None


def _write_line(stdout: TextIO, line: str) -> None:
    stdout.write(f"{line}\n")


def _group_requests_by_repo(
    requests: list[RepoRequest],
) -> dict[str, list[RepoRequest]]:
    grouped_requests: dict[str, list[RepoRequest]] = defaultdict(list)
    for request in requests:
        grouped_requests[request.repo].append(request)
    return dict(grouped_requests)


def _summarize_by_tool(
    requests: list[RepoRequest],
    *,
    scratch_root: Path,
    results: list[ScanResult],
    missing_files: dict[str, tuple[str, ...]],
    repo_failures: dict[str, str],
) -> list[tuple[str, int, int, int, int]]:
    result_by_directory = {result.directory: result for result in results}
    counts_by_tool: dict[str, dict[str, int]] = defaultdict(
        lambda: {"files": 0, "parsed": 0, "tasks": 0, "errors": 0}
    )
    directories_by_tool: dict[str, set[Path]] = defaultdict(set)

    for request in requests:
        tool_counts = counts_by_tool[request.tool]
        repo_root = checkout_path_for_repo(request.repo, scratch_root)

        for file_path in request.files:
            tool_counts["files"] += 1

            if request.repo in repo_failures:
                tool_counts["errors"] += 1
                continue

            if file_path in missing_files.get(request.repo, ()):
                tool_counts["errors"] += 1
                continue

            full_path = repo_root / file_path
            directories_by_tool[request.tool].add(full_path.parent)
            scan_result = result_by_directory.get(full_path.parent)
            if scan_result is None or _scan_result_has_error_for_file(
                scan_result, full_path
            ):
                tool_counts["errors"] += 1
                continue

            tool_counts["parsed"] += 1

    for tool, directories in directories_by_tool.items():
        runner_name = _tool_runner_name(tool)
        counts_by_tool[tool]["tasks"] = sum(
            result_by_directory[directory].task_counts_by_runner.get(runner_name, 0)
            for directory in directories
            if directory in result_by_directory
        )

    return [
        (
            _tool_display_name(tool),
            counts["files"],
            counts["parsed"],
            counts["tasks"],
            counts["errors"],
        )
        for tool, counts in sorted(counts_by_tool.items(), key=lambda item: item[0])
    ]


def _scan_result_has_error_for_file(scan_result: ScanResult, file_path: Path) -> bool:
    if scan_result.returncode != 0 and not scan_result.parse_errors:
        return True

    file_path_str = str(file_path)
    return any(file_path_str in error for error in scan_result.parse_errors)


def _tool_display_name(tool: str) -> str:
    tool_names = {
        "bun": "package.json (bun)",
        "cmake": "CMakeLists.txt",
        "docker-compose": "docker-compose.yml",
        "github-actions": "GitHub Actions",
        "gradle": "Gradle",
        "just": "Justfile",
        "make": "Makefile",
        "maven": "pom.xml",
        "npm": "package.json (npm)",
        "pnpm": "package.json (pnpm)",
        "poetry": "pyproject.toml (poetry)",
        "task": "Taskfile",
        "travis": ".travis.yml",
        "uv": "pyproject.toml (uv)",
        "yarn": "package.json (yarn)",
    }
    return tool_names.get(tool, tool)


def _tool_runner_name(tool: str) -> str:
    runner_names = {
        "bun": "bun",
        "cmake": "cmake",
        "docker-compose": "docker compose",
        "github-actions": "act",
        "gradle": "gradle",
        "just": "just",
        "make": "make",
        "maven": "mvn",
        "npm": "npm",
        "pnpm": "pnpm",
        "poetry": "poetry",
        "task": "task",
        "travis": "travis",
        "uv": "uv",
        "yarn": "yarn",
    }
    return runner_names.get(tool, tool)


def _extract_runner_name_from_header(line: str) -> str | None:
    if " — " not in line:
        return None

    header_name = line.split(" — ", 1)[0].strip()
    return re.sub(r"(?:\s+[*§†‡‖]+)$", "", header_name)


def _format_tool_summary(
    tool_summary: list[tuple[str, int, int, int, int]],
) -> list[str]:
    if not tool_summary:
        return []

    name_width = max(len(tool_name) for tool_name, *_ in tool_summary)
    files_width = max(len(str(total_files)) for _, total_files, *_ in tool_summary)
    parsed_width = max(
        len(str(parsed_files)) for _, _, parsed_files, _, _ in tool_summary
    )
    tasks_width = max(len(str(task_count)) for _, _, _, task_count, _ in tool_summary)
    errors_width = max(
        len(str(error_count)) for _, _, _, _, error_count in tool_summary
    )

    return [
        (
            f"{tool_name.ljust(name_width)} : "
            f"{total_files:>{files_width}} files, "
            f"{parsed_files:>{parsed_width}} parsed, "
            f"{task_count:>{tasks_width}} tasks, "
            f"{error_count:>{errors_width}} errors."
        )
        for tool_name, total_files, parsed_files, task_count, error_count in tool_summary
    ]


def test_parse_repo_slug_extracts_owner_and_name() -> None:
    # Arrange
    repo = "github.com/freeCodeCamp/freeCodeCamp"

    # Act
    owner, name = parse_repo_slug(repo)

    # Assert
    assert owner == "freeCodeCamp"
    assert name == "freeCodeCamp"


def test_checkout_path_for_repo_uses_owner_and_repo_name(tmp_path: Path) -> None:
    # Arrange
    scratch_root = tmp_path / "scratch"

    # Act
    repo_path = checkout_path_for_repo("github.com/aleyan/dela", scratch_root)

    # Assert
    assert repo_path == scratch_root / "aleyan" / "dela"


def test_load_repo_requests_merges_duplicate_repos(tmp_path: Path) -> None:
    # Arrange
    jsonl_path = tmp_path / "refs.jsonl"
    jsonl_path.write_text(
        "\n".join(
            [
                json.dumps(
                    {
                        "tool": "npm",
                        "repo": "github.com/aleyan/dela",
                        "files": ["package.json", "nested/package.json"],
                    }
                ),
                json.dumps(
                    {
                        "tool": "npm",
                        "repo": "github.com/aleyan/dela",
                        "files": ["package.json", "pyproject.toml"],
                    }
                ),
                json.dumps(
                    {
                        "tool": "yarn",
                        "repo": "github.com/aleyan/dela",
                        "files": ["package.json"],
                    }
                ),
            ]
        ),
        encoding="utf-8",
    )

    # Act
    requests = load_repo_requests(jsonl_path)

    # Assert
    assert requests == [
        RepoRequest(
            tool="npm",
            repo="github.com/aleyan/dela",
            files=("nested/package.json", "package.json", "pyproject.toml"),
        ),
        RepoRequest(
            tool="yarn",
            repo="github.com/aleyan/dela",
            files=("package.json",),
        ),
    ]


def test_load_repo_requests_rejects_parent_traversal(tmp_path: Path) -> None:
    # Arrange
    jsonl_path = tmp_path / "refs.jsonl"
    jsonl_path.write_text(
        json.dumps(
            {
                "tool": "npm",
                "repo": "github.com/aleyan/dela",
                "files": ["../package.json"],
            }
        ),
        encoding="utf-8",
    )

    # Act
    try:
        load_repo_requests(jsonl_path)
    except ValueError as error:
        # Assert
        assert "Parent traversal" in str(error)
    else:
        raise AssertionError("Expected ValueError for parent traversal")

    # Assert
    assert jsonl_path.exists()


def test_ensure_repo_checkout_clones_and_sets_sparse_paths(tmp_path: Path) -> None:
    # Arrange
    scratch_root = tmp_path / "scratch"
    request = RepoRequest(
        tool="npm",
        repo="github.com/aleyan/dela",
        files=("nested/package.json", "pyproject.toml"),
    )
    calls: list[tuple[str, ...]] = []

    def fake_runner(
        args: list[str],
        *,
        cwd: Path | None = None,
        input: str | None = None,
        capture_output: bool = False,
        text: bool = False,
        check: bool = False,
    ) -> subprocess.CompletedProcess[str]:
        del cwd, capture_output, text, check
        command = tuple(args)
        calls.append(command)
        repo_root = checkout_path_for_repo(request.repo, scratch_root)
        if args[:2] == ["git", "clone"]:
            (repo_root / ".git").mkdir(parents=True)
        if args[3:6] == ["sparse-checkout", "set", "--no-cone"]:
            assert input == "nested/package.json\npyproject.toml\n"
            for relative_path in request.files:
                target = repo_root / relative_path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text("content", encoding="utf-8")
        return subprocess.CompletedProcess(args, 0, "", "")

    # Act
    repo_root = ensure_repo_checkout(request, scratch_root, runner=fake_runner)

    # Assert
    assert repo_root == scratch_root / "aleyan" / "dela"
    assert calls[0][:4] == (
        "git",
        "clone",
        "--filter=blob:none",
        "--no-checkout",
    )
    assert any(call[3:6] == ("sparse-checkout", "set", "--no-cone") for call in calls)
    assert (repo_root / "nested" / "package.json").exists()
    assert (repo_root / "pyproject.toml").exists()


def test_collect_target_directories_returns_existing_parent_dirs(
    tmp_path: Path,
) -> None:
    # Arrange
    request = RepoRequest(
        tool="npm",
        repo="github.com/aleyan/dela",
        files=("nested/package.json", "nested/pyproject.toml", "missing/package.json"),
    )
    repo_root = tmp_path / "repo"
    (repo_root / "nested").mkdir(parents=True)
    (repo_root / "nested" / "package.json").write_text("{}", encoding="utf-8")
    (repo_root / "nested" / "pyproject.toml").write_text("", encoding="utf-8")

    # Act
    directories = collect_target_directories(request, repo_root)

    # Assert
    assert directories == [repo_root / "nested"]


def test_resolve_dela_executable_builds_binary(tmp_path: Path) -> None:
    # Arrange
    build_calls: list[tuple[str, ...]] = []
    project_root = tmp_path / "project"
    target_binary = project_root / "target" / "debug" / "dela"
    target_binary.parent.mkdir(parents=True, exist_ok=True)

    original_default = globals()["DEFAULT_DELA_PATH"]
    globals()["DEFAULT_DELA_PATH"] = target_binary

    def fake_runner(
        args: list[str],
        *,
        cwd: Path | None = None,
        input: str | None = None,
        capture_output: bool = False,
        text: bool = False,
        check: bool = False,
    ) -> subprocess.CompletedProcess[str]:
        del input, capture_output, text, check
        build_calls.append(tuple(args))
        assert cwd == project_root
        target_binary.write_text("binary", encoding="utf-8")
        return subprocess.CompletedProcess(args, 0, "", "")

    try:
        # Act
        resolved = resolve_dela_executable(project_root, runner=fake_runner)

        # Assert
        assert resolved == target_binary
        assert build_calls == [("cargo", "build", "--quiet")]
    finally:
        globals()["DEFAULT_DELA_PATH"] = original_default


def test_run_dela_scan_extracts_parse_errors(tmp_path: Path) -> None:
    # Arrange
    dela_binary = tmp_path / "dela"
    directory = tmp_path / "repo"
    directory.mkdir()

    def fake_runner(
        args: list[str],
        *,
        cwd: Path | None = None,
        input: str | None = None,
        capture_output: bool = False,
        text: bool = False,
        check: bool = False,
    ) -> subprocess.CompletedProcess[str]:
        del input, capture_output, text, check
        assert cwd == directory
        return subprocess.CompletedProcess(
            args,
            0,
            "\n".join(
                [
                    "Task definition files:",
                    "",
                    "npm — package.json",
                    "  build               - build app",
                    "  test                - run tests",
                    "Errors encountered:",
                    "  • Failed to parse /tmp/repo/package.json: bad json",
                ]
            ),
            "",
        )

    # Act
    result = run_dela_scan(dela_binary, directory, runner=fake_runner)

    # Assert
    assert result.returncode == 0
    assert result.parse_errors == ("Failed to parse /tmp/repo/package.json: bad json",)
    assert result.task_counts_by_runner == {"npm": 2}


def test_render_report_returns_non_zero_when_issues_are_present(
    tmp_path: Path,
) -> None:
    # Arrange
    output_path = tmp_path / "output.txt"
    result = ScanResult(
        directory=tmp_path / "repo",
        command=("dela", "list", "--verbose"),
        returncode=0,
        stdout="",
        stderr="",
        parse_errors=("Failed to parse /tmp/repo/package.json: bad json",),
        task_counts_by_runner={},
    )

    with output_path.open("w+", encoding="utf-8") as handle:
        # Act
        exit_code = render_report(
            [result],
            {"github.com/aleyan/dela": ("missing/package.json",)},
            {"github.com/other/repo": "git clone failed"},
            [
                RepoRequest(
                    tool="make",
                    repo="github.com/aleyan/dela",
                    files=("missing/package.json",),
                ),
                RepoRequest(
                    tool="npm",
                    repo="github.com/example/repo",
                    files=("package.json",),
                ),
            ],
            tmp_path / "scratch",
            stdout=handle,
        )
        handle.seek(0)
        report = handle.read()

    # Assert
    assert exit_code == 1
    assert "Repository checkout failures:" in report
    assert "Missing requested files:" in report
    assert "Parse failures:" in report
    assert "Summary by tool:" in report
    assert "Makefile           : 1 files, 0 parsed, 0 tasks, 1 errors." in report
    assert "package.json (npm) : 1 files, 0 parsed, 0 tasks, 1 errors." in report
    assert "Detected 3 problem areas." in report


def test_main_processes_requests_and_reports_success(tmp_path: Path) -> None:
    # Arrange
    jsonl_path = tmp_path / "refs.jsonl"
    jsonl_path.write_text(
        json.dumps(
            {
                "tool": "npm",
                "repo": "github.com/aleyan/dela",
                "files": ["nested/package.json"],
            }
        ),
        encoding="utf-8",
    )
    scratch_root = tmp_path / "scratch"
    dela_binary = tmp_path / "dela"
    output_path = tmp_path / "output.txt"

    def fake_runner(
        args: list[str],
        *,
        cwd: Path | None = None,
        input: str | None = None,
        capture_output: bool = False,
        text: bool = False,
        check: bool = False,
    ) -> subprocess.CompletedProcess[str]:
        del capture_output, text, check
        if args[:2] == ["git", "clone"]:
            repo_root = Path(args[-1])
            (repo_root / ".git").mkdir(parents=True)
            return subprocess.CompletedProcess(args, 0, "", "")
        if args[3:6] == ["sparse-checkout", "set", "--no-cone"]:
            assert input == "nested/package.json\n"
            repo_root = Path(args[2])
            target = repo_root / "nested" / "package.json"
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text("{}", encoding="utf-8")
            return subprocess.CompletedProcess(args, 0, "", "")
        if cwd is not None:
            return subprocess.CompletedProcess(
                args,
                0,
                "\n".join(
                    [
                        "npm — package.json",
                        "  build               - build app",
                        "  test                - run tests",
                    ]
                ),
                "",
            )
        return subprocess.CompletedProcess(args, 0, "", "")

    with output_path.open("w+", encoding="utf-8") as handle:
        # Act
        exit_code = main(
            [
                "--jsonl",
                str(jsonl_path),
                "--scratch-root",
                str(scratch_root),
                "--dela-bin",
                str(dela_binary),
            ],
            runner=fake_runner,
            stdout=handle,
        )
        handle.seek(0)
        report = handle.read()

    # Assert
    assert exit_code == 0
    assert "[1/1] github.com/aleyan/dela" in report
    assert "Scanned 1 directories." in report
    assert "package.json (npm) : 1 files, 1 parsed, 2 tasks, 0 errors." in report
    assert "No parse issues detected." in report


def test_format_tool_summary_aligns_columns() -> None:
    # Arrange
    tool_summary = [
        ("package.json (bun)", 10, 9, 13, 1),
        ("Makefile", 2404, 1289, 5555, 1115),
    ]

    # Act
    lines = _format_tool_summary(tool_summary)

    # Assert
    assert lines == [
        "package.json (bun) :   10 files,    9 parsed,   13 tasks,    1 errors.",
        "Makefile           : 2404 files, 1289 parsed, 5555 tasks, 1115 errors.",
    ]


if __name__ == "__main__":
    raise SystemExit(main())
