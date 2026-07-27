export function getLocale(): string {
  return localStorage.getItem('siegu_language') || 'en'
}

export function setLocale(locale: string): void {
  localStorage.setItem('siegu_language', locale)
}

export const SUPPORTED_LOCALES = [
  { code: 'en', name: 'English' },
  { code: 'nl', name: 'Nederlands' },
  { code: 'fr', name: 'Français' },
  { code: 'es', name: 'Español' },
  { code: 'pap', name: 'Papiamentu' },
  { code: 'de', name: 'Deutsch' },
  { code: 'it', name: 'Italiano' },
  { code: 'pt', name: 'Português' },
] as const
