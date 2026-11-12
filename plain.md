# consensus-meeting 專案資料

## 1) 專案簡介
- 這是一個「一致性會議平台」專案，提供：
  - 前台：公開會議與決議查詢
  - 後台：管理者登入與資料管理（使用者、會議、決議、文件、角色）

## 2) 技術架構
- Frontend：Vue 3 + Vite + Tailwind（shadcn 風格元件）
- Backend：Rust + Axum
- Database：PostgreSQL
- ORM / Migration：SQLx
- Auth：JWT（Access/Refresh Token）+ Argon2 密碼雜湊

## 3) 主要目錄
- `frontend/`：前端程式
- `backend/`：後端 API 與資料庫 migration
- `backend/migrations/`：資料表與種子資料 migration

## 4) 前端路由（仍可直接輸入）
- `/`：前台查詢頁
- `/admin/login`：後台登入頁
- `/admin`：後台管理頁

> 目前前端 UI 已移除導覽入口，不會在畫面上顯示前台/後台/登入連結；需手動輸入網址進入。

## 5) 後端 API 重點
- 健康檢查：`GET /api/health`
- 登入：`POST /api/auth/login`
- 建立初始管理員：`POST /api/auth/bootstrap`
- 目前使用者：`GET /api/auth/me`
- 公開會議：`GET /api/public/meetings`
- 公開決議：`GET /api/public/meetings/{meeting_id}/resolutions`
- 管理 API：`/api/admin/...`（需登入，無角色權限限制）

## 6) 本機環境變數（常用）
- Backend（`backend/.env`）：
  - `APP_BIND_ADDR`（預設 `127.0.0.1:8080`）
  - `DATABASE_URL`
  - `JWT_SECRET`（至少 32 字元）
  - `CORS_ALLOWED_ORIGINS`（逗號分隔）
  - `UPLOAD_DIR`
  - `MAX_UPLOAD_BYTES`
- Frontend（`frontend/.env`）：
  - `VITE_API_BASE_URL`（例如 `http://127.0.0.1:8123/api`）

## 7) Demo 測試帳號（由 seed migration 建立）
- `admin@consensus.test` / `Admin123!`
- `viewer@consensus.test` / `Viewer123!`
- `staff@consensus.test` / `Staff123!`

## 8) 啟動方式（建議）
1. 啟動 PostgreSQL 並建立資料庫 `consensus_meeting`
2. 啟動後端（會自動跑 migration）
   - `cd backend`
   - `cargo run`
3. 啟動前端
   - `cd frontend`
   - `pnpm install`
   - `pnpm dev`
4. 或直接於專案根目錄執行：
   - `./dev-up.sh`
5. 一鍵重置資料庫（測試用）：
   - `./reset-db.sh`
   - （純 Rust，內部執行 `cargo run --bin reset-db`）

## 9) 現況摘要
- 前端已完成 shadcn + Tailwind 風格化
- 後端已加入 CORS 設定，可透過 `CORS_ALLOWED_ORIGINS` 控制允許來源
- 已有可用的 DB 假資料與測試帳號
