<template>
  <v-container class="pa-0 bg-siegu-main min-h-100 fill-height align-start" fluid>
    <!-- Header Section -->
    <div class="header-banner px-6 pt-8 pb-10 bg-white border-bottom-subtle w-100">
      <div class="w-100">
        <div class="d-flex align-center justify-space-between flex-wrap ga-4">
          <div>
            <h1 class="text-h3 font-weight-black tracking-tight text-zinc-primary mb-2">
              {{ $t('people.title') }}
            </h1>
            <p class="text-body-1 text-zinc-secondary font-weight-medium">
              {{ $t('people.desc') }}
            </p>
          </div>
          <div class="d-flex align-center ga-3 flex-wrap">
            <v-btn
              v-if="indexingCount === 0"
              variant="flat"
              class="siegu-btn text-none font-weight-bold rounded-lg px-6 h-100 py-3"
              prepend-icon="mdi-face-recognition"
              @click="startIndexing"
            >
              {{ $t('people.index_faces') }}
            </v-btn>
            <div
              v-else
              class="d-flex align-center bg-white border-subtle rounded-lg px-4 py-2 shadow-sm animate-pulse"
            >
              <v-progress-circular
                indeterminate
                size="16"
                width="2"
                color="#18181b"
                class="mr-3"
              ></v-progress-circular>
              <div class="text-caption font-weight-black text-zinc-primary">
                {{ $t('people.indexing_remaining', { count: formatIndexingCount(indexingCount) }) }}
              </div>
            </div>

            <v-chip
              class="px-4 py-5 font-weight-bold border-subtle"
              variant="flat"
              color="white"
              rounded="lg"
            >
              <v-icon start size="18" color="#18181b">mdi-account-check</v-icon>
              {{ $t('people.named_count', { count: people.length }) }}
            </v-chip>
          </div>
        </div>
      </div>
    </div>

    <div class="px-6 py-10 w-100 h-100">
      <!-- Named People Section -->
      <section v-if="people.length > 0" class="mb-12 animate-fade-up">
        <div class="d-flex align-center mb-8 flex-nowrap">
          <h2 class="text-h5 font-weight-black text-zinc-primary pr-6 flex-shrink-0">
            {{ $t('people.your_people') }}
          </h2>
          <v-divider class="border-subtle border-opacity-100"></v-divider>
        </div>

        <v-row class="ga-y-6">
          <v-col cols="6" sm="4" md="3" lg="2" xl="1" v-for="person in people" :key="person.id">
            <v-card
              class="person-card-reimagined overflow-hidden border-subtle"
              variant="flat"
              color="white"
              rounded="xl"
              @click="viewPerson(person)"
            >
              <div class="image-wrapper pos-rel">
                <v-img
                  :src="getFaceImageSrc(person.representative_crop, person.encoded)"
                  aspect-ratio="1"
                  cover
                  class="hover-scale transition-slow"
                ></v-img>
                <v-chip
                  size="x-small"
                  color="black"
                  variant="flat"
                  class="cluster-badge font-weight-bold"
                >
                  {{ person.face_count }}
                </v-chip>
              </div>

              <div class="pa-3 bg-white text-center">
                <h3 class="text-subtitle-2 font-weight-bold text-zinc-primary text-truncate">
                  {{ person.name }}
                </h3>
              </div>

              <div class="card-action-overlay">
                <v-btn
                  icon="mdi-pencil"
                  size="x-small"
                  color="white"
                  variant="flat"
                  class="shadow-sm border-subtle"
                  @click.stop="openManageDialog(person)"
                ></v-btn>
              </div>
            </v-card>
          </v-col>
        </v-row>
      </section>

      <!-- New Identified Faces Section (Anonymous Clusters) -->
      <section
        v-if="unnamedFaces.length > 0"
        class="animate-fade-up"
        :style="{ animationDelay: '0.1s' }"
      >
        <div class="d-flex align-center mb-8 flex-nowrap">
          <h2 class="text-h5 font-weight-black text-zinc-primary pr-6 flex-shrink-0">
            {{ $t('people.new_faces') }}
          </h2>
          <v-divider class="border-subtle border-opacity-100"></v-divider>
        </div>

        <v-row class="ga-y-6">
          <v-col cols="6" sm="4" md="3" lg="2" xl="1" v-for="group in unnamedFaces" :key="group.id">
            <v-card
              class="unnamed-card-reimagined overflow-hidden border-subtle"
              variant="flat"
              color="white"
              rounded="xl"
              @click="viewCluster(group)"
            >
              <div class="image-wrapper pos-rel">
                <v-img
                  :src="getFaceImageSrc(group.representative_crop, group.encoded)"
                  aspect-ratio="1"
                  cover
                  class="hover-scale transition-slow"
                ></v-img>
                <v-chip
                  v-if="group.face_count > 1"
                  size="x-small"
                  color="black"
                  variant="flat"
                  class="cluster-badge font-weight-bold"
                >
                  {{ group.face_count }}
                </v-chip>
              </div>
              <div class="pa-2 bg-white">
                <v-btn
                  block
                  variant="flat"
                  color="#f4f4f5"
                  size="small"
                  class="text-none font-weight-bold rounded-lg py-4 text-zinc-primary border-subtle"
                  @click.stop="promptName(group)"
                >
                  {{ $t('people.name_group') }}
                </v-btn>
              </div>
            </v-card>
          </v-col>
        </v-row>
      </section>

      <!-- Empty State -->
      <div
        v-if="people.length === 0 && unnamedFaces.length === 0"
        class="d-flex flex-column align-center justify-center py-16 animate-fade-in"
      >
        <div class="empty-icon-pulse mb-8">
          <v-icon size="80" color="#d4d4d8">mdi-face-recognition</v-icon>
        </div>
        <h3 class="text-h5 font-weight-bold text-zinc-primary mb-2">{{ $t('people.no_faces') }}</h3>
        <p class="text-zinc-secondary text-center max-w-400">
          {{ $t('people.no_faces_desc') }}
        </p>
      </div>
    </div>

    <!-- Name Dialog -->
    <v-dialog v-model="nameDialog" max-width="440" transition="dialog-bottom-transition">
      <v-card class="rounded-xl pa-2 elevation-24 border-subtle" color="surface">
        <div class="pa-6">
          <div class="d-flex align-center justify-space-between mb-8">
            <h3 class="text-h5 font-weight-black text-zinc-primary">
              {{ $t('people.who_is_this') }}
            </h3>
            <v-btn icon="mdi-close" variant="text" size="small" @click="nameDialog = false"></v-btn>
          </div>

          <div class="d-flex justify-center mb-8">
            <v-avatar size="160" class="border-subtle shadow-xl elevation-2 bg-zinc-100">
              <v-img
                v-if="activeFace"
                :src="getFaceImageSrc(activeFace.representative_crop, activeFace.encoded)"
                cover
              ></v-img>
            </v-avatar>
          </div>

          <v-combobox
            v-model="newName"
            :items="people"
            item-title="name"
            item-value="name"
            :return-object="false"
            :placeholder="$t('people.name_placeholder')"
            variant="outlined"
            density="comfortable"
            class="name-field-modern mb-6"
            persistent-placeholder
            autofocus
            hide-details
            @keyup.enter="saveName"
          >
            <template v-slot:item="{ props, item }">
              <v-list-item v-bind="props" class="py-2">
                <template v-slot:prepend>
                  <v-avatar size="32" class="mr-2 border-subtle">
                    <v-img
                      :src="getFaceImageSrc(item.raw.representative_crop, item.raw.encoded)"
                    ></v-img>
                  </v-avatar>
                </template>
              </v-list-item>
            </template>
          </v-combobox>

          <v-btn
            block
            size="x-large"
            variant="flat"
            class="siegu-btn rounded-xl text-none font-weight-bold shadow-lg py-7"
            :disabled="!newName"
            @click="saveName"
          >
            {{ $t('people.confirm_identity') }}
          </v-btn>
        </div>
      </v-card>
    </v-dialog>

    <!-- Manage Dialog -->
    <v-dialog v-model="manageDialog" max-width="480" transition="scale-transition">
      <v-card class="rounded-xl pa-2 elevation-24 overflow-hidden border-subtle" color="surface">
        <div class="pa-6">
          <div class="d-flex align-center justify-space-between mb-6">
            <h3 class="text-h5 font-weight-black text-zinc-primary">
              {{ $t('people.profile_actions') }}
            </h3>
            <v-btn
              icon="mdi-close"
              variant="text"
              size="small"
              @click="manageDialog = false"
            ></v-btn>
          </div>

          <v-tabs
            v-model="manageTab"
            bg-color="#f4f4f5"
            color="#18181b"
            grow
            mandatory
            class="rounded-xl mb-8 p-1 border-subtle"
          >
            <v-tab value="rename" class="rounded-lg text-none font-weight-bold">{{
              $t('people.rename')
            }}</v-tab>
            <v-tab value="merge" class="rounded-lg text-none font-weight-bold">{{
              $t('people.merge')
            }}</v-tab>
          </v-tabs>

          <v-window v-model="manageTab" class="py-2">
            <v-window-item value="rename">
              <label class="text-caption font-weight-bold text-zinc-muted mb-2 d-block px-1">{{
                $t('people.new_name_for', { name: activePerson?.name })
              }}</label>
              <v-text-field
                v-model="newName"
                variant="outlined"
                density="comfortable"
                class="name-field-modern mb-8"
                hide-details
                @keyup.enter="renamePerson"
              ></v-text-field>

              <v-btn
                block
                size="x-large"
                variant="flat"
                class="siegu-btn rounded-xl text-none font-weight-bold py-7 shadow-lg"
                @click="renamePerson"
              >
                {{ $t('people.update_name') }}
              </v-btn>
            </v-window-item>

            <v-window-item value="merge">
              <div
                class="bg-amber-50 rounded-xl pa-4 mb-8 d-flex align-start ga-3 border-amber-subtle"
              >
                <v-icon color="#b45309" size="20" class="mt-1">mdi-alert-circle-outline</v-icon>
                <div class="text-body-2 text-amber-darken-4 font-weight-medium">
                  <span>{{ $t('people.merge_desc', { name: activePerson?.name }) }}</span>
                </div>
              </div>

              <v-select
                v-model="mergeTargetId"
                :items="otherPeople"
                item-title="name"
                item-value="id"
                :label="$t('people.target_profile')"
                variant="outlined"
                density="comfortable"
                class="name-field-modern mb-8"
                hide-details
              ></v-select>

              <v-btn
                block
                size="x-large"
                variant="flat"
                class="siegu-btn rounded-xl text-none font-weight-bold py-7 shadow-sm"
                :disabled="!mergeTargetId"
                @click="mergePerson"
              >
                {{ $t('people.confirm_merge') }}
              </v-btn>
            </v-window-item>
          </v-window>
        </div>
      </v-card>
    </v-dialog>

    <!-- Cluster View Dialog -->
    <v-dialog
      v-model="clusterDialog"
      max-width="800"
      transition="dialog-bottom-transition"
      scrollable
    >
      <v-card class="rounded-xl border-subtle overflow-hidden" color="surface">
        <v-card-title class="pa-6 bg-zinc-100 border-bottom-subtle d-flex align-center">
          <div>
            <div class="text-h5 font-weight-black text-zinc-primary">
              {{ $t('people.grouped_faces') }}
            </div>
            <div
              class="text-caption text-zinc-secondary font-weight-bold uppercase tracking-widest"
            >
              {{ $t('people.appearances_in_cluster', { count: clusterFaces.length }) }}
            </div>
          </div>
          <v-spacer></v-spacer>
          <v-btn
            icon="mdi-close"
            variant="text"
            size="small"
            @click="clusterDialog = false"
          ></v-btn>
        </v-card-title>

        <v-card-text class="pa-6">
          <v-row class="ga-4">
            <v-col cols="4" sm="3" md="2" v-for="face in clusterFaces" :key="face.face_id">
              <v-card
                variant="flat"
                border
                class="border-subtle overflow-hidden rounded-lg pos-rel group-face-card"
              >
                <v-img
                  :src="getFaceImageSrc(face.crop_path, face.encoded)"
                  aspect-ratio="1"
                  cover
                ></v-img>
                <div class="face-remove-btn">
                  <v-btn
                    icon="mdi-close"
                    size="x-small"
                    color="error"
                    variant="flat"
                    @click="removeFromCluster(face.face_id)"
                  ></v-btn>
                </div>
              </v-card>
            </v-col>
          </v-row>
        </v-card-text>

        <v-card-actions class="pa-6 bg-zinc-50 border-top-subtle">
          <v-btn
            block
            color="black"
            variant="flat"
            height="56"
            class="rounded-xl font-weight-bold text-none"
            prepend-icon="mdi-pencil"
            @click="promptNameFromCluster"
          >
            {{ $t('people.name_this_group') }}
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </v-container>
</template>

<script>
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export default {
  name: 'People',
  data: () => ({
    people: [],
    unnamedFaces: [], // Now contains anonymous person groups from backend
    nameDialog: false,
    manageDialog: false,
    activeFace: null,
    activePerson: null,
    newName: '',
    manageTab: 'rename',
    mergeTargetId: null,
    indexingCount: 0,
    unlistenProgress: null,
    clusterDialog: false,
    clusterFaces: [],
    activeCluster: null,
  }),
  computed: {
    otherPeople() {
      return this.people.filter((p) => p.id !== this.activePerson?.id);
    },
  },
  async mounted() {
    this.fetchData();

    // Check initial indexing status
    invoke('get_indexing_status').then((count) => {
      this.indexingCount = this.normalizeIndexingCount(count);
    });

    // Listen for progress updates
    this.unlistenProgress = await listen('indexing-progress', (event) => {
      this.indexingCount = this.normalizeIndexingCount(event.payload);
      if (this.indexingCount === 0) {
        this.fetchData(); // Refresh data when indexing is complete
      }
    });
  },
  beforeUnmount() {
    if (this.unlistenProgress) this.unlistenProgress();
  },
  methods: {
    normalizeIndexingCount(value) {
      const count = Number(value);
      if (!Number.isSafeInteger(count) || count < 0 || count > 1000000) return 0;
      return count;
    },
    formatIndexingCount(value) {
      return this.normalizeIndexingCount(value).toLocaleString(
        localStorage.getItem('siegu_language') || 'en',
      );
    },
    async startIndexing() {
      try {
        await invoke('index_faces');
      } catch (e) {
        console.error('Failed to start indexing:', e);
      }
    },
    async fetchData() {
      try {
        const peopleStr = await invoke('get_people');
        this.people = JSON.parse(peopleStr);
        const unnamedStr = await invoke('get_unnamed_faces');
        this.unnamedFaces = JSON.parse(unnamedStr);
      } catch (e) {
        console.error('Failed to fetch people data:', e);
      }
    },
    getFaceImageSrc(crop_path, encoded) {
      return encoded || '';
    },
    promptName(group) {
      this.activeFace = group; // group contains representative_face_id and representative_crop
      this.newName = '';
      this.nameDialog = true;
    },
    async saveName() {
      if (!this.newName || !this.activeFace) return;

      try {
        // Naming the representative face will now automatically name the entire cluster in the backend
        await invoke('assign_name_to_face', {
          faceId: this.activeFace.representative_face_id,
          name: this.newName,
        });
        this.nameDialog = false;
        this.activeFace = null;
        this.fetchData();
      } catch (e) {
        console.error('Failed to assign name:', e);
      }
    },
    viewPerson(person) {
      this.$emit('search-person', person);
    },
    openManageDialog(person) {
      this.activePerson = person;
      this.newName = person.name;
      this.manageTab = 'rename';
      this.mergeTargetId = null;
      this.manageDialog = true;
    },
    async renamePerson() {
      if (!this.activePerson || !this.newName) return;
      try {
        await invoke('rename_person', { id: this.activePerson.id, newName: this.newName });
        this.manageDialog = false;
        this.fetchData();
      } catch (e) {
        console.error('Failed to rename person:', e);
      }
    },
    async mergePerson() {
      if (!this.activePerson || !this.mergeTargetId) return;
      try {
        await invoke('merge_people', { fromId: this.activePerson.id, toId: this.mergeTargetId });
        this.manageDialog = false;
        this.fetchData();
      } catch (e) {
        console.error('Failed to merge people:', e);
      }
    },
    async viewCluster(group) {
      this.activeCluster = group;
      try {
        const facesStr = await invoke('get_person_faces', { personId: group.id });
        this.clusterFaces = JSON.parse(facesStr);
        this.clusterDialog = true;
      } catch (e) {
        console.error('Failed to fetch cluster faces:', e);
      }
    },
    promptNameFromCluster() {
      if (!this.activeCluster) return;
      this.promptName(this.activeCluster);
    },
    async removeFromCluster(faceId) {
      if (!confirm(this.$t('people.confirm_remove_face'))) return;
      try {
        await invoke('delete_face', { faceId });
        // Refresh local cluster faces
        this.clusterFaces = this.clusterFaces.filter((f) => f.face_id !== faceId);
        // If last face removed, close dialog
        if (this.clusterFaces.length === 0) {
          this.clusterDialog = false;
        }
        this.fetchData(); // Refresh main lists
      } catch (e) {
        console.error('Failed to remove face:', e);
      }
    },
  },
};
</script>

<style scoped>
.bg-siegu-main {
  background-color: var(--color-bg-primary);
}
.bg-zinc-100 {
  background-color: var(--color-bg-zinc-100);
}
.text-zinc-primary {
  color: var(--color-text-primary) !important;
}
.text-zinc-secondary {
  color: var(--color-text-secondary) !important;
}
.text-zinc-muted {
  color: var(--color-text-muted) !important;
}
.border-subtle {
  border: 1px solid var(--color-border-default) !important;
}
.border-bottom-subtle {
  border-bottom: 1px solid var(--color-border-default) !important;
}
.max-w-400 {
  max-width: 400px !important;
}

.header-banner {
  box-shadow: 0 1px 2px 0 rgba(0, 0, 0, 0.05);
}

.person-card-reimagined {
  cursor: pointer;
  background: white;
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  position: relative;
}

.person-card-reimagined:hover {
  transform: translateY(-4px);
  box-shadow: 0 12px 20px -5px rgba(0, 0, 0, 0.1) !important;
  border-color: rgba(0, 0, 0, 0.2) !important;
}

.card-action-overlay {
  position: absolute;
  top: 12px;
  right: 12px;
  opacity: 0;
  transform: scale(0.8);
  transition: all 0.2s ease;
}

.person-card-reimagined:hover .card-action-overlay {
  opacity: 1;
  transform: scale(1);
}

.unnamed-card-reimagined {
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  background: white !important;
}

.unnamed-card-reimagined:hover {
  transform: scale(1.02);
  box-shadow: 0 8px 12px -3px rgba(0, 0, 0, 0.1) !important;
}

.image-wrapper {
  background: #f1f5f9;
  overflow: hidden;
}

.hover-scale:hover {
  transform: scale(1.1);
}

.transition-slow {
  transition: all 0.6s cubic-bezier(0.4, 0, 0.2, 1);
}

.name-field-modern :deep(.v-field) {
  border-radius: 12px !important;
  background: white !important;
}

.name-field-modern :deep(.v-field__outline) {
  --v-field-border-opacity: 0.15 !important;
}

.bg-amber-50 {
  background-color: #fffbeb !important;
}
.border-amber-subtle {
  border: 1px solid #fde68a !important;
}

.empty-icon-pulse {
  animation: pulse 2s infinite ease-in-out;
}

@keyframes pulse {
  0% {
    transform: scale(1);
    opacity: 0.8;
  }
  50% {
    transform: scale(1.1);
    opacity: 1;
  }
  100% {
    transform: scale(1);
    opacity: 0.8;
  }
}

.animate-pulse {
  animation: pulse 2s infinite ease-in-out;
}

.animate-fade-up {
  animation: fadeUp 0.6s cubic-bezier(0.16, 1, 0.3, 1) forwards;
}

@keyframes fadeUp {
  from {
    opacity: 0;
    transform: translateY(20px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.animate-fade-in {
  animation: fadeIn 0.8s ease-out forwards;
}

@keyframes fadeIn {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

.tracking-tight {
  letter-spacing: -0.025em !important;
}
.shadow-xl {
  box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.25) !important;
}

.cluster-badge {
  position: absolute;
  top: 12px;
  left: 12px;
  z-index: 2;
  opacity: 0.9;
}

.group-face-card:hover .face-remove-btn {
  opacity: 1;
}

.face-remove-btn {
  position: absolute;
  top: 4px;
  right: 4px;
  opacity: 0;
  transition: opacity 0.2s ease;
  z-index: 3;
}

.pos-rel {
  position: relative;
}
</style>
