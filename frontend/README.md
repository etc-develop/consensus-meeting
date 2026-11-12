# consensus-meeting frontend

Vue 3 + Vite frontend skeleton for:
- public meeting/resolution query
- admin login and basic dashboard

## Run

```bash
npm install
npm run dev
```

## Environment

Create `.env` if needed:

```bash
VITE_API_BASE_URL=/api
```

`/api` 會透過 Vite dev server proxy 轉送到 `http://127.0.0.1:8123`。
