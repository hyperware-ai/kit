import { useState, useEffect } from "react";
import reactLogo from "./assets/react.svg";
import viteLogo from "./assets/vite.svg";
import HyperwareClientApi from "@hyperware-ai/client-api";
import "./App.css";
import { 
  MessageLog, 
  StatusResponse 
} from "./types/types";

const BASE_URL = import.meta.env.BASE_URL;
if (window.our) window.our.process = BASE_URL?.replace("/", "");

const PROXY_TARGET = `${(import.meta.env.VITE_NODE_URL || "http://localhost:8080")}${BASE_URL}`;

// This env also has BASE_URL which should match the process + package name
const WEBSOCKET_URL = import.meta.env.DEV
  ? `${PROXY_TARGET.replace('http', 'ws')}`
  : `${window.location.origin.replace('http', 'ws')}${BASE_URL}`;

console.log('WEBSOCKET URL configured as:', WEBSOCKET_URL);

function App() {
  const [customMessage, setCustomMessage] = useState("");
  const [customType, setCustomType] = useState("info");
  const [statusData, setStatusData] = useState<StatusResponse | null>(null);
  const [historyData, setHistoryData] = useState<MessageLog[]>([]);
  const [nodeConnected, setNodeConnected] = useState(true);
  const [api, setApi] = useState<HyperwareClientApi | undefined>();
  const [wsMessages, setWsMessages] = useState<string[]>([]);
  const [activeTab, setActiveTab] = useState("status");
  const [messageMethod, setMessageMethod] = useState<"http" | "websocket">("http");
  const [wsStatus, setWsStatus] = useState<'disconnected' | 'connecting' | 'connected'>('disconnected');

  useEffect(() => {
    let mounted = true;

    const connectWebSocket = async () => {
      if (!WEBSOCKET_URL || wsStatus === 'connecting') return;

      try {
        setWsStatus('connecting');
        console.log('Attempting to connect to WebSocket at:', WEBSOCKET_URL);

        const newApi = new HyperwareClientApi({
          uri: WEBSOCKET_URL,
          nodeId: window.our?.node || 'unknown',
          processId: window.our?.process || BASE_URL?.replace("/", "") || 'unknown',
          onOpen: (_event, _api) => {
            if (mounted) {
              console.log("WebSocket Connected");
              setWsStatus('connected');
              setNodeConnected(true);
            }
          },
          onClose: () => {
            if (mounted) {
              console.log("WebSocket Disconnected");
              setWsStatus('disconnected');
              setApi(undefined);
            }
          },
          onMessage: (json, _api) => {
            if (!mounted) return;

            console.log('WEBSOCKET MESSAGE', json);
            try {
              const data = JSON.parse(json);
              console.log("WebSocket received message", data);
              
              setWsMessages(prev => [...prev, JSON.stringify(data)]);
              
              if (data.type === "status_update") {
                setStatusData(data);
              }
            } catch (error) {
              console.error("Error parsing WebSocket message", error);
            }
          },
        });

        if (mounted) {
          setApi(newApi);
        }
      } catch (error) {
        console.error("Error connecting to WebSocket:", error);
        if (mounted) {
          setWsStatus('disconnected');
          setNodeConnected(false);
        }
      }
    };

    connectWebSocket();

    // Cleanup function
    return () => {
      mounted = false;
      setApi(undefined);
      setWsStatus('disconnected');
    };
  }, []); // Empty dependency array to run only once

  const reconnectWebSocket = () => {
    setApi(undefined);
    setWsStatus('disconnected');
    // The WebSocket will automatically reconnect due to the useEffect dependency
  };

  const fetchStatus = async () => {
    try {
      const response = await fetch(`${BASE_URL}/api/status`, {
        method: "GET",
        headers: {
          "Content-Type": "application/json"
        }
      });
      
      if (!response.ok) {
        const errorData = await response.json();
        throw new Error(errorData.message || "Failed to fetch status");
      }
      
      const data = await response.json();
      console.log("Status response:", data);
      
      if (data.Status) {
        setStatusData(data.Status);
      } else {
        console.error("Unexpected response format:", data);
        throw new Error("Invalid response format");
      }
    } catch (error) {
      console.error("Error fetching status:", error);
      setNodeConnected(false);
    }
  };

  const fetchHistory = async () => {
    try {
      const response = await fetch(`${BASE_URL}/api/history`, {
        method: "GET",
        headers: {
          "Content-Type": "application/json"
        }
      });
      
      if (!response.ok) {
        const errorData = await response.json();
        throw new Error(errorData.message || "Failed to fetch history");
      }
      
      const data = await response.json();
      console.log("History response:", data);
      
      if (data.History && Array.isArray(data.History.messages)) {
        setHistoryData(data.History.messages);
      } else {
        console.error("Unexpected response format:", data);
        throw new Error("Invalid response format");
      }
    } catch (error) {
      console.error("Error fetching history:", error);
    }
  };

  const clearHistory = async () => {
    try {
      // Create a properly formatted request
      const requestData = {
        ClearHistory: null
      };
      
      // Use POST request to the API endpoint with a body
      const response = await fetch(`${BASE_URL}/api`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify(requestData)
      });
      
      if (!response.ok) {
        const errorData = await response.json();
        throw new Error(errorData.message || "Failed to clear history");
      }
      
      // Refresh history data
      setHistoryData([]);
      setWsMessages(prev => [...prev, "History cleared"]);
    } catch (error) {
      console.error("Error clearing history:", error);
    }
  };

  const sendCustomMessage = async (event: React.FormEvent) => {
    event.preventDefault();

    if (!customMessage) return;

    const requestData = {
        CustomMessage: {
            message_type: customType,
            content: customMessage
        }
    };

    try {
        if (messageMethod === "websocket") {
            // Send via WebSocket only
            if (!api) {
                throw new Error("WebSocket not connected");
            }
            api.send({ data: requestData });
            setWsMessages(prev => [...prev, `Sent: ${JSON.stringify(requestData)}`]);
        } else {
            // Send via HTTP only
            const response = await fetch(`${BASE_URL}/api`, {
                method: "POST",
                headers: {
                    "Content-Type": "application/json"
                },
                body: JSON.stringify(requestData)
            });
            
            if (!response.ok) {
                const errorData = await response.json();
                throw new Error(errorData.message || "Failed to send custom message");
            }
        }
        
        setCustomMessage("");
        
        // Refresh data after sending a message
        if (activeTab === "history") {
            fetchHistory();
        } else {
            fetchStatus();
        }
    } catch (error) {
        console.error("Error sending message:", error);
    }
  };

  // When tab changes, fetch appropriate data
  useEffect(() => {
    if (activeTab === "history") {
      fetchHistory();
    } else if (activeTab === "status") {
      fetchStatus();
    }
  }, [activeTab]);

  return (
    <div style={{ width: "100%" }}>
      <div style={{ position: "absolute", top: 4, left: 8 }}>
        Node ID: <strong>{window.our?.node}</strong>
      </div>
      {!nodeConnected && (
        <div className="node-not-connected">
          <h2 style={{ color: "red" }}>Node not connected</h2>
          <h4>
            You need to start a node at {PROXY_TARGET} before you can use this UI
            in development.
          </h4>
        </div>
      )}
      <h2>Hyperware LLM Template</h2>
      <div className="card">
        <div className="tabs">
          <button 
            className={activeTab === "status" ? "active" : ""} 
            onClick={() => setActiveTab("status")}
          >
            Status
          </button>
          <button 
            className={activeTab === "history" ? "active" : ""} 
            onClick={() => setActiveTab("history")}
          >
            Message History
          </button>
          <button 
            className={activeTab === "websocket" ? "active" : ""} 
            onClick={() => setActiveTab("websocket")}
          >
            WebSocket Log
          </button>
        </div>
        
        <div
          style={{
            display: "flex",
            flexDirection: "row",
            border: "1px solid gray",
          }}
        >
          <div
            style={{
              display: "flex",
              flexDirection: "column",
              justifyContent: "space-between",
              flex: 1,
              padding: "1em",
            }}
          >
            <h3 style={{ marginTop: 0, textAlign: 'left' }}>Send Custom Message</h3>
            <form
              onSubmit={sendCustomMessage}
              style={{
                display: "flex",
                flexDirection: "column",
                width: "100%",
                marginBottom: "20px",
              }}
            >
              <div style={{ marginBottom: "10px", display: "flex", gap: "20px", alignItems: "center" }}>
                <div>
                  <label htmlFor="messageType">Message Type: </label>
                  <select 
                    id="messageType" 
                    value={customType}
                    onChange={(e) => setCustomType(e.target.value)}
                  >
                    <option value="info">Info</option>
                    <option value="warning">Warning</option>
                    <option value="error">Error</option>
                    <option value="debug">Debug</option>
                    <option value="custom">Custom</option>
                  </select>
                </div>
                <div>
                  <label htmlFor="messageMethod">Send via: </label>
                  <select
                    id="messageMethod"
                    value={messageMethod}
                    onChange={(e) => setMessageMethod(e.target.value as "http" | "websocket")}
                  >
                    <option value="http">HTTP</option>
                    <option value="websocket">WebSocket</option>
                  </select>
                </div>
              </div>
              <div className="input-row">
                <input
                  type="text"
                  id="message"
                  placeholder="Enter message content"
                  value={customMessage}
                  onChange={(event) => setCustomMessage(event.target.value)}
                  autoFocus
                />
                <button type="submit">Send</button>
              </div>
            </form>
            
            {activeTab === "status" && (
              <div>
                <div className="header-row">
                  <h3>System Status</h3>
                  <button onClick={fetchStatus}>Refresh</button>
                </div>
                <pre style={{ textAlign: "left", maxHeight: "300px", overflow: "auto" }}>
                  {statusData ? JSON.stringify(statusData, null, 2) : "No data yet"}
                </pre>
              </div>
            )}
            
            {activeTab === "history" && (
              <div>
                <div className="header-row">
                  <h3>Message History</h3>
                  <div>
                    <button onClick={fetchHistory} style={{ marginRight: "10px" }}>Refresh</button>
                    <button onClick={clearHistory}>Clear History</button>
                  </div>
                </div>
                <div style={{ maxHeight: "300px", overflow: "auto", textAlign: "left" }}>
                  {historyData.length > 0 ? (
                    <table className="message-table">
                      <thead>
                        <tr>
                          <th>Time</th>
                          <th>Source</th>
                          <th>Channel</th>
                          <th>Type</th>
                          <th>Content</th>
                        </tr>
                      </thead>
                      <tbody>
                        {historyData.map((msg, index) => (
                          <tr key={index}>
                            <td>{new Date(msg.timestamp * 1000).toLocaleTimeString()}</td>
                            <td>{msg.source}</td>
                            <td>{msg.channel}</td>
                            <td>{typeof msg.message_type === 'string' ? msg.message_type : JSON.stringify(msg.message_type)}</td>
                            <td>{msg.content || "-"}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  ) : (
                    <p>No message history available</p>
                  )}
                </div>
              </div>
            )}
            
            {activeTab === "websocket" && (
              <div>
                <div className="header-row">
                  <div>
                    <h3>WebSocket Messages</h3>
                    <div className="ws-status">
                      Status: <span className={`status-${wsStatus}`}>{wsStatus}</span>
                      {wsStatus !== 'connected' && (
                        <button 
                          onClick={reconnectWebSocket}
                          className="reconnect-button"
                        >
                          Reconnect
                        </button>
                      )}
                    </div>
                  </div>
                  <button onClick={() => setWsMessages([])}>Clear</button>
                </div>
                <div style={{ maxHeight: "300px", overflow: "auto", textAlign: "left" }}>
                  <ul className="ws-message-list">
                    {wsMessages.length > 0 ? (
                      wsMessages.map((msg, index) => (
                        <li key={index}>{msg}</li>
                      ))
                    ) : (
                      <p>No WebSocket messages yet</p>
                    )}
                  </ul>
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
      <div>
        <a href="https://vitejs.dev" target="_blank">
          <img src={viteLogo} className="logo" alt="Vite logo" />
        </a>
        <a href="https://react.dev" target="_blank">
          <img src={reactLogo} className="logo react" alt="React logo" />
        </a>
      </div>
      <p className="read-the-docs">
        Click on the Vite and React logos to learn more
      </p>
    </div>
  );
}

export default App;
