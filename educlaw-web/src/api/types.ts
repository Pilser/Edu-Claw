// API types shared across the frontend

export interface LoginResponse {
  token: string
  student_id: string
  display_name: string
  role: string
  cohort_id: string
  subject_ids: string[]
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
  | 'chat_start'
  | 'chat_text'
  | 'chat_end'

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

// Chat panel message types
export type ChatRole = 'user' | 'assistant'

export interface ChatMessage {
  id: string
  role: ChatRole
  text: string        // accumulated markdown text
  pending: boolean    // true while still streaming
}

// Admin entity shapes
export interface Cohort {
  id: string
  title: string
  created_at: string
}

export interface Subject {
  id: string
  cohort_id: string
  slug: string
  title: string
  created_at: string
}

export interface Assignment {
  id: string
  subject_id: string
  title: string
  description: string
  due_date: string | null
  created_by: string
  created_at: string
}

