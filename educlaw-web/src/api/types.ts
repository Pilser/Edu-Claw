// API types shared across the frontend

export interface LoginResponse {
  token: string
  student_id: string
  display_name: string
  role: string
}

export interface Session {
  session_id: string
  token: string
}

export interface FileNode {
  name: string
  path: string
  kind: 'file' | 'dir'
  size?: number
  children?: FileNode[]
}

export type WsEnvelopeType =
  | 'show_image'
  | 'file_change'
  | 'progress_update'

export interface WsEnvelope {
  type: WsEnvelopeType
  payload: unknown
}

export interface ShowImagePayload {
  path: string        // path inside student workspace
  data_url: string    // base64 PNG data-URL
}

export interface AssignmentProgress {
  assignment_id: string
  title: string
  total_score: number
  max_score: number
  submitted: boolean
  questions: QuestionProgress[]
}

export interface QuestionProgress {
  question_id: string
  prompt: string
  type: string
  answered: boolean
  correct: boolean
  score: number
  max_points: number
  attempts: number
}
