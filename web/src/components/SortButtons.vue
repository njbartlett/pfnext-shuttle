<template>
  <div class="btn-group ms-2">
    <button type="button" class="btn btn-sm" :class="{ 'btn-primary': active && ascending }" title="Sort ascending" @click="emit('sort', field, true)"><i class="bi bi-caret-down-fill"></i></button>
    <button type="button" class="btn btn-sm" :class="{ 'btn-primary': active && !ascending }" title="Sort descending" @click="emit('sort', field, false)"><i class="bi bi-caret-up-fill"></i></button>
  </div>
</template>

<script setup lang="ts">
// Ascending/descending toggle for one table column
defineProps<{ field: string; active: boolean; ascending: boolean }>()
const emit = defineEmits<{ sort: [field: string, ascending: boolean] }>()
</script>
