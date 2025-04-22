import { create } from 'zustand'
import { FrameworkProcessState } from '../types/FrameworkProcess' // Updated import
import { persist, createJSONStorage } from 'zustand/middleware'

export interface FrameworkProcessStore extends FrameworkProcessState {
  setStateItems: (items: string[]) => void;
  get: () => FrameworkProcessStore;
  set: (partial: FrameworkProcessStore | Partial<FrameworkProcessStore>) => void;
}

const useFrameworkProcessStore = create<FrameworkProcessStore>()( // Renamed store hook
  persist(
    (set, get) => ({
      items: [], // Initialize state with an empty array
      setStateItems: (newItems: string[]) => {
        set({ items: newItems });
      },
      get,
      set,
    }),
    {
      name: 'framework-process', // Changed persistence key
      storage: createJSONStorage(() => sessionStorage), 
    }
  )
)

export default useFrameworkProcessStore; // Export renamed hook 