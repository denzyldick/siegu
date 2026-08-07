export interface AppConfig {
  language: string;
  theme: 'light' | 'dark' | 'system';
  indexed: boolean;
  directories: string[];
}

export type ThemePreference = 'light' | 'dark' | 'system';

export interface OsInfo {
  platform: string;
  arch: string;
}
