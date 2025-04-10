import { MessageLog, StatusResponse } from "../types/types";

const BASE_URL = import.meta.env.BASE_URL;

export const fetchStatus = async (): Promise<StatusResponse | null> => {
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
      return data.Status;
    } else {
      console.error("Unexpected response format:", data);
      throw new Error("Invalid response format");
    }
  } catch (error) {
    console.error("Error fetching status:", error);
    return null;
  }
};

export const fetchHistory = async (): Promise<MessageLog[]> => {
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
      return data.History.messages;
    } else {
      console.error("Unexpected response format:", data);
      throw new Error("Invalid response format");
    }
  } catch (error) {
    console.error("Error fetching history:", error);
    return [];
  }
};

export const clearHistory = async (): Promise<boolean> => {
  try {
    const requestData = {
      ClearHistory: null
    };
    
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
    
    return true;
  } catch (error) {
    console.error("Error clearing history:", error);
    return false;
  }
};

export const sendCustomMessage = async (
  message: string,
  messageType: string,
  messageMethod: "http" | "websocket",
  api?: any
): Promise<boolean> => {
  if (!message) return false;

  const requestData = {
    CustomMessage: {
      message_type: messageType,
      content: message
    }
  };

  try {
    if (messageMethod === "websocket") {
      if (!api) {
        throw new Error("WebSocket not connected");
      }
      api.send({ data: requestData });
    } else {
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
    return true;
  } catch (error) {
    console.error("Error sending message:", error);
    return false;
  }
}; 