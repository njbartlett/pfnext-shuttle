<template>
  <input
    type="search"
    class="form-control"
    :id="inputId"
    :placeholder="placeholder"
    :list="listId"
    :value="text"
    @input="onInput"
  >
  <datalist :id="listId">
    <option v-for="member in members" :key="member.id" :value="formatNameAndEmail(member.name, member.email)"></option>
  </datalist>
</template>

<script setup lang="ts">
// Search box with a datalist of members. The model is the selected member,
// or null while the typed text does not match anyone. Call clear() through
// a template ref to empty the box after acting on a selection.
import { ref, useId, watch } from 'vue'
import { formatNameAndEmail, type UserSummary } from '@pfnext/shared'

const props = withDefaults(
  defineProps<{
    members: UserSummary[]
    modelValue: UserSummary | null
    placeholder?: string
    inputId?: string
  }>(),
  { placeholder: 'Type member name to search…' }
)
const emit = defineEmits<{ 'update:modelValue': [member: UserSummary | null] }>()

const listId = useId()
const text = ref(labelOf(props.modelValue))

watch(
  () => props.modelValue,
  (member) => {
    if (member) {
      text.value = labelOf(member)
    }
  }
)

function labelOf(member: UserSummary | null): string {
  return member ? formatNameAndEmail(member.name, member.email) : ''
}

function onInput(event: Event) {
  text.value = (event.target as HTMLInputElement).value
  const match = props.members.find((member) => labelOf(member) === text.value) ?? null
  if (match?.id !== props.modelValue?.id) {
    emit('update:modelValue', match)
  }
}

function clear() {
  text.value = ''
  if (props.modelValue) {
    emit('update:modelValue', null)
  }
}

defineExpose({ clear })
</script>
