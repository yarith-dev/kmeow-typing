//! Isolated synthetic stress harness. File I/O stays outside the portable engine.
use khmer_engine::{ApprovedEnglishWord, ApprovedStatus, Engine, SuggestionFeatures, TypingMode};
use std::{collections::BinaryHeap, fs, time::Instant};
fn spelling(i: u64, n: u64) -> String {
    let mut number = i * (26u64.pow(7) / n);
    let mut bytes = [b'a'; 8];
    bytes[0] = b'x';
    for byte in bytes[1..].iter_mut().rev() {
        *byte += (number % 26) as u8;
        number /= 26;
    }
    String::from_utf8(bytes.to_vec()).unwrap()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("compile") {
        let count: u64 = args[2].parse()?;
        if count == 0 || count > 10_000_000 {
            return Err("Use 1–10M synthetic words".into());
        }
        let mut best = BinaryHeap::new();
        let start = Instant::now();
        let engine = Engine::compile_sorted_english((0..count).map(|i| {
            let rank = ((i * 15485863) % 1_000_001) as u32;
            if best.len() < 20 {
                best.push((rank, i + 1));
            } else if (rank, i + 1) < *best.peek().unwrap() {
                *best.peek_mut().unwrap() = (rank, i + 1);
            }
            ApprovedEnglishWord {
                id: i + 1,
                text: spelling(i, count),
                rank,
                status: ApprovedStatus::Approved,
            }
        }))?;
        let elapsed = start.elapsed().as_secs_f64() * 1000.;
        let expected: Vec<u64> = best
            .into_sorted_vec()
            .into_iter()
            .map(|(_, id)| id)
            .collect();
        let result = engine.suggest_with_features(
            "x",
            SuggestionFeatures {
                mode: TypingMode::English,
                ..Default::default()
            },
        )?;
        assert_eq!(
            result
                .candidates
                .iter()
                .map(|s| s.candidate.mapping_id().unwrap())
                .collect::<Vec<_>>(),
            expected
        );
        fs::write(&args[3], engine.compiled_bytes())?;
        println!(
            "{}",
            serde_json::json!({"kind":"synthetic-shared-prefix-dispersed-suffixes","entries":count,"encodedBytes":engine.compiled_bytes().len(),"compileAndValidateMs":elapsed,"top20MatchesIndependentExhaustiveOracle":true})
        );
    } else if args.get(1).map(String::as_str) == Some("load") {
        let read = Instant::now();
        let bytes = fs::read(&args[2])?;
        let read_ms = read.elapsed().as_secs_f64() * 1000.;
        let start = Instant::now();
        let engine = Engine::from_binary(bytes)?;
        let load_ms = start.elapsed().as_secs_f64() * 1000.;
        let mut times = Vec::new();
        let features = SuggestionFeatures {
            mode: TypingMode::English,
            ..Default::default()
        };
        for i in 0..240 {
            let start = Instant::now();
            let result = engine.suggest_with_features("x", features)?;
            assert_eq!(result.candidates.len(), 20);
            if i >= 40 {
                times.push(start.elapsed().as_secs_f64() * 1000.);
            }
        }
        times.sort_by(f64::total_cmp);
        println!(
            "{}",
            serde_json::json!({"stats":serde_json::from_str::<serde_json::Value>(&engine.stats_json())?,"fileReadMs":read_ms,"validateAndLoadMs":load_ms,"commonPrefix":{"samples":times.len(),"p50Ms":times[times.len()/2],"p95Ms":times[times.len()*95/100],"maxMs":times.last()}})
        );
    } else {
        return Err("compact_scale compile COUNT FILE | load FILE".into());
    }
    Ok(())
}
