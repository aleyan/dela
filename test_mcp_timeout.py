#!/usr/bin/env python3
"""
Test script to verify that task_start with test_mcp returns within 1 second
and doesn't timeout after 15 seconds.
"""

import json
import subprocess
import time
import sys

def test_task_start_timeout():
    """Test that task_start with test_mcp returns within 1 second"""
    
    # Create a test directory with a Makefile that has a test_mcp task
    test_dir = "/tmp/dela_test_mcp"
    subprocess.run(["mkdir", "-p", test_dir], check=True)
    
    # Create a Makefile with a test_mcp task that takes longer than 1 second
    makefile_content = '''test_mcp:
\techo "Starting test_mcp task..."
\tsleep 5
\techo "test_mcp task completed"
'''
    
    with open(f"{test_dir}/Makefile", "w") as f:
        f.write(makefile_content)
    
    # Start the MCP server
    print("Starting MCP server...")
    server_process = subprocess.Popen(
        ["/Users/alex/Projects/dela/target/debug/dela", "mcp"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        cwd=test_dir
    )
    
    try:
        # Wait a moment for server to start
        time.sleep(1)
        
        # Send initialize request
        init_request = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "test-client",
                    "version": "1.0.0"
                }
            }
        }
        
        print("Sending initialize request...")
        server_process.stdin.write(json.dumps(init_request) + "\n")
        server_process.stdin.flush()
        
        # Read initialize response
        response_line = server_process.stdout.readline()
        print(f"Initialize response: {response_line.strip()}")
        
        # Send initialized notification
        initialized_notification = {
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        }
        
        print("Sending initialized notification...")
        server_process.stdin.write(json.dumps(initialized_notification) + "\n")
        server_process.stdin.flush()
        
        # Send list_tasks request
        list_tasks_request = {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "list_tasks",
                "arguments": {}
            }
        }
        
        print("Sending list_tasks request...")
        server_process.stdin.write(json.dumps(list_tasks_request) + "\n")
        server_process.stdin.flush()
        
        # Read list_tasks response
        response_line = server_process.stdout.readline()
        print(f"List tasks response: {response_line.strip()}")
        
        # Check for any stderr output
        stderr_output = server_process.stderr.read()
        if stderr_output:
            print(f"Server stderr: {stderr_output}")
        
        # Send task_start request for test_mcp
        task_start_request = {
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "task_start",
                "arguments": {
                    "unique_name": "test_mcp"
                }
            }
        }
        
        print("Sending task_start request for test_mcp...")
        start_time = time.time()
        server_process.stdin.write(json.dumps(task_start_request) + "\n")
        server_process.stdin.flush()
        
        # Read task_start response
        response_line = server_process.stdout.readline()
        end_time = time.time()
        duration = end_time - start_time
        
        print(f"Task start response: {response_line.strip()}")
        print(f"Response time: {duration:.2f} seconds")
        
        if duration > 2.0:  # Allow some buffer, but should be much less than 15 seconds
            print(f"❌ FAIL: task_start took {duration:.2f} seconds (expected < 2 seconds)")
            return False
        else:
            print(f"✅ PASS: task_start took {duration:.2f} seconds")
            return True
            
    finally:
        # Clean up
        server_process.terminate()
        server_process.wait()
        subprocess.run(["rm", "-rf", test_dir], check=True)

if __name__ == "__main__":
    success = test_task_start_timeout()
    sys.exit(0 if success else 1)
