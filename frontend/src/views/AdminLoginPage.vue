<script setup lang="ts">
import { ref } from "vue";
import { useRouter } from "vue-router";
import { adminLogin } from "@/lib/api";

const router = useRouter();
const email = ref("admin@consensus.test");
const password = ref("Admin123!");
const loading = ref(false);
const error = ref<string | null>(null);

async function submit() {
  loading.value = true;
  error.value = null;

  try {
    const result = await adminLogin(email.value, password.value);
    localStorage.setItem("consensus_meeting_token", result.access_token);
    router.push("/admin");
  } catch (requestError) {
    error.value = requestError instanceof Error ? requestError.message : "登入失敗。";
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <div class="flex min-h-[70vh] items-center justify-center px-4">
    <div class="w-full max-w-md rounded-2xl border border-slate-200 bg-white p-6 shadow-sm">
      <div class="mb-6 text-center">
        <p class="text-sm font-medium uppercase tracking-[0.2em] text-sky-600">管理端登入</p>
        <h1 class="mt-2 text-2xl font-bold text-slate-900">登入系統</h1>
      </div>

      <form class="space-y-4" @submit.prevent="submit">
        <div>
          <label class="mb-2 block text-sm font-medium text-slate-700">電子郵件</label>
          <input v-model="email" type="email" class="w-full rounded-xl border border-slate-200 bg-slate-50 px-3 py-2.5 text-sm text-slate-700 outline-none transition focus:border-sky-500 focus:bg-white" />
        </div>

        <div>
          <label class="mb-2 block text-sm font-medium text-slate-700">密碼</label>
          <input v-model="password" type="password" class="w-full rounded-xl border border-slate-200 bg-slate-50 px-3 py-2.5 text-sm text-slate-700 outline-none transition focus:border-sky-500 focus:bg-white" />
        </div>

        <div v-if="error" class="rounded-xl border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
          {{ error }}
        </div>

        <button type="submit" :disabled="loading" class="w-full rounded-xl bg-sky-600 px-4 py-3 text-sm font-medium text-white transition hover:bg-sky-500 disabled:cursor-not-allowed disabled:opacity-70">
          {{ loading ? "登入中..." : "登入" }}
        </button>
      </form>
    </div>
  </div>
</template>
