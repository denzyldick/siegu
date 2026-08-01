export interface SearchTag {
  title: string
  type: string
}

export type FacetType = 'person' | 'location' | 'tag' | 'month'

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

export interface SearchStats {
  photos: number
  videos: number
  favorites: number
  ocr_photos: number
  faces: number
  named_people: number
}

export interface SearchFacetsData {
  people: FacetGroup[]
  unnamed_faces: FacetGroup[]
  locations: FacetCount[]
  tags: FacetCount[]
  months: FacetCount[]
  stats: SearchStats
}

export interface ActiveFilter {
  type: FacetType
  value: string
  label: string
}
