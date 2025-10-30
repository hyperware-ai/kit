// Main App component for Hyperapp Skeleton
import { useEffect } from 'react';
import './App.css';
import { useHyperappSkeletonStore } from './store/hyperapp-skeleton';

function App() {
  // Store state and actions
  const {
    nodeId,
    isConnected,
    counter,
    messages,
    isLoading,
    error,
    initialize,
    fetchStatus,
    incrementCounter,
    clearError,
  } = useHyperappSkeletonStore();


  // Initialize on mount
  useEffect(() => {
    initialize();
  }, [initialize]);

  // Auto-refresh status every 30 seconds if connected
  useEffect(() => {
    if (!isConnected) return;
    
    const interval = setInterval(() => {
      fetchStatus();
    }, 30000);
    
    return () => clearInterval(interval);
  }, [isConnected, fetchStatus]);


  return (
    <div className="app">
      {/* Header */}
      <header className="app-header">
        <h1 className="app-title">🦴 HyperappSkeleton App</h1>
        <div className="node-info">
          {isConnected ? (
            <>
              Connected as <span className="node-id">{nodeId}</span>
            </>
          ) : (
            <span className="not-connected">Not connected to Hyperware</span>
          )}
        </div>
      </header>

      {/* Error display */}
      {error && (
        <div className="error error-message">
          {error}
          <button onClick={clearError} style={{ marginLeft: '1rem' }}>
            Dismiss
          </button>
        </div>
      )}

      {/* Main content */}
      {isConnected && (
        <>
          {/* Counter Section */}
          <section className="section">
            <h2 className="section-title">Counter Demo</h2>
            <p>This demonstrates basic state management and HTTP endpoints.</p>
            
            <div className="counter-section">
              <div className="counter-display">{counter}</div>
              <div className="button-group">
                <button 
                  onClick={() => incrementCounter(1)} 
                  disabled={isLoading}
                >
                  +1
                </button>
                <button 
                  onClick={() => incrementCounter(5)} 
                  disabled={isLoading}
                >
                  +5
                </button>
                <button 
                  onClick={() => incrementCounter(10)} 
                  disabled={isLoading}
                >
                  +10
                </button>
              </div>
            </div>
          </section>

          {/* Messages Section */}
          <section className="section">
            <h2 className="section-title">Messages</h2>
            <p>Messages received by this node:</p>
            
            <div className="messages-list">
              {messages.length > 0 ? (
                messages.map((msg, index) => (
                  <div key={index} className="message-item">
                    {msg}
                  </div>
                ))
              ) : (
                <div className="no-messages">No messages yet</div>
              )}
            </div>
            
            <button onClick={fetchStatus} disabled={isLoading}>
              {isLoading ? <span className="spinner" /> : 'Refresh'}
            </button>
          </section>


          {/* Instructions */}
          <section className="section">
            <h2 className="section-title">How to Use This Skeleton</h2>
            <div style={{ textAlign: 'left', maxWidth: '600px', margin: '0 auto' }}>
              <p>This HyperappSkeleton app demonstrates:</p>
              <ul>
                <li>Basic state management with a counter</li>
                <li>HTTP communication between frontend and backend</li>
                <li>Error handling and loading states</li>
                <li>Persistent state across app restarts</li>
              </ul>
              
              <p>To customize this app:</p>
              <ol>
                <li>Modify <code>AppState</code> in <code>lib.rs</code></li>
                <li>Add new HTTP endpoints with <code>#[http]</code></li>
                <li>Update the UI components and API calls</li>
                <li>Build with <code>kit b --hyperapp</code></li>
                <li>Test with <code>kit s</code></li>
              </ol>
            </div>
          </section>
        </>
      )}
    </div>
  );
}

export default App;
