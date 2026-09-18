<template>
  <BsModal ref="modal" :dialog-class="centered ? 'modal-dialog-centered' : undefined">
    <div class="modal-header" :class="headerClass">
      <h5 class="modal-title"><i v-if="icon" :class="'bi ' + icon"></i>&nbsp;{{ title }}</h5>
      <button type="button" class="btn-close" data-bs-dismiss="modal" aria-label="Close"></button>
    </div>
    <div class="modal-body">
      <slot />
    </div>
    <div class="modal-footer">
      <button type="button" class="btn btn-secondary" data-bs-dismiss="modal">{{ cancelLabel }}</button>
      <button type="button" class="btn" :class="confirmClass" data-bs-dismiss="modal" @click="emit('confirm')">
        <i v-if="confirmIcon" :class="'bi ' + confirmIcon"></i> {{ confirmLabel }}
      </button>
    </div>
  </BsModal>
</template>

<script setup lang="ts">
// Confirmation dialog: the slot is the question, `confirm` fires on the
// primary button. Open it with a template ref: confirmModal.value?.show().
import { ref } from 'vue'
import BsModal from './BsModal.vue'

withDefaults(
  defineProps<{
    title: string
    icon?: string
    headerClass?: string
    confirmLabel?: string
    confirmIcon?: string
    confirmClass?: string
    cancelLabel?: string
    centered?: boolean
  }>(),
  {
    headerClass: 'bg-primary',
    confirmLabel: 'Yes',
    confirmClass: 'btn-primary',
    cancelLabel: 'Cancel',
    centered: false
  }
)
const emit = defineEmits<{ confirm: [] }>()

const modal = ref<InstanceType<typeof BsModal> | null>(null)

defineExpose({
  show: () => modal.value?.show(),
  hide: () => modal.value?.hide()
})
</script>
