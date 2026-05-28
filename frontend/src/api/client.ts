import axios, { AxiosError, AxiosInstance } from 'axios'

const API_BASE_URL = import.meta.env.VITE_API_URL || '/api'

export const apiClient: AxiosInstance = axios.create({
  baseURL: API_BASE_URL,
  headers: {
    'Content-Type': 'application/json',
  },
  timeout: 30000,
})

// Request interceptor for logging/auth
apiClient.interceptors.request.use(
  (config) => {
    // Add auth token if needed
    // const token = localStorage.getItem('token')
    // if (token) {
    //   config.headers.Authorization = `Bearer ${token}`
    // }
    return config
  },
  (error) => Promise.reject(error)
)

// Response interceptor for error handling
apiClient.interceptors.response.use(
  (response) => response,
  (error: AxiosError) => {
    if (error.response) {
      // Server responded with error status
      const message = (error.response.data as { detail?: string })?.detail || error.message
      console.error(`API Error: ${error.response.status} - ${message}`)
    } else if (error.request) {
      // Request made but no response
      console.error('Network error: No response received')
    } else {
      console.error('Request setup error:', error.message)
    }
    return Promise.reject(error)
  }
)

export interface ApiError {
  status: number
  message: string
  detail?: unknown
}

export function isApiError(error: unknown): error is AxiosError {
  return axios.isAxiosError(error)
}

export function getErrorMessage(error: unknown): string {
  if (isApiError(error)) {
    return (error.response?.data as { detail?: string })?.detail || error.message
  }
  if (error instanceof Error) {
    return error.message
  }
  return 'An unexpected error occurred'
}
