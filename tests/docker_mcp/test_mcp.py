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
    
    print("✓ All MCP protocol integration tests passed!")
    return True

if __name__ == "__main__":
    success = test_mcp_protocol()
    sys.exit(0 if success else 1)
