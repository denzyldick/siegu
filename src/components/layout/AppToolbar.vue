<script setup lang="ts">
import SearchBar from '@/components/search/SearchBar.vue'
</script>

<template>
  <v-app-bar
    elevation="0"
    color="surface"
    class="border-bottom-subtle px-2"
  >
    <v-row class="px-2 align-center no-gutters">
      <v-col class="mx-2 flex-grow-1">
        <SearchBar />
      </v-col>
    </v-row>
  </v-app-bar>
</template>

<style scoped></style>
