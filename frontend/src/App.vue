<template>
  <div class="min-h-screen bg-background text-foreground">
    <header class="sticky top-0 z-30 border-b border-border/80 bg-background/80 backdrop-blur-xl">
      <div class="mx-auto flex max-w-6xl items-center px-4 py-3 sm:px-6 lg:px-8">
        <div class="flex items-center gap-3">
          <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-primary text-lg font-bold text-primary-foreground shadow-sm">C</div>
          <div class="hidden text-sm font-semibold tracking-tight text-foreground sm:block">一致性會議平台</div>
        </div>
      </div>
    </header>

    <main class="px-4 py-6 sm:px-6 lg:px-8">
      <div class="mx-auto max-w-6xl">
        <RouterView />
      </div>
    </main>
  </div>
</template>

<script setup lang="ts"></script>
