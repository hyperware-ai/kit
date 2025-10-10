import { useState, useEffect, useCallback } from "react";
import HyperwareClientApi from "@hyperware-ai/client-api";
import "./App.css";
import useIdStore from "./store/id";
import { sign, verify, ApiError } from "../../target/ui/caller-utils";

const BASE_URL = import.meta.env.BASE_URL;
if (window.our) window.our.process = BASE_URL?.replace("/", "");

const PROXY_TARGET = `${(import.meta.env.VITE_NODE_URL || "http://localhost:8080")}${BASE_URL}`;

// This env also has BASE_URL which should match the process + package name
const WEBSOCKET_URL = import.meta.env.DEV
  ? `${PROXY_TARGET.replace('http', 'ws')}`
  : undefined;

function App() {
  const { messageHistory, addSignedMessage, updateVerificationStatus } = useIdStore();
  const [message, setMessage] = useState("");
  const [nodeConnected, setNodeConnected] = useState(true);
  const [api, setApi] = useState<HyperwareClientApi | undefined>();

  useEffect(() => {
    // Connect to the Hyperdrive via websocket
    console.log('WEBSOCKET URL', WEBSOCKET_URL)
    if (window.our?.node && window.our?.process) {
      const api = new HyperwareClientApi({
        uri: WEBSOCKET_URL,
        nodeId: window.our.node,
        processId: window.our.process,
        onOpen: (_event, _api) => {
          console.log("Connected to Hyperware");
        },
        onMessage: (json, _api) => {
          console.log('WEBSOCKET MESSAGE', json)
          try {
            const data = JSON.parse(json);
            console.log("WebSocket received message", data);
          } catch (error) {
            console.error("Error parsing WebSocket message", error);
          }
        },
      });

      setApi(api);
    } else {
      setNodeConnected(false);
    }
  }, []);

  const sendMessage = useCallback(
    async (event) => {
      event.preventDefault();

      if (!message) return;

      // Create a message object
      const encoder = new TextEncoder();
      const messageBytes = encoder.encode(message);
      const messageArray = Array.from(messageBytes);

      // Send a message to the node via the sign function
      try {
        const signature = await sign(messageArray);

        // Add the message and its signature to the store
        addSignedMessage(message, signature);
        setMessage("");
      } catch (error) {
        if (error instanceof ApiError) {
          console.error("API Error:", error.message, error.details);
        } else {
          console.error(error);
        }
      }
    },
    [message, setMessage, addSignedMessage]
  );

  const verifyMessage = useCallback(
    async (index: number) => {
      const signedMessage = messageHistory.messages[index];
      if (!signedMessage) return;

      // Create message bytes
      const encoder = new TextEncoder();
      const messageBytes = encoder.encode(signedMessage.message);
      const messageArray = Array.from(messageBytes);

      // Send a verification request via the verify function
      try {
        const isValid = await verify(messageArray, signedMessage.signature);

        // Update the verification status in the store
        updateVerificationStatus(index, isValid);
      } catch (error) {
        if (error instanceof ApiError) {
          console.error("API Error:", error.message, error.details);
        } else {
          console.error(error);
        }
      }
    },
    [messageHistory, updateVerificationStatus]
  );

  return (
    <div style={{ width: "100%" }}>
      <div style={{ position: "absolute", top: 4, left: 8 }}>
        ID: <strong>{window.our?.node}</strong>
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
      <h2>Signature Verifier</h2>
      <div className="card">
        <div style={{ border: "1px solid gray", padding: "1em" }}>
          <h3 style={{ marginTop: 0, textAlign: 'left' }}>Message History</h3>
          <div>
            <ul className="message-list">
              {messageHistory.messages.map((signedMessage, index) => (
                <li key={index} className="signed-message">
                  <div className="message-content">
                    <span className="message-text">{signedMessage.message}</span>
                    <span className="message-signature">
                      Signature: [{signedMessage.signature.slice(0, 5).join(', ')}
                      {signedMessage.signature.length > 5 ? '...' : ''}]
                    </span>
                  </div>
                  <div className="verification">
                    <button
                      onClick={() => verifyMessage(index)}
                      className="verify-button"
                    >
                      Verify
                    </button>
                    {signedMessage.verified !== undefined && (
                      <span
                        className={`verification-result ${signedMessage.verified ? 'verified' : 'failed'}`}
                      >
                        {signedMessage.verified ? '✓' : '✗'}
                      </span>
                    )}
                  </div>
                </li>
              ))}
            </ul>
          </div>
          <form
            onSubmit={sendMessage}
            style={{
              display: "flex",
              flexDirection: "column",
              width: "100%",
              marginTop: "1em",
            }}
          >
            <div className="input-row">
              <input
                type="text"
                id="message"
                placeholder="Enter message to sign"
                value={message}
                onChange={(event) => setMessage(event.target.value)}
                autoFocus
              />
              <button type="submit">Sign</button>
            </div>
          </form>
        </div>
      </div>
    </div>
  );
}

export default App;
