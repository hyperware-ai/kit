// Zustand store for Skeleton App state management
import { create } from 'zustand';
import type { SkeletonState } from '../types/skeleton';
import { getNodeId } from '../types/global';
import * as api from '../utils/api';

interface SkeletonStore extends SkeletonState {
  // Actions
  initialize: () => void;
  fetchStatus: () => Promise<void>;
  incrementCounter: (amount?: number) => Promise<void>;
  fetchMessages: () => Promise<void>;
  setError: (error: string | null) => void;
  clearError: () => void;
}

// Create the Zustand store
export const useSkeletonStore = create<SkeletonStore>((set, get) => ({
  // Initial state
  nodeId: null,
  isConnected: false,
  counter: 0,
  messages: [],
  isLoading: false,
  error: null,

  // Initialize the store and check connection
  initialize: () => {
    const nodeId = getNodeId();
    set({
      nodeId,
      isConnected: nodeId !== null,
    });
    
    // Fetch initial status if connected
    if (nodeId) {
      get().fetchStatus();
    }
  },

  // Fetch current status from backend
  fetchStatus: async () => {
    set({ isLoading: true, error: null });
    try {
      const status = await api.getStatus();
      set({
        counter: status.counter,
        isLoading: false,
      });
      
      // Also fetch messages
      await get().fetchMessages();
    } catch (error) {
      set({
        error: api.getErrorMessage(error),
        isLoading: false,
      });
    }
  },

  // Increment the counter
  incrementCounter: async (amount = 1) => {
    set({ isLoading: true, error: null });
    try {
      const newCounter = await api.incrementCounter(amount);
      set({
        counter: newCounter,
        isLoading: false,
      });
    } catch (error) {
      set({
        error: api.getErrorMessage(error),
        isLoading: false,
      });
    }
  },

  // Fetch all messages
  fetchMessages: async () => {
    try {
      const messages = await api.getMessages();
      set({ messages });
    } catch (error) {
      console.error('Failed to fetch messages:', error);
      // Don't set error state for this, as it's a secondary operation
    }
  },


  // Error management
  setError: (error) => set({ error }),
  clearError: () => set({ error: null }),
}));

// Selector hooks for common use cases
export const useNodeId = () => useSkeletonStore((state) => state.nodeId);
export const useIsConnected = () => useSkeletonStore((state) => state.isConnected);
export const useCounter = () => useSkeletonStore((state) => state.counter);
export const useMessages = () => useSkeletonStore((state) => state.messages);
export const useIsLoading = () => useSkeletonStore((state) => state.isLoading);
export const useError = () => useSkeletonStore((state) => state.error);