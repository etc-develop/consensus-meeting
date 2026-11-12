import os
import re
import json
import requests
import pymupdf as fitz
from rapidocr_onnxruntime import RapidOCR
from collections import Counter
from tabulate import tabulate

# ==================== 設定參數 ====================
PDF_DIR = "./meeting_pdfs"
OLLAMA_API = "http://localhost:11434/api/chat"
MODEL_NAME = "qwen2.5:14b"
CACHE_FILE = "all_402_discovered_cache.json"
FINAL_SCHEMA_FILE = "pure_data_driven_schema.json"

ocr_engine = RapidOCR()

def extract_clean_text(pdf_path: str, max_pages: int = 4) -> str:
    """提取前幾頁文字，純圖片自動 OCR"""
    try:
        doc = fitz.open(pdf_path)
    except Exception as e:
        print(f"  ❌ 開啟檔案失敗: {e}")
        return ""

    extracted = []
    total_pages = len(doc)
    for p_idx in range(min(max_pages, total_pages)):
        page = doc[p_idx]
        text = page.get_text().strip()
        if len(text) < 40:
            pix = page.get_pixmap(dpi=150)
            res, _ = ocr_engine(pix.tobytes("png"))
            text = "\n".join([line[1] for line in res]) if res else ""
        if text.strip():
            extracted.append(f"--- [Page {p_idx + 1}] ---\n{text}")

    return "\n\n".join(extracted)[:5000]

def clean_extracted_fields(raw_fields: list) -> list:
    """
    Python 端防護過濾：
    1. 過濾長度過長（超過 8 個字）的條列句子
    2. 去除標點符號與多餘空白
    3. 自動過濾常見的文件細項內容（如照片、線路圖等非屬性詞彙）
    """
    cleaned = []
    # 常見容易被誤判為欄位名稱的具體物品/文件清單黑名單
    BLACKLIST_WORDS = {
        "相片內容", "影像尺寸", "內部線路圖", "本體外型", "材質證明", 
        "型式分類表", "試驗報告", "線路圖", "組立圖", "使用說明書"
    }

    for item in raw_fields:
        if not isinstance(item, dict):
            continue
            
        name = item.get("field_name") or item.get("field_name:") or ""
        name = str(name).strip()
        sample = str(item.get("sample_value") or "").strip()

        # 去除項次前綴（如 "一、"、"（一）"）
        name = re.sub(r"^[（(]?[一二三四五六七八九十0-9]+[）)]?[、. ]*", "", name).strip()

        # 欄位必須在 2~8 個繁體中文字之間，不可有句子標點，且不在黑名單中
        if 2 <= len(name) <= 8 and not any(p in name for p in "，。；？！\n"):
            if name not in BLACKLIST_WORDS:
                cleaned.append({
                    "field_name": name,
                    "sample_value": sample[:120]
                })
    return cleaned

def discover_single_doc_fields(file_name: str, content: str) -> dict:
    """
    【階段一：單檔結構層級探勘】
    嚴格劃分「公文/資料庫欄位容器」與「討論具體內容細項」
    """
    system_prompt = """你是一位精通台灣公文與商品檢驗研討會紀錄的資料架構師。
請分析這份會議紀錄，提煉其「資料庫欄位名稱（資訊結構維度）」。

【結構定義重要原則】：
1. 什麼是「欄位（field_name）」？
   - 欄位是「承載資料的抽象容器名詞」（長度 2 到 8 個字）。
   - 全域欄位（document_level_fields）例如：會議名稱、開會時間、開會地點、主席、出席人員、記錄。
   - 項目/表格欄位（item_level_fields）例如：項次、案號、議題、案由、討論事項、法規依據/標準、決議事項/審查原則/結論、權責單位、備註。
2. 什麼是「資料內容（sample_value）」，絕對嚴禁當成欄位？
   - 具體討論的物品、測試項目、送審文件清單（例如「相片內容」、「內部線路圖」、「材質證明」、「溫升試驗」等），這些是「議題或決議裡的內容」，絕對不是欄位！請勿將其拆成 field_name，應統一收納為「討論事項」或「決議內容」的 sample_value。
3. 原文真實對齊：
   - 原文標示「議題：」就輸出「議題」，標示「案由：」就輸出「案由」，嚴禁擅自改寫為「題目」或自創名詞。

請嚴格輸出合法 JSON：
{
  "document_level_fields": [
    {"field_name": "全域欄位名", "sample_value": "實際範例"}
  ],
  "item_level_fields": [
    {"field_name": "項目欄位名", "sample_value": "實際範例"}
  ]
}"""

    user_prompt = f"""檔案名稱：{file_name}
文件內文：
{content}

請分析並輸出本文件的標準欄位維度："""

    try:
        res = requests.post(
            OLLAMA_API,
            json={
                "model": MODEL_NAME,
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": user_prompt}
                ],
                "format": "json",
                "options": {"temperature": 0.0},
                "stream": False
            },
            timeout=180
        )
        parsed = json.loads(res.json()["message"]["content"])
        return {
            "document_level_fields": clean_extracted_fields(parsed.get("document_level_fields", [])),
            "item_level_fields": clean_extracted_fields(parsed.get("item_level_fields", []))
        }
    except Exception as e:
        print(f"  ❌ 探勘失敗 [{file_name}]: {e}")
        return {"document_level_fields": [], "item_level_fields": []}

def synthesize_all_fields_with_llm(all_field_stats: dict, total_docs: int) -> dict:
    """
    【階段二：全量語意聚類】
    聚合 402 份檔案的所有屬性，收斂出兼具歷史與現代標準的統一 Schema。
    """
    system_prompt = """你是一位首席資料庫架構師。
以下是從全部會議紀錄中，100% 完整探勘提煉出的「所有原始欄位名稱標籤、出現頻率與範例」。

請依據數據進行「語意聚類與標準 Schema 定型」：
1. 合併語意相同但用字不同的同義詞（例如將「開會時間」、「時間」、「會議日期」收納為同一個標準欄位；將「議題」、「案由」、「討論事項」合理對齊；將「一致性結論」、「審查原則」、「決議」收攏對齊）。
2. 將欄位劃分至合理的資料庫結構：
   - "core_common_fields": 出現頻率高、多數會議皆具備的「核心共同欄位」
   - "extended_special_fields": 特定年份或少數會議才具備的「擴充/長尾欄位」
3. 為每個標準欄位定義：
   - "field_key": 標準英文代碼（snake_case）
   - "field_name": 標準繁體中文名稱
   - "scope": "document"（全域）或 "item"（提案明細）
   - "data_type": 建議的資料庫型別（VARCHAR, DATE, TEXT, JSON_ARRAY 等）
   - "synonyms_matched": 收納進來的原始名稱標籤清單
   - "description": 業務意涵說明

輸出必須為嚴格 JSON 格式：
{
  "core_common_fields": [
    {
      "field_key": "代碼",
      "field_name": "標準名稱",
      "scope": "document/item",
      "data_type": "型態",
      "synonyms_matched": ["標籤1", "標籤2"],
      "description": "說明"
    }
  ],
  "extended_special_fields": [
    {
      "field_key": "代碼",
      "field_name": "標準名稱",
      "scope": "document/item",
      "data_type": "型態",
      "synonyms_matched": ["標籤"],
      "description": "說明"
    }
  ]
}"""

    user_prompt = f"""總分析文件數量：{total_docs}
全量探勘出的原始欄位標籤統計：
{json.dumps(all_field_stats, ensure_ascii=False, indent=2)}

請依據全量真實數據產出標準化 Schema："""

    res = requests.post(
        OLLAMA_API,
        json={
            "model": MODEL_NAME,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_prompt}
            ],
            "format": "json",
            "options": {"temperature": 0.0, "num_ctx": 16384},
            "stream": False
        },
        timeout=300
    )
    return json.loads(res.json()["message"]["content"])

def main():
    if not os.path.exists(PDF_DIR):
        print(f"❌ 找不到目錄: {PDF_DIR}")
        return

    pdf_files = sorted([f for f in os.listdir(PDF_DIR) if f.lower().endswith(".pdf")])
    total = len(pdf_files)
    if total == 0:
        print("未找到 PDF 檔案。")
        return

    print(f"🚀 開始【100% 全量無偏見欄位探勘】，總共 {total} 份檔案...")

    cached_data = {}
    if os.path.exists(CACHE_FILE):
        try:
            with open(CACHE_FILE, "r", encoding="utf-8") as f:
                cached_data = json.load(f)
            print(f"📦 偵測到歷史快取，已完成 {len(cached_data)} 份，接續執行剩餘檔案...")
        except Exception:
            cached_data = {}

    for idx, filename in enumerate(pdf_files, start=1):
        if filename in cached_data:
            continue

        print(f"[{idx}/{total}] 正在探勘: {filename}")
        path = os.path.join(PDF_DIR, filename)
        text = extract_clean_text(path)
        if not text.strip():
            print(f"  ⚠️ 無有效內文，跳過")
            continue

        discovered = discover_single_doc_fields(filename, text)
        cached_data[filename] = discovered

        # 單檔即時寫入快取，避免中途中斷
        with open(CACHE_FILE, "w", encoding="utf-8") as f:
            json.dump(cached_data, f, ensure_ascii=False, indent=2)

    print(f"\n✅ 全部 {total} 份檔案分析完成！正在統計全域出現頻率...")

    doc_field_counts = Counter()
    item_field_counts = Counter()
    sample_values = {}

    for doc_name, schema in cached_data.items():
        for field in schema.get("document_level_fields", []):
            name = field.get("field_name", "").strip()
            if name:
                doc_field_counts[name] += 1
                if name not in sample_values and field.get("sample_value"):
                    sample_values[name] = field.get("sample_value")

        for field in schema.get("item_level_fields", []):
            name = field.get("field_name", "").strip()
            if name:
                item_field_counts[name] += 1
                if name not in sample_values and field.get("sample_value"):
                    sample_values[name] = field.get("sample_value")

    aggregated_stats = {
        "document_level_candidates": [
            {"name": k, "frequency": v, "sample": sample_values.get(k, "")[:80]}
            for k, v in doc_field_counts.most_common() if v >= 2
        ],
        "item_level_candidates": [
            {"name": k, "frequency": v, "sample": sample_values.get(k, "")[:80]}
            for k, v in item_field_counts.most_common() if v >= 2
        ]
    }

    print(f"📊 統計完成：挖掘出 {len(aggregated_stats['document_level_candidates'])} 個全域欄位、{len(aggregated_stats['item_level_candidates'])} 個項目欄位標籤。")
    print(f"🧠 正在呼叫 [{MODEL_NAME}] 執行全域語意聚類與 Schema 定型...")

    final_schema = synthesize_all_fields_with_llm(aggregated_stats, total)

    # 輸出表格
    table_rows = []
    for field in final_schema.get("core_common_fields", []):
        synonyms = ", ".join(field.get("synonyms_matched", []))[:30]
        matched_count = sum(doc_field_counts[syn] + item_field_counts[syn] for syn in field.get("synonyms_matched", []))
        coverage_rate = f"{min(100.0, (matched_count / total) * 100):.1f}%"
        table_rows.append([
            "⭐ 共同核心",
            field.get("field_name"),
            field.get("field_key"),
            field.get("scope"),
            field.get("data_type"),
            coverage_rate,
            synonyms,
            field.get("description", "")
        ])

    for field in final_schema.get("extended_special_fields", []):
        synonyms = ", ".join(field.get("synonyms_matched", []))[:30]
        matched_count = sum(doc_field_counts[syn] + item_field_counts[syn] for syn in field.get("synonyms_matched", []))
        coverage_rate = f"{min(100.0, (matched_count / total) * 100):.1f}%"
        table_rows.append([
            "🔹 特殊/擴充",
            field.get("field_name"),
            field.get("field_key"),
            field.get("scope"),
            field.get("data_type"),
            coverage_rate,
            synonyms,
            field.get("description", "")
        ])

    print("\n### 402 份會議紀錄【全量真實資料驅動】之 Schema 整合結果：\n")
    headers = ["分類", "標準欄位名稱", "資料庫 Key", "層級", "建議型態", "預估覆蓋率", "收納之原始詞彙", "業務意涵"]
    print(tabulate(table_rows, headers=headers, tablefmt="github"))

    with open(FINAL_SCHEMA_FILE, "w", encoding="utf-8") as out_f:
        json.dump(final_schema, out_f, ensure_ascii=False, indent=2)

    print(f"\n🎉 100% 全量真實歸納之 Schema 規格書已存至 {FINAL_SCHEMA_FILE}")

if __name__ == "__main__":
    main()