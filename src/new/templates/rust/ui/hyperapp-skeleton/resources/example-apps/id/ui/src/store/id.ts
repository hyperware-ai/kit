import { create } from 'zustand'
import { SignedMessage, MessageHistory } from '../types/Id'
import { persist, createJSONStorage } from 'zustand/middleware'

export interface IdStore {
  messageHistory: MessageHistory
  addSignedMessage: (message: string, signature: number[]) => void
  updateVerificationStatus: (index: number, verified: boolean) => void
  get: () => IdStore
  set: (partial: IdStore | Partial<IdStore>) => void
}

const useIdStore = create<IdStore>()(
  persist(
    (set, get) => ({
      messageHistory: { messages: [] },
      addSignedMessage: (message: string, signature: number[]) => {
        const { messageHistory } = get()
        messageHistory.messages.push({ message, signature })
        set({ messageHistory })
      },
      updateVerificationStatus: (index: number, verified: boolean) => {
        const { messageHistory } = get()
        if (index >= 0 && index < messageHistory.messages.length) {
          messageHistory.messages[index].verified = verified
          set({ messageHistory })
        }
      },
      get,
      set,
    }),
    {
      name: 'id', // unique name
      storage: createJSONStorage(() => sessionStorage), // (optional) by default, 'localStorage' is used
    }
  )
)

export default useIdStore
