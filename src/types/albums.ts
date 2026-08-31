export type AlbumKind = 'manual' | 'smart' | 'trip';

export interface Album {
  id: string;
  name: string;
  created_at: string;
  cover_photo_id: string | null;
  sort_order: number;
  item_count: number;
  kind: AlbumKind;
  rule: string | null;
  updated_at: string | null;
  share_count: number;
}

export interface AlbumSectionItem {
  id: string;
  name: string;
  count: number;
  cover_encoded: string | null;
  cover_location: string | null;
  cover_crop: string | null;
  kind: string;
  album: Album | null;
}

export interface AlbumSection {
  id: string;
  items: AlbumSectionItem[];
}
