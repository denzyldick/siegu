<script setup lang="ts">
import { ref } from 'vue'
import { usePeople } from '@/composables/usePeople'
import type { Person, UnnamedFace } from '@/types/person'
import PeopleHeader from './people/PeopleHeader.vue'
import PeopleGrid from './people/PeopleGrid.vue'
import UnnamedFacesGrid from './people/UnnamedFacesGrid.vue'
import PeopleEmptyState from './people/PeopleEmptyState.vue'
import NameDialog from './people/NameDialog.vue'
import ManageDialog from './people/ManageDialog.vue'
import ClusterDialog from './people/ClusterDialog.vue'

const emit = defineEmits<{
  'search-person': [person: Person]
}>()

const {
  people,
  unnamedFaces,
  indexingCount,
  fetchData,
  startIndexing,
  saveName,
  renamePersonById,
  mergePersonById,
  fetchClusterFaces,
  removeFace,
} = usePeople()

const nameDialog = ref(false)
const manageDialog = ref(false)
const clusterDialog = ref(false)
const activeFace = ref<Person | null>(null)
const activePerson = ref<Person | null>(null)
const activeCluster = ref<Person | null>(null)
const clusterFaces = ref<UnnamedFace[]>([])

function viewPerson(person: Person): void {
  emit('search-person', person)
}

function promptName(group: Person): void {
  activeFace.value = group
  nameDialog.value = true
}

function openManageDialog(person: Person): void {
  activePerson.value = person
  manageDialog.value = true
}

async function handleViewCluster(group: Person): Promise<void> {
  activeCluster.value = group
  const faces = await fetchClusterFaces(group.id)
  clusterFaces.value = faces
  clusterDialog.value = true
}

function promptNameFromCluster(): void {
  if (activeCluster.value) promptName(activeCluster.value)
}

async function handleRemoveFace(faceId: number): Promise<void> {
  if (!confirm('Remove this face?')) return
  const ok = await removeFace(faceId)
  if (ok) {
    clusterFaces.value = clusterFaces.value.filter((f) => f.face_id !== faceId)
    if (clusterFaces.value.length === 0) clusterDialog.value = false
    fetchData()
  }
}
</script>

<template>
  <v-container class="pa-0 bg-siegu-main min-h-100 fill-height align-start" fluid>
    <PeopleHeader
      :indexing-count="indexingCount"
      :named-count="people.length"
      @start-indexing="startIndexing"
    />

    <div class="px-6 py-10 w-100 h-100">
      <PeopleGrid
        :people="people"
        @view-person="viewPerson"
        @open-manage="openManageDialog"
      />

      <UnnamedFacesGrid
        :faces="unnamedFaces"
        @view-cluster="handleViewCluster"
        @prompt-name="promptName"
      />

      <PeopleEmptyState
        v-if="people.length === 0 && unnamedFaces.length === 0"
      />
    </div>

    <NameDialog
      v-model="nameDialog"
      :active-face="activeFace"
      :people="people"
      @save="saveName"
    />

    <ManageDialog
      v-model="manageDialog"
      :active-person="activePerson"
      :people="people"
      @rename="renamePersonById"
      @merge="mergePersonById"
    />

    <ClusterDialog
      v-model="clusterDialog"
      :cluster="activeCluster"
      :faces="clusterFaces"
      @remove-face="handleRemoveFace"
      @prompt-name="promptNameFromCluster"
    />
  </v-container>
</template>

<style scoped>
.bg-siegu-main {
  background-color: var(--color-bg-primary);
}
</style>
