use std::{collections::HashSet, env, fs, io::Read, path::Path, process::Command};

use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STD;
use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
use consensus_meeting::config::AppConfig;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Executor, postgres::PgPoolOptions};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AiMeetingPayload {
    meeting: AiMeeting,
    #[serde(default)]
    promotions: Vec<AiPromotion>,
    #[serde(default)]
    agenda_resolution_pairs: Vec<AiAgendaResolutionPair>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AiMeetingPayloadV2 {
    meeting: AiMeetingV2,
    #[serde(default)]
    meeting_promotions: Vec<AiPromotion>,
    #[serde(default)]
    items: Vec<AiItemV2>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PartialAiMeetingPayload {
    meeting: Option<AiMeeting>,
    #[serde(default)]
    promotions: Vec<AiPromotion>,
    #[serde(default)]
    agenda_resolution_pairs: Vec<AiAgendaResolutionPair>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PartialAiMeetingPayloadV2 {
    meeting: Option<AiMeetingV2>,
    #[serde(default)]
    meeting_promotions: Vec<AiPromotion>,
    #[serde(default)]
    items: Vec<AiItemV2>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AiMeeting {
    meeting_no: Option<String>,
    meeting_title: Option<String>,
    title: Option<String>,
    summary: Option<String>,
    meeting_date: Option<String>,
    meeting_time: Option<String>,
    chairperson: Option<String>,
    attendees: Option<Vec<String>>,
    meeting_location: Option<String>,
    recorder: Option<String>,
    published_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AiMeetingV2 {
    meeting_no: Option<String>,
    meeting_date: Option<String>,
    meeting_time: Option<String>,
    meeting_title: Option<String>,
    title: Option<String>,
    chairperson: Option<String>,
    attendees: Option<String>,
    meeting_location: Option<String>,
    recorder: Option<String>,
    summary: Option<String>,
    published_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AiItemV2 {
    item_no: Option<String>,
    agenda_item: String,
    agenda_notes: Option<String>,
    resolution: AiResolutionV2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AiResolutionV2 {
    title: Option<String>,
    content: Option<String>,
    resolution: Option<String>,
    result: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
struct AiAgendaResolutionPair {
    item_no: Option<String>,
    agenda_item: String,
    agenda_notes: Option<String>,
    resolution_title: Option<String>,
    resolution_content: Option<String>,
    resolution_text: Option<String>,
    resolution_result: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
struct AiPromotion {
    promotion_no: Option<i32>,
    content: String,
    notes: Option<String>,
}

#[derive(Debug, Clone)]
struct CliArgs {
    file: Option<String>,
    dry_run: bool,
    extract_only: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let args = parse_args()?;
    // let input_document = read_input_document(args.file.as_deref())?;
    // if input_document.trim().is_empty() {
    //     bail!("輸入文件為空，請提供會議文件內容");
    // }

    // if args.extract_only {
    //     println!("{input_document}");
    //     return Ok(());
    // }
    let input_document = "";
    let mut payload = extract_with_ai(&input_document, args.file.as_deref()).await?;
    let raw_promotion_count = payload.promotions.len();
    let raw_pair_count = payload.agenda_resolution_pairs.len();
    deduplicate_ai_payload(&mut payload);
    println!(
        "[ingest-meeting-ai] 去重筆數：promotions {} -> {}, agenda_resolution_pairs {} -> {}",
        raw_promotion_count,
        payload.promotions.len(),
        raw_pair_count,
        payload.agenda_resolution_pairs.len()
    );
    // println!(
    //     "[ingest-meeting-ai] AI 解析結果：\n{}",
    //     serde_json::to_string_pretty(&payload)?
    // );

    if args.dry_run {
        println!("[ingest-meeting-ai] dry-run 模式，不寫入資料庫。");
        return Ok(());
    }

    persist_to_database(payload, args.file.as_deref()).await?;
    Ok(())
}

fn parse_args() -> Result<CliArgs> {
    let mut args = env::args().skip(1);
    let mut file = None;
    let mut dry_run = false;
    let mut extract_only = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--file" => {
                let value = args
                    .next()
                    .ok_or_else(|| anyhow!("--file 後面需要接檔案路徑"))?;
                file = Some(value);
            }
            "--dry-run" => dry_run = true,
            "--extract-only" => extract_only = true,
            "--help" | "-h" => {
                println!("Usage: cargo run --bin ingest-meeting-ai -- [--file <path>] [--dry-run]");
                println!("  --file <path>  讀取會議文件（支援 txt/pdf，不提供時改讀 stdin）");
                println!("  --dry-run      僅顯示 AI 解析結果，不寫入資料庫");
                println!("  --extract-only 僅輸出 PDF 抽取 / OCR 後的文字，不呼叫 AI");
                println!("  PDF 若需 OCR，系統需可用 pdftoppm + tesseract。");
                std::process::exit(0);
            }
            other => bail!("未知參數: {other}"),
        }
    }

    Ok(CliArgs {
        file,
        dry_run,
        extract_only,
    })
}

fn read_input_document(file: Option<&str>) -> Result<String> {
    let max_pages = env::var("PDF_MAX_PAGES")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(50);
    const PDF_MAX_CHARS: usize = 120_000;

    if let Some(path) = file {
        let file_path = Path::new(path);
        let extension = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();

        if extension == "pdf" {
            return extract_clean_text_from_pdf(file_path, max_pages, "chi_tra+eng", PDF_MAX_CHARS)
                .with_context(|| format!("讀取 PDF 會議文件失敗: {path}"));
        }

        return fs::read_to_string(path).with_context(|| format!("讀取會議文件失敗: {path}"));
    }

    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .context("讀取 stdin 失敗")?;
    Ok(input)
}

fn extract_clean_text_from_pdf(
    path: &Path,
    max_pages: usize,
    _ocr_lang: &str,
    max_chars: usize,
) -> Result<String> {
    let document = lopdf::Document::load(path)
        .with_context(|| format!("無法開啟 PDF 檔案: {}", path.display()))?;
    let pdftotext_available = command_exists("pdftotext");

    let mut page_numbers = document.get_pages().keys().copied().collect::<Vec<_>>();
    page_numbers.sort_unstable();
    let total_pages = page_numbers.len();
    if total_pages > max_pages {
        eprintln!(
            "[ingest-meeting-ai] warning: PDF 共 {total_pages} 頁，僅處理前 {max_pages} 頁（可透過 PDF_MAX_PAGES 環境變數調整）。"
        );
    }
    let page_count = total_pages.min(max_pages);

    let mut extracted = Vec::new();
    for page_number in page_numbers.into_iter().take(page_count) {
        let mut text = String::new();

        if pdftotext_available {
            match extract_pdf_page_with_pdftotext(path, page_number) {
                Ok(content) => text = content.trim().to_string(),
                Err(error) => {
                    eprintln!(
                        "[ingest-meeting-ai] warning: 第 {page_number} 頁 pdftotext 抽取失敗，改用 lopdf：{error}"
                    );
                }
            }
        }

        if text.is_empty() {
            match document.extract_text(&[page_number]) {
                Ok(content) => text = content.trim().to_string(),
                Err(error) => {
                    if !pdftotext_available {
                        eprintln!(
                            "[ingest-meeting-ai] warning: 第 {page_number} 頁原生文字抽取失敗：{error}"
                        );
                    }
                }
            }
        }

        if !text.trim().is_empty() {
            let normalized_text = normalize_cjk_compat_text(text.trim());
            extracted.push(format!("--- [Page {page_number}] ---\n{normalized_text}"));
        }
    }

    let merged = extracted.join("\n\n");
    if merged.trim().is_empty() {
        bail!(
            "PDF 幾乎無可擷取文字，請確認 PDF 是否為可讀文字檔，或啟用 AI_VL_MODEL 進行視覺轉 Markdown"
        )
    }
    eprintln!(
        "[ingest-meeting-ai] PDF 擷取完成：共處理 {page_count}/{total_pages} 頁，有效文字頁數 {}, 總字數 {}",
        extracted.len(),
        merged.chars().count()
    );
    let total_chars = merged.chars().count();
    if total_chars > max_chars {
        eprintln!(
            "[ingest-meeting-ai] warning: PDF 抽取文字過長，已截斷到 {max_chars} 字（原始 {total_chars} 字）"
        );
        return Ok(merged.chars().take(max_chars).collect());
    }

    Ok(merged)
}

fn normalize_cjk_compat_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ('\u{F900}'..='\u{FAFF}').contains(&ch) || ('\u{2F800}'..='\u{2FA1F}').contains(&ch) {
            unicode_normalization::char::decompose_compatible(ch, |d| out.push(d));
        } else {
            out.push(ch);
        }
    }
    let mut lines: Vec<&str> = out.lines().collect();
    while let Some(last) = lines.last() {
        let trimmed = last.trim();
        if trimmed.is_empty()
            || trimmed.chars().all(|c| c.is_ascii_digit())
            || trimmed.contains("本局相關法規法律、標準位階高於檢驗一致性會議")
        {
            lines.pop();
        } else {
            break;
        }
    }
    unwrap_hard_wrapped_lines(&lines.join("\n"))
}

fn extract_pdf_page_with_pdftotext(path: &Path, page_number: u32) -> Result<String> {
    let output = Command::new("pdftotext")
        .arg("-layout")
        .arg("-f")
        .arg(page_number.to_string())
        .arg("-l")
        .arg(page_number.to_string())
        .arg(path)
        .arg("-")
        .output()
        .with_context(|| "執行 pdftotext 失敗")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("pdftotext 執行失敗：{stderr}");
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn count_meaningful_page_chars(text: &str) -> usize {
    text.lines()
        .map(str::trim)
        .filter(|line| {
            !line.is_empty() && !line.contains("本局相關法規法律、標準位階高於檢驗一致性會議")
        })
        .map(|line| line.chars().filter(|c| !c.is_whitespace()).count())
        .sum()
}

/// 將掃描頁以 OCR 辨識成文字。
///
/// 支援方向不同的掃描檔：
/// 1. 先用 tesseract OSD (`--psm 0`) 偵測頁面需要旋轉的角度，並在記憶體中旋轉影像。
/// 2. 同時嘗試橫式（`chi_tra+eng`）與直式（`chi_tra_vert`，若系統有安裝）辨識，
///    依 tesseract 回報的平均信心值挑選最佳結果。
fn ocr_pdf_page(path: &Path, page_number: u32, lang: &str) -> Result<String> {
    let unique = Uuid::new_v4().to_string();
    let work_dir = env::temp_dir().join(format!("consensus-meeting-ocr-{page_number}-{unique}"));
    fs::create_dir_all(&work_dir).with_context(|| "建立 OCR 暫存目錄失敗")?;

    let result = ocr_pdf_page_in_dir(path, page_number, lang, &work_dir);

    if let Err(error) = fs::remove_dir_all(&work_dir) {
        eprintln!(
            "[ingest-meeting-ai] warning: 清理暫存 OCR 目錄失敗 ({}): {}",
            work_dir.display(),
            error
        );
    }

    result
}

fn ocr_pdf_page_in_dir(
    path: &Path,
    page_number: u32,
    lang: &str,
    work_dir: &Path,
) -> Result<String> {
    let dpi = env::var("OCR_DPI").unwrap_or_else(|_| "200".to_string());
    let output_prefix = work_dir.join("page");
    let pdftoppm_output = Command::new("pdftoppm")
        .arg("-f")
        .arg(page_number.to_string())
        .arg("-l")
        .arg(page_number.to_string())
        .arg("-r")
        .arg(&dpi)
        .arg("-singlefile")
        .arg("-gray")
        .arg(path)
        .arg(&output_prefix)
        .output()
        .with_context(|| "執行 pdftoppm 失敗")?;

    if !pdftoppm_output.status.success() {
        let stderr = String::from_utf8_lossy(&pdftoppm_output.stderr);
        bail!("pdftoppm 執行失敗：{stderr}");
    }

    let original_path = output_prefix.with_extension("pgm");
    let image = GrayImage::read_pgm(&original_path)?;

    let osd_rotation = detect_osd_rotation(&original_path).unwrap_or(0);
    let mut rotations = vec![osd_rotation];
    if osd_rotation != 0 {
        rotations.push(0);
    }

    let vertical_lang = vertical_ocr_lang(lang);
    let mut best: Option<OcrCandidate> = None;

    'outer: for rotation in rotations {
        let rotated_image = if rotation == 0 {
            None
        } else {
            Some(image.rotate_clockwise(rotation))
        };
        let image_path = if let Some(ref rot_img) = rotated_image {
            let rotated_path = work_dir.join(format!("page-rot{rotation}.pgm"));
            rot_img.write_pgm(&rotated_path)?;
            rotated_path
        } else {
            original_path.clone()
        };

        let mut modes: Vec<(String, &str)> = vec![(lang.to_string(), "3")];
        if let Some(vert) = &vertical_lang {
            modes.push((vert.clone(), "5"));
        }

        for (mode_lang, psm) in modes {
            let mut candidate = match run_tesseract_with_confidence(
                &image_path,
                &mode_lang,
                psm,
                work_dir,
            ) {
                Ok(c) => c,
                Err(error) => {
                    eprintln!(
                        "[ingest-meeting-ai] warning: 第 {page_number} 頁 OCR（旋轉 {rotation}°, {mode_lang}）失敗：{error}"
                    );
                    continue;
                }
            };
            candidate.rotation = rotation;
            candidate.mode_lang = mode_lang.clone();
            candidate.image_path = image_path.clone();
            eprintln!(
                "[ingest-meeting-ai] 第 {page_number} 頁 OCR 候選：旋轉 {rotation}°、語言 {mode_lang}、信心 {:.1}、字數 {}",
                candidate.score,
                candidate.text.chars().count()
            );
            let is_better = best
                .as_ref()
                .map(|b| candidate.score > b.score)
                .unwrap_or(true);
            if is_better {
                best = Some(candidate);
            }
            if best.as_ref().is_some_and(|b| b.score >= 80.0) {
                break 'outer;
            }
        }
    }

    let Some(best) = best else {
        bail!("tesseract OCR 無任何可用結果");
    };

    if best.score < 60.0 && vertical_lang.is_none() {
        eprintln!(
            "[ingest-meeting-ai] warning: 第 {page_number} 頁 OCR 信心偏低 ({:.1})。若為直式排版文件，請安裝直式中文模型：sudo apt install tesseract-ocr-chi-tra-vert（或將 chi_tra_vert.traineddata 放到 OCR_TESSDATA_DIR）",
            best.score
        );
    }

    if best.mode_lang.contains("_vert") {
        let vert_image = if best.rotation == 0 {
            image
        } else {
            image.rotate_clockwise(best.rotation)
        };
        if let Ok(reconstructed) = recover_horizontal_english_in_vertical_page(
            &vert_image,
            &best.image_path,
            &best.mode_lang,
            &best.tsv,
            work_dir,
        ) {
            if !reconstructed.trim().is_empty() {
                return Ok(reconstructed);
            }
        }
    }

    Ok(best.text)
}

struct OcrCandidate {
    text: String,
    tsv: String,
    score: f64,
    rotation: u32,
    mode_lang: String,
    image_path: std::path::PathBuf,
}

fn tesseract_command() -> Command {
    let mut cmd = Command::new("tesseract");
    if let Ok(dir) = env::var("OCR_TESSDATA_DIR") {
        if !dir.trim().is_empty() {
            cmd.arg("--tessdata-dir").arg(dir.trim());
            return cmd;
        }
    }
    for candidate in ["./storage/tessdata", "../storage/tessdata"] {
        let path = Path::new(candidate);
        if path.join("chi_tra_vert.traineddata").is_file() {
            cmd.arg("--tessdata-dir").arg(path);
            break;
        }
    }
    cmd
}

fn available_tesseract_langs() -> HashSet<String> {
    let Ok(output) = tesseract_command().arg("--list-langs").output() else {
        return HashSet::new();
    };
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.contains(' ') && !l.ends_with(':'))
        .map(ToOwned::to_owned)
        .collect()
}

/// 由橫式語言設定推導直式語言（例如 `chi_tra+eng` -> `chi_tra_vert`），未安裝時回傳 None。
fn vertical_ocr_lang(lang: &str) -> Option<String> {
    if let Ok(custom) = env::var("OCR_VERTICAL_LANG") {
        let custom = custom.trim();
        return if custom.is_empty() {
            None
        } else {
            Some(custom.to_string())
        };
    }
    let installed = available_tesseract_langs();
    let candidates: Vec<String> = lang
        .split('+')
        .filter(|l| l.starts_with("chi_") || l.starts_with("jpn") || l.starts_with("kor"))
        .map(|l| format!("{l}_vert"))
        .filter(|l| installed.contains(l))
        .collect();
    if candidates.is_empty() {
        None
    } else {
        Some(candidates.join("+"))
    }
}

/// 使用 tesseract OSD 偵測頁面需要順時針旋轉的角度（0/90/180/270）。
fn detect_osd_rotation(image_path: &Path) -> Option<u32> {
    let output = tesseract_command()
        .arg(image_path)
        .arg("stdout")
        .arg("--psm")
        .arg("0")
        .output()
        .ok()?;
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    parse_osd_rotation(&text)
}

fn parse_osd_rotation(osd_output: &str) -> Option<u32> {
    osd_output.lines().find_map(|line| {
        let value = line.trim().strip_prefix("Rotate:")?.trim();
        let degrees = value.parse::<u32>().ok()? % 360;
        matches!(degrees, 0 | 90 | 180 | 270).then_some(degrees)
    })
}

fn run_tesseract_with_confidence(
    image_path: &Path,
    lang: &str,
    psm: &str,
    work_dir: &Path,
) -> Result<OcrCandidate> {
    let base = work_dir.join(format!("ocr-{}", Uuid::new_v4()));
    let output = tesseract_command()
        .arg(image_path)
        .arg(&base)
        .arg("-l")
        .arg(lang)
        .arg("--psm")
        .arg(psm)
        .arg("txt")
        .arg("tsv")
        .output()
        .with_context(|| "執行 tesseract 失敗")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("tesseract OCR 失敗：{stderr}");
    }

    let text = fs::read_to_string(base.with_extension("txt")).unwrap_or_default();
    let tsv = fs::read_to_string(base.with_extension("tsv")).unwrap_or_default();

    Ok(OcrCandidate {
        text: collapse_cjk_spaces(text.trim()),
        tsv,
        score: tesseract_tsv_score_ref(&base.with_extension("tsv")),
        rotation: 0,
        mode_lang: lang.to_string(),
        image_path: image_path.to_path_buf(),
    })
}

fn tesseract_tsv_score_ref(path: &Path) -> f64 {
    let tsv = fs::read_to_string(path).unwrap_or_default();
    tesseract_tsv_score(&tsv)
}

#[derive(Debug, Clone)]
struct TsvWord {
    left: usize,
    top: usize,
    w: usize,
    h: usize,
    conf: f64,
    text: String,
}

type TsvLineKey = (i32, i32, i32, i32);

fn parse_tsv_lines(tsv: &str) -> Vec<(TsvLineKey, Vec<TsvWord>)> {
    let mut map: std::collections::BTreeMap<TsvLineKey, Vec<TsvWord>> =
        std::collections::BTreeMap::new();
    for row in tsv.lines().skip(1) {
        let cols: Vec<&str> = row.split('\t').collect();
        if cols.len() < 12 || cols[0].trim() != "5" {
            continue;
        }
        let text = cols[11].trim();
        if text.is_empty() {
            continue;
        }
        let Ok(page_num) = cols[1].trim().parse::<i32>() else {
            continue;
        };
        let Ok(block_num) = cols[2].trim().parse::<i32>() else {
            continue;
        };
        let Ok(par_num) = cols[3].trim().parse::<i32>() else {
            continue;
        };
        let Ok(line_num) = cols[4].trim().parse::<i32>() else {
            continue;
        };
        let Ok(left) = cols[6].trim().parse::<usize>() else {
            continue;
        };
        let Ok(top) = cols[7].trim().parse::<usize>() else {
            continue;
        };
        let Ok(w) = cols[8].trim().parse::<usize>() else {
            continue;
        };
        let Ok(h) = cols[9].trim().parse::<usize>() else {
            continue;
        };
        let Ok(conf) = cols[10].trim().parse::<f64>() else {
            continue;
        };

        map.entry((page_num, block_num, par_num, line_num))
            .or_default()
            .push(TsvWord {
                left,
                top,
                w,
                h,
                conf,
                text: text.to_string(),
            });
    }
    map.into_iter().collect()
}

/// 針對直式中文頁面（`chi_tra_vert`）中夾雜的橫向英文/數字（如 `CNS 690`、`IEC1058`、`UL`、`6002679`），
/// 自動定位直式辨識信心低或亂碼的區段，將該局部影像逆時針旋轉 90°（順時針 270°）轉為正向橫式後，
/// 以 `tesseract -l eng --psm 7` 辨識並鑲回原直式行內；若 `--psm 5` 遇到表格區塊留白，亦自動以 `--psm 3` 補齊。
fn recover_horizontal_english_in_vertical_page(
    vert_image: &GrayImage,
    vert_image_path: &Path,
    vert_lang: &str,
    psm5_tsv: &str,
    work_dir: &Path,
) -> Result<String> {
    let lines_psm5 = parse_tsv_lines(psm5_tsv);
    if lines_psm5.is_empty() {
        return Ok(String::new());
    }

    let mut ordered: Vec<(usize, TsvLineKey, Vec<TsvWord>, bool)> = lines_psm5
        .into_iter()
        .map(|(k, words)| {
            let mut lefts: Vec<usize> = words.iter().map(|w| w.left).collect();
            lefts.sort_unstable();
            let med_left = lefts[lefts.len() / 2];
            (med_left, k, words, false)
        })
        .collect();
    ordered.sort_by(|a, b| b.0.cmp(&a.0));

    // 若直式段落之間存在大範圍空白（例如第 5 頁中間的表格被 --psm 5 略過），改用 --psm 3 補回該區間的行
    if ordered.len() >= 2 {
        let max_gap = ordered
            .windows(2)
            .map(|w| w[0].0.saturating_sub(w[1].0))
            .max()
            .unwrap_or(0);
        if max_gap > 250 {
            if let Ok(psm3_cand) =
                run_tesseract_with_confidence(vert_image_path, vert_lang, "3", work_dir)
            {
                let lines_psm3 = parse_tsv_lines(&psm3_cand.tsv);
                let gaps: Vec<(usize, usize)> = ordered
                    .windows(2)
                    .filter_map(|w| {
                        let (hi, lo) = (w[0].0, w[1].0);
                        (hi.saturating_sub(lo) > 250).then_some((lo, hi))
                    })
                    .collect();
                for (k3, w3) in lines_psm3 {
                    let mut lefts: Vec<usize> = w3.iter().map(|w| w.left).collect();
                    lefts.sort_unstable();
                    let ml3 = lefts[lefts.len() / 2];
                    if gaps
                        .iter()
                        .any(|&(lo, hi)| ml3 > lo.saturating_add(40) && ml3 + 40 < hi)
                    {
                        ordered.push((ml3, k3, w3, true));
                    }
                }
                ordered.sort_by(|a, b| b.0.cmp(&a.0));
            }
        }
    }

    let mut out_lines = Vec::new();
    for (_, _, words, is_gap_table_row) in ordered {
        let line_str = reconstruct_vertical_line_with_eng(&words, vert_image, work_dir);
        let collapsed = collapse_cjk_spaces(line_str.trim());
        if !collapsed.is_empty() {
            if is_gap_table_row {
                out_lines.push(format!("- {collapsed}"));
            } else {
                out_lines.push(collapsed);
            }
        }
    }

    Ok(out_lines.join("\n"))
}

fn is_core_cjk_ideograph(ch: char) -> bool {
    matches!(ch as u32, 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x2FA1F)
}

fn is_suspicious_vert_token(text: &str, conf: f64) -> bool {
    let cjk_count = text.chars().filter(|&c| is_core_cjk_ideograph(c)).count();
    let has_ascii_noise = text.chars().any(|c| {
        c.is_ascii_alphanumeric()
            || matches!(
                c,
                '@' | '&' | '~' | '=' | '<' | '>' | ':' | ';' | '/' | '\\' | '$' | '*' | '#'
            )
    });
    if !has_ascii_noise && cjk_count >= 2 && conf > 5.0 {
        return false;
    }
    if !has_ascii_noise
        && cjk_count == 1
        && text.chars().count() == 1
        && conf >= 72.0
        && !matches!(text, "二" | "三" | "一")
    {
        return false;
    }
    if !has_ascii_noise
        && matches!(
            text,
            "號" | "第"
                | "年"
                | "月"
                | "日"
                | "依"
                | "用"
                | "對"
                | "類"
                | "定"
                | "規"
                | "線"
                | "電"
                | "纜"
                | "試"
                | "驗"
                | "標"
                | "準"
        )
        && conf >= 40.0
    {
        return false;
    }
    if has_ascii_noise {
        return true;
    }
    if text.chars().any(|c| matches!(c, '(' | ')' | '「' | '」')) && conf < 85.0 {
        return true;
    }
    if conf < 72.0 {
        return true;
    }
    if matches!(text, "二" | "三" | "一") && conf < 89.0 {
        return true;
    }
    false
}

fn clean_vertical_embedded_eng_token(raw: &str, conf: f64) -> Option<String> {
    let trimmed = raw.trim();
    let trim_chars: &[char] = &[
        ' ', '\t', '\r', '\n', '.', ',', ':', ';', '!', '?', '\'', '"', '`', '~', '*', '|', '_',
        '{', '}', '[', ']', '<', '>', '\\', '/', '-', '+', '=', '(', ')',
    ];
    let core = trimmed.trim_matches(trim_chars);
    if core.is_empty() || !core.chars().any(|c| c.is_ascii_alphanumeric()) {
        return None;
    }

    let mut t = if trimmed.starts_with('(') && trimmed.ends_with(')') {
        format!("({core})")
    } else if trimmed.ends_with(')')
        && !core.contains('(')
        && (2..=3).contains(&core.len())
        && core.chars().all(|c| c.is_ascii_digit())
    {
        format!("({core})")
    } else {
        core.to_string()
    };

    // 修正常見的新細明體/標楷體有襯線大寫 I 與 C 在 200 DPI 下的辨識誤差（如 TEC730 -> IEC730, ONS690 -> CNS690）
    for prefix in ["IECL", "TECL"] {
        if let Some(rest) = t.strip_prefix(prefix) {
            if rest.len() >= 2 && rest.chars().take(2).all(|c| c.is_ascii_digit()) {
                t = format!("IEC1{rest}");
                break;
            }
        }
    }
    for prefix in ["TEC", "EC"] {
        if let Some(rest) = t.strip_prefix(prefix) {
            if rest.len() >= 2 && rest.chars().take(2).all(|c| c.is_ascii_digit()) {
                t = format!("IEC{rest}");
                break;
            }
        }
    }
    if matches!(t.as_str(), "IEC10S8" | "TEC10S8") {
        t = "IEC1058".to_string();
    } else if matches!(t.as_str(), "IECO91" | "TECO91") {
        t = "IEC691".to_string();
    } else if matches!(t.as_str(), "TEC" | "EC") {
        t = "IEC".to_string();
    } else if let Some(rest) = t.strip_prefix("ONS") {
        if rest.is_empty() || rest.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            t = format!("CNS{rest}");
        }
    }
    if t == "CNS1091" {
        t = "CNS10917-1".to_string();
    }
    t = t.replace("--", "~");

    let known_std = [
        "CNS", "IEC", "UL", "CSA", "VDE", "IMQ", "ISO", "EN", "BS", "JIS", "IEEE",
    ]
    .iter()
    .any(|p| t.starts_with(p));
    if known_std && conf >= 30.0 {
        return Some(t);
    }

    if t.starts_with('(')
        && t.ends_with(')')
        && (4..=5).contains(&t.len())
        && t[1..t.len() - 1].chars().all(|c| c.is_ascii_digit())
        && conf >= 50.0
    {
        return Some(t);
    }

    // 數值與單位（如 10917, 6002679, 245, 0.8~5.0mm, 0.9~500m, 10）
    let unit_stripped = ["mm", "cm", "m", "Hz", "kg", "A", "V", "W", "g"]
        .iter()
        .find_map(|u| t.strip_suffix(u))
        .unwrap_or(&t);
    if !unit_stripped.is_empty()
        && unit_stripped
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
        && unit_stripped
            .chars()
            .last()
            .is_some_and(|c| c.is_ascii_digit())
        && unit_stripped
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | '~' | '-'))
        && t.len() >= 2
        && conf >= 55.0
    {
        if t.len() == 2 && t.chars().all(|c| c.is_ascii_digit()) && conf < 80.0 {
            return None;
        }
        return Some(t);
    }

    if conf >= 80.0
        && t.len() >= 2
        && t.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && t.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '(' | ')' | '.' | '~' | '/' | '+' | '-')
        })
    {
        if (2..=3).contains(&t.len()) && t.chars().all(|c| c.is_ascii_lowercase()) {
            return None;
        }
        if t.len() == 2 && t.starts_with('4') {
            return None;
        }
        return Some(t);
    }

    None
}

fn crop_and_rotate270_with_border(
    img_vert: &GrayImage,
    x_left: usize,
    x_right: usize,
    y_top: usize,
    y_bot: usize,
    border: usize,
) -> GrayImage {
    let x1 = x_left.min(img_vert.width);
    let x2 = x_right.min(img_vert.width).max(x1);
    let y1 = y_top.min(img_vert.height);
    let y2 = y_bot.min(img_vert.height).max(y1);
    let w = x2 - x1;
    let h = y2 - y1;
    let cw = h;
    let ch = w;
    let out_w = cw + 2 * border;
    let out_h = ch + 2 * border;
    let mut pixels = vec![255u8; out_w * out_h];

    for r_y in 0..ch {
        let xv = x1 + (w - 1 - r_y);
        let mut dark_count = 0usize;
        let dst_row_start = (r_y + border) * out_w + border;
        for r_x in 0..cw {
            let yv = y1 + r_x;
            let px = img_vert.pixels[yv * img_vert.width + xv];
            if px < 190 {
                dark_count += 1;
            }
            pixels[dst_row_start + r_x] = px;
        }
        // 若裁切區塊上下邊緣緊貼表格橫線（整列超過 55% 為深色像素），將該表格框線塗白避免干擾英文辨識
        if (r_y < 6 || r_y + 6 >= ch) && cw > 40 && dark_count * 100 > cw * 55 {
            pixels[dst_row_start..dst_row_start + cw].fill(255);
        }
    }

    GrayImage {
        width: out_w,
        height: out_h,
        pixels,
    }
}

fn reconstruct_vertical_line_with_eng(
    words: &[TsvWord],
    img_vert: &GrayImage,
    work_dir: &Path,
) -> String {
    if words.is_empty() {
        return String::new();
    }
    let mut lefts: Vec<usize> = words.iter().map(|w| w.left).collect();
    let mut widths: Vec<usize> = words.iter().map(|w| w.w).collect();
    lefts.sort_unstable();
    widths.sort_unstable();
    let med_left = lefts[lefts.len() / 2];
    let med_w = widths[widths.len() / 2];
    if med_w <= 8 || med_w > 120 {
        return words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join("");
    }

    let hv = img_vert.height;
    let wv = img_vert.width;
    let mut y_has_ink = vec![false; hv];
    let core_x1 = (med_left + 4).min(wv);
    let core_x2 = (med_left + med_w)
        .saturating_sub(4)
        .max(core_x1 + 1)
        .min(wv);
    for (y, has_ink) in y_has_ink.iter_mut().enumerate() {
        for x in core_x1..core_x2 {
            if img_vert.pixels[y * wv + x] < 180 {
                *has_ink = true;
                break;
            }
        }
    }

    let has_white_gap = |y: usize, min_gap: usize| -> bool {
        if y >= hv || y_has_ink[y] {
            return false;
        }
        let mut l = y;
        while l > 0 && !y_has_ink[l - 1] {
            l -= 1;
        }
        let mut r = y;
        while r + 1 < hv && !y_has_ink[r + 1] {
            r += 1;
        }
        (r - l + 1) >= min_gap
    };

    let expand_up = |y_start: usize, max_step: usize| -> usize {
        let mut y = y_start.saturating_sub(4);
        let limit = y_start.saturating_sub(max_step);
        while y > limit && !has_white_gap(y, 4) {
            y -= 1;
        }
        y.saturating_sub(6)
    };

    let expand_down = |y_end: usize, max_step: usize| -> usize {
        let mut y = (y_end + 4).min(hv.saturating_sub(1));
        let limit = (y_end + max_step).min(hv.saturating_sub(1));
        while y < limit && !has_white_gap(y, 8) {
            y += 1;
        }
        (y + 8).min(hv)
    };

    let n = words.len();
    let mut susp: Vec<bool> = words
        .iter()
        .map(|w| is_suspicious_vert_token(&w.text, w.conf))
        .collect();
    for i in 1..n.saturating_sub(1) {
        if !susp[i]
            && susp[i - 1]
            && susp[i + 1]
            && matches!(words[i].text.as_str(), "或" | "及" | "、" | ",")
        {
            susp[i] = true;
        }
    }

    let mut out_tokens: Vec<String> = Vec::new();
    let mut i = 0;
    while i < n {
        if !susp[i] {
            out_tokens.push(words[i].text.clone());
            i += 1;
            continue;
        }
        let mut j = i;
        while j + 1 < n
            && susp[j + 1]
            && words[j + 1].top.saturating_sub(words[j].top + words[j].h) <= 45
        {
            j += 1;
        }

        let raw_top = words[i..=j].iter().map(|w| w.top).min().unwrap_or(0);
        let raw_bot = words[i..=j].iter().map(|w| w.top + w.h).max().unwrap_or(0);
        let y1 = expand_up(raw_top, 24);
        let y2 = expand_down(raw_bot, 28);

        // 使用該區段內的 left 與 width 中位數（並以整行中位數限制上限），避免表格框線被納入裁切高度
        let mut span_lefts: Vec<usize> = words[i..=j].iter().map(|w| w.left).collect();
        let mut span_widths: Vec<usize> = words[i..=j].iter().map(|w| w.w).collect();
        span_lefts.sort_unstable();
        span_widths.sort_unstable();
        let span_med_left = span_lefts[span_lefts.len() / 2].max(med_left.saturating_sub(6));
        let span_med_w = span_widths[span_widths.len() / 2].min(med_w + 4);
        let x_left = span_med_left.saturating_sub(5);
        let x_right = (span_med_left + span_med_w + 5).min(wv);

        let mut eng_spans: Vec<(isize, isize, String)> = Vec::new();
        if y2 > y1 + 20 && x_right > x_left + 8 {
            let border = 20usize;
            let patch = crop_and_rotate270_with_border(img_vert, x_left, x_right, y1, y2, border);
            let patch_path = work_dir.join(format!("vert-patch-{}.pgm", Uuid::new_v4()));
            if patch.write_pgm(&patch_path).is_ok() {
                if let Ok(cand) = run_tesseract_with_confidence(&patch_path, "eng", "7", work_dir) {
                    for row in cand.tsv.lines().skip(1) {
                        let cols: Vec<&str> = row.split('\t').collect();
                        if cols.len() < 12 || cols[0].trim() != "5" {
                            continue;
                        }
                        let Ok(lx) = cols[6].trim().parse::<isize>() else {
                            continue;
                        };
                        let Ok(lw) = cols[8].trim().parse::<isize>() else {
                            continue;
                        };
                        let Ok(conf) = cols[10].trim().parse::<f64>() else {
                            continue;
                        };
                        if let Some(cleaned) = clean_vertical_embedded_eng_token(cols[11], conf) {
                            let wy1 = y1 as isize + (lx - border as isize);
                            let wy2 = wy1 + lw;
                            eng_spans.push((wy1, wy2, cleaned));
                        }
                    }
                }
                let _ = fs::remove_file(&patch_path);
            }
        }

        if eng_spans.is_empty() {
            for w in &words[i..=j] {
                out_tokens.push(w.text.clone());
            }
        } else {
            let mut items: Vec<(f64, bool, String)> = eng_spans
                .iter()
                .map(|(sy1, sy2, stxt)| ((*sy1 as f64 + *sy2 as f64) / 2.0, true, stxt.clone()))
                .collect();

            let is_lead_punc =
                |c: char| matches!(c, '、' | '，' | '。' | '：' | '（' | '「' | '『' | '〔');
            let is_trail_punc =
                |c: char| matches!(c, '、' | '，' | '。' | '：' | '）' | '」' | '』' | '〕');

            let lead_punc: String = words[i]
                .text
                .chars()
                .take_while(|&c| is_lead_punc(c))
                .collect();
            let trail_punc: String = words[j]
                .text
                .chars()
                .rev()
                .take_while(|&c| is_trail_punc(c))
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();

            for w in &words[i..=j] {
                let wt1 = w.top as isize;
                let wt2 = (w.top + w.h) as isize;
                let w_center = (wt1 as f64 + wt2 as f64) / 2.0;
                if matches!(w.text.as_str(), "或" | "及") {
                    items.push((w_center, false, w.text.clone()));
                } else {
                    let overlaps_eng = eng_spans
                        .iter()
                        .any(|(sy1, sy2, _)| wt2.min(*sy2) - wt1.max(*sy1) > -4);
                    if !overlaps_eng {
                        let cjk_only: String = w
                            .text
                            .chars()
                            .filter(|&c| {
                                is_core_cjk_ideograph(c)
                                    || matches!(
                                        c,
                                        '、' | '，' | '。' | '：' | '（' | '）' | '「' | '」'
                                    )
                            })
                            .collect();
                        if !cjk_only.is_empty() {
                            items.push((w_center, false, cjk_only));
                        }
                    }
                }
            }

            items.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

            if !lead_punc.is_empty()
                && !out_tokens
                    .last()
                    .is_some_and(|last| last.ends_with(&lead_punc))
            {
                out_tokens.push(lead_punc);
            }
            for (_, is_eng, txt) in items {
                let needs_space = is_eng
                    && out_tokens
                        .last()
                        .and_then(|last| last.chars().last())
                        .is_some_and(|c| c.is_ascii_alphanumeric())
                    && txt
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_alphanumeric());
                if needs_space {
                    out_tokens.push(format!(" {txt}"));
                } else {
                    out_tokens.push(txt);
                }
            }
            if !trail_punc.is_empty()
                && !out_tokens
                    .last()
                    .is_some_and(|last| last.ends_with(&trail_punc))
            {
                out_tokens.push(trail_punc);
            }
        }

        i = j + 1;
    }

    out_tokens.join("")
}

/// 以字元數加權計算 tesseract 平均信心值。
fn tesseract_tsv_score(tsv: &str) -> f64 {
    let mut weighted = 0.0;
    let mut total = 0.0;
    for line in tsv.lines().skip(1) {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 12 {
            continue;
        }
        let Ok(conf) = cols[10].trim().parse::<f64>() else {
            continue;
        };
        let word = cols[11].trim();
        if conf < 0.0 || word.is_empty() {
            continue;
        }
        let len = word.chars().count() as f64;
        weighted += conf * len;
        total += len;
    }
    if total == 0.0 { 0.0 } else { weighted / total }
}

fn is_cjk_char(c: char) -> bool {
    matches!(c as u32,
        0x3000..=0x303F | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF | 0x20000..=0x2FA1F)
}

/// tesseract 辨識中文時常在每個字之間插入空白，移除 CJK 字元之間的多餘空白。
fn collapse_cjk_spaces(input: &str) -> String {
    input
        .lines()
        .map(|line| {
            let chars: Vec<char> = line.chars().collect();
            let mut out = String::with_capacity(line.len());
            for (i, &c) in chars.iter().enumerate() {
                if c == ' ' {
                    let prev = chars[..i].iter().rev().find(|ch| **ch != ' ');
                    let next = chars[i + 1..].iter().find(|ch| **ch != ' ');
                    let touches_cjk = prev.is_some_and(|p| is_cjk_char(*p))
                        || next.is_some_and(|n| is_cjk_char(*n));
                    if touches_cjk || out.ends_with(' ') {
                        continue;
                    }
                }
                out.push(c);
            }
            out.trim().to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

struct GrayImage {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

impl GrayImage {
    fn read_pgm(path: &Path) -> Result<Self> {
        let data =
            fs::read(path).with_context(|| format!("讀取 OCR 影像失敗: {}", path.display()))?;
        let mut pos = 0usize;
        let mut fields: Vec<String> = Vec::new();
        while fields.len() < 4 {
            while pos < data.len() && data[pos].is_ascii_whitespace() {
                pos += 1;
            }
            if pos < data.len() && data[pos] == b'#' {
                while pos < data.len() && data[pos] != b'\n' {
                    pos += 1;
                }
                continue;
            }
            let start = pos;
            while pos < data.len() && !data[pos].is_ascii_whitespace() {
                pos += 1;
            }
            if start == pos {
                bail!("PGM 標頭格式錯誤");
            }
            fields.push(String::from_utf8_lossy(&data[start..pos]).to_string());
        }
        pos += 1; // 單一空白分隔標頭與像素資料
        if fields[0] != "P5" {
            bail!("僅支援 P5 格式的 PGM 影像");
        }
        let width: usize = fields[1].parse().context("PGM 寬度格式錯誤")?;
        let height: usize = fields[2].parse().context("PGM 高度格式錯誤")?;
        let maxval: usize = fields[3].parse().context("PGM maxval 格式錯誤")?;
        if maxval > 255 {
            bail!("不支援 16-bit PGM 影像");
        }
        let needed = width * height;
        if data.len() < pos + needed {
            bail!("PGM 像素資料不完整");
        }
        Ok(Self {
            width,
            height,
            pixels: data[pos..pos + needed].to_vec(),
        })
    }

    fn write_pgm(&self, path: &Path) -> Result<()> {
        let mut out = format!("P5\n{} {}\n255\n", self.width, self.height).into_bytes();
        out.extend_from_slice(&self.pixels);
        fs::write(path, out).with_context(|| format!("寫入 OCR 影像失敗: {}", path.display()))
    }

    fn rotate_clockwise(&self, degrees: u32) -> Self {
        let (w, h) = (self.width, self.height);
        match degrees % 360 {
            90 => {
                let mut pixels = vec![0u8; w * h];
                for y in 0..h {
                    for x in 0..w {
                        pixels[x * h + (h - 1 - y)] = self.pixels[y * w + x];
                    }
                }
                Self {
                    width: h,
                    height: w,
                    pixels,
                }
            }
            180 => {
                let mut pixels = self.pixels.clone();
                pixels.reverse();
                Self {
                    width: w,
                    height: h,
                    pixels,
                }
            }
            270 => {
                let mut pixels = vec![0u8; w * h];
                for y in 0..h {
                    for x in 0..w {
                        pixels[(w - 1 - x) * h + y] = self.pixels[y * w + x];
                    }
                }
                Self {
                    width: h,
                    height: w,
                    pixels,
                }
            }
            _ => Self {
                width: w,
                height: h,
                pixels: self.pixels.clone(),
            },
        }
    }
}

fn command_exists(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

async fn extract_with_ai(document: &str, source_file: Option<&str>) -> Result<AiMeetingPayload> {
    let api_url = env::var("AI_API_URL").unwrap_or_else(|_| {
        env::var("OLLAMA_API_URL").unwrap_or_else(|_| "http://127.0.0.1:11434/api/chat".to_string())
    });
    let is_ollama = api_url.contains("/api/chat");
    let model = env::var("AI_MODEL").unwrap_or_else(|_| {
        if is_ollama {
            "qwen2.5:14b".to_string()
        } else {
            "gpt-4o-mini".to_string()
        }
    });
    let vl_model = env::var("AI_VL_MODEL")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let vl_document = if let Some(vl_model_name) = vl_model.as_deref() {
        match maybe_run_vl_whole_document(
            api_url.as_str(),
            is_ollama,
            vl_model_name,
            source_file,
            document,
        )
        .await
        {
            Ok(v) => v,
            Err(error) => {
                eprintln!(
                    "[ingest-meeting-ai] warning: VL 整份文件前處理失敗，改用原始文字。原因：{error}"
                );
                document.to_string()
            }
        }
    } else {
        document.to_string()
    };
    let working_document = if vl_document.trim().is_empty() {
        document.to_string()
    } else {
        vl_document
    };

    let system_prompt = r#"你是一個專門處理技術會議紀錄的結構化資料萃取引擎。請閱讀提供的「會議紀錄原始文本」，將內容精準萃取並轉換為符合資料庫 Schema 的結構化 JSON 物件。

### 核心處理規則：

1. **一字不漏依照原文文字呈現（禁止自行整理、刪減或改寫）**：
   - **嚴禁自行摘要、改寫、精簡或捏造標題**，所有內容必須完全依照原本內文的文字一字不漏呈現。
   - **一律維持繁體中文原文**，嚴禁將文字轉換為簡體中文。
   - **保留原文換行與條列層級**：原文中的 `1.`、`2.`、`3.` 以及次項目 `(1)`、`(2)`、`(3)`、`2.1`、`2.2` 請務必保留換行（`\n`），不要把多個項目擠在同一行。

2. **宣導事項 / 宣告事項（`meeting_promotions`）**：
   - 原文中的「宣導事項」、「宣告事項」、「宣布事項」、「報告事項」皆等同宣導事項，請放入 `meeting_promotions`。
   - **如有多個項目請務必拆分**：若宣導/宣告事項包含多個大項（例如 `1.`、`2.`、`3.` 或 `一、`、`二、`），**嚴禁合併在同一筆**，必須拆分為多筆獨立的物件，`promotion_no` 依序為 `1`, `2`, `3`...。
   - 若某一大項底下包含次層級子項（例如第 2 項底下的 `2.1`、`2.2`、`2.3`，或 `(一)`、`(二)`），請保留在該筆 `content` 內並保留換行。
   - `content` 請直接放該項目的原文內容（移除開頭多餘的「宣告事項：」或「宣導事項：」標題字樣，以及最外層的大題號如 `1.`、`2.`）。
   - 如無資料請勿塞入預設資料，保持該欄位為空或不輸出。

3. **提案討論與決議（`agenda_resolution_pairs`）**：
   - **編號（`item_no`）**：**只使用純數字依序編號**（`"1"`, `"2"`, `"3"`, `"4"`...），**嚴禁**加上任何分類文字或中文數字（例如禁止輸出 `家電安規-一-1` 或 `資訊安規-六-1`）。
   - **多子題拆分**：若某提案單位包含多個獨立子議題（例如某公司提案含 `1.`、`2.`、`3.` 或 `1.1`、`1.2`），**嚴禁將多個子議題合併在同一個物件**，必須拆分為多筆獨立資料項，且 `item_no` 繼續依序以純數字編號（`"1"`, `"2"`, `"3"`...）。
   - **議題欄位（`agenda_item`）**：
     - **直接放入該議題的完整原文**，嚴禁自行濃縮成短標題，也**不需要自行分類**（嚴禁在開頭加上 `**資訊安規**`、`**家電安規**` 等分類標籤）。
     - 若議題為某公司或單位提案，僅顯示：`XX公司提案：提案議題原文`（並完整保留提案內的所有 `(1)`、`(2)`、`(3)` 條件與換行）。
   - **決議欄位（`resolution`）**：
     - **直接放入該議題對應之「決議」完整原文**（移除開頭「決議：」字樣）。
     - **嚴禁遺漏決議的最後一點或參考說明**：決議底下所有的編號項目（如 `1.`、`2.`、`3.`，包含最後一點如 `3. 可詳閱...之技術會議記錄。`、`請參考...` 等）皆屬於決議正文，**必須全部完整保留在 `resolution` 內**，嚴禁省略任何一點！
     - **保留換行**：決議內的 `1.`、`2.`、`3.` 與 `(1)`、`(2)`、`(3)` 之間請保留換行（`\n`）。
   - **結論標籤（`result`）**：簡要歸納該決議結果狀態（如：`不予接受`、`同意辦理`、`依規定辦理`、`暫時接受`、`不要求` 等）。
   - **僅有「會議結論」條列之格式（無獨立提案/決議字樣時）**：若文件結構為「伍、會議結論：」底下直接列出「一、...」「二、...」至「十六、...」等結論條目（沒有標示「XX提案」與「決議：」），請將每一個大項（「一、」、「二、」...）各自拆為一筆 `agenda_resolution_pairs`（若該大項包含冒號與子項目 `(一)`、`(二)`，則冒號前的大項說明放 `agenda_item`，子項目原文放 `resolution`；若該大項僅為單一段落敘述，則將該段完整原文放入 `resolution`，並取其主題或原文放入 `agenda_item`），**嚴禁因沒有「決議：」字樣而回傳空陣列**！
   - 如無資料請勿塞入預設資料，保持該欄位為空或不輸出。

4. **日期時間與輸出限制**：
   - 日期時間：民國年轉為西元 ISO 8601 格式（如民國94年8月24日轉為 `2005-08-24`）。
   - 僅輸出合法的純 JSON，禁止輸出任何前言或結尾閒聊。

---

### 輸出 JSON 結構規範：

{
  "meeting": {
    "meeting_no": "會議編號（無則填 null）",
    "meeting_date": "YYYY-MM-DD",
    "meeting_time": "YYYY-MM-DDTHH:mm:ss+08:00（若無具體時間則填 null）",
    "meeting_title": "會議原始全稱",
    "title": "會議主標題",
    "chairperson": "主席",
    "attendees": "出席人員原文",
    "meeting_location": "地點",
    "recorder": "記錄人及聯絡電話",
    "summary": "會議重點概述",
    "published_at": null
  },
  "meeting_promotions": [
    {
      "promotion_no": 1,
      "content": "第1項宣導/宣告事項完整原文（不含「宣告事項:」標題）",
      "notes": null
    },
    {
      "promotion_no": 2,
      "content": "第2項宣導/宣告事項完整原文（如含 2.1, 2.2 等子項請保留在同一筆內並換行）",
      "notes": null
    }
  ],
  "agenda_resolution_pairs": [
    {
      "item_no": "1",
      "agenda_item": "議題完整原文（若為某公司提案，僅顯示「XX公司提案：提案議題完整原文」，保留換行，不加分類標籤、不自行整理）",
      "resolution": "該議題對應之決議完整原文（一字不漏保留決議下所有 1.、2.、3. 及 (1)(2)(3) 點次與「可詳閱...」等說明，並保留換行）",
      "result": "結論標籤（例：不要求 / 依規定辦理 / 同意辦理 / 不予接受）"
    }
  ]
}"#;

    let max_chunk_chars = env::var("AI_CHUNK_CHARS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(12_000);
    let chunks = split_document_into_chunks(&working_document, max_chunk_chars);
    if chunks.len() > 1 {
        eprintln!(
            "[ingest-meeting-ai] 文件較長（{} 字），拆分為 {} 個區塊單輪擷取並合併...",
            working_document.chars().count(),
            chunks.len()
        );
        let mut combined_payload = AiMeetingPayload {
            meeting: empty_meeting(),
            promotions: Vec::new(),
            agenda_resolution_pairs: Vec::new(),
        };

        for (idx, chunk) in chunks.iter().enumerate() {
            let chunk_prompt = format!(
                "以下是會議文件第 {}/{} 區段原文，請完整擷取此區段內出現的所有會議資訊、宣導事項（meeting_promotions）與討論議題及決議（agenda_resolution_pairs），不可遺漏任何子議題或決議點次：\n\n{}",
                idx + 1,
                chunks.len(),
                chunk
            );
            match request_ai_payload(
                &api_url,
                is_ollama,
                &model,
                system_prompt,
                &chunk_prompt,
                Some(&combined_payload.meeting),
            )
            .await
            {
                Ok(chunk_payload) => {
                    eprintln!(
                        "[ingest-meeting-ai] 區段 {}/{} 擷取完成：promotions={}, agenda_resolution_pairs={}",
                        idx + 1,
                        chunks.len(),
                        chunk_payload.promotions.len(),
                        chunk_payload.agenda_resolution_pairs.len()
                    );
                    merge_ai_payload(&mut combined_payload, chunk_payload);
                }
                Err(error) => {
                    eprintln!(
                        "[ingest-meeting-ai] warning: 區段 {}/{} 解析失敗：{error}",
                        idx + 1,
                        chunks.len()
                    );
                }
            }
        }

        if !combined_payload.agenda_resolution_pairs.is_empty()
            || !combined_payload.promotions.is_empty()
        {
            backfill_truncated_resolutions(&working_document, &mut combined_payload);
            return Ok(combined_payload);
        }
    }

    let base_user_prompt = format!(
        "以下是會議文件原文，請一字不漏完整擷取所有宣導事項、議題與決議（包含決議下所有點次）：\n\n{working_document}"
    );
    let mut best_payload = request_ai_payload(
        &api_url,
        is_ollama,
        &model,
        system_prompt,
        &base_user_prompt,
        None,
    )
    .await?;
    eprintln!(
        "[ingest-meeting-ai] AI 單輪擷取結果: promotions={}, agenda_resolution_pairs={}",
        best_payload.promotions.len(),
        best_payload.agenda_resolution_pairs.len()
    );

    if best_payload.agenda_resolution_pairs.is_empty()
        && (working_document.contains("議題")
            || working_document.contains("提案")
            || working_document.contains("決議"))
    {
        eprintln!("[ingest-meeting-ai] 單輪未抽到議題但原文含議題關鍵字，啟動補救重試...");
        let fallback_prompt = format!(
            r#"請只根據原文，重新輸出完整 JSON，特別確認 agenda_resolution_pairs 不得遺漏。
若遇到多段或跨頁同一議題，請保留在同一 pair 內，不可省略。

原文：
{working_document}"#
        );
        match request_ai_payload(
            &api_url,
            is_ollama,
            &model,
            system_prompt,
            &fallback_prompt,
            Some(&best_payload.meeting),
        )
        .await
        {
            Ok(fallback_payload) => {
                eprintln!(
                    "[ingest-meeting-ai] AI 補救重試結果: promotions={}, agenda_resolution_pairs={}",
                    fallback_payload.promotions.len(),
                    fallback_payload.agenda_resolution_pairs.len()
                );
                merge_ai_payload(&mut best_payload, fallback_payload);
            }
            Err(error) => {
                eprintln!(
                    "[ingest-meeting-ai] warning: AI 補救重試解析失敗，保留單輪結果。原因：{error}"
                );
            }
        }
    }

    backfill_truncated_resolutions(&working_document, &mut best_payload);
    Ok(best_payload)
}

fn split_document_into_chunks(document: &str, max_chunk_chars: usize) -> Vec<String> {
    if document.chars().count() <= max_chunk_chars {
        return vec![document.to_string()];
    }

    let pages: Vec<&str> = document.split("\n\n--- [Page ").collect();
    if pages.len() <= 1 {
        return vec![document.to_string()];
    }

    let header_context: String = pages[0].chars().take(600).collect();
    let mut chunks = Vec::new();
    let mut current_chunk = String::new();
    let mut prev_tail = String::new();

    for (idx, raw_page) in pages.into_iter().enumerate() {
        let page_block = if idx == 0 {
            raw_page.to_string()
        } else {
            format!("--- [Page {raw_page}")
        };

        let projected_len = current_chunk.chars().count() + page_block.chars().count();
        if !current_chunk.is_empty() && projected_len > max_chunk_chars {
            chunks.push(current_chunk);
            current_chunk = format!(
                "【會議基本資訊參考】\n{}\n\n【前頁結尾銜接參考】\n{}\n\n{}",
                header_context.trim(),
                prev_tail.trim(),
                page_block
            );
        } else {
            if !current_chunk.is_empty() {
                current_chunk.push_str("\n\n");
            }
            current_chunk.push_str(&page_block);
        }

        let page_chars: Vec<char> = page_block.chars().collect();
        let tail_start = page_chars.len().saturating_sub(450);
        prev_tail = page_chars[tail_start..].iter().collect();
    }

    if !current_chunk.trim().is_empty() {
        chunks.push(current_chunk);
    }

    chunks
}

fn merge_ai_payload(base: &mut AiMeetingPayload, incoming: AiMeetingPayload) {
    merge_option_string(&mut base.meeting.meeting_no, incoming.meeting.meeting_no);
    merge_option_string(
        &mut base.meeting.meeting_title,
        incoming.meeting.meeting_title,
    );
    merge_option_string(&mut base.meeting.title, incoming.meeting.title);
    merge_longer_option_string(&mut base.meeting.summary, incoming.meeting.summary);
    merge_option_string(
        &mut base.meeting.meeting_date,
        incoming.meeting.meeting_date,
    );
    merge_option_string(
        &mut base.meeting.meeting_time,
        incoming.meeting.meeting_time,
    );
    merge_option_string(&mut base.meeting.chairperson, incoming.meeting.chairperson);
    if base.meeting.attendees.is_none() {
        base.meeting.attendees = incoming.meeting.attendees;
    } else if let (Some(existing), Some(new_list)) =
        (&mut base.meeting.attendees, incoming.meeting.attendees)
    {
        for item in new_list {
            if !existing.contains(&item) {
                existing.push(item);
            }
        }
    }
    merge_option_string(
        &mut base.meeting.meeting_location,
        incoming.meeting.meeting_location,
    );
    merge_option_string(&mut base.meeting.recorder, incoming.meeting.recorder);
    merge_option_string(
        &mut base.meeting.published_at,
        incoming.meeting.published_at,
    );

    for promo in incoming.promotions {
        if promo.content.trim().is_empty() {
            continue;
        }
        let norm_incoming = normalize_for_compare(&promo.content);
        if let Some(existing) = base.promotions.iter_mut().find(|p| {
            let norm_existing = normalize_for_compare(&p.content);
            norm_existing == norm_incoming
                || (!norm_existing.is_empty()
                    && !norm_incoming.is_empty()
                    && (norm_existing.contains(&norm_incoming)
                        || norm_incoming.contains(&norm_existing)))
        }) {
            if promo.content.chars().count() > existing.content.chars().count() {
                existing.content = promo.content;
            }
            merge_longer_option_string(&mut existing.notes, promo.notes);
            if existing.promotion_no.is_none() {
                existing.promotion_no = promo.promotion_no;
            }
        } else {
            base.promotions.push(promo);
        }
    }

    for pair in incoming.agenda_resolution_pairs {
        if pair.agenda_item.trim().is_empty()
            && pair
                .resolution_title
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
            && pair
                .resolution_text
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
        {
            continue;
        }

        let incoming_no = pair
            .item_no
            .as_deref()
            .map(normalize_for_compare)
            .unwrap_or_default();
        let incoming_title = normalize_for_compare(&pair.agenda_item);

        let matched = base.agenda_resolution_pairs.iter_mut().find(|existing| {
            let existing_no = existing
                .item_no
                .as_deref()
                .map(normalize_for_compare)
                .unwrap_or_default();
            let existing_title = normalize_for_compare(&existing.agenda_item);

            if !incoming_no.is_empty() && incoming_no == existing_no {
                return true;
            }
            !incoming_title.is_empty()
                && incoming_title == existing_title
                && existing_no == incoming_no
        });

        if let Some(existing) = matched {
            if pair.agenda_item.chars().count() > existing.agenda_item.chars().count() {
                existing.agenda_item = pair.agenda_item;
            }
            merge_longer_option_string(&mut existing.agenda_notes, pair.agenda_notes);
            merge_longer_option_string(&mut existing.resolution_title, pair.resolution_title);
            merge_longer_option_string(&mut existing.resolution_content, pair.resolution_content);
            merge_longer_option_string(&mut existing.resolution_text, pair.resolution_text);
            merge_longer_option_string(&mut existing.resolution_result, pair.resolution_result);
        } else {
            base.agenda_resolution_pairs.push(pair);
        }
    }
}

fn merge_option_string(base: &mut Option<String>, incoming: Option<String>) {
    let is_empty = base.as_deref().map(|s| s.trim().is_empty()).unwrap_or(true);
    if is_empty {
        if let Some(val) = incoming {
            if !val.trim().is_empty() {
                *base = Some(val);
            }
        }
    }
}

fn merge_longer_option_string(base: &mut Option<String>, incoming: Option<String>) {
    if let Some(inc) = incoming {
        let inc_len = inc.trim().chars().count();
        if inc_len == 0 {
            return;
        }
        let base_len = base
            .as_deref()
            .map(|s| s.trim().chars().count())
            .unwrap_or(0);
        if inc_len > base_len {
            *base = Some(inc);
        }
    }
}

fn normalize_for_compare(input: &str) -> String {
    input
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '*' | '#' | '-' | '_' | ':' | '：'))
        .collect::<String>()
        .to_lowercase()
}

async fn request_ai_payload(
    api_url: &str,
    is_ollama: bool,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    fallback_meeting: Option<&AiMeeting>,
) -> Result<AiMeetingPayload> {
    let content = request_ai_content(api_url, is_ollama, model, system_prompt, user_prompt).await?;
    let cleaned = strip_markdown_fence(&content);
    match parse_ai_payload(&cleaned, fallback_meeting) {
        Ok(parsed) => Ok(parsed),
        Err(primary_error) => {
            eprintln!(
                "[ingest-meeting-ai] warning: 初次 JSON 解析失敗，嘗試修復格式後重試。原因：{primary_error}"
            );
            let repair_system_prompt = "你是 JSON 修復器。請將使用者提供的內容修正為合法 JSON，不得補造新資料，不得刪除原有欄位語意。只輸出 JSON 本體。";
            let repair_user_prompt = format!(
                "請將以下內容修復成合法 JSON（只輸出 JSON）：\n\n{}",
                cleaned
            );
            let repaired = request_ai_content(
                api_url,
                is_ollama,
                model,
                repair_system_prompt,
                &repair_user_prompt,
            )
            .await
            .context("AI JSON 修復請求失敗")?;
            let repaired_cleaned = strip_markdown_fence(&repaired);
            parse_ai_payload(&repaired_cleaned, fallback_meeting)
                .context("AI 回傳格式非預期 JSON，請調整文件內容重試")
        }
    }
}

async fn maybe_run_vl_whole_document(
    api_url: &str,
    is_ollama: bool,
    vl_model: &str,
    source_file: Option<&str>,
    fallback_text: &str,
) -> Result<String> {
    if !is_ollama {
        return Ok(fallback_text.to_string());
    }

    let Some(file_path) = source_file else {
        return Ok(fallback_text.to_string());
    };

    let pdf_path = Path::new(file_path);
    if pdf_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        != "pdf"
    {
        return Ok(fallback_text.to_string());
    }

    let png_batches = render_pdf_pages_to_base64_batches(pdf_path)?;
    if png_batches.is_empty() {
        return Ok(fallback_text.to_string());
    }

    let vl_max_chars = env::var("AI_VL_MAX_CHARS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(24_000);
    let mut markdown_parts = Vec::new();

    for (batch_idx, batch) in png_batches.iter().enumerate() {
        if batch.is_empty() {
            continue;
        }

        let batch_prompt = format!(
            "請閱讀這一批 PDF 頁面，直接輸出整合後的 Markdown 文本。請保留標題、日期、會議內容、議題、決議與表格資訊，並保留原文與層級。此批為 PDF 頁面 {} 至 {}。只輸出 Markdown，不要附加說明。",
            batch_idx * 2 + 1,
            batch_idx * 2 + batch.len()
        );

        let content = request_ai_content_with_images(
            api_url,
            vl_model,
            "你是一位專業 PDF 視覺閱讀與 Markdown 結構化整理助手。請逐頁檢視這批頁面，保留所有原文與表格資訊，僅輸出可直接供後續資料抽取的 Markdown 內容。",
            &batch_prompt,
            batch,
        )
        .await?;

        let cleaned = normalize_vl_markdown_output(&content);
        if cleaned.trim().is_empty() {
            continue;
        }

        markdown_parts.push(format!("{}", cleaned));
    }

    let combined = markdown_parts.join("\n\n");
    let trimmed = if combined.chars().count() > vl_max_chars {
        combined.chars().take(vl_max_chars).collect::<String>()
    } else {
        combined
    };
    let normalized = normalize_vl_markdown_output(&trimmed);
    println!("Markdown Output:\n{}", normalized);

    if normalized.trim().is_empty() {
        Ok(fallback_text.to_string())
    } else {
        Ok(normalized)
    }
}

fn normalize_vl_markdown_output(input: &str) -> String {
    let cleaned = strip_markdown_fence(input)
        .replace("\r\n", "\n")
        .replace("\r", "\n")
        .trim()
        .to_string();
    if cleaned.is_empty() {
        return String::new();
    }

    cleaned
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_pdf_pages_to_base64_batches(file_path: &Path) -> Result<Vec<Vec<String>>> {
    let max_pages = env::var("AI_VL_MAX_PAGES")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(20);
    let batch_size = env::var("AI_VL_BATCH_SIZE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|size| *size > 0)
        .unwrap_or(1);

    let work_dir = env::temp_dir().join(format!("consensus-meeting-vl-{}", Uuid::new_v4()));
    fs::create_dir_all(&work_dir).context("建立 VL 暫存目錄失敗")?;
    let output_prefix = work_dir.join("page");
    let dpi = env::var("AI_VL_DPI").unwrap_or_else(|_| "180".to_string());
    let output = Command::new("pdftoppm")
        .arg("-png")
        .arg("-r")
        .arg(&dpi)
        .arg(file_path)
        .arg(&output_prefix)
        .output()
        .context("執行 pdftoppm 產生 VL 頁面圖片失敗")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("pdftoppm 產生 VL 圖片失敗：{stderr}");
    }

    let mut png_paths = Vec::new();
    for entry in fs::read_dir(&work_dir).context("讀取 VL 圖片目錄失敗")? {
        let entry = entry.context("讀取 VL 圖片失敗")?;
        let path = entry.path();
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_default()
            == "png"
        {
            png_paths.push(path);
        }
    }
    png_paths.sort();
    png_paths.truncate(max_pages);

    let mut batches = Vec::new();
    for chunk in png_paths.chunks(batch_size) {
        let mut encoded = Vec::new();
        for path in chunk {
            let bytes =
                fs::read(path).with_context(|| format!("讀取 VL 圖片失敗：{}", path.display()))?;
            encoded.push(BASE64_STD.encode(bytes));
            let _ = fs::remove_file(path);
        }
        batches.push(encoded);
    }

    let _ = fs::remove_dir_all(&work_dir);
    Ok(batches)
}

async fn request_ai_content_with_images(
    api_url: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    images: &[String],
) -> Result<String> {
    if images.is_empty() {
        return request_ai_content(api_url, true, model, system_prompt, user_prompt).await;
    }

    let client = Client::new();
    let body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system_prompt },
            {
                "role": "user",
                "content": user_prompt,
                "images": images
            }
        ],
        "stream": false,
        "options": {
            "temperature": 0.0,
            "num_ctx": env::var("AI_NUM_CTX").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(32768),
            "num_predict": env::var("AI_NUM_PREDICT").ok().and_then(|v| v.parse::<i32>().ok()).unwrap_or(16384)
        }
    });

    let response = client
        .post(api_url)
        .json(&body)
        .send()
        .await
        .context("呼叫 Ollama VL 模型失敗")?;
    let status = response.status();
    let payload: Value = response.json().await.context("解析 Ollama VL 回應失敗")?;
    if !status.is_success() {
        bail!("AI VL API 回應錯誤 ({status}): {payload}");
    }

    extract_ai_content(&payload)
}

async fn request_ai_content(
    api_url: &str,
    is_ollama: bool,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String> {
    let client = Client::new();
    let response = if is_ollama {
        let num_ctx = env::var("AI_NUM_CTX")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(32768);
        let num_predict = env::var("AI_NUM_PREDICT")
            .ok()
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(16384);
        let body = serde_json::json!({
            "model": model,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": user_prompt }
            ],
            "format": "json",
            "options": {
                "temperature": 0.0,
                "num_ctx": num_ctx,
                "num_predict": num_predict
            },
            "stream": false
        });
        client
            .post(api_url)
            .json(&body)
            .send()
            .await
            .context("呼叫 Ollama API 失敗")?
    } else {
        let api_key = env::var("AI_API_KEY")
            .or_else(|_| env::var("OPENAI_API_KEY"))
            .context("非 Ollama 模式請設定 AI_API_KEY 或 OPENAI_API_KEY")?;
        let body = serde_json::json!({
            "model": model,
            "temperature": 0.0,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": user_prompt }
            ]
        });
        client
            .post(api_url)
            .bearer_auth(api_key)
            .json(&body)
            .send()
            .await
            .context("呼叫 AI API 失敗")?
    };

    let status = response.status();
    let payload: Value = response.json().await.context("解析 AI API 回應失敗")?;
    if !status.is_success() {
        bail!("AI API 回應錯誤 ({status}): {payload}");
    }

    extract_ai_content(&payload)
}

fn parse_ai_payload(
    cleaned: &str,
    fallback_meeting: Option<&AiMeeting>,
) -> Result<AiMeetingPayload> {
    let candidates = [
        cleaned.to_string(),
        extract_json_object(cleaned).unwrap_or_default(),
    ];

    for candidate in candidates {
        if candidate.trim().is_empty() {
            continue;
        }

        if let Ok(value) = serde_json::from_str::<Value>(&candidate) {
            if let Some(parsed) = parse_ai_payload_from_value(&value, fallback_meeting) {
                return Ok(parsed);
            }
        }

        if let Ok(parsed) = serde_json::from_str::<AiMeetingPayload>(&candidate) {
            return Ok(parsed);
        }

        if let Ok(partial) = serde_json::from_str::<PartialAiMeetingPayload>(&candidate) {
            let meeting = partial
                .meeting
                .or_else(|| fallback_meeting.cloned())
                .unwrap_or_else(empty_meeting);
            return Ok(AiMeetingPayload {
                meeting,
                promotions: partial.promotions,
                agenda_resolution_pairs: partial.agenda_resolution_pairs,
            });
        }

        if let Ok(v2) = serde_json::from_str::<AiMeetingPayloadV2>(&candidate) {
            return Ok(map_v2_payload(v2));
        }

        if let Ok(partial_v2) = serde_json::from_str::<PartialAiMeetingPayloadV2>(&candidate) {
            let meeting = partial_v2
                .meeting
                .map(map_v2_meeting)
                .or_else(|| fallback_meeting.cloned())
                .unwrap_or_else(empty_meeting);
            return Ok(AiMeetingPayload {
                meeting,
                promotions: partial_v2.meeting_promotions,
                agenda_resolution_pairs: partial_v2
                    .items
                    .into_iter()
                    .map(map_v2_item_to_pair)
                    .collect(),
            });
        }
    }

    bail!("AI 回傳格式非預期 JSON，請調整文件內容重試")
}

fn parse_ai_payload_from_value(
    root: &Value,
    fallback_meeting: Option<&AiMeeting>,
) -> Option<AiMeetingPayload> {
    let obj = root.as_object()?;

    let meeting = if let Some(meeting_val) = obj.get("meeting").and_then(Value::as_object) {
        parse_meeting_from_map(meeting_val)
    } else if obj.contains_key("meeting_title")
        || obj.contains_key("chairperson")
        || obj.contains_key("meeting_date")
    {
        parse_meeting_from_map(obj)
    } else {
        fallback_meeting.cloned().unwrap_or_else(empty_meeting)
    };

    let mut raw_promotions = Vec::new();
    for key in ["meeting_promotions", "promotions"] {
        if let Some(arr) = obj.get(key).and_then(Value::as_array) {
            for (idx, item) in arr.iter().enumerate() {
                if let Some(promo) = parse_promotion_from_value(item, idx) {
                    raw_promotions.push(promo);
                }
            }
        }
    }
    let promotions = split_and_clean_promotions(raw_promotions);

    let mut agenda_resolution_pairs = Vec::new();
    for key in [
        "agenda_resolution_pairs",
        "items",
        "agenda_items",
        "proposals",
    ] {
        if let Some(arr) = obj.get(key).and_then(Value::as_array) {
            for (idx, item) in arr.iter().enumerate() {
                if let Some(pair) = parse_pair_from_value(item, idx) {
                    agenda_resolution_pairs.push(pair);
                }
            }
        }
    }
    for (idx, pair) in agenda_resolution_pairs.iter_mut().enumerate() {
        pair.item_no = Some((idx + 1).to_string());
    }

    Some(AiMeetingPayload {
        meeting,
        promotions,
        agenda_resolution_pairs,
    })
}

fn split_and_clean_promotions(items: Vec<AiPromotion>) -> Vec<AiPromotion> {
    let mut split_items = Vec::new();

    for item in items {
        let cleaned = strip_promotion_header(&item.content);
        if cleaned.is_empty() {
            continue;
        }

        let parts = split_numbered_promotion_blocks(&cleaned);
        if parts.len() > 1 {
            for part in parts {
                let text = format_multiline_numbered_text(&strip_leading_top_number(&part));
                if !text.is_empty() {
                    split_items.push(AiPromotion {
                        promotion_no: None,
                        content: text,
                        notes: item.notes.clone(),
                    });
                }
            }
        } else {
            let text = format_multiline_numbered_text(&strip_leading_top_number(&cleaned));
            if !text.is_empty() {
                split_items.push(AiPromotion {
                    promotion_no: item.promotion_no,
                    content: text,
                    notes: item.notes,
                });
            }
        }
    }

    for (idx, promo) in split_items.iter_mut().enumerate() {
        promo.promotion_no = Some((idx + 1) as i32);
    }

    split_items
}

fn strip_promotion_header(input: &str) -> String {
    let mut lines = input.trim().lines().peekable();
    while let Some(first) = lines.peek() {
        let trimmed = first
            .trim()
            .trim_matches(|c: char| matches!(c, '#' | '*' | ' ' | ':' | '：'));
        if matches!(trimmed, "宣導事項" | "宣告事項" | "宣布事項" | "報告事項") {
            lines.next();
        } else {
            break;
        }
    }
    lines.collect::<Vec<_>>().join("\n").trim().to_string()
}

fn split_numbered_promotion_blocks(input: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current = Vec::new();
    let mut expected_num = 1usize;

    for line in input.lines() {
        if let Some(num) = detect_top_level_item_number(line) {
            if num == expected_num || (expected_num > 1 && num >= expected_num) {
                if !current.is_empty() {
                    blocks.push(current.join("\n").trim().to_string());
                    current.clear();
                }
                expected_num = num + 1;
            }
        }
        current.push(line);
    }

    if !current.is_empty() {
        blocks.push(current.join("\n").trim().to_string());
    }

    if blocks.len() >= 2 {
        blocks
    } else {
        vec![input.trim().to_string()]
    }
}

fn detect_top_level_item_number(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    if trimmed.is_empty() {
        return None;
    }

    let digit_count = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
    if digit_count > 0 {
        let num = trimmed[..digit_count].parse::<usize>().ok()?;
        if num == 0 || num > 50 {
            return None;
        }
        let rest = &trimmed[digit_count..];
        if let Some(after_dot) = rest.strip_prefix('.') {
            if after_dot.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                return None;
            }
            return Some(num);
        }
        if rest.starts_with('、')
            || rest.starts_with(' ')
            || rest.starts_with(')')
            || rest.starts_with('）')
        {
            return Some(num);
        }
    }

    let cn_prefixes = [
        ("一、", 1),
        ("二、", 2),
        ("三、", 3),
        ("四、", 4),
        ("五、", 5),
        ("六、", 6),
        ("七、", 7),
        ("八、", 8),
        ("九、", 9),
        ("十、", 10),
    ];
    for (prefix, num) in cn_prefixes {
        if trimmed.starts_with(prefix) {
            return Some(num);
        }
    }

    None
}

fn strip_leading_top_number(input: &str) -> String {
    let trimmed = input.trim();
    let Some(first_line) = trimmed.lines().next() else {
        return String::new();
    };
    if detect_top_level_item_number(first_line).is_none() {
        return trimmed.to_string();
    }

    for prefix in [
        "一、", "二、", "三、", "四、", "五、", "六、", "七、", "八、", "九、", "十、",
    ] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            return rest.trim().to_string();
        }
    }

    let digit_count = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
    if digit_count > 0 {
        let rest = &trimmed[digit_count..];
        let rest = rest
            .strip_prefix('.')
            .or_else(|| rest.strip_prefix('、'))
            .or_else(|| rest.strip_prefix(')'))
            .or_else(|| rest.strip_prefix('）'))
            .unwrap_or(rest);
        return rest.trim().to_string();
    }

    trimmed.to_string()
}

fn parse_meeting_from_map(map: &serde_json::Map<String, Value>) -> AiMeeting {
    let attendees = match map.get("attendees") {
        Some(Value::Array(arr)) => {
            let list: Vec<String> = arr.iter().filter_map(value_to_non_empty_string).collect();
            if list.is_empty() { None } else { Some(list) }
        }
        Some(Value::String(s)) => normalize_attendees(Some(s.clone())),
        _ => None,
    };

    AiMeeting {
        meeting_no: get_non_empty_string(map, "meeting_no"),
        meeting_title: get_non_empty_string(map, "meeting_title"),
        title: get_non_empty_string(map, "title"),
        summary: get_non_empty_string(map, "summary"),
        meeting_date: get_non_empty_string(map, "meeting_date"),
        meeting_time: get_non_empty_string(map, "meeting_time"),
        chairperson: get_non_empty_string(map, "chairperson"),
        attendees,
        meeting_location: get_non_empty_string(map, "meeting_location"),
        recorder: get_non_empty_string(map, "recorder"),
        published_at: get_non_empty_string(map, "published_at"),
    }
}

fn parse_promotion_from_value(val: &Value, idx: usize) -> Option<AiPromotion> {
    match val {
        Value::String(s) if !s.trim().is_empty() => Some(AiPromotion {
            promotion_no: Some((idx + 1) as i32),
            content: s.trim().to_string(),
            notes: None,
        }),
        Value::Object(map) => {
            let content = get_non_empty_string(map, "content")
                .or_else(|| get_non_empty_string(map, "text"))
                .or_else(|| get_non_empty_string(map, "title"))?;
            let promotion_no = match map.get("promotion_no") {
                Some(Value::Number(n)) => n.as_i64().map(|v| v as i32),
                Some(Value::String(s)) => s.trim().parse::<i32>().ok(),
                _ => Some((idx + 1) as i32),
            };
            let notes =
                get_non_empty_string(map, "notes").or_else(|| get_non_empty_string(map, "note"));
            Some(AiPromotion {
                promotion_no,
                content,
                notes,
            })
        }
        _ => None,
    }
}

fn parse_pair_from_value(val: &Value, idx: usize) -> Option<AiAgendaResolutionPair> {
    let map = val.as_object()?;

    let item_no = Some((idx + 1).to_string());

    let nested_res = map.get("resolution").and_then(Value::as_object);

    let raw_res_title = nested_res
        .and_then(|res| get_non_empty_string(res, "title"))
        .or_else(|| get_non_empty_string(map, "resolution_title"))
        .or_else(|| get_non_empty_string(map, "title"));

    let raw_res_content = nested_res
        .and_then(|res| get_non_empty_string(res, "content"))
        .or_else(|| get_non_empty_string(map, "resolution_content"))
        .or_else(|| get_non_empty_string(map, "content"))
        .or_else(|| get_non_empty_string(map, "description"));

    let raw_res_text = nested_res
        .and_then(|res| {
            get_non_empty_string(res, "resolution").or_else(|| get_non_empty_string(res, "text"))
        })
        .or_else(|| get_non_empty_string(map, "resolution_text"))
        .or_else(|| {
            map.get("resolution")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)
        });

    let resolution_result = nested_res
        .and_then(|res| {
            get_non_empty_string(res, "result").or_else(|| get_non_empty_string(res, "status"))
        })
        .or_else(|| get_non_empty_string(map, "resolution_result"))
        .or_else(|| get_non_empty_string(map, "result"));

    let raw_agenda = get_non_empty_string(map, "agenda_item")
        .or_else(|| get_non_empty_string(map, "agenda"))
        .or_else(|| raw_res_title.clone())
        .unwrap_or_else(|| format!("議題 {}", idx + 1));

    let mut agenda_item = clean_agenda_item_text(&raw_agenda);

    let mut decision_text = match (&raw_res_content, &raw_res_text) {
        (Some(content), Some(res_text)) => {
            let norm_content = normalize_for_compare(content);
            let norm_res = normalize_for_compare(res_text);
            let stripped_content = strip_resolution_prefix(content);
            let stripped_res = strip_resolution_prefix(res_text);

            if norm_content == norm_res {
                Some(stripped_res)
            } else if norm_content.contains(&norm_res)
                || (stripped_content.starts_with("1.") && stripped_res.starts_with("1."))
            {
                if stripped_content.chars().count() >= stripped_res.chars().count() {
                    Some(stripped_content)
                } else {
                    Some(stripped_res)
                }
            } else if norm_res.contains(&norm_content) {
                Some(stripped_res)
            } else {
                if !normalize_for_compare(&agenda_item).contains(&norm_content) {
                    let proposer = extract_proposer_prefix(&agenda_item);
                    if let Some(p) = proposer {
                        agenda_item = format!("{p}：{}", content.trim());
                    } else {
                        agenda_item = format!("{}：{}", agenda_item.trim(), content.trim());
                    }
                }
                Some(stripped_res)
            }
        }
        (None, Some(res_text)) => Some(strip_resolution_prefix(res_text)),
        (Some(content), None) => Some(strip_resolution_prefix(content)),
        (None, None) => None,
    };

    let extra_notes = get_non_empty_string(map, "agenda_notes")
        .or_else(|| get_non_empty_string(map, "notes"))
        .or_else(|| nested_res.and_then(|res| get_non_empty_string(res, "notes")));

    if let Some(note) = &extra_notes {
        let norm_note = normalize_for_compare(note);
        let in_agenda = normalize_for_compare(&agenda_item).contains(&norm_note);
        let in_decision = decision_text
            .as_ref()
            .map(|d| normalize_for_compare(d).contains(&norm_note))
            .unwrap_or(false);
        if !in_agenda && !in_decision {
            if let Some(existing) = &mut decision_text {
                existing.push('\n');
                existing.push_str(note.trim());
            } else {
                decision_text = Some(note.trim().to_string());
            }
        }
    }

    agenda_item = format_multiline_numbered_text(&agenda_item);
    decision_text = decision_text.map(|d| format_multiline_numbered_text(&d));

    if agenda_item.trim().is_empty() && decision_text.is_none() {
        return None;
    }

    Some(AiAgendaResolutionPair {
        item_no,
        agenda_item,
        agenda_notes: None,
        resolution_title: None,
        resolution_content: decision_text.clone(),
        resolution_text: decision_text,
        resolution_result,
    })
}

fn format_multiline_numbered_text(input: &str) -> String {
    let mut s = unwrap_hard_wrapped_lines(input.trim());
    let mut paren_patterns: Vec<String> = Vec::new();
    for n in 1..=15 {
        paren_patterns.push(format!("({n})"));
        paren_patterns.push(format!("（{n}）"));
    }
    for cn in ["一", "二", "三", "四", "五", "六", "七", "八", "九", "十"] {
        paren_patterns.push(format!("({cn})"));
        paren_patterns.push(format!("（{cn}）"));
    }
    for pat in paren_patterns {
        let mut search_from = 0;
        while let Some(rel_idx) = s[search_from..].find(&pat) {
            let idx = search_from + rel_idx;
            if idx > 0 {
                let prev_char = s[..idx].chars().next_back().unwrap_or('\n');
                if prev_char != '\n' {
                    let trimmed_before = s[..idx].trim_end_matches([' ', '\t']);
                    let prefix_len = trimmed_before.len();
                    let after = s[idx..].to_string();
                    s = format!("{}\n{}", &s[..prefix_len], after);
                    search_from = prefix_len + 1 + pat.len();
                    continue;
                }
            }
            search_from = idx + pat.len();
        }
    }
    for n in 2..=15 {
        let pat = format!("{n}.");
        let mut search_from = 0;
        while let Some(rel_idx) = s[search_from..].find(&pat) {
            let idx = search_from + rel_idx;
            if idx > 0 {
                let before = &s[..idx];
                let prev_char = before.chars().next_back().unwrap_or('\n');
                let trimmed_before = before.trim_end_matches([' ', '\t']);
                let before_non_ws = trimmed_before.chars().next_back().unwrap_or('\0');
                if prev_char != '\n'
                    && matches!(before_non_ws, '。' | '；' | ';' | '？' | '?' | ')' | '）')
                {
                    let prefix_len = trimmed_before.len();
                    let after = s[idx..].to_string();
                    s = format!("{}\n{}", &s[..prefix_len], after);
                    search_from = prefix_len + 1 + pat.len();
                    continue;
                }
            }
            search_from = idx + pat.len();
        }
    }
    s
}

fn unwrap_hard_wrapped_lines(input: &str) -> String {
    let mut result_lines: Vec<String> = Vec::new();

    for raw_line in input.lines() {
        let line = normalize_line_spacing(raw_line);
        if line.is_empty() {
            continue;
        }

        if let Some(prev) = result_lines.last_mut() {
            if should_keep_line_break(prev, &line) {
                result_lines.push(line);
            } else {
                append_wrapped_line(prev, &line);
            }
        } else {
            result_lines.push(line);
        }
    }

    result_lines.join("\n")
}

fn normalize_line_spacing(raw_line: &str) -> String {
    let trimmed = raw_line.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let digit_count = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
    let normalized_source = if digit_count > 0 && digit_count <= 2 {
        let rest = &trimmed[digit_count..];
        let space_count = rest.chars().take_while(|c| matches!(c, ' ' | '\t')).count();
        if space_count >= 2 {
            format!("{}. {}", &trimmed[..digit_count], rest.trim_start())
        } else {
            trimmed.to_string()
        }
    } else {
        trimmed.to_string()
    };

    let mut out = String::with_capacity(normalized_source.len());
    let mut prev_space = false;
    for ch in normalized_source.chars() {
        if matches!(ch, ' ' | '\t') {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out
}

fn should_keep_line_break(prev: &str, curr: &str) -> bool {
    if prev.starts_with("--- [Page ") || curr.starts_with("--- [Page ") {
        return true;
    }
    if is_metadata_or_section_header(prev) || is_metadata_or_section_header(curr) {
        return true;
    }
    if is_structural_line_start(curr) {
        return true;
    }
    if (curr.starts_with("上述") || curr.starts_with("以上")) && is_structural_line_start(prev)
    {
        return true;
    }
    let prev_trimmed = prev.trim_end();
    if prev_trimmed.ends_with('：') || prev_trimmed.ends_with(':') {
        return true;
    }
    false
}

fn is_metadata_or_section_header(line: &str) -> bool {
    let trimmed = line.trim();
    for prefix in [
        "開會時間",
        "開會地點",
        "主持人",
        "出席人員",
        "記錄聯絡人",
        "紀錄聯絡人",
        "宣告事項",
        "宣導事項",
        "宣布事項",
        "報告事項",
        "討論事項",
        "臨時動議",
        "下次會議",
        "散會",
    ] {
        if trimmed.starts_with(prefix) {
            return true;
        }
    }
    if trimmed.ends_with("討論議題：")
        || trimmed.ends_with("討論議題:")
        || trimmed.ends_with("會議記錄")
        || trimmed.ends_with("會議紀錄")
    {
        return true;
    }
    false
}

fn is_structural_line_start(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }

    for prefix in [
        "一、",
        "二、",
        "三、",
        "四、",
        "五、",
        "六、",
        "七、",
        "八、",
        "九、",
        "十、",
        "十一、",
        "十二、",
        "十三、",
        "十四、",
        "十五、",
        "十六、",
        "壹、",
        "貳、",
        "參、",
        "肆、",
        "伍、",
        "陸、",
        "柒、",
        "(一)",
        "(二)",
        "(三)",
        "(四)",
        "(五)",
        "(六)",
        "(七)",
        "(八)",
        "(九)",
        "(十)",
        "（一）",
        "（二）",
        "（三）",
        "（四）",
        "（五）",
        "（六）",
        "（七）",
        "（八）",
        "（九）",
        "（十）",
        "- ",
        "* ",
        "• ",
        "決議:",
        "決議：",
        "結論:",
        "結論：",
        "說明:",
        "說明：",
        "備註:",
        "備註：",
        "註:",
        "註：",
        "For example",
        "例如:",
        "例如：",
    ] {
        if trimmed.starts_with(prefix) {
            return true;
        }
    }

    for n in 1..=20 {
        if trimmed.starts_with(&format!("({n})")) || trimmed.starts_with(&format!("（{n}）")) {
            return true;
        }
    }

    if let Some(rest) = trimmed
        .strip_prefix('(')
        .or_else(|| trimmed.strip_prefix('（'))
    {
        let mut chars = rest.chars();
        if let (Some(c1), Some(c2)) = (chars.next(), chars.next()) {
            if c1.is_ascii_alphabetic() && matches!(c2, ')' | '）') {
                return true;
            }
        }
    }

    if is_numbered_item_start(trimmed) {
        return true;
    }

    let head: String = trimmed.chars().take(30).collect();
    if head.contains("提案：") || head.contains("提案:") || head.ends_with("提案") {
        return true;
    }

    false
}

fn is_numbered_item_start(trimmed: &str) -> bool {
    let digit_count = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
    if digit_count == 0 || digit_count > 2 {
        return false;
    }
    let Ok(num) = trimmed[..digit_count].parse::<usize>() else {
        return false;
    };
    if num == 0 || num > 50 {
        return false;
    }
    let rest = &trimmed[digit_count..];
    if rest.starts_with('、') || rest.starts_with(')') || rest.starts_with('）') {
        return true;
    }
    if let Some(after_dot) = rest.strip_prefix('.') {
        let sub_digits = after_dot.chars().take_while(|c| c.is_ascii_digit()).count();
        if sub_digits == 0 {
            return true;
        }
        if sub_digits <= 2 {
            let after_sub = &after_dot[sub_digits..];
            if !after_sub.starts_with('.')
                && !after_sub
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == '-')
            {
                return true;
            }
        }
    }
    false
}

fn append_wrapped_line(prev: &mut String, curr: &str) {
    let prev_trimmed = prev.trim_end();
    let curr_trimmed = curr.trim_start();
    if curr_trimmed.is_empty() {
        return;
    }
    let last_char = prev_trimmed.chars().next_back().unwrap_or('\0');
    let first_char = curr_trimmed.chars().next().unwrap_or('\0');

    let needs_space = (last_char.is_ascii_alphanumeric()
        || matches!(last_char, ',' | '.' | ';' | ')' | '%' | '+' | '-'))
        && (first_char.is_ascii_alphanumeric() || matches!(first_char, '(' | '+' | '-'));

    prev.truncate(prev_trimmed.len());
    if needs_space {
        prev.push(' ');
    }
    prev.push_str(curr_trimmed);
}

fn backfill_truncated_resolutions(document: &str, payload: &mut AiMeetingPayload) {
    let doc_lines: Vec<String> = document
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !(l.starts_with("--- [Page ") && l.ends_with("] ---")))
        .map(ToOwned::to_owned)
        .collect();

    if doc_lines.is_empty() {
        return;
    }

    for idx in 0..payload.agenda_resolution_pairs.len() {
        let Some(current_res) = payload.agenda_resolution_pairs[idx]
            .resolution_text
            .clone()
            .or_else(|| {
                payload.agenda_resolution_pairs[idx]
                    .resolution_content
                    .clone()
            })
        else {
            continue;
        };

        let norm_res = normalize_for_compare(&current_res);
        if norm_res.is_empty() {
            continue;
        }

        let res_lines: Vec<&str> = current_res
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        let Some(last_res_line) = res_lines.last() else {
            continue;
        };
        let norm_last_res_line = normalize_for_compare(last_res_line);
        if norm_last_res_line.chars().count() < 4 {
            continue;
        }

        let matched_doc_idx = doc_lines.iter().position(|dl| {
            let norm_dl = normalize_for_compare(dl);
            !norm_dl.is_empty()
                && (norm_dl.contains(&norm_last_res_line) || norm_last_res_line.contains(&norm_dl))
        });

        let Some(mut scan_idx) = matched_doc_idx else {
            continue;
        };

        let next_agenda_norm = payload
            .agenda_resolution_pairs
            .get(idx + 1)
            .map(|p| normalize_for_compare(&p.agenda_item))
            .unwrap_or_default();

        let mut appended_lines: Vec<String> = Vec::new();
        while scan_idx + 1 < doc_lines.len() {
            scan_idx += 1;
            let candidate = &doc_lines[scan_idx];
            let norm_cand = normalize_for_compare(candidate);
            if norm_cand.is_empty() {
                continue;
            }
            if is_new_section_or_agenda_line(candidate, &norm_cand, &next_agenda_norm) {
                break;
            }
            if !norm_res.contains(&norm_cand) {
                appended_lines.push(candidate.clone());
            }
        }

        if !appended_lines.is_empty() {
            let mut updated = current_res;
            for line in appended_lines {
                updated.push('\n');
                updated.push_str(&line);
            }
            let formatted = format_multiline_numbered_text(&updated);
            payload.agenda_resolution_pairs[idx].resolution_content = Some(formatted.clone());
            payload.agenda_resolution_pairs[idx].resolution_text = Some(formatted);
        }
    }
}

fn is_new_section_or_agenda_line(line: &str, norm_line: &str, next_agenda_norm: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.contains("提案：")
        || trimmed.contains("提案:")
        || trimmed.ends_with("提案")
        || trimmed.contains("討論議題")
        || trimmed.starts_with("下次會議")
        || trimmed.starts_with("散會")
        || trimmed.starts_with("臨時動議")
    {
        return true;
    }
    for prefix in [
        "一、",
        "二、",
        "三、",
        "四、",
        "五、",
        "六、",
        "七、",
        "八、",
        "九、",
        "十、",
        "十一、",
        "十二、",
    ] {
        if trimmed.starts_with(prefix) {
            return true;
        }
    }
    if !next_agenda_norm.is_empty() && norm_line.chars().count() >= 5 {
        let prefix: String = next_agenda_norm.chars().take(8).collect();
        if !prefix.is_empty() && norm_line.contains(&prefix) {
            return true;
        }
    }
    false
}

fn clean_agenda_item_text(input: &str) -> String {
    let mut s = input.trim().to_string();

    for cat in [
        "**資訊安規**",
        "**家電安規**",
        "**電磁相容**",
        "**EMC**",
        "資訊安規討論議題",
        "家電安規討論議題",
        "電磁相容討論議題",
        "資訊安規",
        "家電安規",
        "電磁相容",
    ] {
        if let Some(rest) = s.strip_prefix(cat) {
            let trimmed_rest = rest.trim_start();
            if let Some(after_colon) = trimmed_rest
                .strip_prefix(':')
                .or_else(|| trimmed_rest.strip_prefix('：'))
                .or_else(|| trimmed_rest.strip_prefix('-'))
            {
                s = after_colon.trim().to_string();
                break;
            }
        }
    }

    s = strip_leading_top_number(&s);
    s
}

fn extract_proposer_prefix(agenda_item: &str) -> Option<String> {
    let trimmed = agenda_item.trim();
    if let Some(idx) = trimmed.find("提案") {
        let end = idx + "提案".len();
        let prefix = trimmed[..end].trim();
        if !prefix.is_empty() && prefix.chars().count() <= 30 {
            return Some(prefix.to_string());
        }
    }
    None
}

fn strip_resolution_prefix(input: &str) -> String {
    let trimmed = input.trim();
    for prefix in ["決議：", "決議:", "結論：", "結論:"] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            return rest.trim().to_string();
        }
    }
    trimmed.to_string()
}

fn get_non_empty_string(map: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    map.get(key).and_then(value_to_non_empty_string)
}

fn value_to_non_empty_string(val: &Value) -> Option<String> {
    match val {
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("null") {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn map_v2_payload(input: AiMeetingPayloadV2) -> AiMeetingPayload {
    AiMeetingPayload {
        meeting: map_v2_meeting(input.meeting),
        promotions: input.meeting_promotions,
        agenda_resolution_pairs: input.items.into_iter().map(map_v2_item_to_pair).collect(),
    }
}

fn map_v2_meeting(input: AiMeetingV2) -> AiMeeting {
    AiMeeting {
        meeting_no: input.meeting_no,
        meeting_title: input.meeting_title,
        title: input.title,
        summary: input.summary,
        meeting_date: input.meeting_date,
        meeting_time: input.meeting_time,
        chairperson: input.chairperson,
        attendees: normalize_attendees(input.attendees),
        meeting_location: input.meeting_location,
        recorder: input.recorder,
        published_at: input.published_at,
    }
}

fn map_v2_item_to_pair(item: AiItemV2) -> AiAgendaResolutionPair {
    AiAgendaResolutionPair {
        item_no: item.item_no,
        agenda_item: item.agenda_item,
        agenda_notes: item.agenda_notes,
        resolution_title: item.resolution.title,
        resolution_content: item.resolution.content,
        resolution_text: item.resolution.resolution,
        resolution_result: item.resolution.result,
    }
}

fn normalize_attendees(raw: Option<String>) -> Option<Vec<String>> {
    let raw = raw?;
    let attendees = raw
        .split(['\n', '、', ';', '；', ','])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if attendees.is_empty() {
        None
    } else {
        Some(attendees)
    }
}

fn extract_json_object(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut depth = 0usize;
    let mut start_idx: Option<usize> = None;
    let mut in_string = false;
    let mut escaped = false;

    for (idx, byte) in bytes.iter().enumerate() {
        let ch = *byte as char;
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == '"' {
                in_string = false;
            }
            continue;
        }

        if ch == '"' {
            in_string = true;
            continue;
        }
        if ch == '{' {
            if depth == 0 {
                start_idx = Some(idx);
            }
            depth += 1;
            continue;
        }
        if ch == '}' && depth > 0 {
            depth -= 1;
            if depth == 0 {
                if let Some(start) = start_idx {
                    return Some(input[start..=idx].to_string());
                }
            }
        }
    }
    None
}

fn empty_meeting() -> AiMeeting {
    AiMeeting {
        meeting_no: None,
        meeting_title: None,
        title: None,
        summary: None,
        meeting_date: None,
        meeting_time: None,
        chairperson: None,
        attendees: None,
        meeting_location: None,
        recorder: None,
        published_at: None,
    }
}

fn extract_ai_content(payload: &Value) -> Result<String> {
    if let Some(text) = payload
        .get("message")
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
    {
        return Ok(text.to_string());
    }

    if let Some(content) = payload
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
    {
        if let Some(text) = content.as_str() {
            return Ok(text.to_string());
        }
    }
    bail!("無法從 AI 回應中取得 content 欄位")
}

fn strip_markdown_fence(input: &str) -> String {
    let trimmed = input.trim();
    if !trimmed.starts_with("```") {
        return trimmed.to_string();
    }

    let mut lines = trimmed.lines();
    let _ = lines.next();
    let mut body = lines.collect::<Vec<_>>().join("\n");
    if body.ends_with("```") {
        body.truncate(body.len().saturating_sub(3));
    }
    body.trim().to_string()
}

fn deduplicate_ai_payload(payload: &mut AiMeetingPayload) {
    payload.promotions = deduplicate_promotions(&payload.promotions);
    payload.agenda_resolution_pairs =
        deduplicate_agenda_resolution_pairs(&payload.agenda_resolution_pairs);
}

fn deduplicate_promotions(items: &[AiPromotion]) -> Vec<AiPromotion> {
    let mut seen = HashSet::new();
    let mut unique = Vec::new();

    for item in items {
        if seen.insert(item.clone()) {
            unique.push(item.clone());
        }
    }

    unique
}

fn deduplicate_agenda_resolution_pairs(
    items: &[AiAgendaResolutionPair],
) -> Vec<AiAgendaResolutionPair> {
    let mut seen = HashSet::new();
    let mut unique = Vec::new();

    for item in items {
        if seen.insert(item.clone()) {
            unique.push(item.clone());
        }
    }

    unique
}

/// 回傳原始文件的（檔名, 絕對路徑）。
fn resolve_source_document(file: Option<&str>) -> Option<(String, String)> {
    let path = Path::new(file?);
    let absolute = fs::canonicalize(path).ok()?;
    let file_name = absolute.file_name()?.to_string_lossy().to_string();
    Some((file_name, absolute.to_string_lossy().to_string()))
}

async fn persist_to_database(payload: AiMeetingPayload, source_file: Option<&str>) -> Result<()> {
    let source_document = resolve_source_document(source_file);
    let config = AppConfig::from_env();
    let database_url = config
        .database_url
        .ok_or_else(|| anyhow!("DATABASE_URL is not set"))?;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .context("連線資料庫失敗")?;

    sqlx::migrate!("./src/databases/migrations")
        .run(&pool)
        .await
        .context("執行資料庫 migration 失敗")?;

    let mut tx = pool.begin().await.context("開啟交易失敗")?;

    let now = Utc::now();
    let meeting_id = Uuid::new_v4();
    let meeting_no = payload
        .meeting
        .meeting_no
        .clone()
        .unwrap_or_else(|| format!("AI-{}", now.format("%Y%m%d%H%M%S")));
    let meeting_title = payload.meeting.meeting_title.clone();
    let title = payload
        .meeting
        .title
        .clone()
        .or_else(|| payload.meeting.meeting_title.clone())
        .unwrap_or_else(|| "AI 匯入會議".to_string());
    let summary = payload.meeting.summary.clone();
    let meeting_date = parse_date_opt(payload.meeting.meeting_date.as_deref())?;
    let meeting_time = parse_datetime_opt(payload.meeting.meeting_time.as_deref())?;
    let published_at = parse_datetime_opt(payload.meeting.published_at.as_deref())?;
    let attendees = payload
        .meeting
        .attendees
        .as_ref()
        .map(|list| list.join("、"));
    let chairperson = payload
        .meeting
        .chairperson
        .as_deref()
        .map(|s| truncate_chars(s, 100));
    let recorder = payload
        .meeting
        .recorder
        .as_deref()
        .map(|s| truncate_chars(s, 100));

    tx.execute(
        sqlx::query(
            r#"
            INSERT INTO meetings
                (id, meeting_no, meeting_time, meeting_title, chairperson, attendees, meeting_location, recorder, title, summary, meeting_date, published_at, source_file_name, source_file_path, created_at, updated_at)
            VALUES
                ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, NOW(), NOW())
            "#,
        )
        .bind(meeting_id)
        .bind(meeting_no)
        .bind(meeting_time)
        .bind(meeting_title)
        .bind(chairperson)
        .bind(attendees)
        .bind(payload.meeting.meeting_location.clone())
        .bind(recorder)
        .bind(title)
        .bind(summary)
        .bind(meeting_date)
        .bind(published_at)
        .bind(source_document.as_ref().map(|(name, _)| name.clone()))
        .bind(source_document.as_ref().map(|(_, path)| path.clone())),
    )
    .await
    .context("寫入 meetings 失敗")?;

    let mut used_promotion_nos = HashSet::new();
    for (index, promotion) in payload.promotions.iter().enumerate() {
        let mut promotion_no = promotion.promotion_no.unwrap_or((index + 1) as i32);
        if promotion_no <= 0 || !used_promotion_nos.insert(promotion_no) {
            let mut next_no = (index + 1) as i32;
            while !used_promotion_nos.insert(next_no) {
                next_no += 1;
            }
            promotion_no = next_no;
        }
        tx.execute(
            sqlx::query(
                r#"
                INSERT INTO meeting_promotions
                    (id, meeting_id, promotion_no, content, notes, created_at)
                VALUES
                    ($1, $2, $3, $4, $5, NOW())
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(meeting_id)
            .bind(promotion_no)
            .bind(&promotion.content)
            .bind(&promotion.notes),
        )
        .await
        .context("寫入 meeting_promotions 失敗")?;
    }

    for (index, pair) in payload.agenda_resolution_pairs.iter().enumerate() {
        let agenda_id = Uuid::new_v4();
        let resolution_id = Uuid::new_v4();
        let item_no = (index + 1).to_string();
        let agenda_item = clean_agenda_item_text(&pair.agenda_item);

        tx.execute(
            sqlx::query(
                r#"
                INSERT INTO agenda_items
                    (id, meeting_id, item_no, agenda_item, notes, created_at)
                VALUES
                    ($1, $2, $3, $4, $5, NOW())
                "#,
            )
            .bind(agenda_id)
            .bind(meeting_id)
            .bind(&item_no)
            .bind(&agenda_item)
            .bind(&pair.agenda_notes),
        )
        .await
        .context("寫入 agenda_items 失敗")?;

        let resolution_title = String::new();
        let resolution_body = pair
            .resolution_text
            .clone()
            .or_else(|| pair.resolution_content.clone())
            .or_else(|| pair.agenda_notes.clone())
            .unwrap_or_else(|| "待補充".to_string());
        let resolution_result = pair
            .resolution_result
            .clone()
            .unwrap_or_else(|| "待確認".to_string());

        tx.execute(
            sqlx::query(
                r#"
                INSERT INTO resolutions
                    (id, meeting_id, agenda_item_id, item_no, title, content, resolution, result, created_at)
                VALUES
                    ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
                "#,
            )
            .bind(resolution_id)
            .bind(meeting_id)
            .bind(agenda_id)
            .bind(&item_no)
            .bind(resolution_title)
            .bind(&resolution_body)
            .bind(Some(resolution_body.clone()))
            .bind(resolution_result),
        )
        .await
        .context("寫入 resolutions 失敗")?;
    }

    tx.commit().await.context("提交交易失敗")?;

    println!(
        "[ingest-meeting-ai] 寫入成功: meeting_id={}, promotions={}, agenda_resolution_pairs={}",
        meeting_id,
        payload.promotions.len(),
        payload.agenda_resolution_pairs.len()
    );
    Ok(())
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    input.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deduplicates_promotions_with_variations() {
        let mut payload = AiMeetingPayload {
            meeting: AiMeeting {
                meeting_no: None,
                meeting_title: None,
                title: None,
                summary: None,
                meeting_date: None,
                meeting_time: None,
                chairperson: None,
                attendees: None,
                meeting_location: None,
                recorder: None,
                published_at: None,
            },
            promotions: vec![
                AiPromotion {
                    promotion_no: Some(1),
                    content: "宣導事項 內容".to_string(),
                    notes: Some("備註".to_string()),
                },
                AiPromotion {
                    promotion_no: Some(1),
                    content: "宣導事項 內容".to_string(),
                    notes: Some("備註".to_string()),
                },
                AiPromotion {
                    promotion_no: Some(2),
                    content: "其他事項".to_string(),
                    notes: None,
                },
                AiPromotion {
                    promotion_no: Some(2),
                    content: "其他事項 ".to_string(),
                    notes: None,
                },
            ],
            agenda_resolution_pairs: vec![],
        };

        deduplicate_ai_payload(&mut payload);

        assert_eq!(payload.promotions.len(), 3);
    }

    #[test]
    fn deduplicates_agenda_resolution_pairs() {
        let mut payload = AiMeetingPayload {
            meeting: AiMeeting {
                meeting_no: None,
                meeting_title: None,
                title: None,
                summary: None,
                meeting_date: None,
                meeting_time: None,
                chairperson: None,
                attendees: None,
                meeting_location: None,
                recorder: None,
                published_at: None,
            },
            promotions: vec![],
            agenda_resolution_pairs: vec![
                AiAgendaResolutionPair {
                    item_no: Some("1".to_string()),
                    agenda_item: "審議事項".to_string(),
                    agenda_notes: None,
                    resolution_title: Some("決議".to_string()),
                    resolution_content: Some("通過".to_string()),
                    resolution_text: None,
                    resolution_result: Some("通過".to_string()),
                },
                AiAgendaResolutionPair {
                    item_no: Some("1".to_string()),
                    agenda_item: "審議事項".to_string(),
                    agenda_notes: None,
                    resolution_title: Some("決議".to_string()),
                    resolution_content: Some("通過".to_string()),
                    resolution_text: None,
                    resolution_result: Some("通過".to_string()),
                },
                AiAgendaResolutionPair {
                    item_no: Some("2".to_string()),
                    agenda_item: "另議題".to_string(),
                    agenda_notes: None,
                    resolution_title: Some("修正".to_string()),
                    resolution_content: Some("延後".to_string()),
                    resolution_text: None,
                    resolution_result: Some("修正".to_string()),
                },
                AiAgendaResolutionPair {
                    item_no: Some("2".to_string()),
                    agenda_item: "另議題".to_string(),
                    agenda_notes: None,
                    resolution_title: Some("修正".to_string()),
                    resolution_content: Some("延後 ".to_string()),
                    resolution_text: None,
                    resolution_result: Some("修正".to_string()),
                },
            ],
        };

        deduplicate_ai_payload(&mut payload);

        assert_eq!(payload.agenda_resolution_pairs.len(), 3);
    }

    #[test]
    fn parses_system_prompt_schema_correctly() {
        let raw_json = r#"{
          "meeting": {
            "meeting_no": "115-08",
            "meeting_date": "2026-08-15",
            "meeting_time": "2026-08-15T09:30:00+08:00",
            "meeting_title": "資訊與影音商品檢測技術一致性研討會會議紀錄",
            "title": "資訊與影音商品檢測技術一致性研討會",
            "chairperson": "林簡任技正良陽",
            "attendees": "產品部、研發部、詳如簽名冊",
            "meeting_location": "電氣檢驗科技大樓簡報室",
            "recorder": "廖金隆(02-86488058#6531)",
            "summary": "討論電動車充電設備與 EMC 報告數位化",
            "published_at": null
          },
          "meeting_promotions": [
            {
              "promotion_no": 1,
              "content": "宣告事項:\n1. 自 95.1.1 日起做隨產品檢驗之電源線組。\n2. 內裝式電源供應器需符合規定：\n- 2.1 危險能量測試\n- 2.2 電力限制型 LPS",
              "notes": null
            }
          ],
          "agenda_resolution_pairs": [
            {
              "item_no": "家電安規-一-1",
              "agenda_item": "**家電安規**：ETC 提案 (電器外殼的形狀和裝飾)",
              "agenda_notes": null,
              "resolution": {
                "title": "電器外殼的形狀和裝飾要求",
                "content": "CNS 3765 90.8.1 版 22.44 電器外殼的形狀和裝飾不得處理成類似小孩玩具，現行公告版本 88.8.27 並無此項，是否應要求?",
                "resolution": "決議：標準中若無此規定且不影響安全之情況下，將不要求。",
                "result": "不要求"
              }
            }
          ]
        }"#;

        let parsed = parse_ai_payload(raw_json, None).expect("should parse system_prompt schema");
        assert_eq!(parsed.promotions.len(), 2);
        assert_eq!(parsed.promotions[0].promotion_no, Some(1));
        assert_eq!(
            parsed.promotions[0].content,
            "自 95.1.1 日起做隨產品檢驗之電源線組。"
        );
        assert_eq!(parsed.promotions[1].promotion_no, Some(2));
        assert!(
            parsed.promotions[1]
                .content
                .starts_with("內裝式電源供應器需符合規定：")
        );

        assert_eq!(parsed.agenda_resolution_pairs.len(), 1);
        let pair = &parsed.agenda_resolution_pairs[0];
        assert_eq!(pair.item_no.as_deref(), Some("1"));
        assert_eq!(
            pair.agenda_item,
            "ETC 提案：CNS 3765 90.8.1 版 22.44 電器外殼的形狀和裝飾不得處理成類似小孩玩具，現行公告版本 88.8.27 並無此項，是否應要求?"
        );
        assert_eq!(pair.resolution_title, None);
        assert_eq!(
            pair.resolution_content.as_deref(),
            Some("標準中若無此規定且不影響安全之情況下，將不要求。")
        );
        assert_eq!(
            pair.resolution_text.as_deref(),
            Some("標準中若無此規定且不影響安全之情況下，將不要求。")
        );
        assert_eq!(pair.resolution_result.as_deref(), Some("不要求"));
    }

    #[test]
    fn backfills_truncated_resolution_and_formats_subitems() {
        let doc = r#"--- [Page 1] ---
一、律頻科技股份有限公司提案：
  Power Supply:
  (1)100V - 240V & 200V - 240V / 6 組output / PCB 單面/ 鐵 外殼不同有3款
  (2)100V - 240V & 200V - 240V / 8 組output / PCB 雙面 / 鐵 外殼不同有5款
  (3)初級同線路但layout不同 ，外殼不同
  上述是否可列為同一系列 EMC 及 Safety 。
  決議:1.上述Power Supply因規格不同，不能列為同一系列。
       2.系列分法： (1)同設備類別(例如I or II類) 。
                    (2) 一次側Layout相同。
                    (3)變壓器一次側線圈之線徑、圈數相同且結構完全相同。
       3.可詳閱94.2.1日之技術會議記錄。
--- [Page 2] ---
二、台灣德國萊因技術監護顧問股份有限公司提案：
  決議:依標準規定辦理。"#;

        let raw_json = r#"{
          "meeting": { "title": "測試會議" },
          "meeting_promotions": [],
          "agenda_resolution_pairs": [
            {
              "item_no": "1",
              "agenda_item": "律頻科技股份有限公司提案：Power Supply:\n(1)100V - 240V & 200V - 240V / 6 組output / PCB 單面/ 鐵 外殼不同有3款\n(2)100V - 240V & 200V - 240V / 8 組output / PCB 雙面 / 鐵 外殼不同有5款\n(3)初級同線路但layout不同 ，外殼不同\n上述是否可列為同一系列 EMC 及 Safety 。",
              "resolution": "1.上述Power Supply因規格不同，不能列為同一系列。\n2.系列分法： (1)同設備類別(例如I or II類) 。 (2) 一次側Layout相同。 (3)變壓器一次側線圈之線徑、圈數相同且結構完全相同。",
              "result": "依規定辦理"
            },
            {
              "item_no": "2",
              "agenda_item": "台灣德國萊因技術監護顧問股份有限公司提案",
              "resolution": "依標準規定辦理。",
              "result": "依標準規定辦理"
            }
          ]
        }"#;

        let mut parsed = parse_ai_payload(raw_json, None).expect("should parse");
        backfill_truncated_resolutions(doc, &mut parsed);

        let res = parsed.agenda_resolution_pairs[0]
            .resolution_text
            .as_deref()
            .unwrap();
        assert!(res.contains("3.可詳閱94.2.1日之技術會議記錄。"));
        assert!(res.contains("\n(1)同設備類別"));
        assert!(res.contains("\n(2) 一次側Layout相同。"));
        assert!(res.contains("\n(3)變壓器一次側線圈之線徑、圈數相同且結構完全相同。"));
    }

    #[test]
    fn unwraps_hard_wrapped_lines_while_preserving_structure() {
        let dell_agenda = "DELL 公司提案：\n六月份技術會議中要求台達電子提供關於測試電壓 +10% 是否可替代測試電\n壓 +6%的數據資料，並預計七月份會議公告。不知其結論如何？可否煩請列\n入八月份技術會議中公告？";
        let formatted_agenda = format_multiline_numbered_text(dell_agenda);
        assert_eq!(
            formatted_agenda,
            "DELL 公司提案：\n六月份技術會議中要求台達電子提供關於測試電壓 +10% 是否可替代測試電壓 +6%的數據資料，並預計七月份會議公告。不知其結論如何？可否煩請列入八月份技術會議中公告？"
        );

        let dell_resolution = "試驗室可依下列方式擇其一執行:\n1. Input Test(1.6.2 章節)應評估 +6%及+10%測試電壓的測試結果紀\n錄，用以支持 +10%可取代 +6%所須的工程判定。\n2.可依照 CNS 14336 第 1.4.5 節 試驗之電源電壓\n直接與交流電源連接之設備，電壓之容許差應以+6%及-10%計，以下\n情況例外:\n製造商所訂之更寬的容許差時，則應以較寬之容許差值為準(廠商應\n宣告於說明書內)。";
        let formatted_res = format_multiline_numbered_text(dell_resolution);
        assert_eq!(
            formatted_res,
            "試驗室可依下列方式擇其一執行:\n1. Input Test(1.6.2 章節)應評估 +6%及+10%測試電壓的測試結果紀錄，用以支持 +10%可取代 +6%所須的工程判定。\n2.可依照 CNS 14336 第 1.4.5 節 試驗之電源電壓直接與交流電源連接之設備，電壓之容許差應以+6%及-10%計，以下情況例外:\n製造商所訂之更寬的容許差時，則應以較寬之容許差值為準(廠商應宣告於說明書內)。"
        );

        let tuv_ps = "TUV PS 提案：\n目前手邊有一些案件是資料儲存機或電腦伺服器(CCC Code\n  8471.70.90.00.0A),屬於三相電源, 200-240 Vac, Max. 48Amp,已取得國外安規\n  60950 的 CB 報告,目前被認可的實驗室皆無法提供測試轉 CNS 14336 報告,\n  此國外客戶急需進口該產品,請貴單位協助辦理";
        let formatted_tuv = format_multiline_numbered_text(tuv_ps);
        assert_eq!(
            formatted_tuv,
            "TUV PS 提案：\n目前手邊有一些案件是資料儲存機或電腦伺服器(CCC Code 8471.70.90.00.0A),屬於三相電源, 200-240 Vac, Max. 48Amp,已取得國外安規60950 的 CB 報告,目前被認可的實驗室皆無法提供測試轉 CNS 14336 報告,此國外客戶急需進口該產品,請貴單位協助辦理"
        );
    }
}

fn parse_date_opt(value: Option<&str>) -> Result<Option<NaiveDate>> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let v = raw.trim();
    if v.is_empty() || v.eq_ignore_ascii_case("null") {
        return Ok(None);
    }

    for fmt in ["%Y-%m-%d", "%Y/%m/%d", "%Y.%m.%d"] {
        if let Ok(date) = NaiveDate::parse_from_str(v, fmt) {
            return Ok(Some(date));
        }
    }

    if let Some(date) = try_parse_flexible_date(v) {
        return Ok(Some(date));
    }

    eprintln!("[ingest-meeting-ai] warning: 無法解析 meeting_date ({v})，將以 NULL 寫入");
    Ok(None)
}

fn try_parse_flexible_date(input: &str) -> Option<NaiveDate> {
    let digits: Vec<i32> = input
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<i32>().ok())
        .collect();

    if digits.len() >= 3 {
        let year = if digits[0] < 1900 {
            digits[0] + 1911
        } else {
            digits[0]
        };
        let month = digits[1] as u32;
        let day = digits[2] as u32;
        if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
            return Some(date);
        }
    } else if digits.len() == 2 {
        let year = if digits[0] < 1900 {
            digits[0] + 1911
        } else {
            digits[0]
        };
        let month = digits[1] as u32;
        if let Some(date) = NaiveDate::from_ymd_opt(year, month, 1) {
            return Some(date);
        }
    }

    None
}

fn parse_datetime_opt(value: Option<&str>) -> Result<Option<DateTime<FixedOffset>>> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let v = raw.trim();
    if v.is_empty() || v.eq_ignore_ascii_case("null") {
        return Ok(None);
    }

    if let Ok(dt) = DateTime::parse_from_rfc3339(v) {
        return Ok(Some(dt));
    }

    for candidate in [
        format!("{v}+08:00"),
        format!("{}:00+08:00", v.replace(' ', "T")),
        format!("{}+08:00", v.replace(' ', "T")),
        format!("{v}T00:00:00+08:00"),
    ] {
        if let Ok(dt) = DateTime::parse_from_rfc3339(&candidate) {
            return Ok(Some(dt));
        }
    }

    eprintln!("[ingest-meeting-ai] warning: 無法解析時間欄位 ({v})，將以 NULL 寫入");
    Ok(None)
}
