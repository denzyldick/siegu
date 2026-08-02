export type FacetType =
  | 'person'
  | 'location'
  | 'tag'
  | 'month'
  | 'date'
  | 'camera'
  | 'aesthetics'
  | 'faces'
  | 'papers'
  | 'favorites'
  | 'videos'

export interface FacetGroup {
  id: string
  name: string | null
  representative_crop: string | null
  encoded: string | null
  count: number
}

export interface FacetCount {
  name: string
  count: number
}

export interface LocationGroup {
  name: string
  count: number
  photo_location: string | null
  encoded: string | null
}

export interface PhotoTile {
  id: string
  location: string
  encoded: string
  created: string
  aesthetics_score: number | null
  favorite: boolean
}

export interface SearchStats {
  photos: number
  videos: number
  favorites: number
  ocr_photos: number
  faces: number
  named_people: number
  face_photos: number
  nsfw_photos: number
}

export interface DayCount {
  date: string
  photos: number
  videos: number
}

export interface SearchFacetsData {
  people: FacetGroup[]
  unnamed_faces: FacetGroup[]
  locations: LocationGroup[]
  tags: FacetCount[]
  papers: FacetCount[]
  cameras: FacetCount[]
  months: FacetCount[]
  best_photos: PhotoTile[]
  favorite_photos: PhotoTile[]
  recent_photos: PhotoTile[]
  stats: SearchStats
}

export interface ActiveFilter {
  type: FacetType
  value: string
  label: string
}
