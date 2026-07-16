use std::collections::HashMap;
use std::process::Command;

use ndarray::{Array1, Array2, Array3, Array4};
use rustfft::num_complex::Complex;
use rustfft::FftPlanner;

use super::models::ModelEngine;

const FRAME_INTERVAL_SECS: u32 = 2;
const MAX_FRAMES: usize = 30;

pub fn extract_frames(video_path: &str) -> Vec<image::RgbImage> {
    let fps_filter = format!("fps=1/{}", FRAME_INTERVAL_SECS);
    let Ok(output) = Command::new("ffmpeg")
        .args([
            "-i",
            video_path,
            "-vf",
            &fps_filter,
            "-frames:v",
            &MAX_FRAMES.to_string(),
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-v",
            "quiet",
            "-",
        ])
        .output()
    else {
        return Vec::new();
    };

    if !output.status.success() || output.stdout.is_empty() {
        return Vec::new();
    }

    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=p=0",
            video_path,
        ])
        .output()
        .ok()
        .filter(|p| p.status.success());

    let (width, height) = if let Some(p) = probe {
        let info = String::from_utf8_lossy(&p.stdout);
        let parts: Vec<&str> = info.trim().split(',').collect();
        if parts.len() >= 2 {
            if let (Ok(w), Ok(h)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                (w, h)
            } else {
                (640, 480)
            }
        } else {
            (640, 480)
        }
    } else {
        (640, 480)
    };

    let frame_size = (width * height * 3) as usize;
    let raw = &output.stdout;
    let mut frames = Vec::new();

    for chunk in raw.chunks_exact(frame_size) {
        let img = image::ImageBuffer::from_raw(width, height, chunk.to_vec());
        if let Some(img) = img {
            frames.push(img);
        }
    }

    frames
}

#[allow(unused_macros)]
macro_rules! whisper_debug {
    ($($arg:tt)*) => {
        tracing::debug!("[whisper] {}", format!($($arg)*));
    };
}

const SAMPLE_RATE: usize = 16000;
const N_FFT: usize = 400;
const HOP_LENGTH: usize = 160;
const N_MELS: usize = 80;
const CHUNK_LENGTH: usize = 30;
const N_SAMPLES: usize = SAMPLE_RATE * CHUNK_LENGTH;
const N_FRAMES: usize = N_SAMPLES / HOP_LENGTH;
const MAX_LENGTH: usize = 448;

pub(crate) const SOT: i64 = 50258;
pub(crate) const EOT: i64 = 50257;
pub(crate) const EN_LANG: i64 = 50259;
pub(crate) const TRANSCRIBE: i64 = 50359;
pub(crate) const NO_TIMESTAMPS: i64 = 50363;

pub fn extract_audio(video_path: &str) -> Option<Vec<f32>> {
    let output = Command::new("ffmpeg")
        .args([
            "-i",
            video_path,
            "-ar",
            &SAMPLE_RATE.to_string(),
            "-ac",
            "1",
            "-f",
            "s16le",
            "-acodec",
            "pcm_s16le",
            "-v",
            "quiet",
            "-",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let raw = output.stdout;
    let samples: Vec<f32> = raw
        .chunks_exact(2)
        .map(|b| {
            let s = i16::from_le_bytes([b[0], b[1]]);
            s as f32 / 32768.0
        })
        .collect();

    if samples.is_empty() {
        return None;
    }
    Some(samples)
}

fn hz_to_mel(hz: f64) -> f64 {
    2595.0 * (1.0 + hz / 700.0).log10()
}

fn mel_to_hz(mel: f64) -> f64 {
    700.0 * (10.0_f64.powf(mel / 2595.0) - 1.0)
}

fn mel_filterbank() -> Array2<f32> {
    let magnitude_bins = N_FFT / 2 + 1;
    let frequencies: Vec<f64> = (0..magnitude_bins)
        .map(|i| i as f64 * SAMPLE_RATE as f64 / N_FFT as f64)
        .collect();

    let mel_low = 0.0;
    let mel_high = hz_to_mel(SAMPLE_RATE as f64 / 2.0);
    let mel_points: Vec<f64> = (0..=N_MELS + 1)
        .map(|i| mel_low + (mel_high - mel_low) * i as f64 / (N_MELS + 1) as f64)
        .collect();
    let hz_points: Vec<f64> = mel_points.iter().map(|&m| mel_to_hz(m)).collect();

    let mut filterbank = Array2::<f32>::zeros((N_MELS, magnitude_bins));
    for i in 0..N_MELS {
        let low = hz_points[i];
        let center = hz_points[i + 1];
        let high = hz_points[i + 2];
        for j in 0..magnitude_bins {
            let f = frequencies[j];
            let val = if f < low {
                0.0
            } else if f < center {
                (f - low) / (center - low)
            } else if f < high {
                (high - f) / (high - center)
            } else {
                0.0
            };
            filterbank[[i, j]] = val as f32;
        }
    }
    filterbank
}

fn hann_window(size: usize) -> Vec<f32> {
    (0..size)
        .map(|i| 0.5 - (std::f64::consts::PI * 2.0 * i as f64 / size as f64).cos() as f32 * 0.5)
        .collect()
}

fn stft(signal: &[f32], n_fft: usize, hop_length: usize) -> Array2<Complex<f32>> {
    let win = hann_window(n_fft);
    let padded_len = signal.len() + n_fft;
    let mut padded = vec![0.0f32; padded_len];
    padded[n_fft / 2..n_fft / 2 + signal.len()].copy_from_slice(signal);

    let n_frames = (padded.len() - n_fft) / hop_length + 1;
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(n_fft);

    let mut output = Array2::<Complex<f32>>::zeros((n_fft / 2 + 1, n_frames));
    let mut frame_buf = vec![Complex::<f32>::new(0.0, 0.0); n_fft];

    for frame_idx in 0..n_frames {
        let start = frame_idx * hop_length;
        for i in 0..n_fft {
            frame_buf[i] = Complex::new(padded[start + i] * win[i], 0.0);
        }
        fft.process(&mut frame_buf);
        for i in 0..=n_fft / 2 {
            output[[i, frame_idx]] = frame_buf[i];
        }
    }
    output
}

pub fn compute_mel_spectrogram(audio: &[f32]) -> Array3<f32> {
    let filterbank = mel_filterbank();

    let audio_chunk: Vec<f32> = if audio.len() >= N_SAMPLES {
        audio[..N_SAMPLES].to_vec()
    } else {
        let mut padded = audio.to_vec();
        padded.resize(N_SAMPLES, 0.0);
        padded
    };

    let stft_result = stft(&audio_chunk, N_FFT, HOP_LENGTH);
    let n_freq = stft_result.nrows();
    let n_frames = stft_result.ncols();

    let mut power = Array2::<f32>::zeros((n_freq, n_frames));
    for ((i, j), val) in stft_result.indexed_iter() {
        power[[i, j]] = val.norm_sqr();
    }

    let mel = filterbank.dot(&power);
    let mut log_mel = mel.mapv(|x| (x.max(1e-10)).log10());

    let global_max = log_mel.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let floor = global_max - 8.0;
    for v in log_mel.iter_mut() {
        if *v < floor {
            *v = floor;
        }
        *v = (*v + 4.0) / 4.0;
    }

    let target_frames = N_FRAMES;
    let mut result = Array3::<f32>::zeros((1, N_MELS, target_frames));
    let frames_to_copy = n_frames.min(target_frames);
    for f in 0..frames_to_copy {
        for m in 0..N_MELS {
            result[[0, m, f]] = log_mel[[m, f]];
        }
    }
    result
}

type KvCache = Vec<Array4<f32>>;

fn empty_kv(n_heads: usize, head_dim: usize) -> Array4<f32> {
    Array4::<f32>::zeros((1, n_heads, 1, head_dim))
}

fn make_empty_past(n_layers: usize, n_heads: usize, head_dim: usize) -> KvCache {
    let mut past = Vec::new();
    for _ in 0..n_layers {
        past.push(empty_kv(n_heads, head_dim));
        past.push(empty_kv(n_heads, head_dim));
        past.push(empty_kv(n_heads, head_dim));
        past.push(empty_kv(n_heads, head_dim));
    }
    past
}

fn merge_kv(old: &KvCache, new: &KvCache) -> KvCache {
    old.iter()
        .zip(new.iter())
        .map(|(o, n)| {
            if n.shape().iter().any(|&d| d == 0) {
                o.clone()
            } else {
                n.clone()
            }
        })
        .collect()
}

fn kv_to_tensor(kv: &Array4<f32>, name: &str) -> ort::value::Value {
    let shape = kv.shape().to_vec();
    let data = kv.clone().into_raw_vec();
    whisper_debug!("kv_to_tensor: {name} shape={shape:?}");
    match ort::value::Value::from_array((shape, data)) {
        Ok(v) => v.into_dyn(),
        Err(e) => {
            tracing::error!("kv_to_tensor failed for {name}: {e}");
            panic!("kv_to_tensor failed for {name}: {e}");
        }
    }
}

fn extract_kv(outputs: &ort::session::SessionOutputs, start_idx: usize) -> KvCache {
    let mut kv = Vec::new();
    for i in start_idx..outputs.len() {
        if let Ok((shape, data)) = outputs[i].try_extract_tensor::<f32>() {
            let ndim = shape.len();
            if ndim == 4 {
                let s: [usize; 4] = [
                    shape[0] as usize,
                    shape[1] as usize,
                    shape[2] as usize,
                    shape[3] as usize,
                ];
                let arr = Array4::from_shape_vec(s, data.to_vec()).unwrap();
                kv.push(arr);
            }
        }
    }
    kv
}

pub fn whisper_transcribe(
    encoder: &ModelEngine,
    decoder: &ModelEngine,
    tokenizer: &tokenizers::Tokenizer,
    audio: &[f32],
) -> String {
    let mel = compute_mel_spectrogram(audio);
    let mel_min = mel.iter().cloned().fold(f32::INFINITY, f32::min);
    let mel_max = mel.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let mel_mean: f32 = mel.iter().sum::<f32>() / mel.len() as f32;
    whisper_debug!(
        "mel shape={:?} min={mel_min:.4} max={mel_max:.4} mean={mel_mean:.4}",
        mel.shape()
    );
    let mel_shape = mel.shape().to_vec();
    let mel_data = mel.into_raw_vec();
    let enc_input = ort::value::Value::from_array((mel_shape, mel_data)).unwrap();

    let (enc_seq_len, hidden_dim, enc_data_arr) = {
        let mut lock = encoder.lock().unwrap();

        whisper_debug!("encoder input count: {}", lock.inputs().len());

        let outputs = match lock.run(ort::inputs!["input_features" => enc_input]) {
            Ok(o) => o,
            Err(e) => return format!("Encoder error: {e}"),
        };
        if let Ok((shape, data)) = outputs[0].try_extract_tensor::<f32>() {
            let seq = *shape.get(1).unwrap_or(&1500) as usize;
            let dim = *shape.get(2).unwrap_or(&384) as usize;
            let s = [1, seq, dim];
            let arr = Array3::from_shape_vec(s, data.to_vec()).unwrap();
            whisper_debug!("encoder output: seq={seq} dim={dim}");
            (seq, dim, arr)
        } else {
            return String::new();
        }
    };

    let n_layers = 4;
    let n_heads = 6;
    let head_dim = hidden_dim / n_heads;
    let initial_tokens: Vec<i64> = vec![SOT, EN_LANG, TRANSCRIBE, NO_TIMESTAMPS];

    let kv_input_names: Vec<String> = (0..n_layers)
        .flat_map(|l| {
            vec![
                format!("past_key_values.{l}.decoder.key"),
                format!("past_key_values.{l}.decoder.value"),
                format!("past_key_values.{l}.encoder.key"),
                format!("past_key_values.{l}.encoder.value"),
            ]
        })
        .collect();

    let run_decoder = |input_ids: Vec<i64>,
                       past_kv: &KvCache,
                       use_cache_branch: bool,
                       enc_data: &Array3<f32>,
                       _enc_seq_len: usize|
     -> Option<(Vec<i64>, KvCache)> {
        let seq_len = input_ids.len();
        let ids_arr = Array2::from_shape_vec((1, seq_len), input_ids).ok()?;
        let ids_tensor =
            ort::value::Value::from_array((ids_arr.shape().to_vec(), ids_arr.into_raw_vec()))
                .ok()?;

        let enc_hidden_tensor = ort::value::Value::from_array((
            enc_data.shape().to_vec(),
            enc_data.clone().into_raw_vec(),
        ))
        .ok()?;

        let mut inputs: HashMap<String, ort::value::Value> = HashMap::new();
        inputs.insert("input_ids".into(), ids_tensor.into_dyn());
        inputs.insert("encoder_hidden_states".into(), enc_hidden_tensor.into_dyn());

        let use_cache_arr = Array1::from_vec(vec![use_cache_branch]);
        let use_cache_tensor = ort::value::Value::from_array((
            use_cache_arr.shape().to_vec(),
            use_cache_arr.into_raw_vec(),
        ))
        .ok()?;
        inputs.insert("use_cache_branch".into(), use_cache_tensor.into_dyn());

        for (i, kv) in past_kv.iter().enumerate() {
            inputs.insert(
                kv_input_names[i].clone(),
                kv_to_tensor(kv, &kv_input_names[i]),
            );
        }

        let mut lock = decoder.lock().unwrap();
        let outputs = match lock.run(inputs) {
            Ok(o) => o,
            Err(e) => {
                tracing::warn!("[whisper] decoder run error: {e}");
                return None;
            }
        };

        let (logits_shape, logits_data) = outputs[0].try_extract_tensor::<f32>().ok()?;
        let vocab_size = *logits_shape.last().unwrap_or(&51865) as usize;
        let last_offset = logits_data.len().saturating_sub(vocab_size);
        let last_logits = &logits_data[last_offset..last_offset + vocab_size];

        let mut indexed: Vec<(usize, f32)> = last_logits
            .iter()
            .enumerate()
            .map(|(i, &v)| (i, v))
            .collect();
        indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        whisper_debug!("top5 logits: {:?}", &indexed[..5]);

        let next_token = indexed[0].0 as i64;

        let new_past = extract_kv(&outputs, 1);
        Some((vec![next_token], new_past))
    };

    let empty_past = make_empty_past(n_layers, n_heads, head_dim);

    let (first_token, mut current_past) = match run_decoder(
        initial_tokens,
        &empty_past,
        false,
        &enc_data_arr,
        enc_seq_len,
    ) {
        Some(r) => {
            whisper_debug!("first token: {:?}", r.0);
            whisper_debug!("past kv layers: {}", r.1.len());
            r
        }
        None => return "first decoder call failed".into(),
    };

    let mut generated_tokens: Vec<i64> = vec![first_token[0]];
    whisper_debug!("starting decode loop with first token {}", first_token[0]);

    for step in 0..MAX_LENGTH {
        let last = *generated_tokens.last().unwrap();
        if last == EOT || last >= SOT {
            whisper_debug!("stop at step {step}: token {last}");
            break;
        }

        match run_decoder(vec![last], &current_past, true, &enc_data_arr, enc_seq_len) {
            Some((next, new_past)) => {
                whisper_debug!("step {step}: token {}", next[0]);
                generated_tokens.push(next[0]);
                if !new_past.is_empty() {
                    current_past = merge_kv(&current_past, &new_past);
                }
            }
            None => {
                tracing::warn!("[whisper] decoder returned None at step {step}");
                break;
            }
        }
    }
    whisper_debug!("total tokens generated: {}", generated_tokens.len());

    let skip_tokens = [
        SOT as u32,
        EOT as u32,
        EN_LANG as u32,
        TRANSCRIBE as u32,
        NO_TIMESTAMPS as u32,
    ];
    let text_tokens: Vec<u32> = generated_tokens
        .iter()
        .filter(|&&t| t >= 0 && t < SOT && !skip_tokens.contains(&(t as u32)))
        .map(|&t| t as u32)
        .collect();

    if text_tokens.is_empty() {
        return String::new();
    }

    tokenizer.decode(&text_tokens, true).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mel_spectrogram_output_shape() {
        let audio = vec![0.0f32; SAMPLE_RATE * 10];
        let mel = compute_mel_spectrogram(&audio);
        assert_eq!(mel.shape(), &[1, N_MELS, N_FRAMES]);
    }

    #[test]
    fn test_extract_audio_nonexistent() {
        assert!(extract_audio("/nonexistent/video.mp4").is_none());
    }

    #[test]
    fn test_hann_window_length() {
        let win = hann_window(N_FFT);
        assert_eq!(win.len(), N_FFT);
    }

    #[test]
    fn test_mel_filterbank_shape() {
        let fb = mel_filterbank();
        assert_eq!(fb.nrows(), N_MELS);
        assert_eq!(fb.ncols(), N_FFT / 2 + 1);
    }

    #[test]
    fn test_stft_output() {
        let signal = vec![0.0f32; N_FFT * 2];
        let result = stft(&signal, N_FFT, HOP_LENGTH);
        assert!(result.nrows() > 0);
        assert!(result.ncols() > 0);
    }

    #[test]
    #[ignore]
    fn test_decoder_model_metadata() {
        let models_dir = std::path::PathBuf::from(
            std::env::var("HOME").unwrap_or_default() + "/.config/io.denzyl.siegu/models",
        );
        let enc_path = models_dir.join("whisper.onnx");
        let dec_path = models_dir.join("whisper-decoder.onnx");

        let mut enc = super::super::ep::build_session(&enc_path).unwrap();
        eprintln!("=== ENCODER ===");
        for input in enc.inputs().iter() {
            eprintln!("  input: {:?}", input);
        }
        for output in enc.outputs().iter() {
            eprintln!("  output: {:?}", output);
        }

        let mut dec = super::super::ep::build_session(&dec_path).unwrap();
        eprintln!("=== DECODER ===");
        for input in dec.inputs().iter() {
            eprintln!("  input: {:?}", input);
        }
        for output in dec.outputs().iter() {
            eprintln!("  output: {:?}", output);
        }
    }

    #[test]
    #[ignore]
    fn test_whisper_transcribe_real_video() {
        let models_dir = std::path::PathBuf::from(
            std::env::var("HOME").unwrap_or_default() + "/.config/io.denzyl.siegu/models",
        );
        let enc_path = models_dir.join("whisper.onnx");
        let dec_path = models_dir.join("whisper-decoder.onnx");
        let tok_path = models_dir.join("whisper-tokenizer.json");
        if !enc_path.exists() || !dec_path.exists() || !tok_path.exists() {
            eprintln!("whisper models not found, skipping");
            return;
        }
        let enc = super::super::ep::build_session(&enc_path).unwrap();
        let dec = super::super::ep::build_session(&dec_path).unwrap();
        let tok = tokenizers::Tokenizer::from_file(&tok_path).unwrap();
        let enc = std::sync::Arc::new(std::sync::Mutex::new(enc));
        let dec = std::sync::Arc::new(std::sync::Mutex::new(dec));

        let videos = [
            "/home/denzyl/Pictures/takeout-20260428T162732Z-3-001/Takeout/Google Photos/Moved to van der hoevenplein /VID_20171010_123456.mp4",
        ];
        for path in &videos {
            if !std::path::Path::new(path).exists() {
                eprintln!("test video not found: {path}");
                continue;
            }
            let audio = extract_audio(path).expect("failed to extract audio");
            assert!(!audio.is_empty(), "audio is empty");
            eprintln!("audio samples: {}", audio.len());
            let transcript = whisper_transcribe(&enc, &dec, &tok, &audio);
            eprintln!("transcript: [{transcript}]");
        }
    }

    #[test]
    fn test_extract_frames_nonexistent() {
        let frames = extract_frames("/nonexistent/video.mp4");
        assert!(frames.is_empty());
    }

    #[test]
    fn test_extract_frames_real_video() {
        let path = "/home/denzyl/Pictures/takeout-20260428T162732Z-3-001/Takeout/Google Photos/Moved to van der hoevenplein /VID_20171010_123456.mp4";
        if !std::path::Path::new(path).exists() {
            eprintln!("test video not found: {path}");
            return;
        }
        let frames = extract_frames(path);
        assert!(!frames.is_empty(), "should extract at least one frame");
        assert!(frames.len() <= MAX_FRAMES, "should not exceed MAX_FRAMES");
        for frame in &frames {
            assert!(
                frame.width() > 0 && frame.height() > 0,
                "frame should have non-zero dimensions"
            );
        }
    }

    #[test]
    fn test_extract_audio_real_video() {
        let path = "/home/denzyl/Pictures/takeout-20260428T162732Z-3-001/Takeout/Google Photos/Moved to van der hoevenplein /VID_20171010_123456.mp4";
        if !std::path::Path::new(path).exists() {
            eprintln!("test video not found: {path}");
            return;
        }
        let audio = extract_audio(path);
        assert!(audio.is_some(), "should extract audio from video");
        let samples = audio.unwrap();
        assert!(!samples.is_empty(), "audio should not be empty");
        assert!(
            samples.iter().all(|&s| s >= -1.0 && s <= 1.0),
            "audio samples should be normalized"
        );
    }

    #[test]
    fn test_mel_spectrogram_normalization() {
        let audio = vec![0.5f32; SAMPLE_RATE * 30];
        let mel = compute_mel_spectrogram(&audio);
        let min = mel.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = mel.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        assert!(min >= -10.0, "mel min should be >= -10.0, got {min}");
        assert!(max <= 5.0, "mel max should be <= 5.0, got {max}");
        assert!(max > min, "mel max should be greater than min");
    }

    #[test]
    fn test_merge_kv_preserves_old_when_new_empty() {
        let n_layers = 2;
        let n_heads = 6;
        let head_dim = 64;
        let old = make_empty_past(n_layers, n_heads, head_dim);
        let new: KvCache = (0..old.len())
            .map(|_| Array4::<f32>::zeros((0, 0, 0, 0)))
            .collect();
        let merged = merge_kv(&old, &new);
        assert_eq!(merged.len(), old.len());
        for (m, o) in merged.iter().zip(old.iter()) {
            assert_eq!(m.shape(), o.shape());
        }
    }

    #[test]
    fn test_merge_kv_uses_new_when_nonempty() {
        let n_layers = 2;
        let n_heads = 6;
        let head_dim = 64;
        let old = make_empty_past(n_layers, n_heads, head_dim);
        let new = make_empty_past(n_layers, n_heads, head_dim);
        let merged = merge_kv(&old, &new);
        assert_eq!(merged.len(), old.len());
    }

    #[test]
    fn test_token_constants_sanity() {
        assert!(EOT < SOT, "EOT (50257) should be less than SOT (50258)");
        assert_eq!(SOT, 50258);
        assert_eq!(EOT, 50257);
        assert_eq!(EN_LANG, 50259);
        assert_eq!(TRANSCRIBE, 50359);
        assert_eq!(NO_TIMESTAMPS, 50363);
    }
}
