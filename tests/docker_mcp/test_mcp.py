#!/usr/bin/env python3
"""Dockerized MCP integration test covering the current dela MCP contract."""

import json
import os
import subprocess
import sys
import time


PROJECT_CWD = "/home/testuser/test_project"
MCP_COMMAND = ["/usr/local/bin/dela", "mcp", "--cwd", PROJECT_CWD]


def fail(message, *, payload=None):
    print(f"✗ {message}")
    if payload is not None:
        print(json.dumps(payload, indent=2, sort_keys=True))
    return False


def start_mcp_process():
    env = os.environ.copy()
    env.update(
        {
            "RUST_LOG": "warn",
            "MCPI_NO_COLOR": "1",
        }
    )

    process = subprocess.Popen(
        MCP_COMMAND,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        cwd=PROJECT_CWD,
        env=env,
    )

    initialize_request = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "docker-mcp-test",
                "version": "1.0.0",
            },
        },
    }
    process.stdin.write(json.dumps(initialize_request) + "\n")
    process.stdin.flush()

    init_response, _ = read_until_response(process, initialize_request["id"])
    if "result" not in init_response:
        process.kill()
        raise RuntimeError(f"initialize failed: {init_response}")

    initialized_notification = {
        "jsonrpc": "2.0",
        "method": "notifications/initialized",
    }
    process.stdin.write(json.dumps(initialized_notification) + "\n")
    process.stdin.flush()
    return process, init_response


def stop_process(process):
    if process.poll() is None:
        process.kill()
        try:
            process.communicate(timeout=2)
        except subprocess.TimeoutExpired:
            process.kill()


def read_json_line(process, timeout_seconds=10):
    deadline = time.time() + timeout_seconds
    while time.time() < deadline:
        line = process.stdout.readline()
        if not line:
            stderr = process.stderr.read()
            raise RuntimeError(f"mcp server closed stdout early; stderr={stderr!r}")
        line = line.strip()
        if not line:
            continue
        try:
            return json.loads(line)
        except json.JSONDecodeError:
            continue
    raise TimeoutError("timed out waiting for json-rpc message")


def read_until_response(process, request_id, timeout_seconds=10):
    notifications = []
    deadline = time.time() + timeout_seconds

    while time.time() < deadline:
        message = read_json_line(process, timeout_seconds=max(1, deadline - time.time()))
        if message.get("id") == request_id:
            return message, notifications
        if "method" in message:
            notifications.append(message)

    raise TimeoutError(f"timed out waiting for response id {request_id}")


def send_request(process, request, timeout_seconds=10):
    process.stdin.write(json.dumps(request) + "\n")
    process.stdin.flush()
    return read_until_response(process, request["id"], timeout_seconds=timeout_seconds)


def parse_tool_result(response):
    result = response.get("result")
    if not result:
        raise AssertionError(f"expected result payload, got: {response}")

    content = result.get("content") or []
    if not content:
        raise AssertionError(f"expected content payload, got: {response}")

    text = content[0].get("text")
    if text is None:
        raise AssertionError(f"expected text payload, got: {response}")

    return json.loads(text)


def tool_request(request_id, name, arguments=None):
    return {
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "tools/call",
        "params": {
            "name": name,
            "arguments": arguments or {},
        },
    }


def assert_condition(condition, message, payload=None):
    if not condition:
        raise AssertionError(message if payload is None else f"{message}: {payload}")


def find_task(tasks, unique_name):
    for task in tasks:
        if task.get("unique_name") == unique_name:
            return task
    raise AssertionError(f"task {unique_name!r} not found in {tasks!r}")


def find_job(jobs, unique_name, pid=None):
    for job in jobs:
        if job.get("unique_name") != unique_name:
            continue
        if pid is not None and job.get("pid") != pid:
            continue
        return job
    raise AssertionError(f"job {unique_name!r} pid={pid!r} not found in {jobs!r}")


def logging_notifications(notifications):
    return [n for n in notifications if n.get("method") == "notifications/message"]


def test_initialize_instructions():
    print("Test 1: initialize advertises bounded wait and logging")
    process, init_response = start_mcp_process()
    try:
        info = init_response["result"]["serverInfo"]
        instructions = init_response["result"].get("instructions", "")
        assert_condition(
            "wait_for_exit_seconds" in instructions,
            "instructions missing wait_for_exit_seconds",
            instructions,
        )
        assert_condition(
            "default 1-second capture window" in instructions,
            "instructions missing default capture wording",
            instructions,
        )
        assert_condition(
            info["name"],
            "serverInfo.name should be present",
            info,
        )
        print("✓ initialize response includes current MCP instructions")
        return True
    finally:
        stop_process(process)


def test_tools_list_schema():
    print("Test 2: tools/list exposes MCP tool surface and bounded wait schema")
    process, _ = start_mcp_process()
    try:
        response, _ = send_request(
            process,
            {
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/list",
                "params": {},
            },
        )
        tools = response["result"]["tools"]
        names = {tool["name"] for tool in tools}
        expected = {"list_tasks", "task_start", "status", "task_status", "task_output", "task_stop"}
        assert_condition(expected.issubset(names), "missing tools", list(names))

        task_start = next(tool for tool in tools if tool["name"] == "task_start")
        properties = task_start["inputSchema"]["properties"]
        wait_prop = properties.get("wait_for_exit_seconds")
        assert_condition(wait_prop is not None, "task_start missing wait_for_exit_seconds", properties)
        assert_condition(
            wait_prop.get("type") == "integer",
            "wait_for_exit_seconds should be integer",
            wait_prop,
        )
        print("✓ tools/list exposes current task_start schema")
        return True
    finally:
        stop_process(process)


def test_list_tasks_enriched_fields():
    print("Test 3: list_tasks returns enriched fields")
    process, _ = start_mcp_process()
    try:
        response, _ = send_request(process, tool_request(3, "list_tasks"))
        payload = parse_tool_result(response)
        tasks = payload["tasks"]
        test_task = find_task(tasks, "test-task")
        for field in [
            "unique_name",
            "source_name",
            "runner",
            "command",
            "runner_available",
            "allowlisted",
            "file_path",
        ]:
            assert_condition(field in test_task, f"missing list_tasks field {field}", test_task)
        print("✓ list_tasks returns enriched task metadata")
        return True
    finally:
        stop_process(process)


def test_task_start_quick_exit():
    print("Test 4: task_start returns direct quick-exit payload")
    process, _ = start_mcp_process()
    try:
        response, notifications = send_request(
            process,
            tool_request(4, "task_start", {"unique_name": "test-task"}),
            timeout_seconds=10,
        )
        payload = parse_tool_result(response)
        assert_condition(payload["state"] == "exited", "quick task should exit", payload)
        assert_condition(payload.get("pid") is None, "quick exit should not report pid", payload)
        assert_condition(payload.get("exit_code") == 0, "quick exit should report exit_code 0", payload)
        assert_condition(
            "Test task executed successfully" in payload["initial_output"],
            "quick exit missing task output",
            payload,
        )
        assert_condition(
            all("ok" not in item for item in payload.keys()),
            "quick exit payload should not use legacy ok/result wrapper",
            payload,
        )
        assert_condition(
            any(n.get("method") == "notifications/message" for n in notifications),
            "quick exit should stream at least one logging notification",
            notifications,
        )
        print("✓ task_start quick-exit contract matches current MCP shape")
        return True
    finally:
        stop_process(process)


def test_task_start_args_and_spaces():
    print("Test 5: task_start preserves space-containing args for the underlying runner")
    process, _ = start_mcp_process()
    try:
        response, _ = send_request(
            process,
            tool_request(
                5,
                "task_start",
                {
                    "unique_name": "print-args",
                    "args": ["ARGS=value with spaces"],
                },
            ),
        )
        payload = parse_tool_result(response)
        assert_condition(payload["state"] == "exited", "print-args should exit", payload)
        assert_condition(payload.get("exit_code") == 0, "print-args should exit successfully", payload)
        output = payload["initial_output"]
        assert_condition("value with spaces" in output, "missing spaced arg in output", payload)
        print("✓ task_start preserves passed arguments")
        return True
    finally:
        stop_process(process)


def test_error_taxonomy():
    print("Test 6: task_start returns current TaskNotFound and NotAllowlisted errors")
    process, _ = start_mcp_process()
    try:
        not_found_response, _ = send_request(
            process,
            tool_request(6, "task_start", {"unique_name": "nonexistent-task"}),
        )
        not_found = not_found_response.get("error", {})
        assert_condition(not_found.get("code") == -32012, "wrong TaskNotFound code", not_found_response)
        assert_condition("not found" in not_found.get("message", ""), "wrong TaskNotFound message", not_found)

        deny_response, _ = send_request(
            process,
            tool_request(7, "task_start", {"unique_name": "custom-exe"}),
        )
        denied = deny_response.get("error", {})
        assert_condition(denied.get("code") == -32010, "wrong NotAllowlisted code", deny_response)
        assert_condition(
            "not allowlisted" in denied.get("message", ""),
            "wrong NotAllowlisted message",
            denied,
        )
        print("✓ task_start error taxonomy matches MCP contract")
        return True
    finally:
        stop_process(process)


def test_bounded_wait_completion():
    print("Test 7: task_start bounded wait returns completed task in one round trip")
    process, _ = start_mcp_process()
    try:
        response, notifications = send_request(
            process,
            tool_request(
                8,
                "task_start",
                {
                    "unique_name": "long-running-task",
                    "wait_for_exit_seconds": 7,
                },
            ),
            timeout_seconds=15,
        )
        payload = parse_tool_result(response)
        assert_condition(payload["state"] == "exited", "bounded wait should complete task", payload)
        assert_condition(payload.get("exit_code") == 0, "bounded wait should return exit_code 0", payload)
        assert_condition(payload.get("pid") is None, "bounded wait completion should not return pid", payload)
        assert_condition(
            "Starting long-running task..." in payload["initial_output"],
            "missing initial stdout from bounded wait task",
            payload,
        )
        assert_condition(
            "Long-running task completed successfully" in payload["initial_output"],
            "missing completion stdout from bounded wait task",
            payload,
        )
        assert_condition(
            len(logging_notifications(notifications)) >= 2,
            "bounded wait should stream logging notifications while waiting",
            notifications,
        )
        print("✓ task_start bounded wait works for completed tasks")
        return True
    finally:
        stop_process(process)


def test_task_status_completion_metadata():
    print("Test 8: task_status exposes exit_code and completed_at for completed jobs")
    process, _ = start_mcp_process()
    try:
        send_request(
            process,
            tool_request(9, "task_start", {"unique_name": "test-task"}),
        )
        status_response, _ = send_request(
            process,
            tool_request(10, "task_status", {"unique_name": "test-task"}),
        )
        payload = parse_tool_result(status_response)
        job = find_job(payload["jobs"], "test-task")
        assert_condition(job["state"] == "exited", "completed test-task should be exited", job)
        assert_condition(job["exit_code"] == 0, "completed test-task should expose exit_code", job)
        assert_condition(
            isinstance(job["completed_at"], str) and job["completed_at"],
            "completed test-task should expose completed_at",
            job,
        )
        print("✓ task_status exposes completion metadata")
        return True
    finally:
        stop_process(process)


def test_running_lifecycle_and_stop():
    print("Test 9: background execution, status, output, and stop lifecycle")
    process, _ = start_mcp_process()
    try:
        start_response, _ = send_request(
            process,
            tool_request(11, "task_start", {"unique_name": "long-running-task"}),
            timeout_seconds=10,
        )
        start_payload = parse_tool_result(start_response)
        assert_condition(start_payload["state"] == "running", "default start should background task", start_payload)
        pid = start_payload.get("pid")
        assert_condition(isinstance(pid, int) and pid > 0, "running task should expose pid", start_payload)
        assert_condition(
            "Starting long-running task..." in start_payload["initial_output"],
            "running task missing initial output",
            start_payload,
        )

        status_response, _ = send_request(process, tool_request(12, "status"))
        running = parse_tool_result(status_response)["running"]
        running_job = find_job(running, "long-running-task", pid=pid)
        assert_condition(running_job["pid"] == pid, "status should report the running pid", running_job)
        assert_condition(
            isinstance(running_job.get("elapsed_seconds"), int),
            "status should include elapsed_seconds",
            running_job,
        )

        task_status_response, _ = send_request(
            process,
            tool_request(13, "task_status", {"unique_name": "long-running-task"}),
        )
        task_status_job = find_job(parse_tool_result(task_status_response)["jobs"], "long-running-task", pid=pid)
        assert_condition(task_status_job["state"] == "running", "task_status should report running state", task_status_job)
        assert_condition(task_status_job["exit_code"] is None, "running job should not have exit_code", task_status_job)
        assert_condition(
            task_status_job["completed_at"] is None,
            "running job should not have completed_at",
            task_status_job,
        )

        output_response, _ = send_request(
            process,
            tool_request(14, "task_output", {"pid": pid, "lines": 50, "show_truncation": True}),
        )
        output_payload = parse_tool_result(output_response)
        assert_condition(output_payload["pid"] == pid, "task_output pid mismatch", output_payload)
        assert_condition(output_payload["lines"], "task_output should contain captured lines", output_payload)

        stop_response, _ = send_request(
            process,
            tool_request(15, "task_stop", {"pid": pid, "grace_period": 2}),
            timeout_seconds=10,
        )
        stop_payload = parse_tool_result(stop_response)
        assert_condition(stop_payload["pid"] == pid, "task_stop pid mismatch", stop_payload)
        assert_condition(
            stop_payload["status"] in {"graceful", "killed", "failed"},
            "unexpected task_stop status",
            stop_payload,
        )

        status_after_stop, _ = send_request(process, tool_request(16, "status"))
        running_after_stop = parse_tool_result(status_after_stop)["running"]
        assert_condition(
            all(job.get("pid") != pid for job in running_after_stop),
            "stopped task should disappear from running list",
            running_after_stop,
        )
        print("✓ running-task lifecycle works through stop")
        return True
    finally:
        stop_process(process)


def test_nonexistent_job_tools():
    print("Test 10: task_output and task_stop reject nonexistent jobs")
    process, _ = start_mcp_process()
    try:
        output_response, _ = send_request(
            process,
            tool_request(17, "task_output", {"pid": 99999, "lines": 10, "show_truncation": True}),
        )
        assert_condition("error" in output_response, "task_output should error for bad pid", output_response)

        stop_response, _ = send_request(
            process,
            tool_request(18, "task_stop", {"pid": 99999, "grace_period": 1}),
        )
        assert_condition("error" in stop_response, "task_stop should error for bad pid", stop_response)
        print("✓ nonexistent-job tools return MCP errors")
        return True
    finally:
        stop_process(process)


def test_logging_severity_classification():
    print("Test 11: stderr logging notifications use info, warning, and error levels correctly")
    process, _ = start_mcp_process()
    try:
        response, notifications = send_request(
            process,
            tool_request(
                19,
                "task_start",
                {
                    "unique_name": "stderr-level-task",
                    "wait_for_exit_seconds": 3,
                },
            ),
            timeout_seconds=10,
        )
        payload = parse_tool_result(response)
        assert_condition(payload["state"] == "exited", "stderr-level-task should exit", payload)

        logs = logging_notifications(notifications)
        stderr_logs = [
            log
            for log in logs
            if log.get("params", {}).get("data", {}).get("type") == "stderr"
        ]
        assert_condition(
            len(stderr_logs) == 1,
            "stderr fixture should be delivered as one batched notification",
            stderr_logs,
        )
        batch = stderr_logs[0].get("params", {}).get("data", {})
        lines = batch.get("lines", [])
        batch_level = stderr_logs[0].get("params", {}).get("level")

        assert_condition(
            "plain stderr line" in lines,
            "plain stderr line missing from batch",
            batch,
        )
        assert_condition(
            "warning: this is a warning" in lines,
            "warning stderr line missing from batch",
            batch,
        )
        assert_condition(
            "error: this is an error" in lines,
            "error stderr line missing from batch",
            batch,
        )
        assert_condition(batch_level == "error", "batch severity should escalate to error", stderr_logs[0])
        assert_condition("line" not in batch, "batched payload should not include singular line", batch)
        assert_condition("entries" not in batch, "batched payload should not include per-line entries", batch)
        assert_condition("chunk" not in batch, "batched payload should not include chunk", batch)
        assert_condition("byte_count" not in batch, "batched payload should not include byte_count", batch)
        assert_condition("line_count" not in batch, "batched payload should not include line_count", batch)
        print("✓ stderr notification levels are classified correctly")
        return True
    finally:
        stop_process(process)


def main():
    tests = [
        test_initialize_instructions,
        test_tools_list_schema,
        test_list_tasks_enriched_fields,
        test_task_start_quick_exit,
        test_task_start_args_and_spaces,
        test_error_taxonomy,
        test_bounded_wait_completion,
        test_task_status_completion_metadata,
        test_running_lifecycle_and_stop,
        test_nonexistent_job_tools,
        test_logging_severity_classification,
    ]

    print("Starting MCP protocol integration tests...")
    for test in tests:
        try:
            if not test():
                return 1
        except Exception as exc:
            return 1 if fail(f"{test.__name__} failed: {exc}") else 1

    print("✓ All MCP protocol integration tests passed!")
    return 0


if __name__ == "__main__":
    sys.exit(main())
