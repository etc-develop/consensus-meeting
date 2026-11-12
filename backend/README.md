# consensus-meeting backend

Rust + Axum API server for consensus meeting query and admin management.

## Local setup

1. Prepare PostgreSQL database:
   - Create database: `consensus_meeting`
2. Copy env file:
   - `cp .env.example .env`
3. Update `DATABASE_URL` and `JWT_SECRET`.
4. Optional local frontend config:
   - `CORS_ALLOWED_ORIGINS` (default `http://localhost:5173,http://127.0.0.1:5173,http://localhost:4173,http://127.0.0.1:4173`)
5. Optional file upload config:
   - `UPLOAD_DIR` (default `./uploads`)
   - `MAX_UPLOAD_BYTES` (default `26214400`)

## Run

```bash
cargo run --bin consensus-meeting
```

Server will:
- connect to PostgreSQL
- run migrations from [`migrations/`](./migrations/)
- start HTTP API at `APP_BIND_ADDR`

## Demo accounts

The migration `migrations/20261102140000_seed_demo_data.sql` creates a few sample users for local testing:

- `admin@consensus.test` / `Admin123!`
- `viewer@consensus.test` / `Viewer123!`
- `staff@consensus.test` / `Staff123!`

## Initial endpoints

- `GET /api/health`
- `POST /api/auth/login`
- `POST /api/auth/bootstrap` (create first admin, only works when no users exist)
- `GET /api/auth/me` (resolve current user from Bearer token)
- `GET /api/public/meetings`
- `GET /api/public/meetings/{meeting_id}/resolutions`
- `GET/POST/PATCH /api/admin/...` for users/meetings/resolutions/documents (requires authenticated user)
- `POST /api/admin/documents/upload` (multipart upload to local filesystem)

## AI 匯入會議文件（寫入資料庫）

設定 `.env` 內至少包含：

- `DATABASE_URL`
- `AI_API_KEY`（或 `OPENAI_API_KEY`）
- `AI_API_URL`（預設 `https://api.openai.com/v1/chat/completions`）
- `AI_MODEL`（預設 `gpt-4o-mini`）
- `AI_VL_MODEL`（預設 `qwen2.5vl:7b`，若設定則先用視覺模型整份 PDF 逐頁閱讀，再交給 `AI_MODEL` 做決議萃取）

執行方式：

```bash
cargo run --bin ingest-meeting-ai -- --file ./meeting.txt
```

或從 stdin 輸入：

```bash
cat ./meeting.txt | cargo run --bin ingest-meeting-ai -- --dry-run
```

`--dry-run` 只顯示 AI 解析結果，不寫入資料庫。

補充：
- 匯入器會要求 AI 完整輸出所有議題與對應決議，不可遺漏。
- 匯入器會做多輪 AI 校對，優先採用議題筆數較完整的結果。
- 宣導與決議內容若含條列，會以 Markdown 階層清單（`-` / `  -`）和換行格式保留，前端會以 Markdown 渲染。
