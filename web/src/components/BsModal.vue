<template>
  <div ref="element" class="modal fade" tabindex="-1" aria-hidden="true">
    <div class="modal-dialog" :class="dialogClass">
      <div class="modal-content">
        <slot />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
// Bootstrap modal owned by Vue: the slot holds the modal-header/body/footer,
// and the parent calls show()/hide() through a template ref.
import { onBeforeUnmount, onMounted, ref } from 'vue'

defineProps<{ dialogClass?: string }>()
const emit = defineEmits<{ hidden: [] }>()

const element = ref<HTMLElement | null>(null)
let modal: InstanceType<typeof bootstrap.Modal> | null = null

onMounted(() => {
  if (element.value) {
    modal = new bootstrap.Modal(element.value, { focus: true, keyboard: true })
    element.value.addEventListener('hidden.bs.modal', () => emit('hidden'))
  }
})

onBeforeUnmount(() => {
  modal?.dispose()
  modal = null
})

defineExpose({
  show: () => modal?.show(),
  hide: () => modal?.hide()
})
</script>
