<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { getAdminMeetings, getAdminUsers, getCurrentUser, type AdminMeetingItem, type AdminUserItem } from "@/lib/api";

const router = useRouter();
const token = localStorage.getItem("consensus_meeting_token") ?? "";
const currentUser = ref<AdminUserItem | null>(null);
const users = ref<AdminUserItem[]>([]);
const meetings = ref<AdminMeetingItem[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);

const isLoggedIn = computed(() => Boolean(token));

async function loadDashboard() {
  if (!token) {
    router.push("/admin/login");
    return;
  }

  loading.value = true;
  error.value = null;

  try {
    const [user, adminUsers, adminMeetings] = await Promise.all([
      getCurrentUser(token),
      getAdminUsers(token),
      getAdminMeetings(token),
    ]);

    currentUser.value = user;
    users.value = adminUsers;
    meetings.value = adminMeetings;
  } catch (requestError) {
    error.value = requestError instanceof Error ? requestError.message : "無法載入儀表板資料。";
  } finally {
    loading.value = false;
  }
}

function logout() {
  localStorage.removeItem("consensus_meeting_token");
  router.push("/admin/login");
}

onMounted(() => {
  loadDashboard();
});
</script>

<template>
  <div class="space-y-6">
    <section class="rounded-2xl border border-slate-200 bg-white p-6 shadow-sm">
      <div class="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
        <div>
          <p class="text-sm font-medium uppercase tracking-[0.2em] text-sky-600">管理儀表板</p>
          <h1 class="mt-2 text-3xl font-bold tracking-tight text-slate-900">系統總覽</h1>
        </div>

        <div class="flex items-center gap-3">
          <span class="rounded-full bg-sky-100 px-3 py-1 text-xs font-medium text-sky-700">
            {{ currentUser?.display_name ?? "管理員" }}
          </span>
          <button class="rounded-xl border border-slate-200 px-3 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50" @click="logout">
            登出
          </button>
        </div>
      </div>
    </section>

    <div v-if="error" class="rounded-xl border border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">
      {{ error }}
    </div>

    <div v-if="loading" class="rounded-xl border border-slate-200 bg-slate-50 px-4 py-3 text-sm text-slate-600">
      載入儀表板中...
    </div>

    <div v-else class="grid gap-6 lg:grid-cols-2">
      <div class="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm">
        <h2 class="text-lg font-semibold text-slate-900">使用者</h2>
        <div class="mt-4 space-y-3">
          <div v-for="user in users" :key="user.id" class="flex items-center justify-between rounded-xl border border-slate-200 bg-slate-50 px-3 py-2">
            <div>
              <div class="font-medium text-slate-800">{{ user.display_name }}</div>
              <div class="text-xs text-slate-500">{{ user.email }}</div>
            </div>
          </div>
        </div>
      </div>

      <div class="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm">
        <h2 class="text-lg font-semibold text-slate-900">會議</h2>
        <div class="mt-4 space-y-3">
          <div v-for="meeting in meetings" :key="meeting.id" class="rounded-xl border border-slate-200 bg-slate-50 px-3 py-2">
            <div class="flex items-center justify-between gap-2">
              <div class="font-medium text-slate-800">{{ meeting.title }}</div>
              <span class="rounded-full bg-slate-100 px-2 py-0.5 text-[11px] font-medium text-slate-600">{{ meeting.meeting_no }}</span>
            </div>
            <div class="mt-2 text-sm text-slate-600">{{ meeting.summary }}</div>
          </div>
        </div>
      </div>
    </div>

    <div v-if="!isLoggedIn" class="rounded-xl border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-800">
      尚未登入，請重新登入。
    </div>
  </div>
</template>
