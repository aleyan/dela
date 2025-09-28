#!/usr/bin/env python3
"""
Simple MCP protocol test using Python subprocess and JSON-RPC
"""
import json
import subprocess
import sys
import time
import os

def start_mcp_process(cwd):
    """Start MCP process and perform handshake"""
    env = os.environ.copy()
    env.update({
        "RUST_LOG": "warn",
        "MCPI_NO_COLOR": "1"
    })
    
    process = subprocess.Popen(
        ["/usr/local/bin/dela", "mcp", "--cwd", cwd],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        cwd=cwd,
        env=env
    )
    
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
    
    process.stdin.write(json.dumps(init_request) + "\n")
    process.stdin.flush()
    
    # Wait for initialize response
    time.sleep(1)
    
    # Send initialized notification
    initialized_notification = {
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }
    
    process.stdin.write(json.dumps(initialized_notification) + "\n")
    process.stdin.flush()
    
    # Wait for handshake to complete
    time.sleep(1)
    
    return process

def test_mcp_protocol():
    """Test MCP protocol communication"""
    print("Starting MCP protocol integration tests...")
    
    # Test 1: Test MCP initialize handshake
    print("Test 1: Testing MCP initialize handshake")
    
    # Start dela mcp process
    cwd = "/home/testuser/test_project"
    process = start_mcp_process(cwd)
    
    try:
        # Read response
        stdout, stderr = process.communicate(timeout=5)
        
        if "jsonrpc" in stdout and "2.0" in stdout:
            print("✓ MCP initialize handshake works")
        else:
            print("✗ MCP initialize handshake failed")
            print("STDOUT:", stdout)
            print("STDERR:", stderr)
            return False
            
    except subprocess.TimeoutExpired:
        print("✗ MCP initialize handshake timed out")
        process.kill()
        return False
    except Exception as e:
        print(f"✗ MCP initialize handshake error: {e}")
        process.kill()
        return False
    finally:
        if process.poll() is None:
            process.kill()
    
    # Test 2: Test MCP tools/list
    print("Test 2: Testing MCP tools/list")
    
    process = start_mcp_process(cwd)
    
    try:
        # Send tools/list request
        tools_request = {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }
        
        process.stdin.write(json.dumps(tools_request) + "\n")
        process.stdin.flush()
        
        # Wait for response
        time.sleep(2)
        
        # Read response
        stdout, stderr = process.communicate(timeout=5)
        
        if "list_tasks" in stdout and "task_start" in stdout:
            print("✓ MCP tools/list works")
        else:
            print("✗ MCP tools/list failed")
            print("STDOUT:", stdout)
            print("STDERR:", stderr)
            return False
            
    except subprocess.TimeoutExpired:
        print("✗ MCP tools/list timed out")
        process.kill()
        return False
    except Exception as e:
        print(f"✗ MCP tools/list error: {e}")
        process.kill()
        return False
    finally:
        if process.poll() is None:
            process.kill()
    
    # Test 3: Test MCP list_tasks
    print("Test 3: Testing MCP list_tasks")
    
    process = start_mcp_process(cwd)
    
    try:
        # Send list_tasks request
        list_tasks_request = {
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "list_tasks",
                "arguments": {}
            }
        }
        
        process.stdin.write(json.dumps(list_tasks_request) + "\n")
        process.stdin.flush()
        
        # Wait for response
        time.sleep(2)
        
        # Read response
        stdout, stderr = process.communicate(timeout=5)
        
        if "build" in stdout or "test" in stdout:
            print("✓ MCP list_tasks works")
        else:
            print("✗ MCP list_tasks failed")
            print("STDOUT:", stdout)
            print("STDERR:", stderr)
            return False
            
    except subprocess.TimeoutExpired:
        print("✗ MCP list_tasks timed out")
        process.kill()
        return False
    except Exception as e:
        print(f"✗ MCP list_tasks error: {e}")
        process.kill()
        return False
    finally:
        if process.poll() is None:
            process.kill()
    
    # Test 4: Test MCP status tool (DTKT-172)
    print("Test 4: Testing MCP status tool (DTKT-172)")
    
    process = start_mcp_process(cwd)
    
    try:
        # Send status request
        status_request = {
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "status",
                "arguments": {}
            }
        }
        
        process.stdin.write(json.dumps(status_request) + "\n")
        process.stdin.flush()
        
        # Wait for response
        time.sleep(2)
        
        # Read response
        stdout, stderr = process.communicate(timeout=5)
        
        # Parse JSON response to verify structure
        try:
            lines = stdout.strip().split('\n')
            json_response = None
            for line in lines:
                if line.strip().startswith('{') and '"id":4' in line:
                    json_response = json.loads(line)
                    break
            
            if json_response and "result" in json_response:
                result = json_response["result"]
                if "content" in result and len(result["content"]) > 0:
                    content = result["content"][0]
                    if "text" in content:
                        status_data = json.loads(content["text"])
                        if "running" in status_data and isinstance(status_data["running"], list):
                            print("✓ MCP status tool works (returns running jobs array)")
                        else:
                            print("✗ MCP status tool failed - invalid response structure")
                            print("STDOUT:", stdout)
                            return False
                    else:
                        print("✗ MCP status tool failed - no text content")
                        print("STDOUT:", stdout)
                        return False
                else:
                    print("✗ MCP status tool failed - no content")
                    print("STDOUT:", stdout)
                    return False
            else:
                print("✗ MCP status tool failed - no result")
                print("STDOUT:", stdout)
                return False
        except json.JSONDecodeError as e:
            print(f"✗ MCP status tool failed - JSON decode error: {e}")
            print("STDOUT:", stdout)
            return False
            
    except subprocess.TimeoutExpired:
        print("✗ MCP status tool timed out")
        process.kill()
        return False
    except Exception as e:
        print(f"✗ MCP status tool error: {e}")
        process.kill()
        return False
    finally:
        if process.poll() is None:
            process.kill()
    
    # Test 5: Test MCP task_start quick execution
    print("Test 5: Testing MCP task_start quick execution")
    
    process = start_mcp_process(cwd)
    
    try:
        # Send task_start request for a quick task
        task_start_request = {
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": {
                "name": "task_start",
                "arguments": {
                    "unique_name": "test-task"
                }
            }
        }
        
        process.stdin.write(json.dumps(task_start_request) + "\n")
        process.stdin.flush()
        
        # Wait for response
        time.sleep(3)
        
        # Read response
        stdout, stderr = process.communicate(timeout=5)
        
        if "ok" in stdout and "result" in stdout:
            print("✓ MCP task_start quick execution works")
        else:
            print("✗ MCP task_start quick execution failed")
            print("STDOUT:", stdout)
            print("STDERR:", stderr)
            return False
            
    except subprocess.TimeoutExpired:
        print("✗ MCP task_start quick execution timed out")
        process.kill()
        return False
    except Exception as e:
        print(f"✗ MCP task_start quick execution error: {e}")
        process.kill()
        return False
    finally:
        if process.poll() is None:
            process.kill()
    
    # Test 6: Test MCP task_start with arguments
    print("Test 6: Testing MCP task_start with arguments")
    
    process = start_mcp_process(cwd)
    
    try:
        # Send task_start request with arguments
        task_start_request = {
            "jsonrpc": "2.0",
            "id": 6,
            "method": "tools/call",
            "params": {
                "name": "task_start",
                "arguments": {
                    "unique_name": "print-args",
                    "args": ["--verbose", "--debug"]
                }
            }
        }
        
        process.stdin.write(json.dumps(task_start_request) + "\n")
        process.stdin.flush()
        
        # Wait for response
        time.sleep(3)
        
        # Read response
        stdout, stderr = process.communicate(timeout=5)
        
        if "ok" in stdout and "result" in stdout:
            print("✓ MCP task_start with arguments works")
        else:
            print("✗ MCP task_start with arguments failed")
            print("STDOUT:", stdout)
            print("STDERR:", stderr)
            return False
            
    except subprocess.TimeoutExpired:
        print("✗ MCP task_start with arguments timed out")
        process.kill()
        return False
    except Exception as e:
        print(f"✗ MCP task_start with arguments error: {e}")
        process.kill()
        return False
    finally:
        if process.poll() is None:
            process.kill()
    
    # Test 7: Test MCP error taxonomy - TaskNotFound
    print("Test 7: Testing MCP error taxonomy - TaskNotFound")
    
    process = start_mcp_process(cwd)
    
    try:
        # Send task_start request for non-existent task
        task_start_request = {
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": {
                "name": "task_start",
                "arguments": {
                    "unique_name": "nonexistent-task"
                }
            }
        }
        
        process.stdin.write(json.dumps(task_start_request) + "\n")
        process.stdin.flush()
        
        # Wait for response
        time.sleep(2)
        
        # Read response
        stdout, stderr = process.communicate(timeout=5)
        
        if "error" in stdout and "not found" in stdout:
            print("✓ MCP error taxonomy - TaskNotFound works")
        else:
            print("✗ MCP error taxonomy - TaskNotFound failed")
            print("STDOUT:", stdout)
            print("STDERR:", stderr)
            return False
            
    except subprocess.TimeoutExpired:
        print("✗ MCP error taxonomy - TaskNotFound timed out")
        process.kill()
        return False
    except Exception as e:
        print(f"✗ MCP error taxonomy - TaskNotFound error: {e}")
        process.kill()
        return False
    finally:
        if process.poll() is None:
            process.kill()
    
    # Test 8: Test MCP error taxonomy - NotAllowlisted
    print("Test 8: Testing MCP error taxonomy - NotAllowlisted")
    
    process = start_mcp_process(cwd)
    
    try:
        # Send task_start request for task not in allowlist
        task_start_request = {
            "jsonrpc": "2.0",
            "id": 8,
            "method": "tools/call",
            "params": {
                "name": "task_start",
                "arguments": {
                    "unique_name": "custom-exe"  # This task exists but is not in allowlist
                }
            }
        }
        
        process.stdin.write(json.dumps(task_start_request) + "\n")
        process.stdin.flush()
        
        # Wait for response
        time.sleep(2)
        
        # Read response
        stdout, stderr = process.communicate(timeout=5)
        
        if "error" in stdout and ("not allowlisted" in stdout or "NotAllowlisted" in stdout):
            print("✓ MCP error taxonomy - NotAllowlisted works")
        else:
            print("✗ MCP error taxonomy - NotAllowlisted failed")
            print("STDOUT:", stdout)
            print("STDERR:", stderr)
            return False
            
    except subprocess.TimeoutExpired:
        print("✗ MCP error taxonomy - NotAllowlisted timed out")
        process.kill()
        return False
    except Exception as e:
        print(f"✗ MCP error taxonomy - NotAllowlisted error: {e}")
        process.kill()
        return False
    finally:
        if process.poll() is None:
            process.kill()
    
    # Test 9: Test MCP list_tasks enriched fields
    print("Test 9: Testing MCP list_tasks enriched fields")
    
    process = start_mcp_process(cwd)
    
    try:
        # Send list_tasks request
        list_tasks_request = {
            "jsonrpc": "2.0",
            "id": 9,
            "method": "tools/call",
            "params": {
                "name": "list_tasks",
                "arguments": {}
            }
        }
        
        process.stdin.write(json.dumps(list_tasks_request) + "\n")
        process.stdin.flush()
        
        # Wait for response
        time.sleep(2)
        
        # Read response
        stdout, stderr = process.communicate(timeout=5)
        
        # Check for enriched fields in the response
        if ("unique_name" in stdout and "source_name" in stdout and 
            "runner" in stdout and "command" in stdout and 
            "runner_available" in stdout and "allowlisted" in stdout and 
            "file_path" in stdout):
            print("✓ MCP list_tasks enriched fields work")
        else:
            print("✗ MCP list_tasks enriched fields failed")
            print("STDOUT:", stdout)
            print("STDERR:", stderr)
            return False
            
    except subprocess.TimeoutExpired:
        print("✗ MCP list_tasks enriched fields timed out")
        process.kill()
        return False
    except Exception as e:
        print(f"✗ MCP list_tasks enriched fields error: {e}")
        process.kill()
        return False
    finally:
        if process.poll() is None:
            process.kill()
    
    # Test 10: Test MCP task_status tool (DTKT-173)
    print("Test 10: Testing MCP task_status tool (DTKT-173)")
    
    process = start_mcp_process(cwd)
    
    try:
        # Send task_status request for a non-existent task
        task_status_request = {
            "jsonrpc": "2.0",
            "id": 10,
            "method": "tools/call",
            "params": {
                "name": "task_status",
                "arguments": {
                    "unique_name": "nonexistent-task"
                }
            }
        }
        
        process.stdin.write(json.dumps(task_status_request) + "\n")
        process.stdin.flush()
        
        # Wait for response
        time.sleep(2)
        
        # Read response
        stdout, stderr = process.communicate(timeout=5)
        
        # Parse JSON response to verify structure
        try:
            lines = stdout.strip().split('\n')
            json_response = None
            for line in lines:
                if line.strip().startswith('{') and '"id":10' in line:
                    json_response = json.loads(line)
                    break
            
            if json_response and "result" in json_response:
                result = json_response["result"]
                if "content" in result and len(result["content"]) > 0:
                    content = result["content"][0]
                    if "text" in content:
                        status_data = json.loads(content["text"])
                        if "jobs" in status_data and isinstance(status_data["jobs"], list):
                            print("✓ MCP task_status tool works (returns jobs array)")
                        else:
                            print("✗ MCP task_status tool failed - invalid response structure")
                            print("STDOUT:", stdout)
                            return False
                    else:
                        print("✗ MCP task_status tool failed - no text content")
                        print("STDOUT:", stdout)
                        return False
                else:
                    print("✗ MCP task_status tool failed - no content")
                    print("STDOUT:", stdout)
                    return False
            else:
                print("✗ MCP task_status tool failed - no result")
                print("STDOUT:", stdout)
                return False
        except json.JSONDecodeError as e:
            print(f"✗ MCP task_status tool failed - JSON decode error: {e}")
            print("STDOUT:", stdout)
            return False
            
    except subprocess.TimeoutExpired:
        print("✗ MCP task_status tool timed out")
        process.kill()
        return False
    except Exception as e:
        print(f"✗ MCP task_status tool error: {e}")
        process.kill()
        return False
    finally:
        if process.poll() is None:
            process.kill()
    
    print("✓ All MCP protocol integration tests passed!")
    return True

if __name__ == "__main__":
    success = test_mcp_protocol()
    sys.exit(0 if success else 1)
