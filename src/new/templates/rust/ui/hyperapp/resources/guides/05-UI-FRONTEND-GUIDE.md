# 💻 UI/Frontend Development Guide

## Frontend Stack Overview

- **React 18** - UI framework
- **TypeScript** - Type safety
- **Zustand** - State management
- **Vite** - Build tool
- **CSS Modules** or plain CSS - Styling

## Critical Setup Requirements

### 1. The `/our.js` Script (MANDATORY)

```html
<!-- ui/index.html -->
<!doctype html>
<html lang="en">
  <head>
    <!-- ⚠️ CRITICAL: Must be FIRST script -->
    <script src="/our.js"></script>
    
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>My Hyperware App</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

### 2. Global Types Setup

```typescript
// src/types/global.ts
declare global {
  interface Window {
    our?: {
      node: string;       // e.g., "alice.os"
      process: string;    // e.g., "myapp:myapp:publisher.os"
    };
  }
}

export const BASE_URL = '';  // Empty in production

export const isHyperwareEnvironment = (): boolean => {
  return typeof window !== 'undefined' && window.our !== undefined;
};

export const getNodeId = (): string | null => {
  return window.our?.node || null;
};
```

## API Communication Patterns

### 1. Basic API Service

```typescript
// src/utils/api.ts
import { BASE_URL } from '../types/global';

// IMPORTANT: Backend HTTP methods return String or Result<String, String>
// Complex data is serialized as JSON strings that must be parsed on frontend

// Generic API call function
export async function makeApiCall<TRequest, TResponse>(
  method: string,
  data?: TRequest
): Promise<TResponse> {
  const body = data !== undefined 
    ? { [method]: data }
    : { [method]: "" };  // Empty string for no params

  const response = await fetch(`${BASE_URL}/api`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(body),
  });

  if (!response.ok) {
    const error = await response.text();
    throw new Error(`API Error: ${response.status} - ${error}`);
  }

  return response.json();
}

// Typed API methods
export const api = {
  // No parameters - backend returns JSON string
  async getStatus() {
    const response = await makeApiCall<string, string>('GetStatus', "");
    return JSON.parse(response) as StatusResponse;
  },

  // Single parameter - backend returns JSON string
  async getItem(id: string) {
    const response = await makeApiCall<string, string>('GetItem', id);
    return JSON.parse(response) as Item;
  },

  // Multiple parameters (as JSON object - common pattern)
  async createItem(name: string, description: string) {
    const response = await makeApiCall<string, string>(
      'CreateItem', 
      JSON.stringify({ name, description })
    );
    return JSON.parse(response) as CreateResponse;
  },

  // Complex object (send as JSON string)
  async updateSettings(settings: Settings) {
    return makeApiCall<string, string>(
      'UpdateSettings',
      JSON.stringify(settings)
    );
  },
};
```

### 2. Error Handling

```typescript
// src/utils/errors.ts
export class ApiError extends Error {
  constructor(
    message: string,
    public status?: number,
    public details?: unknown
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

export function getErrorMessage(error: unknown): string {
  if (error instanceof ApiError) {
    return error.message;
  }
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === 'string') {
    return error;
  }
  return 'An unknown error occurred';
}

// Wrapper with error handling
export async function apiCallWithRetry<T>(
  apiCall: () => Promise<T>,
  maxRetries = 3
): Promise<T> {
  let lastError: unknown;
  
  for (let i = 0; i < maxRetries; i++) {
    try {
      return await apiCall();
    } catch (error) {
      lastError = error;
      if (i < maxRetries - 1) {
        // Exponential backoff
        await new Promise(resolve => 
          setTimeout(resolve, Math.pow(2, i) * 1000)
        );
      }
    }
  }
  
  throw lastError;
}
```

## State Management with Zustand

### 1. Store Structure

```typescript
// src/store/app.ts
import { create } from 'zustand';
import { devtools, persist } from 'zustand/middleware';
import { immer } from 'zustand/middleware/immer';

interface AppState {
  // Connection
  nodeId: string | null;
  isConnected: boolean;
  
  // Data
  items: Item[];
  currentItem: Item | null;
  
  // UI State
  isLoading: boolean;
  error: string | null;
  
  // Filters/Settings
  filters: {
    search: string;
    category: string | null;
    sortBy: 'name' | 'date' | 'priority';
  };
}

interface AppActions {
  // Connection
  initialize: () => void;
  
  // Data operations
  fetchItems: () => Promise<void>;
  createItem: (data: CreateItemData) => Promise<void>;
  updateItem: (id: string, updates: Partial<Item>) => Promise<void>;
  deleteItem: (id: string) => Promise<void>;
  selectItem: (id: string | null) => void;
  
  // UI operations
  setError: (error: string | null) => void;
  clearError: () => void;
  setFilter: (filter: Partial<AppState['filters']>) => void;
  
  // P2P operations
  syncWithNode: (nodeId: string) => Promise<void>;
}

export const useAppStore = create<AppState & AppActions>()(
  devtools(
    persist(
      immer((set, get) => ({
        // Initial state
        nodeId: null,
        isConnected: false,
        items: [],
        currentItem: null,
        isLoading: false,
        error: null,
        filters: {
          search: '',
          category: null,
          sortBy: 'name',
        },

        // Actions
        initialize: () => {
          const nodeId = getNodeId();
          set(state => {
            state.nodeId = nodeId;
            state.isConnected = nodeId !== null;
          });
          
          if (nodeId) {
            get().fetchItems();
          }
        },

        fetchItems: async () => {
          set(state => {
            state.isLoading = true;
            state.error = null;
          });

          try {
            const items = await api.getItems();
            set(state => {
              state.items = items;
              state.isLoading = false;
            });
          } catch (error) {
            set(state => {
              state.error = getErrorMessage(error);
              state.isLoading = false;
            });
          }
        },

        createItem: async (data) => {
          set(state => { state.isLoading = true; });

          try {
            const response = await api.createItem(data);
            
            // Optimistic update
            const newItem: Item = {
              id: response.id,
              ...data,
              createdAt: new Date().toISOString(),
            };
            
            set(state => {
              state.items.push(newItem);
              state.currentItem = newItem;
              state.isLoading = false;
            });
            
            // Refresh to ensure consistency
            await get().fetchItems();
          } catch (error) {
            set(state => {
              state.error = getErrorMessage(error);
              state.isLoading = false;
            });
            throw error; // Re-throw for form handling
          }
        },

        // ... other actions
      })),
      {
        name: 'app-storage',
        partialize: (state) => ({
          // Only persist UI preferences, not data
          filters: state.filters,
        }),
      }
    )
  )
);

// Selector hooks
export const useItems = () => {
  const { items, filters } = useAppStore();
  
  return items.filter(item => {
    if (filters.search && !item.name.toLowerCase().includes(filters.search.toLowerCase())) {
      return false;
    }
    if (filters.category && item.category !== filters.category) {
      return false;
    }
    return true;
  }).sort((a, b) => {
    switch (filters.sortBy) {
      case 'name':
        return a.name.localeCompare(b.name);
      case 'date':
        return b.createdAt.localeCompare(a.createdAt);
      case 'priority':
        return b.priority - a.priority;
    }
  });
};

export const useCurrentItem = () => useAppStore(state => state.currentItem);
export const useIsLoading = () => useAppStore(state => state.isLoading);
export const useError = () => useAppStore(state => state.error);
```

### 2. React Components

```typescript
// src/components/ItemList.tsx
import React, { useEffect } from 'react';
import { useAppStore, useItems } from '../store/app';
import { ErrorMessage } from './ErrorMessage';
import { LoadingSpinner } from './LoadingSpinner';

export const ItemList: React.FC = () => {
  const items = useItems();
  const { isLoading, error, selectItem, currentItem } = useAppStore();

  if (error) return <ErrorMessage error={error} />;
  if (isLoading && items.length === 0) return <LoadingSpinner />;

  return (
    <div className="item-list">
      {items.map(item => (
        <div
          key={item.id}
          className={`item ${currentItem?.id === item.id ? 'selected' : ''}`}
          onClick={() => selectItem(item.id)}
        >
          <h3>{item.name}</h3>
          <p>{item.description}</p>
          <span className="date">
            {new Date(item.createdAt).toLocaleDateString()}
          </span>
        </div>
      ))}
      
      {items.length === 0 && (
        <div className="empty-state">
          <p>No items found</p>
          <button onClick={() => /* open create modal */}>
            Create your first item
          </button>
        </div>
      )}
    </div>
  );
};
```

### 3. Forms with Validation

```typescript
// src/components/CreateItemForm.tsx
import React, { useState } from 'react';
import { useAppStore } from '../store/app';

interface FormData {
  name: string;
  description: string;
  category: string;
}

interface FormErrors {
  name?: string;
  description?: string;
  category?: string;
}

export const CreateItemForm: React.FC<{ onClose: () => void }> = ({ onClose }) => {
  const { createItem, isLoading } = useAppStore();
  const [formData, setFormData] = useState<FormData>({
    name: '',
    description: '',
    category: '',
  });
  const [errors, setErrors] = useState<FormErrors>({});
  const [submitError, setSubmitError] = useState<string | null>(null);

  const validate = (): boolean => {
    const newErrors: FormErrors = {};
    
    if (!formData.name.trim()) {
      newErrors.name = 'Name is required';
    } else if (formData.name.length < 3) {
      newErrors.name = 'Name must be at least 3 characters';
    }
    
    if (!formData.description.trim()) {
      newErrors.description = 'Description is required';
    }
    
    if (!formData.category) {
      newErrors.category = 'Please select a category';
    }
    
    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    
    if (!validate()) return;
    
    setSubmitError(null);
    
    try {
      await createItem(formData);
      onClose();
    } catch (error) {
      setSubmitError(getErrorMessage(error));
    }
  };

  const handleChange = (field: keyof FormData) => (
    e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement>
  ) => {
    setFormData(prev => ({ ...prev, [field]: e.target.value }));
    // Clear error when user types
    if (errors[field]) {
      setErrors(prev => ({ ...prev, [field]: undefined }));
    }
  };

  return (
    <form onSubmit={handleSubmit} className="create-form">
      <h2>Create New Item</h2>
      
      {submitError && (
        <div className="error-banner">{submitError}</div>
      )}
      
      <div className="form-group">
        <label htmlFor="name">Name *</label>
        <input
          id="name"
          type="text"
          value={formData.name}
          onChange={handleChange('name')}
          className={errors.name ? 'error' : ''}
          disabled={isLoading}
        />
        {errors.name && <span className="error-text">{errors.name}</span>}
      </div>
      
      <div className="form-group">
        <label htmlFor="description">Description *</label>
        <textarea
          id="description"
          value={formData.description}
          onChange={handleChange('description')}
          className={errors.description ? 'error' : ''}
          rows={4}
          disabled={isLoading}
        />
        {errors.description && (
          <span className="error-text">{errors.description}</span>
        )}
      </div>
      
      <div className="form-group">
        <label htmlFor="category">Category *</label>
        <select
          id="category"
          value={formData.category}
          onChange={handleChange('category')}
          className={errors.category ? 'error' : ''}
          disabled={isLoading}
        >
          <option value="">Select a category</option>
          <option value="work">Work</option>
          <option value="personal">Personal</option>
          <option value="other">Other</option>
        </select>
        {errors.category && (
          <span className="error-text">{errors.category}</span>
        )}
      </div>
      
      <div className="form-actions">
        <button type="button" onClick={onClose} disabled={isLoading}>
          Cancel
        </button>
        <button type="submit" disabled={isLoading}>
          {isLoading ? 'Creating...' : 'Create Item'}
        </button>
      </div>
    </form>
  );
};
```

## Real-time Updates with WebSockets

### WebSocket Connection

```typescript
// src/hooks/useWebSocket.ts
import { useEffect, useRef } from 'react';

export function useWebSocket(
  url: string,
  onMessage: (data: any) => void,
  onConnect?: () => void,
  onDisconnect?: () => void
) {
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout>>();
  
  useEffect(() => {
    const connect = () => {
      try {
        // Determine protocol based on current page
        const wsProtocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const wsUrl = `${wsProtocol}//${window.location.host}${url}`;
        
        const ws = new WebSocket(wsUrl);
        wsRef.current = ws;
        
        ws.onopen = () => {
          console.log('WebSocket connected');
          onConnect?.();
        };
        
        ws.onmessage = (event) => {
          try {
            const data = JSON.parse(event.data);
            onMessage(data);
          } catch (error) {
            console.error('Failed to parse WebSocket message:', error);
          }
        };
        
        ws.onerror = (error) => {
          console.error('WebSocket error:', error);
        };
        
        ws.onclose = () => {
          console.log('WebSocket disconnected');
          onDisconnect?.();
          
          // Auto-reconnect after 3 seconds
          reconnectTimeoutRef.current = setTimeout(connect, 3000);
        };
      } catch (error) {
        console.error('Failed to create WebSocket:', error);
      }
    };
    
    connect();
    
    return () => {
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current);
      }
      if (wsRef.current) {
        wsRef.current.close();
      }
    };
  }, [url]);
  
  const send = (data: any) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(data));
    }
  };
  
  return { send, ws: wsRef.current };
}

// Usage in store
export const useAppStore = create<AppState>((set, get) => ({
  // ... state
  
  connectWebSocket: (url: string) => {
    const ws = new WebSocket(url);
    
    ws.onopen = () => {
      set({ wsConnection: ws, connectionStatus: 'connected' });
      // Send initial handshake/auth
      ws.send(JSON.stringify({ type: 'auth', token: get().authToken }));
    };
    
    ws.onmessage = (event) => {
      const message = JSON.parse(event.data);
      get().handleWebSocketMessage(message);
    };
    
    ws.onclose = () => {
      set({ wsConnection: null, connectionStatus: 'disconnected' });
      // Attempt reconnect
      setTimeout(() => get().connectWebSocket(url), 3000);
    };
  },
  
  handleWebSocketMessage: (message: any) => {
    switch (message.type) {
      case 'update':
        set(state => ({
          items: [...state.items, message.data]
        }));
        break;
      case 'delete':
        set(state => ({
          items: state.items.filter(i => i.id !== message.id)
        }));
        break;
      // Handle other message types
    }
  }
}));
```

### Polling Pattern (Fallback)

For cases where WebSockets aren't available:

```typescript
// src/hooks/usePolling.ts
import { useEffect, useRef } from 'react';

export function usePolling(
  callback: () => void | Promise<void>,
  interval: number,
  enabled: boolean = true
) {
  const savedCallback = useRef(callback);
  
  useEffect(() => {
    savedCallback.current = callback;
  }, [callback]);
  
  useEffect(() => {
    if (!enabled) return;
    
    const tick = () => {
      savedCallback.current();
    };
    
    // Call immediately
    tick();
    
    const id = setInterval(tick, interval);
    return () => clearInterval(id);
  }, [interval, enabled]);
}

// Usage in component
export const LiveDataView: React.FC = () => {
  const { fetchUpdates, isConnected } = useAppStore();
  
  // Poll every 2 seconds when connected
  usePolling(
    async () => {
      try {
        await fetchUpdates();
      } catch (error) {
        console.error('Polling error:', error);
      }
    },
    2000,
    isConnected
  );
  
  return <div>...</div>;
};
```

## Common UI Patterns

### 1. Connection Status Banner

```typescript
// src/components/ConnectionStatus.tsx
export const ConnectionStatus: React.FC = () => {
  const { isConnected, nodeId } = useAppStore();
  
  if (!isConnected) {
    return (
      <div className="connection-banner error">
        <span>⚠️ Not connected to Hyperware</span>
      </div>
    );
  }
  
  return (
    <div className="connection-banner success">
      <span>✅ Connected as {nodeId}</span>
    </div>
  );
};
```

### 2. Modal System

```typescript
// src/components/Modal.tsx
import React, { useEffect } from 'react';
import { createPortal } from 'react-dom';

interface ModalProps {
  isOpen: boolean;
  onClose: () => void;
  children: React.ReactNode;
  title?: string;
}

export const Modal: React.FC<ModalProps> = ({ 
  isOpen, 
  onClose, 
  children, 
  title 
}) => {
  useEffect(() => {
    if (isOpen) {
      // Prevent body scroll
      document.body.style.overflow = 'hidden';
      
      // Close on escape
      const handleEscape = (e: KeyboardEvent) => {
        if (e.key === 'Escape') onClose();
      };
      document.addEventListener('keydown', handleEscape);
      
      return () => {
        document.body.style.overflow = '';
        document.removeEventListener('keydown', handleEscape);
      };
    }
  }, [isOpen, onClose]);
  
  if (!isOpen) return null;
  
  return createPortal(
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal" onClick={e => e.stopPropagation()}>
        {title && (
          <div className="modal-header">
            <h2>{title}</h2>
            <button className="close-button" onClick={onClose}>
              ×
            </button>
          </div>
        )}
        <div className="modal-content">
          {children}
        </div>
      </div>
    </div>,
    document.body
  );
};
```

### 3. Optimistic Updates

```typescript
// src/store/optimistic.ts
const deleteItem = async (id: string) => {
  // Optimistic update - remove immediately
  set(state => {
    state.items = state.items.filter(item => item.id !== id);
    if (state.currentItem?.id === id) {
      state.currentItem = null;
    }
  });
  
  try {
    await api.deleteItem(id);
  } catch (error) {
    // Revert on error
    await get().fetchItems();
    throw error;
  }
};

const updateItem = async (id: string, updates: Partial<Item>) => {
  // Store original for rollback
  const original = get().items.find(i => i.id === id);
  
  // Optimistic update
  set(state => {
    const index = state.items.findIndex(i => i.id === id);
    if (index !== -1) {
      state.items[index] = { ...state.items[index], ...updates };
    }
  });
  
  try {
    await api.updateItem(id, updates);
  } catch (error) {
    // Rollback
    if (original) {
      set(state => {
        const index = state.items.findIndex(i => i.id === id);
        if (index !== -1) {
          state.items[index] = original;
        }
      });
    }
    throw error;
  }
};
```

### 4. P2P Node Selector

```typescript
// src/components/NodeSelector.tsx
export const NodeSelector: React.FC = () => {
  const { knownNodes, connectToNode } = useAppStore();
  const [selectedNode, setSelectedNode] = useState('');
  const [customNode, setCustomNode] = useState('');
  const [isConnecting, setIsConnecting] = useState(false);
  
  const handleConnect = async () => {
    const nodeToConnect = customNode || selectedNode;
    if (!nodeToConnect) return;
    
    setIsConnecting(true);
    try {
      await connectToNode(nodeToConnect);
      setCustomNode('');
    } catch (error) {
      alert(`Failed to connect: ${getErrorMessage(error)}`);
    } finally {
      setIsConnecting(false);
    }
  };
  
  return (
    <div className="node-selector">
      <h3>Connect to Node</h3>
      
      <div className="node-options">
        <label>
          <input
            type="radio"
            checked={!customNode}
            onChange={() => setCustomNode('')}
          />
          Known Nodes
        </label>
        <select
          value={selectedNode}
          onChange={e => setSelectedNode(e.target.value)}
          disabled={!!customNode || isConnecting}
        >
          <option value="">Select a node...</option>
          {knownNodes.map(node => (
            <option key={node} value={node}>{node}</option>
          ))}
        </select>
      </div>
      
      <div className="node-options">
        <label>
          <input
            type="radio"
            checked={!!customNode}
            onChange={() => setCustomNode('custom')}
          />
          Custom Node
        </label>
        <input
          type="text"
          placeholder="node-name.os"
          value={customNode}
          onChange={e => setCustomNode(e.target.value)}
          disabled={!customNode || isConnecting}
        />
      </div>
      
      <button 
        onClick={handleConnect}
        disabled={(!selectedNode && !customNode) || isConnecting}
      >
        {isConnecting ? 'Connecting...' : 'Connect'}
      </button>
    </div>
  );
};
```

## Styling Best Practices

### 1. CSS Organization

```css
/* src/styles/variables.css */
:root {
  /* Colors */
  --primary: #007bff;
  --primary-hover: #0056b3;
  --danger: #dc3545;
  --success: #28a745;
  --background: #f8f9fa;
  --surface: #ffffff;
  --text: #212529;
  --text-secondary: #6c757d;
  
  /* Spacing */
  --spacing-xs: 0.25rem;
  --spacing-sm: 0.5rem;
  --spacing-md: 1rem;
  --spacing-lg: 1.5rem;
  --spacing-xl: 2rem;
  
  /* Borders */
  --border-radius: 0.25rem;
  --border-color: #dee2e6;
}

/* Dark mode */
@media (prefers-color-scheme: dark) {
  :root {
    --background: #121212;
    --surface: #1e1e1e;
    --text: #ffffff;
    --text-secondary: #adb5bd;
    --border-color: #495057;
  }
}
```

### 2. Component Styles

```css
/* src/components/ItemList.module.css */
.container {
  display: grid;
  gap: var(--spacing-md);
  padding: var(--spacing-lg);
}

.item {
  background: var(--surface);
  border: 1px solid var(--border-color);
  border-radius: var(--border-radius);
  padding: var(--spacing-md);
  cursor: pointer;
  transition: all 0.2s ease;
}

.item:hover {
  border-color: var(--primary);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

.item.selected {
  border-color: var(--primary);
  background: rgba(0, 123, 255, 0.05);
}

.emptyState {
  text-align: center;
  padding: var(--spacing-xl);
  color: var(--text-secondary);
}
```

## Performance Optimization

### 1. Memoization

```typescript
// src/components/ExpensiveList.tsx
import React, { useMemo, memo } from 'react';

interface ListItemProps {
  item: Item;
  onSelect: (id: string) => void;
}

// Memoize individual items
const ListItem = memo<ListItemProps>(({ item, onSelect }) => {
  return (
    <div onClick={() => onSelect(item.id)}>
      {item.name}
    </div>
  );
}, (prevProps, nextProps) => {
  // Custom comparison
  return (
    prevProps.item.id === nextProps.item.id &&
    prevProps.item.name === nextProps.item.name
  );
});

export const ExpensiveList: React.FC = () => {
  const items = useItems();
  const { selectItem } = useAppStore();
  
  // Memoize filtered/sorted items
  const processedItems = useMemo(() => {
    return items
      .filter(item => item.active)
      .sort((a, b) => b.priority - a.priority);
  }, [items]);
  
  return (
    <div>
      {processedItems.map(item => (
        <ListItem 
          key={item.id} 
          item={item} 
          onSelect={selectItem}
        />
      ))}
    </div>
  );
};
```

### 2. Lazy Loading

```typescript
// src/App.tsx
import React, { Suspense, lazy } from 'react';

// Lazy load heavy components
const AdminPanel = lazy(() => import('./components/AdminPanel'));
const Analytics = lazy(() => import('./components/Analytics'));

export const App: React.FC = () => {
  const { userRole } = useAppStore();
  
  return (
    <div className="app">
      <Header />
      <MainContent />
      
      <Suspense fallback={<LoadingSpinner />}>
        {userRole === 'admin' && <AdminPanel />}
        {showAnalytics && <Analytics />}
      </Suspense>
    </div>
  );
};
```

## Testing Patterns

### 1. Component Testing

```typescript
// src/components/__tests__/ItemList.test.tsx
import { render, screen, fireEvent } from '@testing-library/react';
import { ItemList } from '../ItemList';
import { useAppStore } from '../../store/app';

// Mock the store
jest.mock('../../store/app');

describe('ItemList', () => {
  const mockItems = [
    { id: '1', name: 'Item 1', description: 'Desc 1' },
    { id: '2', name: 'Item 2', description: 'Desc 2' },
  ];
  
  beforeEach(() => {
    (useAppStore as jest.Mock).mockReturnValue({
      items: mockItems,
      isLoading: false,
      error: null,
      selectItem: jest.fn(),
    });
  });
  
  it('renders all items', () => {
    render(<ItemList />);
    
    expect(screen.getByText('Item 1')).toBeInTheDocument();
    expect(screen.getByText('Item 2')).toBeInTheDocument();
  });
  
  it('calls selectItem on click', () => {
    const selectItem = jest.fn();
    (useAppStore as jest.Mock).mockReturnValue({
      items: mockItems,
      selectItem,
    });
    
    render(<ItemList />);
    fireEvent.click(screen.getByText('Item 1'));
    
    expect(selectItem).toHaveBeenCalledWith('1');
  });
});
```

## Audio/WebRTC Patterns

### Basic Audio Capture

```typescript
// src/services/audioService.ts
export class AudioService {
  private audioContext: AudioContext | null = null;
  private mediaStream: MediaStream | null = null;
  private processor: ScriptProcessorNode | null = null;
  
  async initialize() {
    this.audioContext = new (window.AudioContext || (window as any).webkitAudioContext)();
    
    // Get user media
    this.mediaStream = await navigator.mediaDevices.getUserMedia({
      audio: {
        echoCancellation: true,
        noiseSuppression: true,
        autoGainControl: true,
      }
    });
    
    const source = this.audioContext.createMediaStreamSource(this.mediaStream);
    this.processor = this.audioContext.createScriptProcessor(4096, 1, 1);
    
    this.processor.onaudioprocess = (e) => {
      const inputData = e.inputBuffer.getChannelData(0);
      this.processAudio(inputData);
    };
    
    source.connect(this.processor);
    this.processor.connect(this.audioContext.destination);
  }
  
  private processAudio(data: Float32Array) {
    // Convert to appropriate format and send via WebSocket
    const encoded = this.encodeAudio(data);
    this.sendAudioData(encoded);
  }
  
  stop() {
    this.processor?.disconnect();
    this.mediaStream?.getTracks().forEach(track => track.stop());
    this.audioContext?.close();
  }
}

// React hook for audio
export function useAudio() {
  const audioServiceRef = useRef<AudioService | null>(null);
  const [isRecording, setIsRecording] = useState(false);
  const [error, setError] = useState<string | null>(null);
  
  const startRecording = async () => {
    try {
      audioServiceRef.current = new AudioService();
      await audioServiceRef.current.initialize();
      setIsRecording(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to start audio');
    }
  };
  
  const stopRecording = () => {
    audioServiceRef.current?.stop();
    audioServiceRef.current = null;
    setIsRecording(false);
  };
  
  useEffect(() => {
    return () => {
      stopRecording();
    };
  }, []);
  
  return { startRecording, stopRecording, isRecording, error };
}
```

### Voice Activity Detection (VAD)

```typescript
// src/services/vadService.ts
export class VadService {
  private threshold: number = 0.01;
  private smoothingFactor: number = 0.95;
  private currentLevel: number = 0;
  
  processAudioLevel(audioData: Float32Array): boolean {
    // Calculate RMS (Root Mean Square)
    let sum = 0;
    for (let i = 0; i < audioData.length; i++) {
      sum += audioData[i] * audioData[i];
    }
    const rms = Math.sqrt(sum / audioData.length);
    
    // Smooth the level
    this.currentLevel = this.smoothingFactor * this.currentLevel + 
                       (1 - this.smoothingFactor) * rms;
    
    return this.currentLevel > this.threshold;
  }
  
  setThreshold(threshold: number) {
    this.threshold = threshold;
  }
}
```

## Dynamic UI Paths

For apps that serve UI at dynamic paths (e.g., `/room/<id>`, `/call/<id>`):

### Extracting Path Parameters

```typescript
// src/utils/routing.ts
export function getPathParam(paramName: string): string | null {
  const pathParts = window.location.pathname.split('/');
  const paramIndex = pathParts.indexOf(paramName);
  
  if (paramIndex !== -1 && paramIndex < pathParts.length - 1) {
    return pathParts[paramIndex + 1];
  }
  
  // For pattern like /call/<id>
  const match = window.location.pathname.match(/\/call\/([^\/]+)/);
  return match ? match[1] : null;
}

// Usage
const callId = getPathParam('call');
```

### Shared Components Pattern

```typescript
// Directory structure for shared UI
ui/
├── shared/
│   ├── components/
│   │   ├── Button.tsx
│   │   ├── Modal.tsx
│   │   └── LoadingSpinner.tsx
│   ├── services/
│   │   ├── audioService.ts
│   │   └── apiService.ts
│   ├── store/
│   │   └── baseStore.ts
│   └── types/
│       └── common.ts
├── src/           // Main UI
│   └── App.tsx
└── ui-call/       // Dynamic path UI
    └── src/
        └── App.tsx
```

### Base Store Pattern

```typescript
// shared/store/baseStore.ts
export function createBaseStore<T extends BaseState>(
  initialState: T,
  actions: (set: SetState<T>, get: GetState<T>) => BaseActions
) {
  return create<T & BaseActions>((set, get) => ({
    ...initialState,
    ...actions(set, get),
    
    // Common actions
    reset: () => set(initialState),
    setError: (error: string | null) => set({ error } as Partial<T>),
  }));
}
```

## Generated TypeScript Types

When using modern HTTP endpoints with direct type deserialization:

```typescript
// target/ui/caller-utils.ts (generated)
export interface CreateCallReq {
  defaultRole: Role;
}

export interface CallInfo {
  id: string;
  createdAt: number;
  participantCount: number;
  defaultRole: Role;
}

// API wrapper
export async function createCall(request: CreateCallReq): Promise<CallInfo> {
  const data = { CreateCall: request };
  return await apiRequest<CallInfo>('POST', '/api', data);
}
```

## P2P Chat UI Patterns (from samchat)

### Message List with Auto-Scrolling

```typescript
// Auto-scroll to bottom when new messages arrive
const messageListRef = useRef<HTMLDivElement>(null);

useEffect(() => {
  if (messageListRef.current) {
    messageListRef.current.scrollTop = messageListRef.current.scrollHeight;
  }
}, [currentConversationMessages]);

// In render
<div className="message-list" ref={messageListRef}>
  {messages.map(message => (
    <div 
      key={message.id}
      className={`message-item ${message.sender === myNodeId ? 'sent' : 'received'}`}
    >
      {/* Message content */}
    </div>
  ))}
</div>
```

### Reply Functionality UI

```typescript
const [replyingTo, setReplyingTo] = useState<ChatMessage | null>(null);

// Reply context display
{replyingTo && (
  <div className="reply-context">
    <div>
      <div style={{ fontSize: '0.8em', opacity: 0.7 }}>
        Replying to {replyingTo.sender}
      </div>
      <div style={{ fontSize: '0.9em', marginTop: '2px' }}>
        {replyingTo.content}
      </div>
    </div>
    <button
      onClick={() => setReplyingTo(null)}
      style={{ background: 'none', border: 'none', cursor: 'pointer' }}
      title="Cancel reply"
    >
      ✕
    </button>
  </div>
)}

// In message display
{message.reply_to && (
  <div className="reply-preview">
    <div style={{ opacity: 0.7 }}>↩️ {message.reply_to.sender}</div>
    <div style={{ opacity: 0.85 }}>{message.reply_to.content}</div>
  </div>
)}
```

### File Upload with Image Preview

```typescript
// File handling
const fileInputRef = useRef<HTMLInputElement>(null);
const [loadedImages, setLoadedImages] = useState<Record<string, string>>({});

// Check if file is image
const isImageFile = (mimeType: string): boolean => {
  return mimeType.startsWith('image/');
};

// Load and cache images
const loadImage = async (fileInfo: FileInfo) => {
  const response = await downloadFile(fileInfo.file_id, fileInfo.sender_node);
  const blob = new Blob([response], { type: fileInfo.mime_type });
  const dataUrl = await new Promise<string>((resolve) => {
    const reader = new FileReader();
    reader.onloadend = () => resolve(reader.result as string);
    reader.readAsDataURL(blob);
  });
  setLoadedImages(prev => ({ ...prev, [fileInfo.file_id]: dataUrl }));
};

// Display in message
{message.file_info && isImageFile(message.file_info.mime_type) && (
  <img 
    src={loadedImages[message.file_info.file_id]} 
    alt={message.file_info.file_name}
    style={{ maxWidth: '300px', maxHeight: '300px' }}
  />
)}
```

### Group Chat UI Patterns

```typescript
// Group header with member management
{currentConv?.is_group && (
  <div className="group-header">
    <div>
      <h3>👥 {currentConv.group_name || 'Unnamed Group'}</h3>
      <p>{currentConv.participants.length} members: {currentConv.participants.join(', ')}</p>
    </div>
    <button onClick={() => setShowAddMember(true)}>Add Member</button>
  </div>
)}

// Show sender names in group chats
const showSender = isGroup && message.sender !== myNodeId;
{showSender && (
  <div className="message-sender">
    {message.sender}
  </div>
)}
```

### Conversation State Management

```typescript
// Multiple conversation states
const [isCreatingNewChat, setIsCreatingNewChat] = useState(false);
const [isCreatingGroup, setIsCreatingGroup] = useState(false);
const [currentConversationId, setCurrentConversationId] = useState<string | null>(null);

// Conditional rendering based on state
{isCreatingGroup ? (
  <GroupCreationForm />
) : isCreatingNewChat ? (
  <NewChatForm />
) : currentConversationId ? (
  <ChatView />
) : (
  <EmptyState />
)}
```

### Utility Functions for Chat

```typescript
// Format timestamps for chat display
function formatDate(dateStr: string): string {
  const date = new Date(dateStr);
  const now = new Date();
  const isToday = date.toDateString() === now.toDateString();
  
  if (isToday) {
    return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  } else {
    return date.toLocaleDateString([], { month: 'short', day: 'numeric' });
  }
}

// Format file sizes
function formatFileSize(bytes: number): string {
  if (bytes < 1024) return bytes + ' B';
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
  return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
}
```

### Chat Input with Multiple Actions

```typescript
<div className="message-input-container">
  {error && <div className="error-message">{error}</div>}
  
  {/* Reply context */}
  {replyingTo && <ReplyContext />}
  
  <div style={{ display: 'flex', gap: '5px' }}>
    <input
      type="text"
      value={messageText}
      onChange={(e) => setMessageText(e.target.value)}
      onKeyDown={(e) => e.key === 'Enter' && !e.shiftKey && handleSendMessage()}
      placeholder="Type your message..."
      style={{ flex: 1 }}
    />
    <button onClick={() => fileInputRef.current?.click()} title="Attach file">
      📎
    </button>
    <button onClick={handleSendMessage} disabled={!messageText.trim()}>
      Send
    </button>
  </div>
  
  <input
    ref={fileInputRef}
    type="file"
    onChange={handleFileSelect}
    style={{ display: 'none' }}
  />
</div>
```

## Remember

1. **Always include `/our.js`** - It's mandatory
2. **Use generated caller-utils** when available for type safety
3. **Handle loading states** - Users need feedback
4. **Design for offline** - Nodes can disconnect
5. **Test with real nodes** - localhost != production
6. **Optimize renders** - React DevTools Profiler helps
7. **Keep state minimal** - Don't store derived data
8. **Error boundaries** - Catch and handle errors gracefully
9. **Request permissions** - Audio/video requires user consent
10. **Clean up resources** - Stop streams, close connections
11. **Auto-scroll messages** - New messages should be visible
12. **Show context in groups** - Display sender names when needed