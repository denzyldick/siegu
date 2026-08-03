export interface Album {
  id: string
  name: string
  created_at: string
  cover_photo_id: string | null
  sort_order: number
  item_count: number
}
