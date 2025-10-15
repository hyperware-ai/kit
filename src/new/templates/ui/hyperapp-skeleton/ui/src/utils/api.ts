// API utilities for communicating with the Hyperware backend

import { BASE_URL } from '../types/global';
import type { ApiCall } from '../types/skeleton';

// Generic API call function
// All HTTP endpoints in Hyperware use POST to /api
export async function makeApiCall<TRequest, TResponse>(
  call: ApiCall<TRequest>
): Promise<TResponse> {
  const basePath =
    BASE_URL ||
    (typeof window !== 'undefined'
      ? (() => {
          const [firstSegment] = window.location.pathname.split('/').filter(Boolean);
          return firstSegment ? `/${firstSegment}` : '';
        })()
      : '');

  try {
    const response = await fetch(`${basePath}/api`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(call),
    });

    if (!response.ok) {
      const errorText = await response.text();
      throw new Error(`API call failed: ${response.status} - ${errorText}`);
    }

    const result = await response.json();
    if (result && typeof result === 'object') {
      if ('Ok' in result) {
        return (result as { Ok: TResponse }).Ok;
      }
      if ('Err' in result) {
        throw new Error(`API returned an error: ${(result as { Err: unknown }).Err}`);
      }
    }
    return result as TResponse;
  } catch (error) {
    console.error('API call error:', error);
    throw error;
  }
}

// Convenience functions for specific API calls

export async function getStatus() {
  // For methods with no parameters, pass empty string
  const response = await makeApiCall<string, string>({
    GetStatus: "",
  });
  
  // Response is a JSON string payload
  return JSON.parse(response) as { counter: number; message_count: number; node: string };
}

export async function incrementCounter(amount: number = 1) {
  // Backend expects the request body as a JSON string (per generated WIT signature)
  return makeApiCall<string, number>({
    IncrementCounter: JSON.stringify(amount),
  });
}

export async function getMessages() {
  // This returns a JSON string that we need to parse
  const response = await makeApiCall<string, string>({
    GetMessages: "",
  });
 
  // Parse the JSON string response
  return JSON.parse(response) as string[];
}


// Error handling utilities
export function isApiError(error: unknown): error is Error {
  return error instanceof Error;
}

export function getErrorMessage(error: unknown): string {
  if (isApiError(error)) {
    return error.message;
  }
  return 'An unknown error occurred';
}
