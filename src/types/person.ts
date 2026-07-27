export interface Person {
  id: number
  name: string
  face_count: number
  representative_crop: string | null
  encoded: string | null
  representative_face_id: number | null
}

export interface UnnamedFace {
  face_id: number
  person_id: number | null
  person_name: string | null
  crop_path: string
  encoded: string | null
  photo_id: number
}

export interface PersonPhoto {
  id: number
  location: string
  encoded: string | null
  created: string
  face_id: number
}

export interface FaceCluster {
  person_id: number
  faces: UnnamedFace[]
}

export interface AssignNameRequest {
  face_id: number
  name: string
}

export interface MergePeopleRequest {
  source_ids: number[]
  target_id: number
}
