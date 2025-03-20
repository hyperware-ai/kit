// Define interfaces for response types
export interface MessageLog {
    source: string;
    channel: string;
    message_type: string | { [key: string]: any };
    content?: string;
    timestamp: number;
  }
  
export interface StatusResponse {
    connected_clients: number;
    message_count: number;
    message_counts_by_channel: { [key: string]: number };
  }