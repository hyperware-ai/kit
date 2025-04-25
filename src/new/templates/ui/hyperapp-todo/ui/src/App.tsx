import { useState, useEffect, useCallback } from "react";
import HyperwareClientApi from "@hyperware-ai/client-api";
import "./App.css";
import useHyperappTodoStore from "./store/hyperapp-todo";
import { 
  TodoItem,
  AddTaskRequest, 
  GetTasksRequest,
  ToggleTaskRequest,
  AddTaskResponse,
  GetTasksResponse,
  ToggleTaskResponse
} from "./types/hyperapp-todo";

const BASE_URL = import.meta.env.BASE_URL;
if (window.our) window.our.process = BASE_URL?.replace("/", "");

const PROXY_TARGET = `${(import.meta.env.VITE_NODE_URL || "http://localhost:8080")}${BASE_URL}`;

// This env also has BASE_URL which should match the process + package name
const WEBSOCKET_URL = import.meta.env.DEV
  ? `${PROXY_TARGET.replace('http', 'ws')}/ws`
  : undefined;

function App() {
  const { tasks, setTasks } = useHyperappTodoStore();
  const [nodeConnected, setNodeConnected] = useState(true);
  const [_api, setApi] = useState<HyperwareClientApi | undefined>();
  const [newTaskText, setNewTaskText] = useState("");

  const fetchTasks = useCallback(async () => {
    const requestData: GetTasksRequest = { GetTasks: "" };

    try {
      const result = await fetch(`${BASE_URL}/api`, {
        method: "POST",
        headers: {
          'Content-Type': 'application/json'
        },
        body: JSON.stringify(requestData), 
      });

      if (!result.ok) {
        const errorText = await result.text();
        console.error(`HTTP request failed: ${result.status} ${result.statusText}. Response:`, errorText);
        throw new Error(`HTTP request failed: ${result.statusText}`);
      }
      
      const responseData = await result.json() as GetTasksResponse; 
      
      if (responseData.Ok) {
        console.log("Fetched tasks:", responseData.Ok); 
        setTasks(responseData.Ok); 
      } else {
        console.error("Error fetching tasks:", responseData.Err || "Unknown error"); 
        setTasks([]);
      }
    } catch (error) {
      console.error("Failed to fetch tasks:", error);
      setTasks([]);
    }
  }, [setTasks]);

  const handleAddTask = useCallback(async () => {
    if (!newTaskText.trim()) return;
    const requestData: AddTaskRequest = { AddTask: newTaskText };

    try {
      const result = await fetch(`${BASE_URL}/api`, {
        method: "POST",
        headers: {
          'Content-Type': 'application/json'
        },
        body: JSON.stringify(requestData),
      });

      if (!result.ok) {
        const errorText = await result.text();
        console.error(`HTTP request failed: ${result.status} ${result.statusText}. Response:`, errorText);
        throw new Error(`HTTP request failed: ${result.statusText}`);
      }

      const responseData = await result.json() as AddTaskResponse;

      if (responseData.Ok) { 
        console.log("Task added successfully:", responseData.Ok);
        setNewTaskText("");
        fetchTasks();
      } else {
        console.error("Error adding task:", responseData.Err || "Unknown error");
      }
    } catch (error) {
      console.error("Failed to add task:", error);
    }
  }, [newTaskText, fetchTasks]);

  const handleToggleComplete = useCallback(async (taskId: string) => {
    const requestData: ToggleTaskRequest = { ToggleTask: taskId };

    try {
        const result = await fetch(`${BASE_URL}/api`, {
            method: "POST",
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify(requestData),
        });

        if (!result.ok) {
            const errorText = await result.text();
            console.error(`HTTP request failed: ${result.status} ${result.statusText}. Response:`, errorText);
            throw new Error(`HTTP request failed: ${result.statusText}`);
        }

        const responseData = await result.json() as ToggleTaskResponse;

        if (responseData.Ok) {
            console.log("Task toggled successfully:", responseData.Ok);
            fetchTasks();
        } else {
            console.error("Error toggling task:", responseData.Err || "Unknown error");
        }
    } catch (error) {
        console.error("Failed to toggle task:", error);
    }
  }, [fetchTasks]);

  useEffect(() => {
    fetchTasks(); 

    if (window.our?.node && window.our?.process) {
      const api = new HyperwareClientApi({
        uri: WEBSOCKET_URL,
        nodeId: window.our.node,
        processId: window.our.process,
        onOpen: (_event, _api) => {
          console.log("Connected to Hyperware WebSocket");
        },
        onMessage: (json, _api) => {
          console.log('WEBSOCKET MESSAGE RECEIVED', json)
          try {
            const data = JSON.parse(json);
            console.log("Parsed WebSocket message", data);
          } catch (error) {
            console.error("Error parsing WebSocket message", error);
          }
        },
        onClose: () => {
            console.log("WebSocket connection closed");
        },
        onError: (error) => {
            console.error("WebSocket error:", error);
        }
      });

      setApi(api);
    } else {
      console.warn("Node or process ID not found, cannot connect WebSocket.");
      setNodeConnected(false);
    }

    return () => {
        console.log("Closing WebSocket connection (if open).")
    };
  }, [fetchTasks]);

  return (
    <div style={{ width: "100%" }}>
      <div style={{ position: "absolute", top: 4, left: 8 }}>
        ID: <strong>{window.our?.node}</strong>
      </div>
      {!nodeConnected && (
        <div className="node-not-connected">
          <h2 style={{ color: "red" }}>Node not connected</h2>
          <h4>
            Check console. Connection to {PROXY_TARGET} might be needed.
          </h4>
        </div>
      )}
      <h2>Todo List</h2>
      <div className="card">
        <div className="input-row" style={{ marginBottom: '1em' }}>
          <input 
            type="text" 
            value={newTaskText} 
            onChange={(e) => setNewTaskText(e.target.value)} 
            placeholder="Enter new task..."
            onKeyDown={(e) => e.key === 'Enter' && handleAddTask()}
          />
          <button onClick={handleAddTask}>Add Task</button>
        </div>
        <div style={{ border: "1px solid #ccc", padding: "1em", borderRadius: '0.25em' }}>
          <h3 style={{ marginTop: 0, textAlign: 'left' }}>Tasks</h3>
          <div>
            {tasks.length > 0 ? (
              <ul className="task-list"> 
                {tasks.map((task) => (
                  <li key={task.id} className={`task-item ${task.completed ? 'completed' : ''}`}>
                    <input 
                      type="checkbox"
                      checked={task.completed}
                      onChange={() => handleToggleComplete(task.id)}
                      style={{ marginRight: '0.5em' }}
                    />
                    <span className="task-text">{task.text}</span>
                  </li>
                ))}
              </ul>
            ) : (
              <p>No tasks yet. Add one above!</p>
            )}
          </div>
          <button onClick={fetchTasks} style={{ marginTop: '1em' }}>Refresh Tasks</button> 
        </div>
      </div>
    </div>
  );
}

export default App;
