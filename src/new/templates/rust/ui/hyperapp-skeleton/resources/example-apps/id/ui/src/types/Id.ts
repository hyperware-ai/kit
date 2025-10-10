export interface SignedMessage {
  message: string
  signature: number[]
  verified?: boolean
}

export interface NewMessage {
  id: string
  author: string
  content: string
}

export interface SignIdMessage {
  Sign: number[]
}

export interface VerifyIdMessage {
  Verify: [number[], number[]]
}

export interface SignResponse {
  Ok: number[]
}

export interface VerifyResponse {
  Ok: boolean
}

// MessageHistory consists of a list of signed messages
export interface MessageHistory {
  messages: SignedMessage[]
}
