import { create } from 'zustand'
import { HyperappTodoState, TodoItem } from '../types/hyperapp-todo' // Updated import
import { persist, createJSONStorage } from 'zustand/middleware'

export interface HyperappTodoStore extends HyperappTodoState {
  setTasks: (tasks: TodoItem[]) => void; // Renamed action
  get: () => HyperappTodoStore;
  set: (partial: HyperappTodoStore | Partial<HyperappTodoStore>) => void;
}

// Kept store hook name the same for simplicity, but could be renamed e.g. useTodoStore
const useHyperappTodoStore = create<HyperappTodoStore>()( 
  persist(
    (set, get) => ({
      tasks: [], // Initialize state with an empty array of TodoItems
      setTasks: (newTasks: TodoItem[]) => { // Renamed action implementation
        set({ tasks: newTasks });
      },
      get,
      set,
    }),
    {
      name: 'hyperapp-todo-store', // Changed persistence key for clarity
      storage: createJSONStorage(() => sessionStorage), 
    }
  )
)

export default useHyperappTodoStore; 