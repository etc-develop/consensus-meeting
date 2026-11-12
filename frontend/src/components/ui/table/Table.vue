<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/utils";

const props = defineProps<{ class?: string }>();
const classes = computed(() => cn("w-full caption-bottom text-sm", props.class));
</script>

<template>
  <div class="relative w-full overflow-auto">
    <table :class="classes" v-bind="$attrs"><slot /></table>
  </div>
</template>
