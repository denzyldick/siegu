<template>
  <v-card variant="flat" color="surface" rounded="xl" class="mb-6 border overflow-hidden">
    <v-card-item class="py-4">
      <template v-slot:prepend>
        <v-avatar color="surface" size="32" class="mr-3">
          <v-icon color="on-surface" size="small">mdi-translate</v-icon>
        </v-avatar>
      </template>
      <v-card-title class="text-h6 text-high-emphasis font-weight-bold">{{
        $t('language.label')
      }}</v-card-title>
    </v-card-item>
    <v-card-text class="pt-4">
      <v-select
        :model-value="currentLang"
        @update:model-value="onLanguageChange"
        :items="languages"
        item-title="label"
        item-value="code"
        variant="solo-filled"
        density="compact"
        hide-details
        flat
        rounded="lg"
      ></v-select>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { useI18n } from 'vue-i18n';

interface LanguageOption {
  code: string;
  label: string;
}

const props = defineProps<{
  initialLang: string;
}>();

const { t } = useI18n();
const currentLang = ref(props.initialLang);

const languages: LanguageOption[] = ['en', 'nl', 'fr', 'es', 'pap', 'de', 'it', 'pt'].map(
  (code) => ({
    code,
    label: t(`language.${code}`),
  }),
);

function onLanguageChange(code: string): void {
  currentLang.value = code;
  localStorage.setItem('siegu_language', code);
  window.location.reload();
}
</script>
