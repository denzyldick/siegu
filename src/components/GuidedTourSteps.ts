export interface Step {
  icon: string;
  titleKey: string;
  descKey: string;
  target: string | null;
  position: string;
}

export const defaultTourSteps: Step[] = [
  {
    icon: 'mdi-walk',
    titleKey: 'guided_tour.welcome_title',
    descKey: 'guided_tour.welcome_desc',
    target: null,
    position: 'bottom',
  },
  {
    icon: 'mdi-magnify',
    titleKey: 'guided_tour.search_title',
    descKey: 'guided_tour.search_desc',
    target: "[data-tour='search']",
    position: 'top',
  },
  {
    icon: 'mdi-image-multiple-outline',
    titleKey: 'guided_tour.library_title',
    descKey: 'guided_tour.library_desc',
    target: "[data-tour='photos']",
    position: 'bottom',
  },
  {
    icon: 'mdi-progress-check',
    titleKey: 'guided_tour.scan_progress_title',
    descKey: 'guided_tour.scan_progress_desc',
    target: "[data-tour='scan-progress']",
    position: 'bottom',
  },
  {
    icon: 'mdi-map-outline',
    titleKey: 'guided_tour.map_title',
    descKey: 'guided_tour.map_desc',
    target: "[data-tour='dock-map']",
    position: 'top',
  },
  {
    icon: 'mdi-laptop',
    titleKey: 'guided_tour.devices_title',
    descKey: 'guided_tour.devices_desc',
    target: "[data-tour='dock-devices']",
    position: 'top',
  },
  {
    icon: 'mdi-cog-outline',
    titleKey: 'guided_tour.settings_title',
    descKey: 'guided_tour.settings_desc',
    target: "[data-tour='dock-settings']",
    position: 'top',
  },
  {
    icon: 'mdi-check-decagram',
    titleKey: 'guided_tour.done_title',
    descKey: 'guided_tour.done_desc',
    target: null,
    position: 'bottom',
  },
];

export const settingsTourSteps: Step[] = [
  {
    icon: 'mdi-cog-outline',
    titleKey: 'settings_tour.intro_title',
    descKey: 'settings_tour.intro_desc',
    target: "[data-tour='settings-help']",
    position: 'bottom',
  },
  {
    icon: 'mdi-folder-lock',
    titleKey: 'settings_tour.folders_title',
    descKey: 'settings_tour.folders_desc',
    target: "[data-tour='settings-folders']",
    position: 'bottom',
  },
  {
    icon: 'mdi-folder-plus',
    titleKey: 'settings_tour.folders_add_title',
    descKey: 'settings_tour.folders_add_desc',
    target: "[data-tour='settings-folders-add']",
    position: 'bottom',
  },
  {
    icon: 'mdi-robot-outline',
    titleKey: 'settings_tour.ai_title',
    descKey: 'settings_tour.ai_desc',
    target: "[data-tour='settings-ai']",
    position: 'bottom',
  },
  {
    icon: 'mdi-brain',
    titleKey: 'settings_tour.models_title',
    descKey: 'settings_tour.models_desc',
    target: "[data-tour='settings-models']",
    position: 'bottom',
  },
  {
    icon: 'mdi-image-search-outline',
    titleKey: 'settings_tour.indexing_title',
    descKey: 'settings_tour.indexing_desc',
    target: "[data-tour='settings-indexing']",
    position: 'bottom',
  },
  {
    icon: 'mdi-speedometer',
    titleKey: 'settings_tour.speed_title',
    descKey: 'settings_tour.speed_desc',
    target: "[data-tour='settings-speed']",
    position: 'bottom',
  },
  {
    icon: 'mdi-translate',
    titleKey: 'settings_tour.language_title',
    descKey: 'settings_tour.language_desc',
    target: "[data-tour='settings-language']",
    position: 'bottom',
  },
  {
    icon: 'mdi-theme-light-dark',
    titleKey: 'settings_tour.appearance_title',
    descKey: 'settings_tour.appearance_desc',
    target: "[data-tour='settings-appearance']",
    position: 'bottom',
  },
  {
    icon: 'mdi-wrench-outline',
    titleKey: 'settings_tour.maintenance_title',
    descKey: 'settings_tour.maintenance_desc',
    target: "[data-tour='settings-maintenance']",
    position: 'bottom',
  },
  {
    icon: 'mdi-text-box-outline',
    titleKey: 'settings_tour.logs_title',
    descKey: 'settings_tour.logs_desc',
    target: "[data-tour='settings-logs']",
    position: 'bottom',
  },
  {
    icon: 'mdi-database-outline',
    titleKey: 'settings_tour.storage_title',
    descKey: 'settings_tour.storage_desc',
    target: "[data-tour='settings-storage']",
    position: 'bottom',
  },
  {
    icon: 'mdi-signal-variant',
    titleKey: 'settings_tour.signalling_title',
    descKey: 'settings_tour.signalling_desc',
    target: "[data-tour='settings-signalling']",
    position: 'bottom',
  },
  {
    icon: 'mdi-update',
    titleKey: 'settings_tour.update_title',
    descKey: 'settings_tour.update_desc',
    target: "[data-tour='settings-update']",
    position: 'bottom',
  },
  {
    icon: 'mdi-information-outline',
    titleKey: 'settings_tour.about_title',
    descKey: 'settings_tour.about_desc',
    target: "[data-tour='settings-about']",
    position: 'bottom',
  },
  {
    icon: 'mdi-check-decagram',
    titleKey: 'settings_tour.done_title',
    descKey: 'settings_tour.done_desc',
    target: null,
    position: 'bottom',
  },
];
