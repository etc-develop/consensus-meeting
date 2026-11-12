-- 記錄會議紀錄的原始文件（PDF 等），供前端檢視/下載
ALTER TABLE meetings ADD COLUMN IF NOT EXISTS source_file_name TEXT;
ALTER TABLE meetings ADD COLUMN IF NOT EXISTS source_file_path TEXT;
