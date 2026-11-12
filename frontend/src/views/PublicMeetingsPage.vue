<script setup lang="ts">
  import { onMounted, ref } from "vue";
  import DOMPurify from "dompurify";
  import { marked } from "marked";
  import {
    getPublicAgendaItems,
    getPublicMeetingDocumentUrl,
    getPublicMeetingPromotions,
    getPublicMeetings,
    getPublicResolutions,
    type PublicAgendaItem,
    type PublicMeetingPromotionItem,
    type PublicMeetingItem,
    type PublicResolutionItem,
  } from "@/lib/api";

  const keyword = ref("");
  const meetings = ref<PublicMeetingItem[]>([]);
  const agendaItems = ref<PublicAgendaItem[]>([]);
  const promotions = ref<PublicMeetingPromotionItem[]>([]);
  const resolutions = ref<PublicResolutionItem[]>([]);
  const selectedMeetingId = ref<string | null>(null);
  const loading = ref(false);
  const error = ref<string | null>(null);

  function renderMarkdown(content: string | null | undefined): string {
    if (!content) {
      return "";
    }
    const parsed = marked.parse(content, { async: false, breaks: true });
    const html = typeof parsed === "string" ? parsed : "";
    return DOMPurify.sanitize(html);
  }

  function getAgendaItemTitle(resolution: PublicResolutionItem): string {
    const matchedAgendaItem = agendaItems.value.find((item) => item.id === resolution.agenda_item_id);
    return matchedAgendaItem?.agenda_item || "未綁定議題";
  }
  let latestLoadRequestId = 0;
  let latestSelectionRequestId = 0;

  async function loadMeetings() {
    const requestId = ++latestLoadRequestId;
    loading.value = true;
    error.value = null;

    try {
      const items = await getPublicMeetings(keyword.value);
      if (requestId !== latestLoadRequestId) {
        return;
      }

      meetings.value = items;

      if (items.length > 0) {
        selectedMeetingId.value = items[0].id;
        await selectMeeting(items[0].id);
      } else {
        selectedMeetingId.value = null;
        agendaItems.value = [];
        promotions.value = [];
        resolutions.value = [];
      }
    } catch (requestError) {
      if (requestId === latestLoadRequestId) {
        error.value = requestError instanceof Error ? requestError.message : "無法載入會議資料。";
      }
    } finally {
      if (requestId === latestLoadRequestId) {
        loading.value = false;
      }
    }
  }

  async function selectMeeting(meetingId: string) {
    const requestId = ++latestSelectionRequestId;
    selectedMeetingId.value = meetingId;

    try {
      const [agendaItemsForMeeting, promotionItems, resolutionItems] = await Promise.all([
        getPublicAgendaItems(meetingId),
        getPublicMeetingPromotions(meetingId),
        getPublicResolutions(meetingId),
      ]);
      if (requestId !== latestSelectionRequestId) {
        return;
      }
      agendaItems.value = agendaItemsForMeeting;
      promotions.value = promotionItems;
      resolutions.value = resolutionItems;
    } catch (requestError) {
      if (requestId === latestSelectionRequestId) {
        error.value = requestError instanceof Error ? requestError.message : "無法載入議題、宣導事項或決議事項。";
        agendaItems.value = [];
        promotions.value = [];
        resolutions.value = [];
      }
    }
  }

  onMounted(() => {
    loadMeetings();
  });
</script>

<template>
  <div class="space-y-6">
    <section class="rounded-2xl border border-slate-200 bg-white/80 p-6 shadow-sm backdrop-blur-sm">
      <div class="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
        <div>
          <p class="text-sm font-medium uppercase tracking-[0.2em] text-sky-600">公開查詢</p>
          <h1 class="mt-2 text-3xl font-bold tracking-tight text-slate-900">一致性會議</h1>
        </div>
        <div class="text-sm text-slate-500">可搜尋會議、議題與決議內容</div>
      </div>
    </section>

    <section class="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm">
      <div class="flex flex-col gap-3 sm:flex-row">
        <input v-model="keyword" type="search" placeholder="可搜尋所有會議欄位、議題（agenda items）與決議事項"
          class="flex-1 rounded-xl border border-slate-200 bg-slate-50 px-4 py-3 text-sm text-slate-700 outline-none transition focus:border-sky-500 focus:bg-white"
          @keydown.enter="loadMeetings" />
        <button class="rounded-xl bg-sky-600 px-4 py-3 text-sm font-medium text-white transition hover:bg-sky-500"
          @click="loadMeetings">
          查詢
        </button>
      </div>
    </section>

    <div v-if="error" class="rounded-xl border border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">
      {{ error }}
    </div>

    <div v-if="loading" class="rounded-xl border border-slate-200 bg-slate-50 px-4 py-3 text-sm text-slate-600">
      會議載入中...
    </div>

    <div v-else class="grid gap-6 lg:grid-cols-[1.1fr_0.9fr]">
      <div class="space-y-4">
        <div v-if="meetings.length === 0"
          class="rounded-2xl border border-dashed border-slate-200 bg-slate-50 p-10 text-center text-slate-500">
          查無會議資料。
        </div>

        <div v-for="meeting in meetings" :key="meeting.id" role="button" tabindex="0"
          class="w-full cursor-pointer rounded-2xl border p-4 text-left shadow-sm transition"
          :class="selectedMeetingId === meeting.id ? 'border-sky-500 bg-sky-50' : 'border-slate-200 bg-white hover:border-slate-300'"
          @click="selectMeeting(meeting.id)">
          <div class="flex items-center justify-between gap-3">
            <span class="text-xs font-semibold uppercase tracking-[0.2em] text-sky-600">{{ meeting.meeting_no }}</span>
            <span class="rounded-full bg-slate-100 px-2 py-0.5 text-[11px] font-medium text-slate-600">{{
              meeting.meeting_date }}</span>
          </div>
          <h2 class="mt-3 text-xl font-semibold text-slate-900">{{ meeting.title }}</h2>
          <div v-if="meeting.source_file_name" class="mt-2 flex flex-wrap items-center gap-2">
            <a :href="getPublicMeetingDocumentUrl(meeting.id)" target="_blank" rel="noopener noreferrer"
              class="inline-flex items-center gap-1.5 rounded-lg border border-sky-200 bg-sky-600 px-3 py-1.5 text-xs font-medium text-white shadow-sm transition hover:bg-sky-500"
              @click.stop>
              檢視原始文件
            </a>
            <a :href="getPublicMeetingDocumentUrl(meeting.id, true)" target="_blank" rel="noopener noreferrer"
              class="inline-flex items-center gap-1.5 rounded-lg border border-slate-200 bg-white px-3 py-1.5 text-xs font-medium text-slate-700 shadow-sm transition hover:bg-slate-50"
              @click.stop>
              下載原始文件
            </a>
          </div>
          <div v-else class="mt-2 text-xs text-slate-500">無原始文件可供檢視或下載</div>
          <p class="mt-2 text-sm leading-6 text-slate-600">{{ meeting.summary }}</p>
        </div>
      </div>

      <aside class="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm">
        <div v-if="selectedMeetingId && promotions.length > 0" class="mb-6">
          <h3 class="text-lg font-semibold text-slate-900">宣導事項</h3>
          <div class="mt-4 space-y-3">
            <article v-for="promotion in promotions" :key="promotion.id"
              class="rounded-xl border border-slate-200 bg-amber-50 p-4">
              <div class="text-xs font-semibold tracking-[0.2em] text-amber-700">宣導 {{ promotion.promotion_no }}</div>
              <div
                class="mt-2 text-sm leading-6 text-slate-700 [&_ol]:my-2 [&_ol]:list-decimal [&_ol]:pl-5 [&_p]:my-2 [&_ul]:my-2 [&_ul]:list-disc [&_ul]:pl-5"
                v-html="renderMarkdown(promotion.content)" />
              <div v-if="promotion.notes"
                class="mt-2 text-xs leading-5 text-slate-500 [&_ol]:my-2 [&_ol]:list-decimal [&_ol]:pl-5 [&_p]:my-2 [&_ul]:my-2 [&_ul]:list-disc [&_ul]:pl-5"
                v-html="renderMarkdown(`備註：\n${promotion.notes}`)" />
            </article>
          </div>
        </div>
        <div v-if="selectedMeetingId && resolutions.length > 0">
          <h3 class="text-lg font-semibold text-slate-900">決議事項</h3>
          <div class="mt-4 space-y-4">
            <article v-for="(resolution, index) in resolutions" :key="resolution.id"
              class="rounded-xl border border-slate-200 bg-slate-50 p-4">
              <h4 class="text-base font-semibold text-slate-900">議題{{ resolution.item_no || index + 1 }}</h4>
              <div
                class="mt-2 text-sm font-medium leading-6 text-slate-800 [&_ol]:my-1 [&_ol]:list-decimal [&_ol]:pl-5 [&_p]:my-1 [&_ul]:my-1 [&_ul]:list-disc [&_ul]:pl-5"
                v-html="renderMarkdown(getAgendaItemTitle(resolution))" />
              <div class="mt-3 rounded-lg border border-slate-200 bg-white px-3 py-2">
                <div class="text-[11px] font-semibold uppercase tracking-[0.22em] text-slate-500">決議</div>
                <div
                  class="mt-1 text-sm leading-6 text-slate-600 [&_ol]:my-2 [&_ol]:list-decimal [&_ol]:pl-5 [&_p]:my-2 [&_ul]:my-2 [&_ul]:list-disc [&_ul]:pl-5"
                  v-html="renderMarkdown(resolution.content)" />
              </div>
            </article>
          </div>
        </div>
        <div v-else-if="selectedMeetingId && !promotions.length" class="text-sm text-slate-500">
          此會議目前沒有宣導事項。
        </div>
        <div v-if="selectedMeetingId && resolutions.length === 0" class="mt-4 text-sm text-slate-500">
          此會議目前沒有決議事項。
        </div>
        <div v-if="!selectedMeetingId" class="text-sm text-slate-500">
          請先選擇會議以檢視宣導與決議清單。
        </div>
      </aside>
    </div>
  </div>
</template>
