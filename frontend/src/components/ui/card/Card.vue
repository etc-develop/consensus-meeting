<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/utils";

const props = defineProps<{ class?: string }>();
const classes = computed(() => cn("rounded-xl border bg-card text-card-foreground shadow-card", props.class));
</script>

<template>
  <div :class="classes" v-bind="$attrs"><slot /></div>
</template>
