const API_BASE = (import.meta.env.VITE_API_BASE_URL ?? "/api").replace(/\/+$/, "");

function createApiUrl(path: string): URL {
  const endpoint = `${API_BASE}${path.startsWith("/") ? path : `/${path}`}`;
  if (endpoint.startsWith("http://") || endpoint.startsWith("https://")) {
    return new URL(endpoint);
  }
  const origin = typeof window === "undefined" ? "http://localhost" : window.location.origin;
  return new URL(endpoint, origin);
}

export type PublicMeetingItem = {
  id: string;
  meeting_no: string;
  title: string;
  summary: string;
  meeting_date: string;
  published_at?: string | null;
  source_file_name?: string | null;
};

export type PublicAgendaItem = {
  id: string;
  meeting_id: string;
  item_no: string;
  agenda_item: string;
  notes?: string | null;
  created_at?: string | null;
};

export type PublicResolutionItem = {
  id: string;
  meeting_id: string;
  agenda_item_id?: string | null;
  item_no: string;
  title: string;
  content: string;
  result: string;
};

export type PublicMeetingPromotionItem = {
  id: string;
  meeting_id: string;
  promotion_no: number;
  content: string;
  notes?: string | null;
  created_at?: string | null;
};

export type LoginResult = {
  access_token: string;
  refresh_token: string;
  token_type: string;
  expires_in_seconds: number;
};

export type AdminUserItem = {
  id: string;
  email: string;
  display_name: string;
  is_active: boolean;
  created_at: string;
};

export type AdminMeetingItem = {
  id: string;
  meeting_no: string;
  title: string;
  summary: string;
  meeting_date: string;
  published_at?: string | null;
};

async function fetchJson<T>(input: string, init?: RequestInit): Promise<T> {
  const response = await fetch(input, init);
  if (!response.ok) {
    const data = await response.json().catch(() => ({}));
    const message = typeof data?.error === "string" ? data.error : `請求失敗：${response.status}`;
    throw new Error(message);
  }
  return response.json() as Promise<T>;
}

export async function getPublicMeetings(keyword = ""): Promise<PublicMeetingItem[]> {
  const url = createApiUrl("/public/meetings");
  if (keyword.trim()) {
    url.searchParams.set("keyword", keyword.trim());
  }
  const body = await fetchJson<{ items?: PublicMeetingItem[] }>(url.toString());
  return body.items ?? [];
}

export async function getPublicAgendaItems(meetingId: string): Promise<PublicAgendaItem[]> {
  const body = await fetchJson<{ items?: PublicAgendaItem[] }>(`${API_BASE}/public/meetings/${meetingId}/agenda-items`);
  return body.items ?? [];
}

export async function getPublicResolutions(meetingId: string): Promise<PublicResolutionItem[]> {
  const body = await fetchJson<{ items?: PublicResolutionItem[] }>(`${API_BASE}/public/meetings/${meetingId}/resolutions`);
  return body.items ?? [];
}

export async function getPublicMeetingPromotions(meetingId: string): Promise<PublicMeetingPromotionItem[]> {
  const body = await fetchJson<{ items?: PublicMeetingPromotionItem[] }>(`${API_BASE}/public/meetings/${meetingId}/promotions`);
  return body.items ?? [];
}

export function getPublicMeetingDocumentUrl(meetingId: string, download = false): string {
  const url = createApiUrl(`/public/meetings/${meetingId}/document`);
  if (download) {
    url.searchParams.set("download", "1");
  }
  return url.toString();
}

export async function adminLogin(email: string, password: string): Promise<LoginResult> {
  return fetchJson<LoginResult>(`${API_BASE}/auth/login`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ email, password }),
  });
}

export async function getCurrentUser(token: string): Promise<AdminUserItem> {
  return fetchJson<AdminUserItem>(`${API_BASE}/auth/me`, {
    headers: { Authorization: `Bearer ${token}` },
  });
}

export async function getAdminUsers(token: string): Promise<AdminUserItem[]> {
  const body = await fetchJson<{ items?: AdminUserItem[] }>(`${API_BASE}/admin/users`, {
    headers: { Authorization: `Bearer ${token}` },
  });
  return body.items ?? [];
}

export async function getAdminMeetings(token: string): Promise<AdminMeetingItem[]> {
  const body = await fetchJson<{ items?: AdminMeetingItem[] }>(`${API_BASE}/admin/meetings`, {
    headers: { Authorization: `Bearer ${token}` },
  });
  return body.items ?? [];
}

export async function createAdminMeeting(
  token: string,
  payload: { meeting_no: string; title: string; summary: string; meeting_date: string; published_at?: string | null },
): Promise<AdminMeetingItem> {
  return fetchJson<AdminMeetingItem>(`${API_BASE}/admin/meetings`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify(payload),
  });
}
